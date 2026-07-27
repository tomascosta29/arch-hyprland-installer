use crate::{command, output};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
struct EcosystemReport {
    ecosystem: &'static str,
    title: &'static str,
    status: &'static str,
    unmanaged: Vec<String>,
    missing: Vec<String>,
    message: Option<String>,
}

#[derive(Serialize)]
struct AuditReport {
    schema_version: u8,
    status: &'static str,
    policy_dir: String,
    ecosystems: Vec<EcosystemReport>,
}

struct Ecosystem {
    id: &'static str,
    title: &'static str,
    manifest: &'static str,
}

const ECOSYSTEMS: &[Ecosystem] = &[
    Ecosystem {
        id: "pacman",
        title: "Arch repositories",
        manifest: "common.txt",
    },
    Ecosystem {
        id: "aur",
        title: "AUR / foreign",
        manifest: "aur.txt",
    },
    Ecosystem {
        id: "npm",
        title: "Global NPM",
        manifest: "npm.txt",
    },
    Ecosystem {
        id: "cargo",
        title: "Cargo tools",
        manifest: "cargo.txt",
    },
    Ecosystem {
        id: "python",
        title: "Python tools",
        manifest: "python.txt",
    },
    Ecosystem {
        id: "go",
        title: "Go binaries",
        manifest: "go.txt",
    },
];

fn policy_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("PKG_AUDIT_POLICY_DIR") {
        return Ok(path.into());
    }
    let config = dirs::config_dir().context("could not resolve XDG config directory")?;
    Ok(config.join("packages"))
}

fn read_policy(path: &Path) -> Result<BTreeSet<String>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("missing package policy {}", path.display()))?;
    Ok(text
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn selected(name: &str, fallback: &str) -> String {
    let variable = format!("PKG_AUDIT_{}", name.to_ascii_uppercase());
    if let Ok(value) = std::env::var(variable) {
        return value;
    }
    dirs::config_dir()
        .and_then(|config| {
            fs::read_to_string(config.join("costa").join(format!("install-{name}"))).ok()
        })
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.into())
}

fn expected_policy(dir: &Path, ecosystem: &Ecosystem) -> Result<BTreeSet<String>> {
    let mut expected = read_policy(&dir.join(ecosystem.manifest))?;
    if ecosystem.id == "pacman" {
        let profile = selected("profile", "bare-metal");
        let flavor = selected("flavor", if available("zsh") { "full" } else { "light" });
        expected.extend(read_policy(
            &dir.join("profiles").join(format!("{profile}.txt")),
        )?);
        expected.extend(read_policy(
            &dir.join("flavors").join(format!("{flavor}.txt")),
        )?);
    }
    Ok(expected)
}

fn collect(id: &str) -> Result<Option<BTreeSet<String>>> {
    let lines = match id {
        "pacman" => command::lines("pacman", &["-Qqen"])?,
        "aur" => command::lines("pacman", &["-Qqem"])?,
        "npm" => {
            if !available("npm") {
                return Ok(None);
            }
            let data = command::checked(
                "npm",
                &["list", "-g", "--depth=0", "--json"],
                std::time::Duration::from_secs(30),
            )?;
            let value: serde_json::Value = serde_json::from_str(&data.stdout)?;
            value["dependencies"]
                .as_object()
                .into_iter()
                .flat_map(|map| map.keys())
                .filter(|name| name.as_str() != "npm")
                .cloned()
                .collect()
        }
        "cargo" => {
            if !available("cargo") {
                return Ok(None);
            }
            command::lines("cargo", &["install", "--list"])?
                .into_iter()
                .filter(|line| !line.starts_with(' '))
                .filter_map(|line| line.split_whitespace().next().map(ToOwned::to_owned))
                .collect()
        }
        "python" => {
            if !available("pipx") && !available("uv") {
                return Ok(None);
            }
            let mut packages = BTreeSet::new();
            if available("pipx") {
                let data = command::checked(
                    "pipx",
                    &["list", "--json"],
                    std::time::Duration::from_secs(30),
                )?;
                let value: serde_json::Value = serde_json::from_str(&data.stdout)?;
                packages.extend(
                    value["venvs"]
                        .as_object()
                        .into_iter()
                        .flat_map(|map| map.keys())
                        .cloned(),
                );
            }
            if available("uv") {
                for line in command::lines("uv", &["tool", "list"])? {
                    if !line.starts_with(' ') {
                        if let Some(name) = line.split_whitespace().next() {
                            packages.insert(name.into());
                        }
                    }
                }
            }
            packages.into_iter().collect()
        }
        "go" => {
            let root = std::env::var_os("GOBIN")
                .map(PathBuf::from)
                .or_else(|| dirs::home_dir().map(|p| p.join("go/bin")));
            let Some(root) = root else {
                return Ok(Some(BTreeSet::new()));
            };
            if !root.is_dir() {
                return Ok(Some(BTreeSet::new()));
            }
            fs::read_dir(root)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_file())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        }
        _ => unreachable!(),
    };
    Ok(Some(lines.into_iter().collect()))
}

