//! Network configuration module
//!
//! Handles network configuration from cloud-init network config v1 and v2 formats.
//!
//! Supports:
//! - Network config v2 (Netplan format) - ethernets, bonds, bridges, vlans
//! - Network config v1 (legacy dictionary format)
//! - Multiple renderers: networkd, NetworkManager, ENI

pub mod render;
pub mod v1;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Network configuration (v2 format - Netplan compatible)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Config version (should be 2 for v2 format)
    pub version: u8,

    /// Ethernet interface configurations
    #[serde(default)]
    pub ethernets: HashMap<String, EthernetConfig>,

    /// Bond configurations
    #[serde(default)]
    pub bonds: HashMap<String, BondConfig>,

    /// Bridge configurations
    #[serde(default)]
    pub bridges: HashMap<String, BridgeConfig>,

    /// VLAN configurations
    #[serde(default)]
    pub vlans: HashMap<String, VlanConfig>,

    /// Renderer hint (networkd, NetworkManager)
    pub renderer: Option<String>,
}

/// Common interface configuration fields
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterfaceCommon {
    /// Enable DHCPv4
    pub dhcp4: Option<bool>,
    /// Enable DHCPv6
    pub dhcp6: Option<bool>,
    /// Static addresses (CIDR notation, e.g., "192.168.1.10/24")
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Default gateway for IPv4
    pub gateway4: Option<String>,
    /// Default gateway for IPv6
    pub gateway6: Option<String>,
    /// Nameserver configuration
    #[serde(default)]
    pub nameservers: NameserverConfig,
    /// MTU size
    pub mtu: Option<u32>,
    /// Static routes
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
    /// Routing policy rules
    #[serde(default, rename = "routing-policy")]
    pub routing_policy: Vec<RoutingPolicyConfig>,
    /// MAC address to set
    pub macaddress: Option<String>,
    /// Make this the default route
    #[serde(rename = "set-name")]
    pub set_name: Option<String>,
    /// Wake-on-LAN
    pub wakeonlan: Option<bool>,
    /// Accept Router Advertisements (IPv6)
    #[serde(rename = "accept-ra")]
    pub accept_ra: Option<bool>,
    /// Optional: only configure if this interface exists
    pub optional: Option<bool>,
}

/// Ethernet interface configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EthernetConfig {
    /// Common interface settings
    #[serde(flatten)]
    pub common: InterfaceCommon,
    /// Interface matching configuration
    #[serde(rename = "match")]
    pub match_config: Option<MatchConfig>,
}

/// Bond configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BondConfig {
    /// Common interface settings
    #[serde(flatten)]
    pub common: InterfaceCommon,
    /// Interfaces to bond together
    #[serde(default)]
    pub interfaces: Vec<String>,
    /// Bond parameters
    pub parameters: Option<BondParameters>,
}

/// Bond parameters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BondParameters {
    /// Bond mode (balance-rr, active-backup, balance-xor, broadcast, 802.3ad, balance-tlb, balance-alb)
    pub mode: Option<String>,
    /// Link monitoring method (mii or arp)
    #[serde(rename = "mii-monitor-interval")]
    pub mii_monitor_interval: Option<u32>,
    /// Primary interface for active-backup mode
    pub primary: Option<String>,
    /// Hash policy for load balancing modes
    #[serde(rename = "transmit-hash-policy")]
    pub transmit_hash_policy: Option<String>,
    /// LACP rate (slow or fast)
    #[serde(rename = "lacp-rate")]
    pub lacp_rate: Option<String>,
    /// ARP monitoring interval
    #[serde(rename = "arp-interval")]
    pub arp_interval: Option<u32>,
    /// ARP IP targets
    #[serde(default, rename = "arp-ip-targets")]
    pub arp_ip_targets: Vec<String>,
}

/// Bridge configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Common interface settings
    #[serde(flatten)]
    pub common: InterfaceCommon,
    /// Interfaces to add to bridge
    #[serde(default)]
    pub interfaces: Vec<String>,
    /// Bridge parameters
    pub parameters: Option<BridgeParameters>,
}

/// Bridge parameters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BridgeParameters {
    /// Ageing time in seconds
    #[serde(rename = "ageing-time")]
    pub ageing_time: Option<u32>,
    /// Forward delay in seconds
    #[serde(rename = "forward-delay")]
    pub forward_delay: Option<u32>,
    /// Hello time in seconds
    #[serde(rename = "hello-time")]
    pub hello_time: Option<u32>,
    /// Max age in seconds
    #[serde(rename = "max-age")]
    pub max_age: Option<u32>,
    /// Bridge priority
    pub priority: Option<u32>,
    /// Enable Spanning Tree Protocol
    pub stp: Option<bool>,
}

/// VLAN configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VlanConfig {
    /// Common interface settings
    #[serde(flatten)]
    pub common: InterfaceCommon,
    /// VLAN ID (1-4094)
    pub id: u16,
    /// Parent interface link
    pub link: String,
}

/// Static route configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteConfig {
    /// Destination network (CIDR notation)
    pub to: String,
    /// Gateway address
    pub via: Option<String>,
    /// Metric/priority
    pub metric: Option<u32>,
    /// Route type (unicast, blackhole, unreachable, etc.)
    #[serde(rename = "type")]
    pub route_type: Option<String>,
    /// Routing table
    pub table: Option<u32>,
    /// On-link flag
    #[serde(rename = "on-link")]
    pub on_link: Option<bool>,
}

