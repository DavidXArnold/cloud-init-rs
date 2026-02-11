//! Jinja2-compatible template rendering
//!
//! Renders cloud-config templates using instance metadata.
//!
//! Templates can use the `## template: jinja` header marker to enable
//! Jinja2 processing.

pub mod context;

pub use context::{build_context, merge_context};

use crate::{CloudInitError, InstanceMetadata};
use minijinja::Environment;
use std::collections::HashMap;
use tracing::debug;

/// Check if content is a Jinja template (has the template marker)
pub fn is_jinja_template(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with("## template: jinja") || trimmed.starts_with("## template:jinja")
}

/// Strip the template marker from content
pub fn strip_template_marker(content: &str) -> &str {
    let trimmed = content.trim_start();

    if let Some(rest) = trimmed.strip_prefix("## template: jinja") {
        rest.trim_start_matches(['\n', '\r'])
    } else if let Some(rest) = trimmed.strip_prefix("## template:jinja") {
        rest.trim_start_matches(['\n', '\r'])
    } else {
        content
    }
}

/// Render a Jinja template with instance metadata
pub fn render_template(
    template: &str,
    metadata: &InstanceMetadata,
) -> Result<String, CloudInitError> {
    render_template_with_context(template, &build_context(metadata))
}

/// Render a Jinja template with a custom context
pub fn render_template_with_context(
    template: &str,
    context: &HashMap<String, minijinja::Value>,
) -> Result<String, CloudInitError> {
    debug!("Rendering Jinja template");

    // Strip template marker if present
    let template_content = strip_template_marker(template);

    // Create environment
    let mut env = Environment::new();

    // Add template
    env.add_template("template", template_content)
        .map_err(|e| CloudInitError::InvalidData(format!("Template parse error: {}", e)))?;

    // Get template
    let tmpl = env
        .get_template("template")
        .map_err(|e| CloudInitError::InvalidData(format!("Template error: {}", e)))?;

    // Render with context
    let rendered = tmpl
        .render(context)
        .map_err(|e| CloudInitError::InvalidData(format!("Template render error: {}", e)))?;

    Ok(rendered)
}

/// Process content that may or may not be a template
pub fn process_template(
    content: &str,
    metadata: &InstanceMetadata,
) -> Result<String, CloudInitError> {
    if is_jinja_template(content) {
        render_template(content, metadata)
    } else {
        Ok(content.to_string())
    }
}

/// Template renderer with configurable options
pub struct TemplateRenderer {
    env: Environment<'static>,
    context: HashMap<String, minijinja::Value>,
}

impl TemplateRenderer {
    /// Create a new template renderer
    pub fn new() -> Self {
        Self {
            env: Environment::new(),
            context: HashMap::new(),
        }
    }

    /// Create with instance metadata context
    pub fn with_metadata(metadata: &InstanceMetadata) -> Self {
        Self {
            env: Environment::new(),
            context: build_context(metadata),
        }
    }

    /// Add a variable to the context
    pub fn add_var(&mut self, name: impl Into<String>, value: impl Into<minijinja::Value>) {
        self.context.insert(name.into(), value.into());
    }

    /// Add multiple variables to the context
    pub fn add_vars(&mut self, vars: HashMap<String, minijinja::Value>) {
        merge_context(&mut self.context, vars);
    }

    /// Render a template string
    pub fn render(&self, template: &str) -> Result<String, CloudInitError> {
        let template_content = strip_template_marker(template);

        let mut env = self.env.clone();
        env.add_template("template", template_content)
            .map_err(|e| CloudInitError::InvalidData(format!("Template parse error: {}", e)))?;

        let tmpl = env
            .get_template("template")
            .map_err(|e| CloudInitError::InvalidData(format!("Template error: {}", e)))?;

        tmpl.render(&self.context)
            .map_err(|e| CloudInitError::InvalidData(format!("Template render error: {}", e)))
    }

    /// Check if content needs template processing
    pub fn needs_processing(&self, content: &str) -> bool {
        is_jinja_template(content)
    }

