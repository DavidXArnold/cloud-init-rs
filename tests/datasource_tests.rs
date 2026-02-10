//! Integration tests for cloud datasources using wiremock

use cloud_init_rs::datasources::{Datasource, azure::Azure, gce::Gce, openstack::OpenStack};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
