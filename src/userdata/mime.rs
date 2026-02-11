//! MIME multipart message parsing for cloud-init user-data
//!
//! Parses multipart MIME messages as used by cloud-init for combining
//! multiple user-data parts (scripts, configs, etc.)

use super::types::ContentType;
use crate::CloudInitError;
use std::collections::HashMap;
use tracing::debug;

/// A single part from a MIME multipart message
#[derive(Debug, Clone)]
pub struct MimePart {
    /// Content type of this part
    pub content_type: ContentType,
    /// Raw MIME type string
    pub mime_type: String,
    /// Content of this part
    pub content: String,
    /// Optional filename from Content-Disposition
    pub filename: Option<String>,
    /// Additional headers
    pub headers: HashMap<String, String>,
}

/// Parse a MIME multipart message into parts
pub fn parse_multipart(data: &str) -> Result<Vec<MimePart>, CloudInitError> {
    let mut parts = Vec::new();

    // Find the boundary
    let boundary = find_boundary(data)?;
    debug!("Found MIME boundary: {}", boundary);

    // Split by boundary
    let delimiter = format!("--{}", boundary);
    let sections: Vec<&str> = data.split(&delimiter).collect();

    for (i, section) in sections.iter().enumerate() {
        // Skip preamble (first section) and epilogue (after --)
        if i == 0 || section.trim().starts_with("--") || section.trim().is_empty() {
            continue;
        }

        // Parse this part
        if let Some(part) = parse_part(section.trim_start_matches(['\r', '\n']))? {
            parts.push(part);
        }
    }

    debug!("Parsed {} MIME parts", parts.len());
    Ok(parts)
}

/// Find the boundary string from MIME headers
#[allow(clippy::collapsible_if)]
fn find_boundary(data: &str) -> Result<String, CloudInitError> {
    // Look for Content-Type header with boundary
    for line in data.lines() {
        let line_lower = line.to_lowercase();

        if line_lower.starts_with("content-type:") && line_lower.contains("boundary=") {
            // Extract boundary value
            if let Some(boundary) = extract_boundary_value(line) {
                return Ok(boundary);
            }
        }

        // Also check continuation of Content-Type
        if line.trim_start().starts_with("boundary=") {
            if let Some(boundary) = extract_boundary_value(line) {
                return Ok(boundary);
            }
        }

        // Stop at first empty line (end of headers)
        if line.trim().is_empty() {
            break;
        }
    }

    // Try to find boundary from content (look for --BOUNDARY pattern)
    for line in data.lines() {
        if line.starts_with("--") && !line.starts_with("---") {
            let potential = line.trim_start_matches("--").trim();
            if !potential.is_empty() && !potential.contains(':') {
                return Ok(potential.to_string());
            }
        }
    }

    Err(CloudInitError::InvalidData(
        "No MIME boundary found".to_string(),
    ))
}

/// Extract boundary value from a header line
#[allow(clippy::manual_strip)]
fn extract_boundary_value(line: &str) -> Option<String> {
    // Handle: boundary="value" or boundary=value
    let lower = line.to_lowercase();
    let idx = lower.find("boundary=")?;
    let after = &line[idx + 9..];

    let boundary = if after.starts_with('"') {
        // Quoted value
        let end = after[1..].find('"')?;
        &after[1..=end]
    } else {
        // Unquoted value (ends at ; or whitespace or end of line)
        let end = after
            .find(|c: char| c == ';' || c.is_whitespace())
            .unwrap_or(after.len());
        &after[..end]
    };

    Some(boundary.to_string())
}

/// Parse a single MIME part
fn parse_part(data: &str) -> Result<Option<MimePart>, CloudInitError> {
    if data.trim().is_empty() {
        return Ok(None);
    }

    // Find the header/body separator (empty line)
    let (headers_str, body) = if let Some(idx) = data.find("\r\n\r\n") {
        (&data[..idx], &data[idx + 4..])
    } else if let Some(idx) = data.find("\n\n") {
        (&data[..idx], &data[idx + 2..])
    } else {
        // No headers, entire content is body
        ("", data)
    };

    // Parse headers
    let mut headers = HashMap::new();
    let mut current_header: Option<(String, String)> = None;

    for line in headers_str.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation of previous header
            if let Some((_, ref mut value)) = current_header {
                value.push(' ');
                value.push_str(line.trim());
            }
        } else if let Some((name, value)) = line.split_once(':') {
            // New header
            if let Some((n, v)) = current_header.take() {
                headers.insert(n.to_lowercase(), v);
            }
            current_header = Some((name.trim().to_string(), value.trim().to_string()));
        }
    }

    // Don't forget the last header
    if let Some((n, v)) = current_header {
        headers.insert(n.to_lowercase(), v);
    }

    // Get content type
    let mime_type = headers
        .get("content-type")
        .cloned()
        .unwrap_or_else(|| "text/plain".to_string());

    let content_type = ContentType::from_mime(&mime_type);

    // Get filename from Content-Disposition
    let filename = headers
        .get("content-disposition")
        .and_then(|cd| extract_filename(cd));

    // Handle content transfer encoding
    let content = match headers.get("content-transfer-encoding").map(|s| s.as_str()) {
        Some("base64") => decode_base64(body)?,
        Some("quoted-printable") => decode_quoted_printable(body),
        _ => body.to_string(),
    };

    Ok(Some(MimePart {
        content_type,
        mime_type,
        content,
        filename,
        headers,
    }))
}

