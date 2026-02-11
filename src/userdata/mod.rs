//! User-data parsing and processing
//!
//! Handles parsing of cloud-init user-data in various formats:
//! - Cloud-config YAML
//! - Shell scripts
//! - MIME multipart messages
//! - Gzip compressed data
//! - Include directives

pub mod mime;
pub mod types;

pub use mime::{MimePart, create_multipart, parse_multipart};
pub use types::ContentType;

use crate::{CloudInitError, UserData, UserDataPart, config::CloudConfig};
use base64::Engine;
use flate2::read::GzDecoder;
use std::io::Read;
use tracing::{debug, warn};

/// Parse raw user-data bytes into structured UserData
pub fn parse_userdata(data: &[u8]) -> Result<UserData, CloudInitError> {
    if data.is_empty() {
        return Ok(UserData::None);
    }

    // Detect and handle gzip compression
    let data = decompress_if_needed(data)?;

    // Detect content type
    let content_type = ContentType::detect(&data);
    debug!("Detected user-data content type: {}", content_type);

    // Convert to string for text processing
    let text = String::from_utf8_lossy(&data);

    match content_type {
        ContentType::CloudConfig | ContentType::JinjaTemplate => {
            let config = CloudConfig::from_yaml(&text)?;
            Ok(UserData::CloudConfig(Box::new(config)))
        }
        ContentType::Script | ContentType::CloudBoothook => Ok(UserData::Script(text.into_owned())),
        ContentType::Multipart => {
            let parts = parse_multipart(&text)?;
            let user_parts: Vec<UserDataPart> = parts
                .into_iter()
                .map(|p| UserDataPart {
                    content_type: p.mime_type,
                    content: p.content,
                    filename: p.filename,
                })
                .collect();
            Ok(UserData::MultiPart(user_parts))
        }
        ContentType::IncludeUrl => {
            // Parse include file and return as parts
            let parts = parse_include_urls(&text)?;
            if parts.is_empty() {
                Ok(UserData::None)
            } else {
                Ok(UserData::MultiPart(parts))
            }
        }
        ContentType::Gzip => {
            // Should have been handled by decompress_if_needed, but just in case
            Err(CloudInitError::InvalidData(
                "Gzip data could not be decompressed".to_string(),
            ))
        }
        ContentType::Base64 => {
            // Decode and re-parse
            let decoded = decode_base64(&text)?;
            parse_userdata(&decoded)
        }
        _ => {
            warn!("Unknown user-data type, treating as script");
            Ok(UserData::Script(text.into_owned()))
        }
    }
}

/// Decompress gzip data if needed
fn decompress_if_needed(data: &[u8]) -> Result<Vec<u8>, CloudInitError> {
    // Check for gzip magic bytes
    if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        debug!("Decompressing gzip user-data");
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).map_err(|e| {
            CloudInitError::InvalidData(format!("Gzip decompression failed: {}", e))
        })?;
        Ok(decompressed)
    } else {
        Ok(data.to_vec())
    }
}

/// Decode base64 data
fn decode_base64(data: &str) -> Result<Vec<u8>, CloudInitError> {
    // Remove whitespace and common header lines
    let cleaned: String = data
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");

    let cleaned: String = cleaned.chars().filter(|c| !c.is_whitespace()).collect();

    base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .map_err(|e| CloudInitError::InvalidData(format!("Base64 decode error: {}", e)))
}

/// Parse include URLs from user-data
fn parse_include_urls(data: &str) -> Result<Vec<UserDataPart>, CloudInitError> {
    let mut parts = Vec::new();

    for line in data.lines() {
        let line = line.trim();

        // Skip header and comments
        if line.is_empty() || line.starts_with("#include") || line.starts_with("#") {
            continue;
        }

        // Each line should be a URL
        if line.starts_with("http://") || line.starts_with("https://") {
            // Note: Actual URL fetching should be done by the caller
            // Here we just create placeholders
            parts.push(UserDataPart {
                content_type: "text/x-include-url".to_string(),
                content: line.to_string(),
                filename: None,
            });
        }
    }

    Ok(parts)
}