    /// Process content (render if template, return as-is otherwise)
    pub fn process(&self, content: &str) -> Result<String, CloudInitError> {
        if self.needs_processing(content) {
            self.render(content)
        } else {
            Ok(content.to_string())
        }
    }
}

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metadata() -> InstanceMetadata {
        InstanceMetadata {
            instance_id: Some("i-1234567890abcdef0".to_string()),
            local_hostname: Some("ip-10-0-0-1".to_string()),
            region: Some("us-east-1".to_string()),
            availability_zone: Some("us-east-1a".to_string()),
            cloud_name: Some("aws".to_string()),
            platform: Some("ec2".to_string()),
            instance_type: Some("t3.micro".to_string()),
        }
    }

    #[test]
    fn test_is_jinja_template() {
        assert!(is_jinja_template("## template: jinja\n#cloud-config"));
        assert!(is_jinja_template("## template:jinja\n#cloud-config"));
        assert!(is_jinja_template("  ## template: jinja\n"));
        assert!(!is_jinja_template("#cloud-config\nhostname: test"));
        assert!(!is_jinja_template("#!/bin/bash"));
    }

    #[test]
    fn test_strip_template_marker() {
        assert_eq!(
            strip_template_marker("## template: jinja\n#cloud-config\nhostname: test"),
            "#cloud-config\nhostname: test"
        );
        assert_eq!(
            strip_template_marker("#cloud-config\nhostname: test"),
            "#cloud-config\nhostname: test"
        );
    }

    #[test]
    fn test_render_simple_template() {
        let template = "## template: jinja\n#cloud-config\nhostname: {{ local_hostname }}";
        let metadata = test_metadata();

        let rendered = render_template(template, &metadata).unwrap();
        assert!(rendered.contains("hostname: ip-10-0-0-1"));
    }

    #[test]
    fn test_render_ds_variable() {
        let template = "## template: jinja\ninstance: {{ ds.meta_data.instance_id }}";
        let metadata = test_metadata();

        let rendered = render_template(template, &metadata).unwrap();
        assert!(rendered.contains("i-1234567890abcdef0"));
    }

    #[test]
    fn test_render_v1_variable() {
        let template = "## template: jinja\nregion: {{ v1.region }}";
        let metadata = test_metadata();

        let rendered = render_template(template, &metadata).unwrap();
        assert!(rendered.contains("us-east-1"));
    }

    #[test]
    fn test_render_instance_variable() {
        let template = "## template: jinja\ncloud: {{ instance.cloud }}";
        let metadata = test_metadata();

        let rendered = render_template(template, &metadata).unwrap();
        assert!(rendered.contains("aws"));
    }

    #[test]
    fn test_process_non_template() {
        let content = "#cloud-config\nhostname: test";
        let metadata = test_metadata();

        let result = process_template(content, &metadata).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_process_template() {
        let content = "## template: jinja\n#cloud-config\nhostname: {{ local_hostname }}";
        let metadata = test_metadata();

        let result = process_template(content, &metadata).unwrap();
        assert!(result.contains("hostname: ip-10-0-0-1"));
    }

    #[test]
    fn test_template_renderer() {
        let metadata = test_metadata();
        let mut renderer = TemplateRenderer::with_metadata(&metadata);
        renderer.add_var("custom", "custom_value");

        let template = "## template: jinja\ncustom: {{ custom }}";
        let result = renderer.render(template).unwrap();
        assert!(result.contains("custom: custom_value"));
    }

    #[test]
    fn test_template_renderer_process() {
        let metadata = test_metadata();
        let renderer = TemplateRenderer::with_metadata(&metadata);

        // Template content
        let template = "## template: jinja\nhostname: {{ local_hostname }}";
        let result = renderer.process(template).unwrap();
        assert!(result.contains("hostname: ip-10-0-0-1"));

        // Non-template content
        let plain = "#cloud-config\nhostname: static";
        let result = renderer.process(plain).unwrap();
        assert_eq!(result, plain);
    }

    #[test]
    fn test_render_conditional() {
        let template = r#"## template: jinja
#cloud-config
{% if instance_id %}
hostname: {{ local_hostname }}
{% else %}
hostname: default
{% endif %}"#;
        let metadata = test_metadata();

        let rendered = render_template(template, &metadata).unwrap();
        assert!(rendered.contains("hostname: ip-10-0-0-1"));
    }

    #[test]
    fn test_render_loop() {
        let template = r#"## template: jinja
#cloud-config
packages:
{% for pkg in ["nginx", "vim", "htop"] %}
  - {{ pkg }}
{% endfor %}"#;
        let metadata = InstanceMetadata::default();

        let rendered = render_template(template, &metadata).unwrap();
        assert!(rendered.contains("- nginx"));
        assert!(rendered.contains("- vim"));
        assert!(rendered.contains("- htop"));
    }

    #[test]
    fn test_render_missing_variable() {
        let template = "## template: jinja\nvalue: {{ missing_var }}";
        let metadata = InstanceMetadata::default();

        // minijinja treats missing as empty string by default
        let result = render_template(template, &metadata);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_invalid_syntax() {
        let template = "## template: jinja\nvalue: {{ invalid";
        let metadata = InstanceMetadata::default();

        let result = render_template(template, &metadata);
        assert!(result.is_err());
    }
}
