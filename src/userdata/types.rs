//! User-data content type detection and handling
//!
//! Detects the type of user-data based on magic bytes, headers, or content.

use std::fmt;

/// Content types supported by cloud-init
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    /// Cloud-config YAML (#cloud-config)
    CloudConfig,
    /// Shell script (#! or #!/)
    Script,
    /// Include file (list of URLs to fetch)
    IncludeUrl,
    /// Cloud boothook (runs very early)
    CloudBoothook,
    /// Gzip compressed data
    Gzip,
    /// MIME multipart message
    Multipart,
    /// Jinja template (## template: jinja)
    JinjaTemplate,
    /// Base64 encoded data
    Base64,
    /// Part handler (Python - not supported)
    PartHandler,
    /// Upstart job (legacy)
    UpstartJob,
    /// Unknown/binary data
    Unknown,
}

impl ContentType {
    /// Get the MIME type string for this content type
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::CloudConfig => "text/cloud-config",
            Self::Script => "text/x-shellscript",
            Self::IncludeUrl => "text/x-include-url",
            Self::CloudBoothook => "text/cloud-boothook",
            Self::Gzip => "application/x-gzip",
            Self::Multipart => "multipart/mixed",
            Self::JinjaTemplate => "text/jinja2",
            Self::Base64 => "text/plain",
            Self::PartHandler => "text/part-handler",
            Self::UpstartJob => "text/upstart-job",
            Self::Unknown => "application/octet-stream",
        }
    }

    /// Parse content type from MIME type string
    pub fn from_mime(mime: &str) -> Self {
        let mime = mime.to_lowercase();
        let mime = mime.split(';').next().unwrap_or(&mime).trim();

        match mime {
            "text/cloud-config" | "text/x-cloud-config" => Self::CloudConfig,
            "text/x-shellscript" | "text/x-sh" => Self::Script,
            "text/x-include-url" | "text/x-include-once-url" => Self::IncludeUrl,
            "text/cloud-boothook" => Self::CloudBoothook,
            "application/x-gzip" | "application/gzip" => Self::Gzip,
            "text/jinja2" | "text/x-jinja2" => Self::JinjaTemplate,
            "text/part-handler" => Self::PartHandler,
            "text/upstart-job" => Self::UpstartJob,
            s if s.starts_with("multipart/") => Self::Multipart,
            _ => Self::Unknown,
        }
    }

    /// Detect content type from data (magic bytes and headers)
    pub fn detect(data: &[u8]) -> Self {
        // Check for gzip magic bytes first
        if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
            return Self::Gzip;
        }

        // Try to interpret as text
        let text = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => {
                // Check if it might be base64
                if looks_like_base64(data) {
                    return Self::Base64;
                }
                return Self::Unknown;
            }
        };

        Self::detect_from_text(text)
    }

    /// Detect content type from text content
    pub fn detect_from_text(text: &str) -> Self {
        let trimmed = text.trim_start();

        // Check for cloud-config header
        if trimmed.starts_with("#cloud-config") {
            return Self::CloudConfig;
        }

        // Check for Jinja template marker
        if trimmed.starts_with("## template: jinja") || trimmed.starts_with("## template:jinja") {
            return Self::JinjaTemplate;
        }

        // Check for cloud-boothook
        if trimmed.starts_with("#cloud-boothook") {
            return Self::CloudBoothook;
        }

        // Check for include URLs
        if trimmed.starts_with("#include") {
            return Self::IncludeUrl;
        }

        // Check for upstart job
        if trimmed.starts_with("#upstart-job") {
            return Self::UpstartJob;
        }

        // Check for part handler
        if trimmed.starts_with("#part-handler") {
            return Self::PartHandler;
        }

        // Check for shebang (script)
        if trimmed.starts_with("#!") {
            return Self::Script;
        }

        // Check for MIME multipart
        if trimmed.starts_with("Content-Type:") || trimmed.starts_with("MIME-Version:") {
            // Look for multipart boundary
            if trimmed.contains("multipart/") {
                return Self::Multipart;
            }
        }

        // Check if it looks like YAML without the header
        if looks_like_yaml(trimmed) {
            return Self::CloudConfig;
        }

        Self::Unknown
    }

    /// Check if this content type should be processed as cloud-config
    pub fn is_cloud_config(&self) -> bool {
        matches!(self, Self::CloudConfig | Self::JinjaTemplate)
    }

    /// Check if this content type is executable
    pub fn is_executable(&self) -> bool {
        matches!(self, Self::Script | Self::CloudBoothook)
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.mime_type())
    }
}

