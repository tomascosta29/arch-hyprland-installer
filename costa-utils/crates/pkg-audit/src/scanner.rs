use crate::{command, output};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Serialize)]
struct Finding {
    source: &'static str,
    ecosystem: String,
    package: String,
    installed: String,
    fixed: String,
    severity: String,
    id: String,
    title: String,
    target: String,
}

#[derive(Serialize)]
struct ScanReport {
    schema_version: u8,
    status: &'static str,
    findings: Vec<Finding>,
    notes: Vec<String>,
}

struct Pick<'a> {
    package: &'a str,
    target: &'a str,
    findings: Vec<&'a Finding>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bucket {
    ActNow,
    Waiting,
    Dependencies,
}

fn available(program: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", "command -v \"$1\" >/dev/null", "sh", program])
        .status()
        .is_ok_and(|status| status.success())
}

fn arch_findings() -> Result<Vec<Finding>> {
    if !available("arch-audit") {
        bail!("arch-audit is not installed");
    }
    let format = "%n\t%s\t%c\t%v\t%t";
    let result = command::run(
        "arch-audit",
        &["--color", "never", "--format", format],
        Duration::from_secs(90),
    )?;
    if result.code != 0 && result.stdout.trim().is_empty() {
        bail!(
            "arch-audit could not refresh advisories: {}",
            result.stderr.trim()
        );
    }
    Ok(result
        .stdout
        .lines()
        .filter_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields.len() >= 5).then(|| Finding {
                source: "arch-audit",
                ecosystem: "arch".into(),
                package: fields[0].into(),
                installed: String::new(),
                severity: fields[1].to_ascii_uppercase(),
                id: fields[2].into(),
                fixed: fields[3].into(),
                title: fields[4].into(),
                target: "installed system".into(),
            })
        })
        .collect())
}

fn trivy_findings() -> Result<Vec<Finding>> {
    if !available("trivy") {
        bail!("trivy is not installed");
    }
    let home = dirs::home_dir().context("could not resolve home directory")?;
    let mut targets = vec![
        home.join(".cargo/bin"),
        home.join("go/bin"),
        home.join(".local/pipx"),
        home.join(".local/share/uv/tools"),
        home.join(".local/share/nvim/mason"),
    ];
    if available("npm") {
        if let Ok(result) = command::checked("npm", &["root", "-g"], Duration::from_secs(30)) {
            targets.push(PathBuf::from(result.stdout.trim()));
        }
    }
    let mut findings = Vec::new();
    for target in targets.into_iter().filter(|path| path.exists()) {
        let target_text = target.to_string_lossy();
        let args = [
            "rootfs",
            "--quiet",
            "--scanners",
            "vuln",
            "--pkg-types",
            "library",
            "--severity",
            "HIGH,CRITICAL",
            "--format",
            "json",
            target_text.as_ref(),
        ];
        let result = command::run("trivy", &args, Duration::from_secs(900))?;
        if result.code != 0 {
            bail!(
                "trivy scan failed for {}: {}",
                target.display(),
                result.stderr.trim()
            );
        }
        findings.extend(parse_trivy(
            &result.stdout,
            Some(&target.display().to_string()),
        )?);
    }
    Ok(findings)
}

fn parse_trivy(document: &str, origin: Option<&str>) -> Result<Vec<Finding>> {
    let root: Value = serde_json::from_str(document).context("invalid Trivy JSON")?;
    let mut findings = Vec::new();
    for result in root["Results"].as_array().into_iter().flatten() {
        let target = result["Target"].as_str().unwrap_or("unknown");
        let ecosystem = result["Type"].as_str().unwrap_or("library");
        for vuln in result["Vulnerabilities"].as_array().into_iter().flatten() {
            let target = match origin {
                Some(origin) if target != "." && target != origin => {
                    format!("{origin} · {target}")
                }
                Some(origin) => origin.into(),
                None => target.into(),
            };
            findings.push(Finding {
                source: "trivy",
                ecosystem: ecosystem.into(),
                package: text(vuln, "PkgName"),
                installed: text(vuln, "InstalledVersion"),
                fixed: text(vuln, "FixedVersion"),
                severity: text(vuln, "Severity"),
                id: text(vuln, "VulnerabilityID"),
                title: text(vuln, "Title"),
                target,
            });
        }
    }
    Ok(findings)
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}

