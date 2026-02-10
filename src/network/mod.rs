//! Network configuration module
//!
//! Handles network configuration from cloud-init network config v1 and v2 formats.

use serde::{Deserialize, Serialize};

/// Network configuration (v2 format)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub version: u8,
    #[serde(default)]
    pub ethernets: std::collections::HashMap<String, EthernetConfig>,
}

/// Ethernet interface configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EthernetConfig {
    pub dhcp4: Option<bool>,
    pub dhcp6: Option<bool>,
    #[serde(default)]
    pub addresses: Vec<String>,
    pub gateway4: Option<String>,
    pub gateway6: Option<String>,
    #[serde(default)]
    pub nameservers: NameserverConfig,
    pub mtu: Option<u32>,
    #[serde(rename = "match")]
    pub match_config: Option<MatchConfig>,
}

/// Nameserver configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NameserverConfig {
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub search: Vec<String>,
}

/// Interface matching configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchConfig {
    pub macaddress: Option<String>,
    pub driver: Option<String>,
    pub name: Option<String>,
}

impl NetworkConfig {
    /// Parse network config from YAML
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}
