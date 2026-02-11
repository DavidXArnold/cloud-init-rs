//! Write files module

use crate::CloudInitError;
use crate::config::WriteFileConfig;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use flate2::read::GzDecoder;
use std::io::Read;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info};

/// Write files from cloud-config
pub async fn write_files(files: &[WriteFileConfig]) -> Result<(), CloudInitError> {
    for file in files {
        // Skip deferred files - they'll be written later
        if file.defer == Some(true) {
            debug!("Deferring write of: {}", file.path);
            continue;
        }
        write_file(file).await?;
    }
    Ok(())
}

/// Write deferred files (called in final stage)
pub async fn write_deferred_files(files: &[WriteFileConfig]) -> Result<(), CloudInitError> {
    for file in files {
        if file.defer == Some(true) {
            write_file(file).await?;
        }
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

    // Decode content based on encoding
    let content = decode_content(&config.content, config.encoding.as_deref())?;

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

    // Set permissions (default to 0644 if not specified)
    let perms = config.permissions.as_deref().unwrap_or("0644");
    set_permissions(path, perms).await?;

    // Set ownership
    if let Some(owner) = &config.owner {
        set_ownership(path, owner).await?;
    }

    Ok(())
}

/// Decode content based on encoding type
fn decode_content(content: &str, encoding: Option<&str>) -> Result<String, CloudInitError> {
    match encoding {
        Some("base64") | Some("b64") => {
            let decoded = BASE64
                .decode(content)
                .map_err(|e| CloudInitError::InvalidData(format!("Invalid base64: {}", e)))?;
            String::from_utf8(decoded)
                .map_err(|e| CloudInitError::InvalidData(format!("Invalid UTF-8: {}", e)))
        }
        Some("gzip") | Some("gz") => {
            // Content is raw gzip bytes (unusual but supported)
            decompress_gzip(content.as_bytes())
        }
        Some("gz+base64") | Some("gzip+base64") | Some("gz+b64") => {
            // Base64-encoded gzip data (most common)
            let decoded = BASE64
                .decode(content)
                .map_err(|e| CloudInitError::InvalidData(format!("Invalid base64: {}", e)))?;
            decompress_gzip(&decoded)
        }
        Some("b64+gzip") | Some("base64+gzip") => {
            // Same as above, alternate naming
            let decoded = BASE64
                .decode(content)
                .map_err(|e| CloudInitError::InvalidData(format!("Invalid base64: {}", e)))?;
            decompress_gzip(&decoded)
        }
        Some(other) => Err(CloudInitError::InvalidData(format!(
            "Unknown encoding: {}",
            other
        ))),
        None => Ok(content.to_string()),
    }
}

/// Decompress gzip data
fn decompress_gzip(data: &[u8]) -> Result<String, CloudInitError> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = String::new();
    decoder
        .read_to_string(&mut decompressed)
        .map_err(|e| CloudInitError::InvalidData(format!("Failed to decompress gzip: {}", e)))?;
    Ok(decompressed)
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
