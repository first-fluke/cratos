use super::config::{ExecConfig, ExecHost};
use crate::error::{Error, Result};
use std::process::Stdio;
use tokio::process::Command;

pub async fn run_command(
    config: &ExecConfig,
    host: ExecHost,
    command: &str,
    args: &[String],
    cwd: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, String, i32, bool)> {
    let mut cmd = match host {
        ExecHost::Local => {
            let mut c = Command::new(command);
            c.args(args);
            if let Some(dir) = cwd {
                c.current_dir(dir);
            }
            c
        }
        ExecHost::Sandbox => {
            let mut c = Command::new("docker");
            c.arg("run")
                .arg("--rm")
                .arg("--network=none")
                .arg("--read-only")
                .arg("--tmpfs=/tmp:rw,noexec,nosuid,size=64m")
                .arg(format!("--memory={}", config.sandbox_memory_limit))
                .arg(format!("--cpus={}", config.sandbox_cpu_limit))
                .arg("--pids-limit=64")
                .arg("--security-opt=no-new-privileges");
            if let Some(dir) = cwd {
                c.arg("-w").arg(dir);
            }
            c.arg(&config.sandbox_image).arg(command).args(args);
            c
        }
    };
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| Error::Execution(e.to_string()))?;

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| Error::Timeout(timeout_secs * 1000))?
    .map_err(|e| Error::Execution(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok((stdout, stderr, exit_code, output.status.success()))
}
