use std::io::IsTerminal;

fn color(code: &str, text: &str) -> String {
    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.into()
    }
}

pub fn bold(text: &str) -> String {
    color("1", text)
}
pub fn dim(text: &str) -> String {
    color("2", text)
}
pub fn cyan(text: &str) -> String {
    color("1;36", text)
}
pub fn blue(text: &str) -> String {
    color("1;34", text)
}
pub fn green(text: &str) -> String {
    color("1;32", text)
}
pub fn yellow(text: &str) -> String {
    color("1;33", text)
}
pub fn red(text: &str) -> String {
    color("1;31", text)
}

pub fn banner(label: &str, status: &str) {
    let status = match status {
        "clean" | "created" => green(status),
        "drift" | "vulnerable" | "vulnerable · partial coverage" => red(status),
        "partial" | "incomplete" | "skipped" | "unknown" => yellow(status),
        other => other.into(),
    };
    println!(
        "{}\n{}  {}\n{}",
        dim(&"─".repeat(56)),
        blue(label),
        status,
        dim(&"─".repeat(56))
    );
}

pub fn verdict(parts: &[String]) {
    if parts.is_empty() {
        return;
    }
    println!("  {}", parts.join(&dim("  ·  ")));
}

pub fn section(title: &str) {
    println!("\n{}", cyan(title));
}

pub fn title(text: &str, detail: &str) {
    if detail.is_empty() {
        println!("  {}", bold(text));
    } else {
        println!("  {}  {}", bold(text), dim(detail));
    }
}

pub fn ok(text: &str) {
    println!("  {} {text}", green("✓"));
}
pub fn warn(text: &str) {
    println!("  {} {text}", yellow("!"));
}
pub fn bad(text: &str) {
    println!("  {} {text}", red("✕"));
}
pub fn note(text: &str) {
    println!("  {} {text}", dim("·"));
}
pub fn item(mark: &str, text: &str) {
    println!("      {} {text}", dim(mark));
}
pub fn bullet(text: &str) {
    println!("      {} {text}", dim("•"));
}
pub fn action(text: &str) {
    println!("      {} {text}", cyan("→"));
}
pub fn error(text: &str) {
    eprintln!("{} {text}", red("error:"));
}
pub fn blank() {
    println!();
}

pub fn help() {
    println!(
        "pkg-audit — Costa workstation policy and vulnerability triage

Usage:
  pkg-audit policy [--json]                 Compare installed packages with policy
  pkg-audit vulns [--json]                  Triage high/critical findings (default view)
  pkg-audit vulns --verbose                 List every normalized finding
  pkg-audit vulns --component <name>        Expand one component with remediation
  pkg-audit sbom [--json]                   Generate a Trivy CycloneDX host SBOM

Exit codes:
  0 clean   1 drift or vulnerabilities   2 partial/unavailable   3 error"
    );
}