/// Check if data looks like base64 encoded content
fn looks_like_base64(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    // Must have NO invalid characters (not just 90%)
    // Base64 only contains A-Z, a-z, 0-9, +, /, =, and whitespace
    let all_valid = data.iter().all(|&b| {
        b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=' || b.is_ascii_whitespace()
    });

    if !all_valid {
        return false;
    }

    // Also check it doesn't look like plain English text
    // (base64 rarely has spaces between words or common punctuation patterns)
    let non_whitespace: Vec<u8> = data
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();

    // Should have some actual base64 content
    if non_whitespace.is_empty() {
        return false;
    }

    // Base64 strings are typically longer and have mixed case/numbers
    // Simple heuristic: if it has spaces that split words, it's probably text
    let has_word_spaces = data
        .windows(3)
        .any(|w| w[0].is_ascii_alphabetic() && w[1] == b' ' && w[2].is_ascii_alphabetic());

    !has_word_spaces
}

/// Check if text looks like YAML content
fn looks_like_yaml(text: &str) -> bool {
    // Skip empty or comment-only content
    let meaningful_lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if meaningful_lines.is_empty() {
        return false;
    }

    // Check for common YAML patterns
    meaningful_lines.iter().any(|line| {
        // Key: value pattern
        line.contains(": ")
            // List item pattern
            || line.starts_with("- ")
            // YAML document start
            || *line == "---"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cloud_config() {
        assert_eq!(
            ContentType::detect(b"#cloud-config\nhostname: test"),
            ContentType::CloudConfig
        );
        assert_eq!(
            ContentType::detect(b"  #cloud-config\n"),
            ContentType::CloudConfig
        );
    }

    #[test]
    fn test_detect_script() {
        assert_eq!(
            ContentType::detect(b"#!/bin/bash\necho hello"),
            ContentType::Script
        );
        assert_eq!(
            ContentType::detect(b"#!/usr/bin/env python3\nprint('hi')"),
            ContentType::Script
        );
    }

    #[test]
    fn test_detect_boothook() {
        assert_eq!(
            ContentType::detect(b"#cloud-boothook\n#!/bin/bash\necho early"),
            ContentType::CloudBoothook
        );
    }

    #[test]
    fn test_detect_include() {
        assert_eq!(
            ContentType::detect(b"#include\nhttp://example.com/config.yaml"),
            ContentType::IncludeUrl
        );
    }

    #[test]
    fn test_detect_jinja() {
        assert_eq!(
            ContentType::detect(b"## template: jinja\n#cloud-config\nhostname: {{ ds.hostname }}"),
            ContentType::JinjaTemplate
        );
    }

    #[test]
    fn test_detect_gzip() {
        // Gzip magic bytes
        assert_eq!(
            ContentType::detect(&[0x1f, 0x8b, 0x08, 0x00]),
            ContentType::Gzip
        );
    }

    #[test]
    fn test_detect_yaml_without_header() {
        assert_eq!(
            ContentType::detect(b"hostname: myserver\npackages:\n  - nginx"),
            ContentType::CloudConfig
        );
    }

    #[test]
    fn test_from_mime() {
        assert_eq!(
            ContentType::from_mime("text/cloud-config"),
            ContentType::CloudConfig
        );
        assert_eq!(
            ContentType::from_mime("text/x-shellscript; charset=utf-8"),
            ContentType::Script
        );
        assert_eq!(
            ContentType::from_mime("multipart/mixed; boundary=abc"),
            ContentType::Multipart
        );
    }

    #[test]
    fn test_mime_type() {
        assert_eq!(ContentType::CloudConfig.mime_type(), "text/cloud-config");
        assert_eq!(ContentType::Script.mime_type(), "text/x-shellscript");
    }

    #[test]
    fn test_is_cloud_config() {
        assert!(ContentType::CloudConfig.is_cloud_config());
        assert!(ContentType::JinjaTemplate.is_cloud_config());
        assert!(!ContentType::Script.is_cloud_config());
    }

    #[test]
    fn test_is_executable() {
        assert!(ContentType::Script.is_executable());
        assert!(ContentType::CloudBoothook.is_executable());
        assert!(!ContentType::CloudConfig.is_executable());
    }

    #[test]
    fn test_looks_like_base64() {
        assert!(looks_like_base64(b"SGVsbG8gV29ybGQh"));
        assert!(looks_like_base64(b"SGVsbG8g\nV29ybGQh"));
        assert!(!looks_like_base64(b"Hello World!"));
    }
}