fn available(program: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", "command -v \"$1\" >/dev/null", "sh", program])
        .status()
        .is_ok_and(|status| status.success())
}

fn print_report(report: &AuditReport) {
    output::banner("POLICY", report.status);

    let unmanaged = report
        .ecosystems
        .iter()
        .map(|item| item.unmanaged.len())
        .sum::<usize>();
    let missing = report
        .ecosystems
        .iter()
        .map(|item| item.missing.len())
        .sum::<usize>();
    let unavailable = report
        .ecosystems
        .iter()
        .filter(|item| item.status == "unknown")
        .count();
    let drifted = report
        .ecosystems
        .iter()
        .filter(|item| item.status == "drift")
        .count();

    let mut verdict = Vec::new();
    if report.status == "clean" {
        verdict.push("all ecosystems match policy".into());
    } else {
        if drifted > 0 {
            verdict.push(format!(
                "{drifted} ecosystem{} with drift",
                if drifted == 1 { "" } else { "s" }
            ));
        }
        if unmanaged > 0 {
            verdict.push(format!("{unmanaged} unmanaged"));
        }
        if missing > 0 {
            verdict.push(format!("{missing} missing"));
        }
        if unavailable > 0 {
            verdict.push(format!("{unavailable} unavailable"));
        }
    }
    output::verdict(&verdict);

    let drift_items = report
        .ecosystems
        .iter()
        .filter(|item| item.status == "drift");
    let unknown_items = report
        .ecosystems
        .iter()
        .filter(|item| item.status == "unknown");
    let skipped_items = report
        .ecosystems
        .iter()
        .filter(|item| item.status == "skipped");
    let clean_items = report
        .ecosystems
        .iter()
        .filter(|item| item.status == "clean")
        .collect::<Vec<_>>();

    for item in drift_items {
        output::section(item.title);
        if !item.unmanaged.is_empty() {
            output::bad(&format!("{} unmanaged", item.unmanaged.len()));
            for name in &item.unmanaged {
                output::bullet(name);
            }
        }
        if !item.missing.is_empty() {
            output::warn(&format!("{} missing from install", item.missing.len()));
            for name in &item.missing {
                output::bullet(name);
            }
        }
    }

    for item in unknown_items {
        output::section(item.title);
        let message = item.message.as_deref().unwrap_or("Collector unavailable");
        let short = message.lines().next().unwrap_or(message);
        output::warn(short);
    }

    for item in skipped_items {
        output::section(item.title);
        output::note(item.message.as_deref().unwrap_or("Skipped"));
    }

    if !clean_items.is_empty() {
        output::section("Matching");
        for item in clean_items {
            output::ok(item.title);
        }
    }
    output::blank();
}

pub fn check(json: bool) -> Result<i32> {
    let dir = policy_dir()?;
    let mut reports = Vec::new();
    let mut drift = false;
    let mut partial = false;
    for ecosystem in ECOSYSTEMS {
        let expected = expected_policy(&dir, ecosystem)?;
        let (status, unmanaged, missing, message) = match collect(ecosystem.id) {
            Ok(Some(installed)) => {
                let unmanaged = installed.difference(&expected).cloned().collect::<Vec<_>>();
                let missing = expected.difference(&installed).cloned().collect::<Vec<_>>();
                let status = if unmanaged.is_empty() && missing.is_empty() {
                    "clean"
                } else {
                    drift = true;
                    "drift"
                };
                (status, unmanaged, missing, None)
            }
            Ok(None) if expected.is_empty() => (
                "skipped",
                vec![],
                vec![],
                Some("No tools are managed in this ecosystem".into()),
            ),
            Ok(None) => {
                partial = true;
                (
                    "unknown",
                    vec![],
                    vec![],
                    Some("collector is not installed".into()),
                )
            }
            Err(error) => {
                partial = true;
                let detail = error.to_string();
                let short = detail.lines().next().unwrap_or(&detail);
                let short = if short.chars().count() > 96 {
                    let trimmed: String = short.chars().take(93).collect();
                    format!("{trimmed}...")
                } else {
                    short.to_owned()
                };
                (
                    "unknown",
                    vec![],
                    vec![],
                    Some(format!("collector failed: {short}")),
                )
            }
        };
        reports.push(EcosystemReport {
            ecosystem: ecosystem.id,
            title: ecosystem.title,
            status,
            unmanaged,
            missing,
            message,
        });
    }
    let status = if drift {
        "drift"
    } else if partial {
        "partial"
    } else {
        "clean"
    };
    let report = AuditReport {
        schema_version: 1,
        status,
        policy_dir: dir.display().to_string(),
        ecosystems: reports,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(if drift {
        1
    } else if partial {
        2
    } else {
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_parser_ignores_comments_and_duplicates() {
        let path =
            std::env::temp_dir().join(format!("pkg-audit-policy-test-{}", std::process::id()));
        fs::write(&path, "firefox\n# note\nfirefox\nkitty # desktop\n\n").unwrap();
        let policy = read_policy(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert_eq!(policy.into_iter().collect::<Vec<_>>(), ["firefox", "kitty"]);
    }
}
