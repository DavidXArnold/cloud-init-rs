//! Bootcmd module - execute early boot commands
//!
//! These commands run very early in the boot process, before most other
//! cloud-init modules. They should be used sparingly and only when
//! necessary for early system configuration.

use crate::CloudInitError;
use crate::config::RunCmd;
use tracing::{debug, info, warn};

/// Execute bootcmd directives (early boot commands)
pub async fn execute_bootcmd(commands: &[RunCmd]) -> Result<(), CloudInitError> {
    if commands.is_empty() {
        return Ok(());
    }

    info!("Executing {} bootcmd commands", commands.len());

    for (i, cmd) in commands.iter().enumerate() {
        debug!("Executing bootcmd {}/{}", i + 1, commands.len());
        execute_command(cmd).await?;
    }

    Ok(())
}

async fn execute_command(cmd: &RunCmd) -> Result<(), CloudInitError> {
    let output = match cmd {
        RunCmd::Shell(shell_cmd) => {
            debug!("Running bootcmd shell command: {}", shell_cmd);
            tokio::process::Command::new("sh")
                .args(["-c", shell_cmd])
                .output()
                .await
                .map_err(|e| CloudInitError::Command(e.to_string()))?
        }
        RunCmd::Args(args) => {
            if args.is_empty() {
                return Ok(());
            }
            debug!("Running bootcmd: {:?}", args);
            tokio::process::Command::new(&args[0])
                .args(&args[1..])
                .output()
                .await
                .map_err(|e| CloudInitError::Command(e.to_string()))?
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Bootcmd exited with status {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        );
        // bootcmd failures are typically non-fatal
    }

    if !output.stdout.is_empty() {
        debug!(
            "bootcmd stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    Ok(())
}
