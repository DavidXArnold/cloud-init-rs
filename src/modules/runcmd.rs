//! Runcmd module - execute commands from cloud-config
//!
//! Supports configurable shell selection and error handling modes via `runcmd_config`.
//!
//! # Shell Selection
//!
//! By default, shell string commands are executed with `/bin/sh -c`. Users can
//! override this by specifying a custom shell in `runcmd_config.shell`.
//!
//! # Error Handling Modes
//!
//! - `continue` (default): log failures and continue executing remaining commands.
//! - `abort`: stop execution immediately on the first command failure.

use crate::CloudInitError;
use crate::config::{ErrorHandlingMode, RunCmd, RuncmdConfig};
use tracing::{debug, info, warn};

/// Default shell used for shell string commands.
const DEFAULT_SHELL: &str = "/bin/sh";

/// Execute runcmd directives with optional configuration for shell and error handling.
pub async fn execute_runcmd(
    commands: &[RunCmd],
    config: Option<&RuncmdConfig>,
) -> Result<(), CloudInitError> {
    if commands.is_empty() {
        return Ok(());
    }

    let shell = config
        .and_then(|c| c.shell.as_deref())
        .unwrap_or(DEFAULT_SHELL);
    let error_mode = config
        .and_then(|c| c.error_handling.as_ref())
        .cloned()
        .unwrap_or_default();

    info!(
        "Executing {} runcmd commands (shell={}, error_handling={:?})",
        commands.len(),
        shell,
        error_mode
    );

    for (i, cmd) in commands.iter().enumerate() {
        debug!("Executing command {}/{}", i + 1, commands.len());
        match execute_command(cmd, shell).await {
            Ok(()) => {}
            Err(e) => match error_mode {
                ErrorHandlingMode::Abort => {
                    return Err(e);
                }
                ErrorHandlingMode::Continue => {
                    warn!(
                        "Command {}/{} failed (continuing): {}",
                        i + 1,
                        commands.len(),
                        e
                    );
                }
            },
        }
    }

    Ok(())
}

async fn execute_command(cmd: &RunCmd, shell: &str) -> Result<(), CloudInitError> {
    let output = match cmd {
        RunCmd::Shell(shell_cmd) => {
            debug!("Running shell command via {shell}: {shell_cmd}");
            tokio::process::Command::new(shell)
                .args(["-c", shell_cmd])
                .output()
                .await
                .map_err(|e| CloudInitError::Command(format!("{shell}: {e}")))?
        }
        RunCmd::Args(args) => {
            if args.is_empty() {
                return Ok(());
            }
            debug!("Running command: {args:?}");
            tokio::process::Command::new(&args[0])
                .args(&args[1..])
                .output()
                .await
                .map_err(|e| CloudInitError::Command(e.to_string()))?
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);
        return Err(CloudInitError::Command(format!(
            "command exited with status {exit_code}: {}",
            stderr.trim()
        )));
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
    use crate::config::{ErrorHandlingMode, RunCmd, RuncmdConfig};

    // ==================== Shell Selection Tests ====================

    #[tokio::test]
    async fn test_execute_runcmd_default_shell() {
        let commands = vec![RunCmd::Shell("echo hello".to_string())];
        let result = execute_runcmd(&commands, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_custom_shell_bash() {
        let config = RuncmdConfig {
            shell: Some("/bin/bash".to_string()),
            error_handling: None,
        };
        let commands = vec![RunCmd::Shell("echo hello".to_string())];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_custom_shell_sh() {
        let config = RuncmdConfig {
            shell: Some("/bin/sh".to_string()),
            error_handling: None,
        };
        let commands = vec![RunCmd::Shell("echo test".to_string())];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_invalid_shell() {
        let config = RuncmdConfig {
            shell: Some("/nonexistent/shell".to_string()),
            error_handling: None,
        };
        let commands = vec![RunCmd::Shell("echo hello".to_string())];
        // With default continue mode, this should still return Ok
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_runcmd_invalid_shell_abort() {
        let config = RuncmdConfig {
            shell: Some("/nonexistent/shell".to_string()),
            error_handling: Some(ErrorHandlingMode::Abort),
        };
        let commands = vec![RunCmd::Shell("echo hello".to_string())];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shell_selection_does_not_affect_args_commands() {
        let config = RuncmdConfig {
            shell: Some("/bin/bash".to_string()),
            error_handling: None,
        };
        let commands = vec![RunCmd::Args(vec!["echo".to_string(), "hello".to_string()])];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    // ==================== Error Handling Mode Tests ====================

    #[tokio::test]
    async fn test_continue_mode_runs_all_commands() {
        let config = RuncmdConfig {
            shell: None,
            error_handling: Some(ErrorHandlingMode::Continue),
        };
        let commands = vec![
            RunCmd::Shell("exit 1".to_string()),
            RunCmd::Shell("echo success".to_string()),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_abort_mode_stops_on_failure() {
        let config = RuncmdConfig {
            shell: None,
            error_handling: Some(ErrorHandlingMode::Abort),
        };
        let commands = vec![
            RunCmd::Shell("exit 1".to_string()),
            RunCmd::Shell("echo should-not-run".to_string()),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_abort_mode_succeeds_when_all_pass() {
        let config = RuncmdConfig {
            shell: None,
            error_handling: Some(ErrorHandlingMode::Abort),
        };
        let commands = vec![
            RunCmd::Shell("echo one".to_string()),
            RunCmd::Shell("echo two".to_string()),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_default_mode_is_continue() {
        let commands = vec![
            RunCmd::Shell("exit 1".to_string()),
            RunCmd::Shell("echo success".to_string()),
        ];
        let result = execute_runcmd(&commands, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_abort_mode_with_args_failure() {
        let config = RuncmdConfig {
            shell: None,
            error_handling: Some(ErrorHandlingMode::Abort),
        };
        let commands = vec![
            RunCmd::Args(vec!["false".to_string()]),
            RunCmd::Shell("echo should-not-run".to_string()),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_err());
    }

    // ==================== Edge Case Tests ====================

    #[tokio::test]
    async fn test_empty_commands() {
        let result = execute_runcmd(&[], None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_empty_args_array_skipped() {
        let commands = vec![RunCmd::Args(vec![])];
        let result = execute_runcmd(&commands, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mixed_commands_continue_mode() {
        let config = RuncmdConfig {
            shell: None,
            error_handling: Some(ErrorHandlingMode::Continue),
        };
        let commands = vec![
            RunCmd::Shell("echo first".to_string()),
            RunCmd::Shell("exit 42".to_string()),
            RunCmd::Args(vec!["echo".to_string(), "third".to_string()]),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mixed_commands_abort_mode() {
        let config = RuncmdConfig {
            shell: None,
            error_handling: Some(ErrorHandlingMode::Abort),
        };
        let commands = vec![
            RunCmd::Shell("echo first".to_string()),
            RunCmd::Shell("exit 42".to_string()),
            RunCmd::Args(vec!["echo".to_string(), "should-not-run".to_string()]),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("status 42"));
    }

    #[tokio::test]
    async fn test_config_with_shell_and_abort() {
        let config = RuncmdConfig {
            shell: Some("/bin/bash".to_string()),
            error_handling: Some(ErrorHandlingMode::Abort),
        };
        let commands = vec![
            RunCmd::Shell("echo ok".to_string()),
            RunCmd::Shell("exit 1".to_string()),
        ];
        let result = execute_runcmd(&commands, Some(&config)).await;
        assert!(result.is_err());
    }
}
