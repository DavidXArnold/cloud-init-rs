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

pub async fn write_file(config: &WriteFileConfig) -> Result<(), CloudInitError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_decode_content_no_encoding() {
        assert_eq!(decode_content("hello world", None).unwrap(), "hello world");
    }

    #[test]
    fn test_decode_content_base64() {
        use base64::Engine;
        let encoded = BASE64.encode("decoded text");
        assert_eq!(
            decode_content(&encoded, Some("base64")).unwrap(),
            "decoded text"
        );
    }

    #[test]
    fn test_decode_content_b64_alias() {
        use base64::Engine;
        let encoded = BASE64.encode("b64 alias");
        assert_eq!(decode_content(&encoded, Some("b64")).unwrap(), "b64 alias");
    }

    #[test]
    fn test_decode_content_invalid_base64() {
        let result = decode_content("not-valid-base64!!!", Some("base64"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid base64"));
    }

    #[test]
    fn test_decode_content_gz_base64() {
        use base64::Engine;
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"compressed text").unwrap();
        let compressed = encoder.finish().unwrap();
        let encoded = BASE64.encode(&compressed);

        for enc in &["gz+base64", "gzip+base64", "gz+b64"] {
            assert_eq!(
                decode_content(&encoded, Some(enc)).unwrap(),
                "compressed text",
                "failed for encoding {enc}"
            );
        }
    }

    #[test]
    fn test_decode_content_base64_gzip_alias() {
        use base64::Engine;
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"alt order").unwrap();
        let compressed = encoder.finish().unwrap();
        let encoded = BASE64.encode(&compressed);

        for enc in &["b64+gzip", "base64+gzip"] {
            assert_eq!(
                decode_content(&encoded, Some(enc)).unwrap(),
                "alt order",
                "failed for encoding {enc}"
            );
        }
    }

    #[test]
    fn test_decode_content_gzip_raw() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"raw gz").unwrap();
        let compressed = encoder.finish().unwrap();
        assert_eq!(decompress_gzip(&compressed).unwrap(), "raw gz");
    }

    #[test]
    fn test_decode_content_unknown_encoding() {
        let result = decode_content("data", Some("rot13"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown encoding"));
    }

    #[test]
    fn test_decompress_gzip_invalid_data() {
        assert!(decompress_gzip(&[0x00, 0x01, 0x02]).is_err());
    }

    #[tokio::test]
    async fn test_write_file_basic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.txt");
        let config = WriteFileConfig {
            path: path.to_string_lossy().to_string(),
            content: "hello world".to_string(),
            encoding: None,
            owner: None,
            permissions: Some("0644".to_string()),
            append: None,
            defer: None,
        };
        write_file(&config).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&path).await.unwrap(),
            "hello world"
        );
    }

    #[tokio::test]
    async fn test_write_file_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a/b/c/deep.txt");
        let config = WriteFileConfig {
            path: path.to_string_lossy().to_string(),
            content: "nested".to_string(),
            encoding: None,
            owner: None,
            permissions: Some("0644".to_string()),
            append: None,
            defer: None,
        };
        write_file(&config).await.unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_write_file_append_mode() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("append.txt");
        tokio::fs::write(&path, "first\n").await.unwrap();
        let config = WriteFileConfig {
            path: path.to_string_lossy().to_string(),
            content: "second\n".to_string(),
            encoding: None,
            owner: None,
            permissions: Some("0644".to_string()),
            append: Some(true),
            defer: None,
        };
        write_file(&config).await.unwrap();
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("first") && content.contains("second"));
    }

    #[tokio::test]
    async fn test_write_file_append_to_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new_append.txt");
        let config = WriteFileConfig {
            path: path.to_string_lossy().to_string(),
            content: "content".to_string(),
            encoding: None,
            owner: None,
            permissions: Some("0644".to_string()),
            append: Some(true),
            defer: None,
        };
        write_file(&config).await.unwrap();
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "content");
    }

    #[tokio::test]
    async fn test_write_file_with_base64_encoding() {
        use base64::Engine;
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("b64.txt");
        let config = WriteFileConfig {
            path: path.to_string_lossy().to_string(),
            content: BASE64.encode("base64 content"),
            encoding: Some("base64".to_string()),
            owner: None,
            permissions: Some("0644".to_string()),
            append: None,
            defer: None,
        };
        write_file(&config).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&path).await.unwrap(),
            "base64 content"
        );
    }

    #[tokio::test]
    async fn test_write_file_default_permissions() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("default_perms.txt");
        let config = WriteFileConfig {
            path: path.to_string_lossy().to_string(),
            content: "data".to_string(),
            encoding: None,
            owner: None,
            permissions: None,
            append: None,
            defer: None,
        };
        write_file(&config).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o644);
        }
    }

    #[tokio::test]
    async fn test_write_files_skips_deferred() {
        let tmp = TempDir::new().unwrap();
        let normal_path = tmp.path().join("normal.txt");
        let deferred_path = tmp.path().join("deferred.txt");
        let files = vec![
            WriteFileConfig {
                path: normal_path.to_string_lossy().to_string(),
                content: "normal".to_string(),
                encoding: None,
                owner: None,
                permissions: Some("0644".to_string()),
                append: None,
                defer: None,
            },
            WriteFileConfig {
                path: deferred_path.to_string_lossy().to_string(),
                content: "deferred".to_string(),
                encoding: None,
                owner: None,
                permissions: Some("0644".to_string()),
                append: None,
                defer: Some(true),
            },
        ];
        write_files(&files).await.unwrap();
        assert!(normal_path.exists());
        assert!(!deferred_path.exists());
    }

    #[tokio::test]
    async fn test_write_deferred_files_only() {
        let tmp = TempDir::new().unwrap();
        let normal_path = tmp.path().join("normal.txt");
        let deferred_path = tmp.path().join("deferred.txt");
        let files = vec![
            WriteFileConfig {
                path: normal_path.to_string_lossy().to_string(),
                content: "normal".to_string(),
                encoding: None,
                owner: None,
                permissions: Some("0644".to_string()),
                append: None,
                defer: None,
            },
            WriteFileConfig {
                path: deferred_path.to_string_lossy().to_string(),
                content: "deferred".to_string(),
                encoding: None,
                owner: None,
                permissions: Some("0644".to_string()),
                append: None,
                defer: Some(true),
            },
        ];
        write_deferred_files(&files).await.unwrap();
        assert!(!normal_path.exists());
        assert!(deferred_path.exists());
    }

    #[tokio::test]
    async fn test_set_permissions_0755() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("exec.sh");
        tokio::fs::write(&path, "#!/bin/sh").await.unwrap();
        set_permissions(&path, "0755").await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }

    #[tokio::test]
    async fn test_set_permissions_0600() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("secret.key");
        tokio::fs::write(&path, "secret").await.unwrap();
        set_permissions(&path, "0600").await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn test_set_permissions_invalid() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("file.txt");
        tokio::fs::write(&path, "data").await.unwrap();
        assert!(set_permissions(&path, "not_octal").await.is_err());
    }

    #[tokio::test]
    async fn test_set_ownership_invalid_user() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("owned.txt");
        tokio::fs::write(&path, "data").await.unwrap();
        assert!(
            set_ownership(&path, "nonexistent_user_12345")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_write_files_empty() {
        write_files(&[]).await.unwrap();
    }

    #[tokio::test]
    async fn test_write_deferred_files_empty() {
        write_deferred_files(&[]).await.unwrap();
    }
}