fn severity_rank(severity: &str) -> u8 {
    match severity.to_ascii_uppercase().as_str() {
        "CRITICAL" => 0,
        "HIGH" => 1,
        "MEDIUM" => 2,
        "LOW" => 3,
        _ => 4,
    }
}

fn picks(findings: &[Finding]) -> Vec<Pick<'_>> {
    let mut grouped: BTreeMap<(&str, &str), Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
        grouped
            .entry((&finding.package, &finding.target))
            .or_default()
            .push(finding);
    }
    let mut picks = grouped
        .into_iter()
        .map(|((package, target), findings)| Pick {
            package,
            target,
            findings,
        })
        .collect::<Vec<_>>();
    picks.sort_by_key(|pick| {
        (
            pick.findings
                .iter()
                .map(|finding| severity_rank(&finding.severity))
                .min()
                .unwrap_or(4),
            std::cmp::Reverse(pick.findings.len()),
            pick.package,
            pick.target,
        )
    });
    picks
}

fn display_status(report: &ScanReport) -> &'static str {
    match (report.findings.is_empty(), report.notes.is_empty()) {
        (false, false) => "vulnerable · partial coverage",
        (false, true) => "vulnerable",
        (true, false) => "incomplete",
        (true, true) => "clean",
    }
}

fn pick_label(pick: &Pick<'_>) -> String {
    if let Some(tool) = mason_tool(pick) {
        return format!("Mason · {tool}");
    }
    if pick.target.contains(".local/share/nvim/mason") {
        return format!("Mason dependency · {}", pick.package);
    }
    if pick.target.contains("node_modules") {
        return format!("Global npm · {}", pick.package);
    }
    if pick.target == "installed system" {
        return format!("Arch · {}", pick.package);
    }
    format!("{} · {}", pick.package, short_target(pick.target))
}

fn short_target(target: &str) -> String {
    friendly_location(target)
}

