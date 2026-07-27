use anyhow::{bail, Context, Result};
use std::{
    fs::{self, File},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct ResultData {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

struct Capture {
    stdout: std::path::PathBuf,
    stderr: std::path::PathBuf,
}

impl Capture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let stem = format!("pkg-audit-{}-{nonce}", std::process::id());
        Self {
            stdout: std::env::temp_dir().join(format!("{stem}.out")),
            stderr: std::env::temp_dir().join(format!("{stem}.err")),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        fs::remove_file(&self.stdout).ok();
        fs::remove_file(&self.stderr).ok();
    }
}

pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<ResultData> {
    let capture = Capture::new();
    let stdout = File::create(&capture.stdout)?;
    let stderr = File::create(&capture.stderr)?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("could not start {program}"))?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().ok();
            child.wait().ok();
            bail!("{program} timed out after {} seconds", timeout.as_secs());
        }
        thread::sleep(Duration::from_millis(50));
    };
    Ok(ResultData {
        code: status.code().unwrap_or(-1),
        stdout: fs::read_to_string(&capture.stdout)?,
        stderr: fs::read_to_string(&capture.stderr)?,
    })
}

pub fn checked(program: &str, args: &[&str], timeout: Duration) -> Result<ResultData> {
    let result = run(program, args, timeout)?;
    if result.code != 0 {
        bail!("{program} failed: {}", result.stderr.trim());
    }
    Ok(result)
}

pub fn lines(program: &str, args: &[&str]) -> Result<Vec<String>> {
    let result = checked(program, args, Duration::from_secs(30))?;
    Ok(result
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}