/// Process multipart user-data and merge cloud-configs
pub fn process_multipart(parts: &[UserDataPart]) -> ProcessedUserData {
    let mut cloud_configs = Vec::new();
    let mut scripts = Vec::new();
    let mut boothooks = Vec::new();
    let mut includes = Vec::new();

    for part in parts {
        let content_type = ContentType::from_mime(&part.content_type);

        match content_type {
            ContentType::CloudConfig | ContentType::JinjaTemplate => {
                cloud_configs.push(part.content.clone());
            }
            ContentType::Script => {
                scripts.push(ScriptPart {
                    content: part.content.clone(),
                    filename: part.filename.clone(),
                });
            }
            ContentType::CloudBoothook => {
                boothooks.push(ScriptPart {
                    content: part.content.clone(),
                    filename: part.filename.clone(),
                });
            }
            ContentType::IncludeUrl => {
                includes.push(part.content.clone());
            }
            _ => {
                debug!("Ignoring part with content type: {}", part.content_type);
            }
        }
    }

    ProcessedUserData {
        cloud_configs,
        scripts,
        boothooks,
        includes,
    }
}

/// Processed user-data with parts categorized
#[derive(Debug, Default)]
pub struct ProcessedUserData {
    /// Cloud-config YAML parts (to be merged)
    pub cloud_configs: Vec<String>,
    /// Shell scripts (to be executed in order)
    pub scripts: Vec<ScriptPart>,
    /// Cloud boothooks (to be executed early)
    pub boothooks: Vec<ScriptPart>,
    /// Include URLs (to be fetched and processed)
    pub includes: Vec<String>,
}

/// A script part with optional filename
#[derive(Debug, Clone)]
pub struct ScriptPart {
    pub content: String,
    pub filename: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cloud_config() {
        let data = b"#cloud-config\nhostname: test\npackages:\n  - nginx";
        let result = parse_userdata(data).unwrap();

        match result {
            UserData::CloudConfig(config) => {
                assert_eq!(config.hostname, Some("test".to_string()));
                assert_eq!(config.packages, vec!["nginx"]);
            }
            _ => panic!("Expected CloudConfig"),
        }
    }

    #[test]
    fn test_parse_script() {
        let data = b"#!/bin/bash\necho hello world";
        let result = parse_userdata(data).unwrap();

        match result {
            UserData::Script(script) => {
                assert!(script.contains("echo hello world"));
            }
            _ => panic!("Expected Script"),
        }
    }

    #[test]
    fn test_parse_empty() {
        let result = parse_userdata(b"").unwrap();
        assert!(matches!(result, UserData::None));
    }

    #[test]
    fn test_parse_gzip() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let original = b"#cloud-config\nhostname: compressed";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let result = parse_userdata(&compressed).unwrap();

        match result {
            UserData::CloudConfig(config) => {
                assert_eq!(config.hostname, Some("compressed".to_string()));
            }
            _ => panic!("Expected CloudConfig"),
        }
    }

    #[test]
    fn test_parse_multipart() {
        let data = br#"MIME-Version: 1.0
Content-Type: multipart/mixed; boundary="BOUNDARY"

--BOUNDARY
Content-Type: text/cloud-config

#cloud-config
hostname: test

--BOUNDARY
Content-Type: text/x-shellscript

#!/bin/bash
echo hello

--BOUNDARY--
"#;

        let result = parse_userdata(data).unwrap();

        match result {
            UserData::MultiPart(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(parts[0].content.contains("hostname: test"));
                assert!(parts[1].content.contains("echo hello"));
            }
            _ => panic!("Expected MultiPart"),
        }
    }

    #[test]
    fn test_process_multipart() {
        let parts = vec![
            UserDataPart {
                content_type: "text/cloud-config".to_string(),
                content: "#cloud-config\nhostname: test".to_string(),
                filename: None,
            },
            UserDataPart {
                content_type: "text/x-shellscript".to_string(),
                content: "#!/bin/bash\necho hello".to_string(),
                filename: Some("setup.sh".to_string()),
            },
            UserDataPart {
                content_type: "text/cloud-boothook".to_string(),
                content: "#!/bin/bash\necho early".to_string(),
                filename: None,
            },
        ];

        let processed = process_multipart(&parts);

        assert_eq!(processed.cloud_configs.len(), 1);
        assert_eq!(processed.scripts.len(), 1);
        assert_eq!(processed.boothooks.len(), 1);
        assert_eq!(processed.scripts[0].filename, Some("setup.sh".to_string()));
    }

    #[test]
    fn test_parse_include_urls() {
        let data = "#include\nhttps://example.com/config1.yaml\nhttps://example.com/config2.yaml";
        let parts = parse_include_urls(data).unwrap();

        assert_eq!(parts.len(), 2);
        assert!(parts[0].content.contains("config1.yaml"));
        assert!(parts[1].content.contains("config2.yaml"));
    }
}
