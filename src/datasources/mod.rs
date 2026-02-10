//! Datasource implementations for various cloud providers
//!
//! Datasources provide instance metadata and user data from cloud providers.

pub mod ec2;
pub mod nocloud;

use async_trait::async_trait;
use crate::{CloudInitError, InstanceMetadata, UserData};

/// Trait for cloud metadata datasources
///
/// Each cloud provider implements this trait to provide instance metadata
/// and user data in a consistent way.
#[async_trait]
pub trait Datasource: Send + Sync {
    /// Name of this datasource (e.g., "EC2", "NoCloud", "GCE")
    fn name(&self) -> &'static str;

    /// Check if this datasource is available
    ///
    /// This should be a quick check (e.g., checking for magic files or
    /// attempting a single HTTP request with a short timeout).
    async fn is_available(&self) -> bool;

    /// Fetch instance metadata from this datasource
    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError>;

    /// Fetch user data from this datasource
    async fn get_userdata(&self) -> Result<UserData, CloudInitError>;

    /// Fetch vendor data if available
    async fn get_vendordata(&self) -> Result<Option<UserData>, CloudInitError> {
        Ok(None)
    }
}

/// Detect and return the appropriate datasource for this instance
pub async fn detect_datasource() -> Result<Box<dyn Datasource>, CloudInitError> {
    // Try datasources in order of priority
    let datasources: Vec<Box<dyn Datasource>> = vec![
        Box::new(nocloud::NoCloud::new()),
        Box::new(ec2::Ec2::new()),
        // Add more datasources here
    ];

    for ds in datasources {
        if ds.is_available().await {
            tracing::info!("Detected datasource: {}", ds.name());
            return Ok(ds);
        }
    }

    Err(CloudInitError::NoDatasource)
}
