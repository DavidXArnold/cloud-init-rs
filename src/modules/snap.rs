//! Snap package management module
//!
//! Handles snap package installation, snap assertions, and snap command execution.
//! Supports classic and devmode confinement options.

use crate::CloudInitError;
use crate::config::{RunCmd, SnapConfig};
use tracing::{debug, info, warn};

/// Check if snapd is available on the system
async fn snapd_available() -> bool {
    tokio::process::Command::new("which")
        .arg("snap")
        .output()
        .await
        .is_ok_and(|o| o.status.success())
}

/// Apply snap assertions
///
/// Each assertion is fed to `snap ack` via stdin.
async fn apply_assertions(assertions: &[String]) -> Result<(), CloudInitError> {
    if assertions.is_empty() {
        return Ok(());
    }

    info!("Applying {} snap assertion(s)", assertions.len());

    for (i, assertion) in assertions.iter().enumerate() {
        debug!("Applying snap assertion {}", i + 1);

        let mut child = tokio::process::Command::new("snap")
            .arg("ack")
            .arg("/dev/stdin")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| CloudInitError::Command(e.to_string()))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(assertion.as_bytes())
                .await
                .map_err(|e| CloudInitError::Command(e.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| CloudInitError::Command(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CloudInitError::Module {
                module: "snap".to_string(),
                message: format!("Failed to apply snap assertion {}: {}", i + 1, stderr),
            });
        }
    }

    info!(
        "Successfully applied {} snap assertion(s)",
        assertions.len()
    );
    Ok(())
}

/// Run snap commands
///
/// Each command can be a shell string or an argument list.
async fn run_commands(commands: &[RunCmd]) -> Result<(), CloudInitError> {
    if commands.is_empty() {
        return Ok(());
    }

    info!("Running {} snap command(s)", commands.len());

    for (i, cmd) in commands.iter().enumerate() {
        debug!("Running snap command {}: {:?}", i + 1, cmd);

        let output = match cmd {
            RunCmd::Shell(shell_cmd) => tokio::process::Command::new("sh")
                .arg("-c")
                .arg(shell_cmd)
                .output()
                .await
                .map_err(|e| CloudInitError::Command(e.to_string()))?,
            RunCmd::Args(args) => {
                if args.is_empty() {
                    continue;
                }
                tokio::process::Command::new(&args[0])
                    .args(&args[1..])
                    .output()
                    .await
                    .map_err(|e| CloudInitError::Command(e.to_string()))?
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CloudInitError::Module {
                module: "snap".to_string(),
                message: format!("Snap command {} failed: {}", i + 1, stderr),
            });
        }
    }

    info!("Successfully ran {} snap command(s)", commands.len());
    Ok(())
}

/// Apply snap configuration (assertions then commands)
///
/// If snapd is not available on this system, the configuration is skipped with a warning.
pub async fn apply_snap(config: &SnapConfig) -> Result<(), CloudInitError> {
    if !snapd_available().await {
        warn!("snapd not available, skipping snap configuration");
        return Ok(());
    }

    // Assertions must be applied before installing snaps that depend on them
    apply_assertions(&config.assertions).await?;
    run_commands(&config.commands).await?;
    Ok(())
}
