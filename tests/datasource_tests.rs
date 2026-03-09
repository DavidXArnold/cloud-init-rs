//! Integration tests for cloud datasources using wiremock

use cloud_init_rs::datasources::{
    Datasource, azure::Azure, ec2::Ec2, gce::Gce, openstack::OpenStack,
};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// EC2 Tests
// ============================================================================

#[tokio::test]
async fn test_ec2_get_metadata_imdsv2() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("test-token"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/instance-id"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("i-abc123"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/local-hostname"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ip-10-0-0-1.ec2.internal"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/placement/availability-zone"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("us-east-1a"))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let metadata = ec2.get_metadata().await.unwrap();

    assert_eq!(metadata.cloud_name, Some("aws".to_string()));
    assert_eq!(metadata.platform, Some("ec2".to_string()));
    assert_eq!(metadata.instance_id, Some("i-abc123".to_string()));
    assert_eq!(
        metadata.local_hostname,
        Some("ip-10-0-0-1.ec2.internal".to_string())
    );
    assert_eq!(metadata.availability_zone, Some("us-east-1a".to_string()));
    assert_eq!(metadata.region, Some("us-east-1".to_string()));
}

#[tokio::test]
async fn test_ec2_get_userdata_cloud_config() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("#cloud-config\nhostname: ec2-host"),
        )
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("ec2-host".to_string()));
        }
        _ => panic!("Expected CloudConfig"),
    }
}

#[tokio::test]
async fn test_ec2_get_userdata_script() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#!/bin/bash\necho hello"))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::Script(s) => assert!(s.starts_with("#!/bin/bash")),
        _ => panic!("Expected Script"),
    }
}

#[tokio::test]
async fn test_ec2_get_userdata_404() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_ec2_get_userdata_empty() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_ec2_get_userdata_non_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_ec2_get_userdata_ambiguous_content() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    // Content that isn't cloud-config or script
    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hostname: fallback-host"))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();

    // Should try to parse as cloud-config
    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("fallback-host".to_string()));
        }
        _ => panic!("Expected CloudConfig from fallback parsing"),
    }
}

#[tokio::test]
async fn test_ec2_get_userdata_unparseable_content() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("just some random text content"))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();

    // Should fall back to Script
    assert!(matches!(userdata, cloud_init_rs::UserData::Script(_)));
}

#[tokio::test]
async fn test_ec2_imdsv1_fallback_metadata() {
    let mock_server = MockServer::start().await;

    // IMDSv2 token fails
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    // IMDSv1 works without token header
    Mock::given(method("GET"))
        .and(path("/latest/meta-data/instance-id"))
        .respond_with(ResponseTemplate::new(200).set_body_string("i-v1fallback"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/local-hostname"))
        .respond_with(ResponseTemplate::new(200).set_body_string("v1-host"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/placement/availability-zone"))
        .respond_with(ResponseTemplate::new(200).set_body_string("eu-west-1b"))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let metadata = ec2.get_metadata().await.unwrap();

    assert_eq!(metadata.instance_id, Some("i-v1fallback".to_string()));
    assert_eq!(metadata.local_hostname, Some("v1-host".to_string()));
    assert_eq!(metadata.region, Some("eu-west-1".to_string()));
}

#[tokio::test]
async fn test_ec2_imdsv1_fallback_userdata() {
    let mock_server = MockServer::start().await;

    // IMDSv2 token fails
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("#cloud-config\nhostname: v1-userdata"),
        )
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let userdata = ec2.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("v1-userdata".to_string()));
        }
        _ => panic!("Expected CloudConfig"),
    }
}