/// Extract filename from Content-Disposition header
#[allow(clippy::manual_strip)]
fn extract_filename(cd: &str) -> Option<String> {
    // Handle: filename="name" or filename=name
    let lower = cd.to_lowercase();
    let idx = lower.find("filename=")?;
    let after = &cd[idx + 9..];

    let filename = if after.starts_with('"') {
        let end = after[1..].find('"')?;
        &after[1..=end]
    } else {
        let end = after
            .find(|c: char| c == ';' || c.is_whitespace())
            .unwrap_or(after.len());
        &after[..end]
    };

    Some(filename.to_string())
}

/// Decode base64 content
fn decode_base64(data: &str) -> Result<String, CloudInitError> {
    use base64::Engine;

    // Remove whitespace
    let clean: String = data.chars().filter(|c| !c.is_whitespace()).collect();

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&clean)
        .map_err(|e| CloudInitError::InvalidData(format!("Base64 decode error: {}", e)))?;

    String::from_utf8(decoded)
        .map_err(|e| CloudInitError::InvalidData(format!("UTF-8 decode error: {}", e)))
}

/// Decode quoted-printable content
#[allow(clippy::collapsible_if)]
fn decode_quoted_printable(data: &str) -> String {
    let mut result = String::new();
    let mut chars = data.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '=' {
            // Check for soft line break (=\r\n or =\n)
            if chars.peek() == Some(&'\r') {
                chars.next();
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                continue;
            }
            if chars.peek() == Some(&'\n') {
                chars.next();
                continue;
            }

            // Hex encoded byte
            let h1 = chars.next();
            let h2 = chars.next();

            if let (Some(h1), Some(h2)) = (h1, h2) {
                if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                    result.push(byte as char);
                    continue;
                }
            }

            // Invalid encoding, pass through
            result.push(c);
        } else {
            result.push(c);
        }
    }

    result
}

/// Create a MIME multipart message from parts
pub fn create_multipart(parts: &[MimePart], boundary: &str) -> String {
    let mut output = String::new();

    // MIME headers
    output.push_str("MIME-Version: 1.0\r\n");
    output.push_str(&format!(
        "Content-Type: multipart/mixed; boundary=\"{}\"\r\n",
        boundary
    ));
    output.push_str("\r\n");

    // Parts
    for part in parts {
        output.push_str(&format!("--{}\r\n", boundary));
        output.push_str(&format!("Content-Type: {}\r\n", part.mime_type));

        if let Some(filename) = &part.filename {
            output.push_str(&format!(
                "Content-Disposition: attachment; filename=\"{}\"\r\n",
                filename
            ));
        }

        output.push_str("\r\n");
        output.push_str(&part.content);
        output.push_str("\r\n");
    }

    // End boundary
    output.push_str(&format!("--{}--\r\n", boundary));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_multipart() {
        let data = r#"MIME-Version: 1.0
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

        let parts = parse_multipart(data).unwrap();
        assert_eq!(parts.len(), 2);

        assert_eq!(parts[0].content_type, ContentType::CloudConfig);
        assert!(parts[0].content.contains("hostname: test"));

        assert_eq!(parts[1].content_type, ContentType::Script);
        assert!(parts[1].content.contains("echo hello"));
    }

    #[test]
    fn test_parse_with_filename() {
        let data = r#"Content-Type: multipart/mixed; boundary=abc123

--abc123
Content-Type: text/x-shellscript
Content-Disposition: attachment; filename="setup.sh"

#!/bin/bash
echo setup

--abc123--
"#;

        let parts = parse_multipart(data).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].filename, Some("setup.sh".to_string()));
    }

    #[test]
    fn test_parse_base64_content() {
        let data = r#"Content-Type: multipart/mixed; boundary=test

--test
Content-Type: text/plain
Content-Transfer-Encoding: base64

SGVsbG8gV29ybGQh

--test--
"#;

        let parts = parse_multipart(data).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content.trim(), "Hello World!");
    }

    #[test]
    fn test_find_boundary() {
        assert_eq!(
            find_boundary("Content-Type: multipart/mixed; boundary=\"abc123\"").unwrap(),
            "abc123"
        );
        assert_eq!(
            find_boundary("Content-Type: multipart/mixed; boundary=simple").unwrap(),
            "simple"
        );
    }

    #[test]
    fn test_extract_filename() {
        assert_eq!(
            extract_filename("attachment; filename=\"test.sh\""),
            Some("test.sh".to_string())
        );
        assert_eq!(
            extract_filename("attachment; filename=script.sh"),
            Some("script.sh".to_string())
        );
    }

    #[test]
    fn test_create_multipart() {
        let parts = vec![MimePart {
            content_type: ContentType::CloudConfig,
            mime_type: "text/cloud-config".to_string(),
            content: "#cloud-config\nhostname: test".to_string(),
            filename: None,
            headers: HashMap::new(),
        }];

        let output = create_multipart(&parts, "BOUNDARY");
        assert!(output.contains("multipart/mixed"));
        assert!(output.contains("--BOUNDARY"));
        assert!(output.contains("hostname: test"));
    }

    #[test]
    fn test_decode_quoted_printable() {
        assert_eq!(decode_quoted_printable("Hello=20World"), "Hello World");
        assert_eq!(decode_quoted_printable("Line1=\r\nLine2"), "Line1Line2");
    }
}