/// Routing policy rule
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingPolicyConfig {
    /// Source address/network
    pub from: Option<String>,
    /// Destination address/network
    pub to: Option<String>,
    /// Routing table
    pub table: Option<u32>,
    /// Rule priority
    pub priority: Option<u32>,
    /// Mark for packet matching
    pub mark: Option<u32>,
}

/// Nameserver configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NameserverConfig {
    /// DNS server addresses
    #[serde(default)]
    pub addresses: Vec<String>,
    /// DNS search domains
    #[serde(default)]
    pub search: Vec<String>,
}

/// Interface matching configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchConfig {
    /// Match by MAC address
    pub macaddress: Option<String>,
    /// Match by driver name
    pub driver: Option<String>,
    /// Match by interface name (supports wildcards like eth*)
    pub name: Option<String>,
}

impl NetworkConfig {
    /// Parse network config from YAML
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        // Try to detect version first
        #[derive(Deserialize)]
        struct VersionCheck {
            #[allow(dead_code)]
            version: Option<u8>,
            network: Option<Box<VersionCheck>>,
        }

        let check: VersionCheck = serde_yaml::from_str(yaml)?;

        // Handle both top-level and nested "network:" key
        let yaml = if check.network.is_some() {
            // Extract the network section
            #[derive(Deserialize)]
            struct Wrapper {
                network: NetworkConfig,
            }
            let wrapper: Wrapper = serde_yaml::from_str(yaml)?;
            return Ok(wrapper.network);
        } else {
            yaml
        };

        serde_yaml::from_str(yaml)
    }

    /// Check if this config has any interfaces defined
    pub fn has_interfaces(&self) -> bool {
        !self.ethernets.is_empty()
            || !self.bonds.is_empty()
            || !self.bridges.is_empty()
            || !self.vlans.is_empty()
    }

    /// Get all interface names
    pub fn interface_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        names.extend(self.ethernets.keys().cloned());
        names.extend(self.bonds.keys().cloned());
        names.extend(self.bridges.keys().cloned());
        names.extend(self.vlans.keys().cloned());
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dhcp() {
        let yaml = r#"
version: 2
ethernets:
  eth0:
    dhcp4: true
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.version, 2);
        assert!(config.ethernets.contains_key("eth0"));
        assert_eq!(config.ethernets["eth0"].common.dhcp4, Some(true));
    }

    #[test]
    fn test_parse_static_ip() {
        let yaml = r#"
version: 2
ethernets:
  eth0:
    addresses:
      - 192.168.1.10/24
    gateway4: 192.168.1.1
    nameservers:
      addresses:
        - 8.8.8.8
        - 8.8.4.4
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        let eth0 = &config.ethernets["eth0"];
        assert_eq!(eth0.common.addresses, vec!["192.168.1.10/24"]);
        assert_eq!(eth0.common.gateway4, Some("192.168.1.1".to_string()));
        assert_eq!(
            eth0.common.nameservers.addresses,
            vec!["8.8.8.8", "8.8.4.4"]
        );
    }

    #[test]
    fn test_parse_bond() {
        let yaml = r#"
version: 2
ethernets:
  eth0: {}
  eth1: {}
bonds:
  bond0:
    interfaces:
      - eth0
      - eth1
    dhcp4: true
    parameters:
      mode: 802.3ad
      lacp-rate: fast
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        assert!(config.bonds.contains_key("bond0"));
        let bond = &config.bonds["bond0"];
        assert_eq!(bond.interfaces, vec!["eth0", "eth1"]);
        assert_eq!(
            bond.parameters.as_ref().unwrap().mode,
            Some("802.3ad".to_string())
        );
    }

    #[test]
    fn test_parse_bridge() {
        let yaml = r#"
version: 2
ethernets:
  eth0: {}
bridges:
  br0:
    interfaces:
      - eth0
    dhcp4: true
    parameters:
      stp: true
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        assert!(config.bridges.contains_key("br0"));
        let bridge = &config.bridges["br0"];
        assert_eq!(bridge.interfaces, vec!["eth0"]);
        assert_eq!(bridge.parameters.as_ref().unwrap().stp, Some(true));
    }

    #[test]
    fn test_parse_vlan() {
        let yaml = r#"
version: 2
ethernets:
  eth0:
    dhcp4: true
vlans:
  vlan100:
    id: 100
    link: eth0
    addresses:
      - 10.0.100.1/24
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        assert!(config.vlans.contains_key("vlan100"));
        let vlan = &config.vlans["vlan100"];
        assert_eq!(vlan.id, 100);
        assert_eq!(vlan.link, "eth0");
    }

    #[test]
    fn test_parse_routes() {
        let yaml = r#"
version: 2
ethernets:
  eth0:
    addresses:
      - 192.168.1.10/24
    routes:
      - to: 10.0.0.0/8
        via: 192.168.1.254
        metric: 100
      - to: default
        via: 192.168.1.1
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        let routes = &config.ethernets["eth0"].common.routes;
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].to, "10.0.0.0/8");
        assert_eq!(routes[0].metric, Some(100));
    }

    #[test]
    fn test_parse_with_network_wrapper() {
        let yaml = r#"
network:
  version: 2
  ethernets:
    eth0:
      dhcp4: true
"#;
        let config = NetworkConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.version, 2);
        assert!(config.ethernets.contains_key("eth0"));
    }
}