#[tokio::test]
async fn test_ec2_metadata_partial_failure() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("tok"))
        .mount(&mock_server)
        .await;

    // Only instance-id succeeds, others fail
    Mock::given(method("GET"))
        .and(path("/latest/meta-data/instance-id"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("i-partial"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/local-hostname"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/meta-data/placement/availability-zone"))
        .and(header("X-aws-ec2-metadata-token", "tok"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let ec2 = Ec2::with_base_url(&mock_server.uri());
    let metadata = ec2.get_metadata().await.unwrap();

    assert_eq!(metadata.instance_id, Some("i-partial".to_string()));
    assert!(metadata.local_hostname.is_none());
    assert!(metadata.availability_zone.is_none());
}

#[test]
fn test_ec2_default() {
    let ec2 = Ec2::default();
    assert_eq!(ec2.name(), "EC2");
}

// ============================================================================
// GCE Tests
// ============================================================================

#[tokio::test]
async fn test_gce_metadata() {
    let mock_server = MockServer::start().await;

    // Mock root path for availability check
    Mock::given(method("GET"))
        .and(path("/"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock instance ID
    Mock::given(method("GET"))
        .and(path("/instance/id"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string("12345678901234567"))
        .mount(&mock_server)
        .await;

    // Mock hostname
    Mock::given(method("GET"))
        .and(path("/instance/hostname"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("test-instance.c.project.internal"),
        )
        .mount(&mock_server)
        .await;

    // Mock zone
    Mock::given(method("GET"))
        .and(path("/instance/zone"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("projects/123456789/zones/us-central1-a"),
        )
        .mount(&mock_server)
        .await;

    // Mock machine type
    Mock::given(method("GET"))
        .and(path("/instance/machine-type"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("projects/123456789/machineTypes/n1-standard-1"),
        )
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let metadata = gce.get_metadata().await.expect("Failed to get metadata");

    assert_eq!(metadata.cloud_name, Some("gce".to_string()));
    assert_eq!(metadata.instance_id, Some("12345678901234567".to_string()));
    assert_eq!(
        metadata.local_hostname,
        Some("test-instance.c.project.internal".to_string())
    );
    assert_eq!(
        metadata.availability_zone,
        Some("us-central1-a".to_string())
    );
    assert_eq!(metadata.region, Some("us-central1".to_string()));
    assert_eq!(metadata.instance_type, Some("n1-standard-1".to_string()));
}

#[tokio::test]
async fn test_gce_userdata_cloud_config() {
    let mock_server = MockServer::start().await;

    let cloud_config =
        "#cloud-config\nusers:\n  - name: testuser\n    sudo: ALL=(ALL) NOPASSWD:ALL";

    Mock::given(method("GET"))
        .and(path("/instance/attributes/user-data"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string(cloud_config))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let userdata = gce.get_userdata().await.expect("Failed to get userdata");

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert!(!config.users.is_empty());
            match &config.users[0] {
                cloud_init_rs::config::UserConfig::Full(user) => {
                    assert_eq!(user.name, "testuser");
                }
                _ => panic!("Expected full user config"),
            }
        }
        _ => panic!("Expected CloudConfig userdata"),
    }
}

#[tokio::test]
async fn test_gce_userdata_script() {
    let mock_server = MockServer::start().await;

    let script = "#!/bin/bash\necho 'Hello World'";

    Mock::given(method("GET"))
        .and(path("/instance/attributes/user-data"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string(script))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let userdata = gce.get_userdata().await.expect("Failed to get userdata");

    match userdata {
        cloud_init_rs::UserData::Script(content) => {
            assert!(content.starts_with("#!/bin/bash"));
        }
        _ => panic!("Expected Script userdata"),
    }
}

#[tokio::test]
async fn test_gce_no_userdata() {
    let mock_server = MockServer::start().await;

    // Return 404 for user-data
    Mock::given(method("GET"))
        .and(path("/instance/attributes/user-data"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    // Return 404 for startup-script fallback
    Mock::given(method("GET"))
        .and(path("/instance/attributes/startup-script"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let userdata = gce.get_userdata().await.expect("Failed to get userdata");

    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_gce_startup_script_fallback() {
    let mock_server = MockServer::start().await;

    // user-data returns empty
    Mock::given(method("GET"))
        .and(path("/instance/attributes/user-data"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&mock_server)
        .await;

    // startup-script has content
    Mock::given(method("GET"))
        .and(path("/instance/attributes/startup-script"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#!/bin/bash\necho startup"))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let userdata = gce.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::Script(s) => assert!(s.contains("startup")),
        _ => panic!("Expected Script from startup-script fallback"),
    }
}

#[tokio::test]
async fn test_gce_userdata_ambiguous_content() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/instance/attributes/user-data"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hostname: gce-fallback"))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let userdata = gce.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("gce-fallback".to_string()));
        }
        _ => panic!("Expected CloudConfig from fallback"),
    }
}

#[tokio::test]
async fn test_gce_userdata_unparseable() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/instance/attributes/user-data"))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(ResponseTemplate::new(200).set_body_string("just random text"))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let userdata = gce.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::Script(_)));
}

#[tokio::test]
async fn test_gce_metadata_error() {
    let mock_server = MockServer::start().await;

    // All metadata endpoints return errors
    Mock::given(method("GET"))
        .and(path("/instance/id"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/instance/hostname"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/instance/zone"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/instance/machine-type"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let gce = Gce::with_base_url(&mock_server.uri());
    let metadata = gce.get_metadata().await.unwrap();

    assert_eq!(metadata.cloud_name, Some("gce".to_string()));
    assert!(metadata.instance_id.is_none());
}

#[test]
fn test_gce_default_impl() {
    let gce = Gce::default();
    assert_eq!(gce.name(), "GCE");
}

// ============================================================================
// Azure Tests
// ============================================================================

#[tokio::test]
async fn test_azure_metadata() {
    let mock_server = MockServer::start().await;

    let azure_response = serde_json::json!({
        "compute": {
            "vmId": "azure-vm-12345",
            "name": "test-vm",
            "location": "eastus",
            "vmSize": "Standard_D2s_v3",
            "zone": "1",
            "computerName": "test-hostname"
        }
    });

    Mock::given(method("GET"))
        .and(path("/instance"))
        .and(query_param("api-version", "2021-02-01"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&azure_response))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let metadata = azure.get_metadata().await.expect("Failed to get metadata");

    assert_eq!(metadata.cloud_name, Some("azure".to_string()));
    assert_eq!(metadata.instance_id, Some("azure-vm-12345".to_string()));
    assert_eq!(metadata.local_hostname, Some("test-hostname".to_string()));
    assert_eq!(metadata.region, Some("eastus".to_string()));
    assert_eq!(metadata.availability_zone, Some("eastus-1".to_string()));
    assert_eq!(metadata.instance_type, Some("Standard_D2s_v3".to_string()));
}

#[tokio::test]
async fn test_azure_userdata_base64() {
    use base64::Engine;

    let mock_server = MockServer::start().await;

    let cloud_config = "#cloud-config\nusers:\n  - name: azureuser";
    let encoded = base64::engine::general_purpose::STANDARD.encode(cloud_config);

    Mock::given(method("GET"))
        .and(path("/instance/compute/customData"))
        .and(query_param("api-version", "2021-02-01"))
        .and(query_param("format", "text"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(encoded))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let userdata = azure.get_userdata().await.expect("Failed to get userdata");

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert!(!config.users.is_empty());
            match &config.users[0] {
                cloud_init_rs::config::UserConfig::Full(user) => {
                    assert_eq!(user.name, "azureuser");
                }
                _ => panic!("Expected full user config"),
            }
        }
        _ => panic!("Expected CloudConfig userdata"),
    }
}

#[tokio::test]
async fn test_azure_no_userdata() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/instance/compute/customData"))
        .and(query_param("api-version", "2021-02-01"))
        .and(query_param("format", "text"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let userdata = azure.get_userdata().await.expect("Failed to get userdata");

    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_azure_metadata_empty_fields() {
    let mock_server = MockServer::start().await;

    let azure_response = serde_json::json!({
        "compute": {
            "vmId": "",
            "name": "fallback-name",
            "location": "",
            "vmSize": "",
            "zone": "",
            "computerName": ""
        }
    });

    Mock::given(method("GET"))
        .and(path("/instance"))
        .and(query_param("api-version", "2021-02-01"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&azure_response))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let metadata = azure.get_metadata().await.unwrap();

    assert!(metadata.instance_id.is_none());
    // Falls back to name when computerName is empty
    assert_eq!(metadata.local_hostname, Some("fallback-name".to_string()));
    assert!(metadata.region.is_none());
    assert!(metadata.instance_type.is_none());
}

#[tokio::test]
async fn test_azure_metadata_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/instance"))
        .and(query_param("api-version", "2021-02-01"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let result = azure.get_metadata().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_azure_userdata_empty_content() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/instance/compute/customData"))
        .and(query_param("api-version", "2021-02-01"))
        .and(query_param("format", "text"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let userdata = azure.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_azure_userdata_non_base64() {
    let mock_server = MockServer::start().await;

    // Non-base64 content used as-is
    Mock::given(method("GET"))
        .and(path("/instance/compute/customData"))
        .and(query_param("api-version", "2021-02-01"))
        .and(query_param("format", "text"))
        .and(header("Metadata", "true"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("#cloud-config\nhostname: azure-raw"),
        )
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let userdata = azure.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("azure-raw".to_string()));
        }
        _ => panic!("Expected CloudConfig"),
    }
}

#[tokio::test]
async fn test_azure_userdata_script() {
    use base64::Engine;

    let mock_server = MockServer::start().await;

    let script = "#!/bin/bash\necho azure";
    let encoded = base64::engine::general_purpose::STANDARD.encode(script);

    Mock::given(method("GET"))
        .and(path("/instance/compute/customData"))
        .and(query_param("api-version", "2021-02-01"))
        .and(query_param("format", "text"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(encoded))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let userdata = azure.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::Script(s) => assert!(s.starts_with("#!/bin/bash")),
        _ => panic!("Expected Script"),
    }
}

#[tokio::test]
async fn test_azure_userdata_ambiguous() {
    use base64::Engine;

    let mock_server = MockServer::start().await;

    let content = "hostname: azure-fallback";
    let encoded = base64::engine::general_purpose::STANDARD.encode(content);

    Mock::given(method("GET"))
        .and(path("/instance/compute/customData"))
        .and(query_param("api-version", "2021-02-01"))
        .and(query_param("format", "text"))
        .and(header("Metadata", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(encoded))
        .mount(&mock_server)
        .await;

    let azure = Azure::with_base_url(&mock_server.uri());
    let userdata = azure.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("azure-fallback".to_string()));
        }
        _ => panic!("Expected CloudConfig from fallback"),
    }
}

#[test]
fn test_azure_default_impl() {
    let azure = Azure::default();
    assert_eq!(azure.name(), "Azure");
}

// ============================================================================
// OpenStack Tests
// ============================================================================

#[tokio::test]
async fn test_openstack_metadata_http() {
    let mock_server = MockServer::start().await;

    let openstack_response = serde_json::json!({
        "uuid": "openstack-instance-uuid-1234",
        "name": "test-instance",
        "hostname": "openstack-host",
        "availability_zone": "nova-1",
        "project_id": "project-123"
    });

    Mock::given(method("GET"))
        .and(path("/latest/meta_data.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&openstack_response))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let metadata = openstack
        .get_metadata()
        .await
        .expect("Failed to get metadata");

    assert_eq!(metadata.cloud_name, Some("openstack".to_string()));
    assert_eq!(
        metadata.instance_id,
        Some("openstack-instance-uuid-1234".to_string())
    );
    assert_eq!(metadata.local_hostname, Some("openstack-host".to_string()));
    assert_eq!(metadata.availability_zone, Some("nova-1".to_string()));
    assert_eq!(metadata.region, Some("nova".to_string()));
}

#[tokio::test]
async fn test_openstack_userdata_http() {
    let mock_server = MockServer::start().await;

    let cloud_config = "#cloud-config\nusers:\n  - name: openstackuser";

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(200).set_body_string(cloud_config))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack
        .get_userdata()
        .await
        .expect("Failed to get userdata");

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert!(!config.users.is_empty());
            match &config.users[0] {
                cloud_init_rs::config::UserConfig::Full(user) => {
                    assert_eq!(user.name, "openstackuser");
                }
                _ => panic!("Expected full user config"),
            }
        }
        _ => panic!("Expected CloudConfig userdata"),
    }
}

#[tokio::test]
async fn test_openstack_no_userdata() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack
        .get_userdata()
        .await
        .expect("Failed to get userdata");

    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_openstack_metadata_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/meta_data.json"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let result = openstack.get_metadata().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_openstack_metadata_empty_fields() {
    let mock_server = MockServer::start().await;

    let response = serde_json::json!({
        "uuid": "",
        "name": "os-name",
        "hostname": "",
        "availability_zone": "",
        "project_id": ""
    });

    Mock::given(method("GET"))
        .and(path("/latest/meta_data.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let metadata = openstack.get_metadata().await.unwrap();

    assert!(metadata.instance_id.is_none());
    // Falls back to name when hostname is empty
    assert_eq!(metadata.local_hostname, Some("os-name".to_string()));
    assert!(metadata.availability_zone.is_none());
}

#[tokio::test]
async fn test_openstack_userdata_empty_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[tokio::test]
async fn test_openstack_userdata_script() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(200).set_body_string("#!/bin/bash\necho openstack"))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::Script(s) => assert!(s.starts_with("#!/bin/bash")),
        _ => panic!("Expected Script"),
    }
}

#[tokio::test]
async fn test_openstack_userdata_ambiguous() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hostname: os-fallback"))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack.get_userdata().await.unwrap();

    match userdata {
        cloud_init_rs::UserData::CloudConfig(config) => {
            assert_eq!(config.hostname, Some("os-fallback".to_string()));
        }
        _ => panic!("Expected CloudConfig from fallback"),
    }
}

#[tokio::test]
async fn test_openstack_userdata_unparseable() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(200).set_body_string("random text content"))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::Script(_)));
}

#[tokio::test]
async fn test_openstack_userdata_non_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/latest/user_data"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let openstack = OpenStack::with_base_url(&mock_server.uri());
    let userdata = openstack.get_userdata().await.unwrap();
    assert!(matches!(userdata, cloud_init_rs::UserData::None));
}

#[test]
fn test_openstack_default_impl() {
    let openstack = OpenStack::default();
    assert_eq!(openstack.name(), "OpenStack");
}

// ============================================================================
// Datasource name tests
// ============================================================================

#[tokio::test]
async fn test_datasource_names() {
    let gce = Gce::with_base_url("http://localhost");
    let azure = Azure::with_base_url("http://localhost");
    let openstack = OpenStack::with_base_url("http://localhost");

    assert_eq!(gce.name(), "GCE");
    assert_eq!(azure.name(), "Azure");
    assert_eq!(openstack.name(), "OpenStack");
}
