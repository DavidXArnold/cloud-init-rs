//! Write files module

use crate::CloudInitError;
use crate::config::WriteFileConfig;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Write files from cloud-config
pub async fn write_files(files: &[WriteFileConfig]) -> Result<(), CloudInitError> {
    for file in files {
        write_file(file).await?;
    }
    Ok(())
}

async fn write_file(config: &WriteFileConfig) -> Result<(), CloudInitError> {
    info!("Writing file: {}", config.path);

    let path = Path::new(&config.path);

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(CloudInitError::Io)?;
    }

    // Decode content if needed
    let content = match config.encoding.as_deref() {
        Some("base64") | Some("b64") => {
            let decoded = BASE64
                .decode(&config.content)
                .map_err(|e| CloudInitError::InvalidData(format!("Invalid base64: {}", e)))?;
            String::from_utf8(decoded)
                .map_err(|e| CloudInitError::InvalidData(format!("Invalid UTF-8: {}", e)))?
        }
        Some("gzip") | Some("gz") => {
            // TODO: Implement gzip decompression
            return Err(CloudInitError::InvalidData(
                "gzip encoding not yet supported".into(),
            ));
        }
        _ => config.content.clone(),
    };

    // Write or append
    if config.append == Some(true) {
        let mut existing = fs::read_to_string(path).await.unwrap_or_default();
        existing.push_str(&content);
        fs::write(path, existing)
            .await
            .map_err(CloudInitError::Io)?;
    } else {
        fs::write(path, &content)
            .await
            .map_err(CloudInitError::Io)?;
    }

    // Set permissions
    if let Some(perms) = &config.permissions {
        set_permissions(path, perms).await?;
    }

    // Set ownership
    if let Some(owner) = &config.owner {
        set_ownership(path, owner).await?;
    }

    Ok(())
}

async fn set_permissions(path: &Path, perms: &str) -> Result<(), CloudInitError> {
    debug!("Setting permissions {} on {:?}", perms, path);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Parse octal permission string (e.g., "0644")
        let mode = u32::from_str_radix(perms.trim_start_matches('0'), 8)
            .map_err(|e| CloudInitError::InvalidData(format!("Invalid permissions: {}", e)))?;

        fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .await
            .map_err(CloudInitError::Io)?;
    }

    Ok(())
}

async fn set_ownership(path: &Path, owner: &str) -> Result<(), CloudInitError> {
    debug!("Setting ownership {} on {:?}", owner, path);

    let output = tokio::process::Command::new("chown")
        .args([owner, &path.to_string_lossy()])
        .output()
        .await
        .map_err(|e| CloudInitError::Command(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CloudInitError::Command(format!(
            "Failed to set ownership: {}",
            stderr
        )));
    }

    Ok(())
}
