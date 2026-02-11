//! Template context building
//!
//! Builds the context for Jinja2 template rendering from instance metadata.

use crate::InstanceMetadata;
use minijinja::value::Value;
use std::collections::HashMap;

/// Build the template context from instance metadata
pub fn build_context(metadata: &InstanceMetadata) -> HashMap<String, Value> {
    let mut ctx = HashMap::new();

    // Add datasource (ds) variables
    ctx.insert("ds".to_string(), build_ds_context(metadata));

    // Add instance variables
    ctx.insert("instance".to_string(), build_instance_context(metadata));

    // Add v1 data format (cloud-init compatibility)
    ctx.insert("v1".to_string(), build_v1_context(metadata));

    // Add local_hostname as top-level variable (commonly used)
    if let Some(hostname) = &metadata.local_hostname {
        ctx.insert("local_hostname".to_string(), Value::from(hostname.clone()));
    }

    // Add instance_id as top-level variable
    if let Some(id) = &metadata.instance_id {
        ctx.insert("instance_id".to_string(), Value::from(id.clone()));
    }

    ctx
}

/// Build datasource (ds) context
fn build_ds_context(metadata: &InstanceMetadata) -> Value {
    let mut ds = HashMap::new();

    // ds.meta_data - instance metadata
    let mut meta_data = HashMap::new();

    if let Some(id) = &metadata.instance_id {
        meta_data.insert("instance-id".to_string(), Value::from(id.clone()));
        meta_data.insert("instance_id".to_string(), Value::from(id.clone()));
    }

    if let Some(hostname) = &metadata.local_hostname {
        meta_data.insert("local-hostname".to_string(), Value::from(hostname.clone()));
        meta_data.insert("local_hostname".to_string(), Value::from(hostname.clone()));
    }

    if let Some(region) = &metadata.region {
        meta_data.insert("region".to_string(), Value::from(region.clone()));
    }

    if let Some(az) = &metadata.availability_zone {
        meta_data.insert("availability-zone".to_string(), Value::from(az.clone()));
        meta_data.insert("availability_zone".to_string(), Value::from(az.clone()));
    }

    if let Some(cloud) = &metadata.cloud_name {
        meta_data.insert("cloud-name".to_string(), Value::from(cloud.clone()));
        meta_data.insert("cloud_name".to_string(), Value::from(cloud.clone()));
    }

    if let Some(platform) = &metadata.platform {
        meta_data.insert("platform".to_string(), Value::from(platform.clone()));
    }

    if let Some(instance_type) = &metadata.instance_type {
        meta_data.insert(
            "instance-type".to_string(),
            Value::from(instance_type.clone()),
        );
        meta_data.insert(
            "instance_type".to_string(),
            Value::from(instance_type.clone()),
        );
    }

    ds.insert("meta_data".to_string(), Value::from_serialize(&meta_data));

    Value::from_serialize(&ds)
}

/// Build instance context
fn build_instance_context(metadata: &InstanceMetadata) -> Value {
    let mut instance = HashMap::new();

    if let Some(id) = &metadata.instance_id {
        instance.insert("id".to_string(), Value::from(id.clone()));
    }

    if let Some(hostname) = &metadata.local_hostname {
        instance.insert("hostname".to_string(), Value::from(hostname.clone()));
    }

    if let Some(region) = &metadata.region {
        instance.insert("region".to_string(), Value::from(region.clone()));
    }

    if let Some(az) = &metadata.availability_zone {
        instance.insert("availability_zone".to_string(), Value::from(az.clone()));
    }

    if let Some(cloud) = &metadata.cloud_name {
        instance.insert("cloud".to_string(), Value::from(cloud.clone()));
    }

    if let Some(instance_type) = &metadata.instance_type {
        instance.insert("type".to_string(), Value::from(instance_type.clone()));
    }

    Value::from_serialize(&instance)
}

/// Build v1 data context (cloud-init compatibility format)
fn build_v1_context(metadata: &InstanceMetadata) -> Value {
    let mut v1 = HashMap::new();

    if let Some(id) = &metadata.instance_id {
        v1.insert("instance_id".to_string(), Value::from(id.clone()));
    }

    if let Some(hostname) = &metadata.local_hostname {
        v1.insert("local_hostname".to_string(), Value::from(hostname.clone()));
    }

    if let Some(region) = &metadata.region {
        v1.insert("region".to_string(), Value::from(region.clone()));
    }

    if let Some(az) = &metadata.availability_zone {
        v1.insert("availability_zone".to_string(), Value::from(az.clone()));
    }

    if let Some(cloud) = &metadata.cloud_name {
        v1.insert("cloud_name".to_string(), Value::from(cloud.clone()));
    }

    if let Some(platform) = &metadata.platform {
        v1.insert("platform".to_string(), Value::from(platform.clone()));
    }

    Value::from_serialize(&v1)
}

/// Merge additional variables into context
pub fn merge_context(base: &mut HashMap<String, Value>, additional: HashMap<String, Value>) {
    for (key, value) in additional {
        base.insert(key, value);
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
    fn test_build_context() {
        let metadata = test_metadata();
        let ctx = build_context(&metadata);

        assert!(ctx.contains_key("ds"));
        assert!(ctx.contains_key("instance"));
        assert!(ctx.contains_key("v1"));
        assert!(ctx.contains_key("local_hostname"));
        assert!(ctx.contains_key("instance_id"));
    }

    #[test]
    fn test_build_ds_context() {
        let metadata = test_metadata();
        let ds = build_ds_context(&metadata);

        // Verify structure
        assert!(!ds.is_undefined());
    }

    #[test]
    fn test_build_instance_context() {
        let metadata = test_metadata();
        let instance = build_instance_context(&metadata);

        assert!(!instance.is_undefined());
    }

    #[test]
    fn test_build_v1_context() {
        let metadata = test_metadata();
        let v1 = build_v1_context(&metadata);

        assert!(!v1.is_undefined());
    }

    #[test]
    fn test_merge_context() {
        let metadata = InstanceMetadata::default();
        let mut ctx = build_context(&metadata);

        let mut additional = HashMap::new();
        additional.insert("custom_var".to_string(), Value::from("custom_value"));

        merge_context(&mut ctx, additional);

        assert!(ctx.contains_key("custom_var"));
    }
}
