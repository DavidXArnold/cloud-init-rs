//! Mock datasource for testing
//!
//! Provides a configurable mock datasource that can be used in unit tests.

use async_trait::async_trait;

use super::Datasource;
use crate::{config::CloudConfig, CloudInitError, InstanceMetadata, UserData};

/// Mock datasource for testing
///
/// # Example
/// ```
/// use cloud_init_rs::datasources::mock::MockDatasource;
/// use cloud_init_rs::InstanceMetadata;
///
/// let mock = MockDatasource::new()
///     .with_available(true)
///     .with_metadata(InstanceMetadata {
///         instance_id: Some("test-123".to_string()),
///         ..Default::default()
///     });
/// ```
pub struct MockDatasource {
    name: &'static str,
    available: bool,
    metadata: Option<InstanceMetadata>,
    userdata: Option<UserData>,
    metadata_error: Option<String>,
    userdata_error: Option<String>,
}

impl MockDatasource {
    /// Create a new mock datasource with default values
    pub fn new() -> Self {
        Self {
            name: "Mock",
            available: true,
            metadata: None,
            userdata: None,
            metadata_error: None,
            userdata_error: None,
        }
    }

    /// Set the datasource name
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    /// Set whether the datasource is available
    pub fn with_available(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// Set the metadata to return
    pub fn with_metadata(mut self, metadata: InstanceMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the userdata to return
    pub fn with_userdata(mut self, userdata: UserData) -> Self {
        self.userdata = Some(userdata);
        self
    }

    /// Set cloud-config userdata from YAML string
    pub fn with_cloud_config(mut self, yaml: &str) -> Self {
        match CloudConfig::from_yaml(yaml) {
            Ok(config) => self.userdata = Some(UserData::CloudConfig(Box::new(config))),
            Err(e) => self.userdata_error = Some(e.to_string()),
        }
        self
    }

    /// Set script userdata
    pub fn with_script(mut self, script: &str) -> Self {
        self.userdata = Some(UserData::Script(script.to_string()));
        self
    }

    /// Configure to return an error for metadata
    pub fn with_metadata_error(mut self, error: &str) -> Self {
        self.metadata_error = Some(error.to_string());
        self
    }

    /// Configure to return an error for userdata
    pub fn with_userdata_error(mut self, error: &str) -> Self {
        self.userdata_error = Some(error.to_string());
        self
    }
}

impl Default for MockDatasource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Datasource for MockDatasource {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn is_available(&self) -> bool {
        self.available
    }

    async fn get_metadata(&self) -> Result<InstanceMetadata, CloudInitError> {
        if let Some(error) = &self.metadata_error {
            return Err(CloudInitError::Datasource(error.clone()));
        }

        Ok(self.metadata.clone().unwrap_or_default())
    }

    async fn get_userdata(&self) -> Result<UserData, CloudInitError> {
        if let Some(error) = &self.userdata_error {
            return Err(CloudInitError::Datasource(error.clone()));
        }

        Ok(self.userdata.clone().unwrap_or(UserData::None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_datasource_default() {
        let mock = MockDatasource::new();

        assert_eq!(mock.name(), "Mock");
        assert!(mock.is_available().await);

        let metadata = mock.get_metadata().await.unwrap();
        assert!(metadata.instance_id.is_none());

        let userdata = mock.get_userdata().await.unwrap();
        assert!(matches!(userdata, UserData::None));
    }

    #[tokio::test]
    async fn test_mock_datasource_with_metadata() {
        let mock = MockDatasource::new()
            .with_name("TestDS")
            .with_metadata(InstanceMetadata {
                instance_id: Some("i-test123".to_string()),
                local_hostname: Some("test-host".to_string()),
                cloud_name: Some("test-cloud".to_string()),
                ..Default::default()
            });

        assert_eq!(mock.name(), "TestDS");

        let metadata = mock.get_metadata().await.unwrap();
        assert_eq!(metadata.instance_id, Some("i-test123".to_string()));
        assert_eq!(metadata.local_hostname, Some("test-host".to_string()));
    }

    #[tokio::test]
    async fn test_mock_datasource_with_cloud_config() {
        let yaml = r#"#cloud-config
hostname: mock-host
packages:
  - nginx
"#;

        let mock = MockDatasource::new().with_cloud_config(yaml);

        let userdata = mock.get_userdata().await.unwrap();
        match userdata {
            UserData::CloudConfig(config) => {
                assert_eq!(config.hostname, Some("mock-host".to_string()));
                assert_eq!(config.packages, vec!["nginx"]);
            }
            _ => panic!("Expected CloudConfig userdata"),
        }
    }

    #[tokio::test]
    async fn test_mock_datasource_with_script() {
        let script = "#!/bin/bash\necho hello";
        let mock = MockDatasource::new().with_script(script);

        let userdata = mock.get_userdata().await.unwrap();
        match userdata {
            UserData::Script(s) => assert_eq!(s, script),
            _ => panic!("Expected Script userdata"),
        }
    }

    #[tokio::test]
    async fn test_mock_datasource_unavailable() {
        let mock = MockDatasource::new().with_available(false);

        assert!(!mock.is_available().await);
    }

    #[tokio::test]
    async fn test_mock_datasource_metadata_error() {
        let mock = MockDatasource::new().with_metadata_error("Metadata fetch failed");

        let result = mock.get_metadata().await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("Metadata fetch failed"));
    }

    #[tokio::test]
    async fn test_mock_datasource_userdata_error() {
        let mock = MockDatasource::new().with_userdata_error("Userdata fetch failed");

        let result = mock.get_userdata().await;
        assert!(result.is_err());
    }
}
