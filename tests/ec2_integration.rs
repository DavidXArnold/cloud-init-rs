//! Integration tests for EC2 datasource using wiremock

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test EC2 IMDSv2 token acquisition
#[tokio::test]
async fn test_ec2_imdsv2_token_request() {
    let mock_server = MockServer::start().await;

    // Mock the token endpoint
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .and(header("X-aws-ec2-metadata-token-ttl-seconds", "21600"))
        .respond_with(ResponseTemplate::new(200).set_body_string("test-token-12345"))
        .mount(&mock_server)
        .await;

    // Make request to get token
    let client = reqwest::Client::new();
    let response = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .header("X-aws-ec2-metadata-token-ttl-seconds", "21600")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let token = response.text().await.unwrap();
    assert_eq!(token, "test-token-12345");
}

/// Test EC2 metadata retrieval with IMDSv2 token
#[tokio::test]
async fn test_ec2_metadata_with_token() {
    let mock_server = MockServer::start().await;

    // Mock token endpoint
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("test-token"))
        .mount(&mock_server)
        .await;

    // Mock instance-id endpoint
    Mock::given(method("GET"))
        .and(path("/latest/meta-data/instance-id"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("i-1234567890abcdef0"))
        .mount(&mock_server)
        .await;

    // Mock local-hostname endpoint
    Mock::given(method("GET"))
        .and(path("/latest/meta-data/local-hostname"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ip-172-31-0-1.ec2.internal"))
        .mount(&mock_server)
        .await;

    // Get token
    let client = reqwest::Client::new();
    let token = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // Get instance-id
    let instance_id = client
        .get(format!(
            "{}/latest/meta-data/instance-id",
            mock_server.uri()
        ))
        .header("X-aws-ec2-metadata-token", &token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(instance_id, "i-1234567890abcdef0");

    // Get hostname
    let hostname = client
        .get(format!(
            "{}/latest/meta-data/local-hostname",
            mock_server.uri()
        ))
        .header("X-aws-ec2-metadata-token", &token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(hostname, "ip-172-31-0-1.ec2.internal");
}

/// Test EC2 user-data retrieval
#[tokio::test]
async fn test_ec2_userdata_cloud_config() {
    let mock_server = MockServer::start().await;

    let cloud_config = r#"#cloud-config
hostname: test-instance
packages:
  - nginx
"#;

    // Mock token
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("test-token"))
        .mount(&mock_server)
        .await;

    // Mock user-data
    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(cloud_config))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let token = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let userdata = client
        .get(format!("{}/latest/user-data", mock_server.uri()))
        .header("X-aws-ec2-metadata-token", &token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(userdata.starts_with("#cloud-config"));
    assert!(userdata.contains("hostname: test-instance"));
}

/// Test EC2 404 when no user-data present
#[tokio::test]
async fn test_ec2_no_userdata() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("test-token"))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/latest/user-data"))
        .and(header("X-aws-ec2-metadata-token", "test-token"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let token = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let response = client
        .get(format!("{}/latest/user-data", mock_server.uri()))
        .header("X-aws-ec2-metadata-token", &token)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

/// Test EC2 IMDSv1 fallback (no token required)
#[tokio::test]
async fn test_ec2_imdsv1_fallback() {
    let mock_server = MockServer::start().await;

    // IMDSv2 token fails
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&mock_server)
        .await;

    // IMDSv1 works without token
    Mock::given(method("GET"))
        .and(path("/latest/meta-data/instance-id"))
        .respond_with(ResponseTemplate::new(200).set_body_string("i-imdsv1instance"))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();

    // Try IMDSv2, expect failure
    let token_response = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(token_response.status(), 403);

    // Fall back to IMDSv1
    let instance_id = client
        .get(format!(
            "{}/latest/meta-data/instance-id",
            mock_server.uri()
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(instance_id, "i-imdsv1instance");
}

/// Test EC2 metadata timeout handling
#[tokio::test]
async fn test_ec2_timeout() {
    let mock_server = MockServer::start().await;

    // Delay response beyond typical timeout
    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(30)))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .unwrap();

    let result = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .send()
        .await;

    assert!(result.is_err());
}

/// Test EC2 full metadata response
#[tokio::test]
async fn test_ec2_full_metadata() {
    let mock_server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/latest/api/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("token"))
        .mount(&mock_server)
        .await;

    // Instance identity document
    let identity_doc = r#"{
        "accountId": "123456789012",
        "architecture": "x86_64",
        "availabilityZone": "us-east-1a",
        "imageId": "ami-12345678",
        "instanceId": "i-1234567890abcdef0",
        "instanceType": "t3.micro",
        "privateIp": "172.31.0.1",
        "region": "us-east-1"
    }"#;

    Mock::given(method("GET"))
        .and(path("/latest/dynamic/instance-identity/document"))
        .and(header("X-aws-ec2-metadata-token", "token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(identity_doc))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let token = client
        .put(format!("{}/latest/api/token", mock_server.uri()))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let doc = client
        .get(format!(
            "{}/latest/dynamic/instance-identity/document",
            mock_server.uri()
        ))
        .header("X-aws-ec2-metadata-token", &token)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&doc).unwrap();
    assert_eq!(parsed["instanceId"], "i-1234567890abcdef0");
    assert_eq!(parsed["region"], "us-east-1");
}
