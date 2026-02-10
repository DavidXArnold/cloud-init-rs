//! Error types for cloud-init-rs

use thiserror::Error;

/// Main error type for cloud-init-rs operations
#[derive(Error, Debug)]
pub enum CloudInitError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Datasource error: {0}")]
    Datasource(String),

    #[error("No datasource found")]
    NoDatasource,

    #[error("Network error: {0}")]
    Network(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Module error in '{module}': {message}")]
    Module { module: String, message: String },

    #[error("Stage '{stage}' failed: {message}")]
    Stage { stage: String, message: String },

    #[error("User/group error: {0}")]
    UserGroup(String),

    #[error("Command execution failed: {0}")]
    Command(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Timeout waiting for {0}")]
    Timeout(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

impl CloudInitError {
    /// Create a module error
    pub fn module(module: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Module {
            module: module.into(),
            message: message.into(),
        }
    }

    /// Create a stage error
    pub fn stage(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Stage {
            stage: stage.into(),
            message: message.into(),
        }
    }
}
