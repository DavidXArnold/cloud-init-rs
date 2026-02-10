//! Final stage - runs user scripts and final tasks
//!
//! Responsibilities:
//! - Execute runcmd directives
//! - Run user scripts from scripts-user
//! - Phone home (notify completion)
//! - Final message

use crate::CloudInitError;
use tracing::{debug, info, warn};

/// Run the final stage
pub async fn run() -> Result<(), CloudInitError> {
    info!("Final stage: executing user scripts");

    // Execute runcmd
    execute_runcmd().await?;

    // Run user scripts
    run_user_scripts().await?;

    // Phone home if configured
    phone_home().await?;

    // Write final message
    write_final_message().await?;

    info!("Final stage: completed");
    Ok(())
}

async fn execute_runcmd() -> Result<(), CloudInitError> {
    debug!("Executing runcmd directives");
    // TODO: Parse and execute runcmd from cloud-config
    // Each command should be run via tokio::process::Command
    Ok(())
}

async fn run_user_scripts() -> Result<(), CloudInitError> {
    debug!("Running user scripts");
    // Check for scripts in:
    // - /var/lib/cloud/scripts/per-boot/
    // - /var/lib/cloud/scripts/per-instance/
    // - /var/lib/cloud/scripts/per-once/
    Ok(())
}

async fn phone_home() -> Result<(), CloudInitError> {
    debug!("Checking for phone_home configuration");
    // TODO: POST to configured URL with instance data
    Ok(())
}

async fn write_final_message() -> Result<(), CloudInitError> {
    debug!("Writing final message");
    // Write completion status to /run/cloud-init/result.json
    let result = serde_json::json!({
        "v1": {
            "datasource": null,
            "errors": []
        }
    });

    let result_path = "/run/cloud-init/result.json";

    // Only write if we have permissions (likely won't during development)
    match tokio::fs::create_dir_all("/run/cloud-init").await {
        Ok(_) => {
            if let Err(e) = tokio::fs::write(result_path, result.to_string()).await {
                warn!("Could not write result file: {}", e);
            }
        }
        Err(e) => {
            debug!("Could not create cloud-init run directory: {}", e);
        }
    }

    Ok(())
}