fn mason_tool<'a>(pick: &'a Pick<'_>) -> Option<&'a str> {
    pick.target
        .split("packages/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
}

fn mason_is_current(tool: &str) -> Option<bool> {
    let home = dirs::home_dir()?;
    let receipt: Value = serde_json::from_str(
        &fs::read_to_string(
            home.join(".local/share/nvim/mason/packages")
                .join(tool)
                .join("mason-receipt.json"),
        )
        .ok()?,
    )
    .ok()?;
    let registry: Value = serde_json::from_str(
        &fs::read_to_string(home.join(
            ".local/share/nvim/mason/registries/github/mason-org/mason-registry/registry.json",
        ))
        .ok()?,
    )
    .ok()?;
    let installed = receipt.pointer("/source/id")?.as_str()?;
    let available = registry
        .as_array()?
        .iter()
        .find(|package| package["name"].as_str() == Some(tool))?
        .pointer("/source/id")?
        .as_str()?;
    Some(installed == available)
}

fn update_state(pick: &Pick<'_>) -> Option<String> {
    let tool = mason_tool(pick)?;
    Some(match mason_is_current(tool) {
        Some(true) => "Latest Mason release; waiting for an upstream rebuild".into(),
        Some(false) => "A newer Mason release is available".into(),
        None => "Mason update status could not be verified".into(),
    })
}

fn bucket(pick: &Pick<'_>) -> Bucket {
    if pick.target == "installed system" {
        return Bucket::ActNow;
    }
    if let Some(tool) = mason_tool(pick) {
        return match mason_is_current(tool) {
            Some(false) => Bucket::ActNow,
            Some(true) => Bucket::Waiting,
            None => Bucket::Waiting,
        };
    }
    if pick.target.contains(".local/share/nvim/mason") || pick.target.contains("node_modules") {
        return Bucket::Dependencies;
    }
    Bucket::ActNow
}

fn severity_counts(findings: &[&Finding]) -> (usize, usize) {
    let critical = findings
        .iter()
        .filter(|finding| finding.severity.eq_ignore_ascii_case("critical"))
        .count();
    (critical, findings.len() - critical)
}

fn severity_label(critical: usize, high: usize) -> String {
    match (critical, high) {
        (0, high) => format!("{high} high"),
        (critical, 0) => format!("{critical} critical"),
        (critical, high) => format!("{critical} critical · {high} high"),
    }
}

fn affected_versions(pick: &Pick<'_>) -> String {
    let versions = pick
        .findings
        .iter()
        .filter(|finding| !finding.installed.is_empty())
        .map(|finding| finding.installed.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let version = versions.into_iter().collect::<Vec<_>>().join(", ");
    if pick.target.contains("packages/") && pick.package == "stdlib" {
        format!("Go stdlib {version} embedded in this Mason tool")
    } else if pick.target.contains(".local/share/nvim/mason") {
        format!("{} {version} in Mason-managed Node.js tools", pick.package)
    } else if pick.target.contains("node_modules") {
        format!(
            "{} {version} in globally installed Node.js tools",
            pick.package
        )
    } else if version.is_empty() {
        pick.package.into()
    } else {
        format!("{} {version}", pick.package)
    }
}

fn friendly_location(target: &str) -> String {
    dirs::home_dir()
        .and_then(|home| {
            target
                .strip_prefix(home.to_string_lossy().as_ref())
                .map(|rest| format!("~{rest}"))
        })
        .unwrap_or_else(|| target.into())
}

fn remediation(pick: &Pick<'_>) -> String {
    if let Some(tool) = mason_tool(pick) {
        match mason_is_current(tool) {
            Some(true) => {
                return format!(
                    "No Mason update is available; wait for {tool} to publish a rebuilt binary"
                );
            }
            Some(false) => {
                return format!("Open :Mason in Neovim, select {tool}, and press U");
            }
            None => {
                return format!("Check {tool} in :Mason and update it when available");
            }
        }
    }
    if pick.target.contains(".local/share/nvim/mason") {
        return "Open :Mason in Neovim and update the affected tools".into();
    }
    if pick.target.contains("node_modules") {
        return "Update the owning global npm package, then scan again".into();
    }
    if pick.target == "installed system" {
        return format!("sudo pacman -Syu {}", pick.package);
    }
    "Update or reinstall the affected tool from its package manager".into()
}

fn example_component<'a>(picks: &'a [Pick<'a>]) -> Option<&'a str> {
    picks
        .iter()
        .find(|pick| bucket(pick) == Bucket::ActNow && pick.target != "installed system")
        .or_else(|| {
            picks
                .iter()
                .find(|pick| bucket(pick) == Bucket::Waiting)
        })
        .or_else(|| {
            picks
                .iter()
                .find(|pick| bucket(pick) == Bucket::Dependencies)
        })
        .or_else(|| picks.first())
        .map(|pick| mason_tool(pick).unwrap_or(pick.package))
}

fn print_triage(report: &ScanReport) {
    let all = picks(&report.findings);
    output::banner("VULNS", display_status(report));

    if all.is_empty() {
        output::verdict(&["no high or critical findings".into()]);
        for note in &report.notes {
            output::warn(note.lines().next().unwrap_or(note));
        }
        output::blank();
        return;
    }

    let critical = report
        .findings
        .iter()
        .filter(|f| f.severity.eq_ignore_ascii_case("critical"))
        .count();
    let high = report.findings.len() - critical;
    let act_now = all
        .iter()
        .filter(|pick| bucket(pick) == Bucket::ActNow)
        .count();
    let waiting = all
        .iter()
        .filter(|pick| bucket(pick) == Bucket::Waiting)
        .count();
    let deps = all
        .iter()
        .filter(|pick| bucket(pick) == Bucket::Dependencies)
        .count();

    let mut verdict = vec![severity_label(critical, high)];
    if act_now > 0 {
        verdict.push(format!(
            "{act_now} actionable component{}",
            if act_now == 1 { "" } else { "s" }
        ));
    }
    if waiting > 0 {
        verdict.push(format!("{waiting} waiting on upstream"));
    }
    if deps > 0 {
        verdict.push(format!("{deps} transitive"));
    }
    output::verdict(&verdict);

    print_bucket(
        "Act now",
        all.iter().filter(|pick| bucket(pick) == Bucket::ActNow),
    );
    print_bucket(
        "Waiting on upstream",
        all.iter().filter(|pick| bucket(pick) == Bucket::Waiting),
    );
    print_bucket(
        "Transitive / embedded",
        all.iter()
            .filter(|pick| bucket(pick) == Bucket::Dependencies),
    );

    if let Some(example) = example_component(&all) {
        output::section("Next");
        output::action(&format!("pkg-audit vulns --component {example}"));
        output::note("pkg-audit vulns --verbose  ·  full finding list");
    }

    for note in &report.notes {
        output::warn(note.lines().next().unwrap_or(note));
    }
    output::blank();
}

fn print_bucket<'a>(title: &str, picks: impl Iterator<Item = &'a Pick<'a>>) {
    let picks = picks.collect::<Vec<_>>();
    if picks.is_empty() {
        return;
    }
    output::section(title);

    let arch: Vec<_> = picks
        .iter()
        .copied()
        .filter(|pick| pick.target == "installed system")
        .collect();
    let rest: Vec<_> = picks
        .iter()
        .copied()
        .filter(|pick| pick.target != "installed system")
        .collect();

    if !arch.is_empty() {
        let names = arch
            .iter()
            .map(|pick| pick.package)
            .collect::<Vec<_>>()
            .join(", ");
        let findings = arch
            .iter()
            .flat_map(|pick| pick.findings.iter().copied())
            .collect::<Vec<_>>();
        let (critical, high) = severity_counts(&findings);
        output::title(
            &format!("Arch packages ({})", arch.len()),
            &severity_label(critical, high),
        );
        output::bullet(&names);
        output::action("sudo pacman -Syu");
    }

    for pick in rest {
        let (critical, high) = severity_counts(&pick.findings);
        output::title(&pick_label(pick), &severity_label(critical, high));
        output::note(&affected_versions(pick));
        if let Some(state) = update_state(pick) {
            output::note(&state);
        }
        if bucket(pick) == Bucket::ActNow {
            output::action(&remediation(pick));
        }
    }
}

fn print_verbose(report: &ScanReport) {
    output::banner("VULNS", display_status(report));
    if report.findings.is_empty() {
        output::ok("No high or critical findings");
    } else {
        let critical = report
            .findings
            .iter()
            .filter(|f| f.severity.eq_ignore_ascii_case("critical"))
            .count();
        let high = report.findings.len() - critical;
        output::verdict(&[
            format!("{} findings", report.findings.len()),
            severity_label(critical, high),
        ]);
        for finding in &report.findings {
            output::section(&format!("{}  {}", finding.severity, finding.id));
            output::note(&format!("{} · {}", finding.source, finding.ecosystem));
            println!(
                "  {} {}",
                finding.package,
                if finding.installed.is_empty() {
                    ""
                } else {
                    &finding.installed
                }
            );
            if !finding.fixed.is_empty() {
                output::action(&format!("fixed in {}", finding.fixed));
            }
            if !finding.title.is_empty() {
                output::note(&finding.title);
            }
            output::item("at", &friendly_location(&finding.target));
        }
    }
    for note in &report.notes {
        output::warn(note.lines().next().unwrap_or(note));
    }
    output::blank();
}

fn print_component(report: &ScanReport, selection: &str) -> Result<()> {
    let all_picks = picks(&report.findings);
    let selected = all_picks
        .iter()
        .filter(|pick| {
            pick.package.eq_ignore_ascii_case(selection)
                || mason_tool(pick).is_some_and(|tool| tool.eq_ignore_ascii_case(selection))
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        bail!("component '{selection}' has no current high or critical findings");
    }

    output::banner("VULNS", display_status(report));
    output::verdict(&[format!("component {selection}")]);

    for selected in selected {
        let (critical, high) = severity_counts(&selected.findings);
        output::section(&pick_label(selected));
        output::verdict(&[
            format!("{} findings", selected.findings.len()),
            severity_label(critical, high),
        ]);
        output::note(&affected_versions(selected));
        if let Some(state) = update_state(selected) {
            output::note(&state);
        }
        output::item("location", &friendly_location(selected.target));

        for finding in &selected.findings {
            output::title(
                &format!("{}  {}", finding.severity, finding.id),
                &format!("{} · {}", finding.source, finding.ecosystem),
            );
            if !finding.fixed.is_empty() {
                output::action(&format!("fixed in {}", finding.fixed));
            }
            if !finding.title.is_empty() {
                output::note(&finding.title);
            }
        }

        output::section("Suggested action");
        output::action(&remediation(selected));
    }

    output::section("Verify");
    output::action("pkg-audit vulns");
    if !report.notes.is_empty() {
        output::warn("Partial coverage; notes from the full scan still apply");
    }
    output::blank();
    Ok(())
}

pub fn scan(json: bool, verbose: bool, component: Option<&str>) -> Result<i32> {
    let mut notes = Vec::new();
    let mut partial = false;
    let mut findings = match arch_findings() {
        Ok(items) => items,
        Err(_error) => {
            partial = true;
            notes.push("Arch advisory coverage unavailable (feed refresh failed)".into());
            vec![]
        }
    };
    match trivy_findings() {
        Ok(items) => findings.extend(items),
        Err(error) => {
            partial = true;
            notes.push(format!("Language scan unavailable: {error:#}"));
        }
    }
    findings.sort_by_key(|item| {
        (
            severity_rank(&item.severity),
            item.package.clone(),
            item.id.clone(),
        )
    });
    let status = if partial {
        "partial"
    } else if findings.is_empty() {
        "clean"
    } else {
        "vulnerable"
    };
    let report = ScanReport {
        schema_version: 1,
        status,
        findings,
        notes,
    };
    if let Some(selection) = component {
        print_component(&report, selection)?;
    } else if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if verbose {
        print_verbose(&report);
    } else {
        print_triage(&report);
    }
    Ok(if partial {
        2
    } else if report.findings.is_empty() {
        0
    } else {
        1
    })
}

pub fn sbom(json: bool) -> Result<i32> {
    if !available("trivy") {
        bail!("trivy is not installed");
    }
    let cache = dirs::cache_dir()
        .context("could not resolve cache directory")?
        .join("pkg-audit");
    fs::create_dir_all(&cache)?;
    let destination: PathBuf = cache.join("host.cdx.json");
    let destination_text = destination.to_string_lossy();
    let result = command::run(
        "trivy",
        &[
            "rootfs",
            "--quiet",
            "--format",
            "cyclonedx",
            "--output",
            destination_text.as_ref(),
            "--skip-dirs",
            "/dev",
            "--skip-dirs",
            "/proc",
            "--skip-dirs",
            "/run",
            "--skip-dirs",
            "/sys",
            "--skip-dirs",
            "/tmp",
            "/",
        ],
        Duration::from_secs(900),
    )?;
    if result.code != 0 {
        bail!("Trivy SBOM generation failed: {}", result.stderr.trim());
    }
    let document: Value = serde_json::from_str(&fs::read_to_string(&destination)?)?;
    let count = document["components"].as_array().map_or(0, Vec::len);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": 1, "status": "created",
                "path": destination, "components": count
            })
        );
    } else {
        output::banner("SBOM", "created");
        output::ok(&format!("{count} components"));
        output::action(&destination.display().to_string());
        output::blank();
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_trivy_vulnerabilities() {
        let json = r#"{"Results":[{"Target":"Cargo.lock","Type":"cargo",
          "Vulnerabilities":[{"VulnerabilityID":"CVE-1","PkgName":"demo",
          "InstalledVersion":"1.0","FixedVersion":"1.1","Severity":"HIGH",
          "Title":"unsafe demo"}]}]}"#;
        let findings = parse_trivy(json, None).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package, "demo");
        assert_eq!(findings[0].fixed, "1.1");
        assert_eq!(findings[0].target, "Cargo.lock");
    }
}
