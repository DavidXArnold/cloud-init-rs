//! Runcmd module - execute commands from cloud-config

use crate::CloudInitError;
use crate::config::RunCmd;
use tracing::{debug, info, warn};

/// Execute runcmd directives
pub async fn execute_runcmd(commands: &[RunCmd]) -> Result<(), CloudInitError> {
    info!("Executing {} runcmd commands", commands.len());

    for (i, cmd) in commands.iter().enumerate() {
        debug!("Executing command {}/{}", i + 1, commands.len());
        execute_command(cmd).await?;
    }

    Ok(())
}

async fn execute_command(cmd: &RunCmd) -> Result<(), CloudInitError> {
    let output = match cmd {
        RunCmd::Shell(shell_cmd) => {
            debug!("Running shell command: {}", shell_cmd);
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
            debug!("Running command: {:?}", args);
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
            "Command exited with status {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        );
        // Note: We don't return an error here because cloud-init
        // traditionally continues even if individual commands fail
    }

    // Log stdout for debugging
    if !output.stdout.is_empty() {
        debug!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_runcmd_shell() {
        let cmds = vec![RunCmd::Shell("echo hello".to_string())];
        assert!(execute_runcmd(&cmds).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_args() {
        let cmds = vec![RunCmd::Args(vec!["echo".to_string(), "hello".to_string()])];
        assert!(execute_runcmd(&cmds).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_empty_args() {
        let cmds = vec![RunCmd::Args(vec![])];
        assert!(execute_runcmd(&cmds).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_multiple() {
        let cmds = vec![
            RunCmd::Shell("echo first".to_string()),
            RunCmd::Args(vec!["echo".to_string(), "second".to_string()]),
        ];
        assert!(execute_runcmd(&cmds).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_failed_command_continues() {
        let cmds = vec![
            RunCmd::Shell("false".to_string()),
            RunCmd::Shell("echo still_running".to_string()),
        ];
        assert!(execute_runcmd(&cmds).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_with_stdout() {
        let cmds = vec![RunCmd::Shell("echo 'output'".to_string())];
        assert!(execute_runcmd(&cmds).await.is_ok());
    }

    #[tokio::test]
    async fn test_execute_command_shell_direct() {
        assert!(
            execute_command(&RunCmd::Shell("true".to_string()))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_execute_command_args_direct() {
        assert!(
            execute_command(&RunCmd::Args(vec!["true".to_string()]))
                .await
                .is_ok()
        );
    }
}
