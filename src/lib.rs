//! cloud-init-rs library
//!
//! This crate provides a safe Rust implementation of cloud-init functionality.
//!
//! # Design Principles
//!
//! - **Safety First**: No unsafe code (`#![forbid(unsafe_code)]`)
//! - **Fast Boot**: Minimal dependencies, async I/O, efficient parsing
//! - **80% Compatibility**: Support the most common cloud-init features
//! - **Backwards Compatible**: Parse existing cloud-config formats

pub mod config;
pub mod datasources;
pub mod modules;
pub mod network;
pub mod stages;

mod error;

pub use error::CloudInitError;

use tracing::info;

/// Cloud-init execution stages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Local stage - runs before network is available
    /// Handles: disk setup, growpart, mounts
    Local,
    /// Network stage - runs after network is configured
    /// Handles: metadata retrieval, ssh keys
    Network,
    /// Config stage - applies user configuration
    /// Handles: users, groups, packages, write_files
    Config,
    /// Final stage - runs user scripts
    /// Handles: runcmd, scripts-user, phone_home
    Final,
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stage::Local => write!(f, "local"),
            Stage::Network => write!(f, "network"),
            Stage::Config => write!(f, "config"),
            Stage::Final => write!(f, "final"),
        }
    }
}

/// Run the specified cloud-init stages in order
pub async fn run_stages(stages: &[Stage]) -> Result<(), CloudInitError> {
    for stage in stages {
        info!("Starting stage: {}", stage);
        run_stage(*stage).await?;
        info!("Completed stage: {}", stage);
    }
    Ok(())
}

async fn run_stage(stage: Stage) -> Result<(), CloudInitError> {
    match stage {
        Stage::Local => stages::local::run().await,
        Stage::Network => stages::network::run().await,
        Stage::Config => stages::config::run().await,
        Stage::Final => stages::final_stage::run().await,
    }
}

/// Instance metadata retrieved from datasource
#[derive(Debug, Clone, Default)]
pub struct InstanceMetadata {
    pub instance_id: Option<String>,
    pub local_hostname: Option<String>,
    pub region: Option<String>,
    pub availability_zone: Option<String>,
    pub cloud_name: Option<String>,
    pub platform: Option<String>,
}

/// User data (cloud-config or script)
#[derive(Debug, Clone)]
pub enum UserData {
    /// Cloud-config YAML
    CloudConfig(config::CloudConfig),
    /// Shell script
    Script(String),
    /// Multi-part MIME
    MultiPart(Vec<UserDataPart>),
    /// No user data
    None,
}

/// Part of multi-part user data
#[derive(Debug, Clone)]
pub struct UserDataPart {
    pub content_type: String,
    pub content: String,
    pub filename: Option<String>,
}
