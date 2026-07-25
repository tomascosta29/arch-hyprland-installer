//! Bounded wrapper around external desktop commands.

use crate::{Error, Result};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::{debug, warn};
use wait_timeout::ChildExt;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub argv: Vec<String>,
    pub returncode: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    pub fn ok(&self) -> bool {
        self.returncode == 0
    }
}

/// Run a command with the default 15s budget. `check` turns non-zero exits into errors.
pub fn run(argv: &[&str], check: bool) -> Result<CommandResult> {
    run_with_input(argv, None, Duration::from_secs(15), check)
}

pub fn run_with_timeout(argv: &[&str], timeout: Duration, check: bool) -> Result<CommandResult> {
    run_with_input(argv, None, timeout, check)
}

pub fn run_with_input(
    argv: &[&str],
    input: Option<&str>,
    timeout: Duration,
    check: bool,
) -> Result<CommandResult> {
    if argv.is_empty() {
        return Err(Error::Message("empty command".into()));
    }

    let owned: Vec<String> = argv.iter().map(|s| (*s).to_string()).collect();
    debug!(?owned, ?timeout, "running command");

    let mut cmd = Command::new(&owned[0]);
    if owned.len() > 1 {
        cmd.args(&owned[1..]);
    }
    cmd.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|source| Error::CommandSpawn {
        argv: owned.clone(),
        source,
    })?;

    if let Some(text) = input {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
    } else {
        drop(child.stdin.take());
    }

    let output = match child.wait_timeout(timeout).map_err(|source| Error::CommandSpawn {
        argv: owned.clone(),
        source,
    })? {
        Some(status) => {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = std::io::Read::read_to_end(&mut out, &mut stdout);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = std::io::Read::read_to_end(&mut err, &mut stderr);
            }
            // wait_timeout already reaped; rebuild a synthetic Output.
            std::process::Output {
                status,
                stdout,
                stderr,
            }
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::CommandTimeout { argv: owned });
        }
    };

    let result = CommandResult {
        argv: owned.clone(),
        returncode: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };

    if !result.ok() {
        // playerctl with no active players is an expected idle state.
        let quiet_no_players = owned.first().is_some_and(|bin| bin == "playerctl")
            && result.stderr.to_ascii_lowercase().contains("no players");
        if !quiet_no_players {
            warn!(
                code = result.returncode,
                argv = ?result.argv,
                stderr = %result.stderr.trim(),
                "command exited non-zero"
            );
        }
        if check {
            return Err(Error::CommandFailed {
                argv: owned,
                code: result.returncode,
                stderr: result.stderr,
            });
        }
    }

    Ok(result)
}

/// Fire-and-forget spawn (power menu, lock, etc.).
pub fn spawn(argv: &[&str]) -> Result<()> {
    if argv.is_empty() {
        return Err(Error::Message("empty command".into()));
    }
    let owned: Vec<String> = argv.iter().map(|s| (*s).to_string()).collect();
    Command::new(&owned[0])
        .args(&owned[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| Error::CommandSpawn {
            argv: owned,
            source,
        })?;
    Ok(())
}
