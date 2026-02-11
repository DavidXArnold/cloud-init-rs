//! Network config v1 (legacy format) parsing
//!
//! Parses the older dictionary-based network configuration format.
//! This format is still used by some cloud providers and tools.

use super::{
    BondConfig, BondParameters, BridgeConfig, EthernetConfig, InterfaceCommon, MatchConfig,
    NameserverConfig, NetworkConfig, RouteConfig, VlanConfig,
};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Network config v1 format
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfigV1 {
    /// Version (should be 1)
    pub version: u8,
    /// Network configuration items
    #[serde(default)]
    pub config: Vec<ConfigItem>,
}

/// Individual configuration item in v1 format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConfigItem {
    /// Physical network interface
    #[serde(rename = "physical")]
    Physical(PhysicalConfig),
    /// Bond interface
    #[serde(rename = "bond")]
    Bond(BondConfigV1),
    /// Bridge interface
    #[serde(rename = "bridge")]
    Bridge(BridgeConfigV1),
    /// VLAN interface
    #[serde(rename = "vlan")]
    Vlan(VlanConfigV1),
    /// Nameserver configuration
    #[serde(rename = "nameserver")]
    Nameserver(NameserverConfigV1),
    /// Route configuration
    #[serde(rename = "route")]
    Route(RouteConfigV1),
}

/// Physical interface configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhysicalConfig {
    /// Interface name
    pub name: String,
    /// MAC address for matching
    pub mac_address: Option<String>,
    /// MTU
    pub mtu: Option<u32>,
    /// Subnets (IP configuration)
    #[serde(default)]
    pub subnets: Vec<SubnetConfig>,
    /// Wake-on-LAN
    pub wakeonlan: Option<bool>,
}

/// Bond configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BondConfigV1 {
    /// Interface name
    pub name: String,
    /// Interfaces to bond
    #[serde(default)]
    pub bond_interfaces: Vec<String>,
    /// Bond mode
    pub bond_mode: Option<String>,
    /// Bond MII monitoring interval
    pub bond_miimon: Option<u32>,
    /// Hash policy
    pub bond_xmit_hash_policy: Option<String>,
    /// MTU
    pub mtu: Option<u32>,
    /// MAC address
    pub mac_address: Option<String>,
    /// Subnets
    #[serde(default)]
    pub subnets: Vec<SubnetConfig>,
}

/// Bridge configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BridgeConfigV1 {
    /// Interface name
    pub name: String,
    /// Bridge interfaces
    #[serde(default)]
    pub bridge_interfaces: Vec<String>,
    /// Bridge STP
    pub bridge_stp: Option<bool>,
    /// Bridge forward delay
    pub bridge_fd: Option<u32>,
    /// MTU
    pub mtu: Option<u32>,
    /// Subnets
    #[serde(default)]
    pub subnets: Vec<SubnetConfig>,
}

/// VLAN configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VlanConfigV1 {
    /// Interface name (e.g., "eth0.100")
    pub name: String,
    /// VLAN ID
    pub vlan_id: u16,
    /// Parent interface
    pub vlan_link: String,
    /// MTU
    pub mtu: Option<u32>,
    /// Subnets
    #[serde(default)]
    pub subnets: Vec<SubnetConfig>,
}

/// Subnet/IP configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubnetConfig {
    /// Subnet type: static, dhcp4, dhcp6, ipv6_slaac, etc.
    #[serde(rename = "type")]
    pub subnet_type: String,
    /// IP address (for static)
    pub address: Option<String>,
    /// Network prefix/netmask
    pub netmask: Option<String>,
    /// Gateway
    pub gateway: Option<String>,
    /// DNS servers
    #[serde(default)]
    pub dns_nameservers: Vec<String>,
    /// DNS search domains
    #[serde(default)]
    pub dns_search: Vec<String>,
    /// Routes
    #[serde(default)]
    pub routes: Vec<RouteConfigV1>,
}

/// Nameserver configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NameserverConfigV1 {
    /// DNS server addresses
    #[serde(default)]
    pub address: Vec<String>,
    /// Search domains
    #[serde(default)]
    pub search: Vec<String>,
}

/// Route configuration (v1)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteConfigV1 {
    /// Destination network
    pub destination: Option<String>,
    /// Gateway
    pub gateway: Option<String>,
    /// Metric
    pub metric: Option<u32>,
    /// Network (alternative to destination)
    pub network: Option<String>,
    /// Netmask for network
    pub netmask: Option<String>,
}

impl NetworkConfigV1 {
    /// Parse v1 config from YAML
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        // Handle both top-level and nested "network:" key
        #[derive(Deserialize)]
        struct Wrapper {
            network: Option<NetworkConfigV1>,
            #[serde(flatten)]
            config: Option<NetworkConfigV1>,
        }

        let wrapper: Wrapper = serde_yaml::from_str(yaml)?;

        if let Some(network) = wrapper.network {
            Ok(network)
        } else if let Some(config) = wrapper.config {
            Ok(config)
        } else {
            Ok(NetworkConfigV1::default())
        }
    }

    /// Convert v1 config to v2 format
    pub fn to_v2(&self) -> NetworkConfig {
        debug!("Converting network config v1 to v2");

        let mut v2 = NetworkConfig {
            version: 2,
            renderer: None,
            ..Default::default()
        };

        // Global nameservers (collected from nameserver items)
        let mut global_dns: Vec<String> = Vec::new();
        let mut global_search: Vec<String> = Vec::new();

        for item in &self.config {
            match item {
                ConfigItem::Physical(phys) => {
                    let eth = self.convert_physical(phys);
                    v2.ethernets.insert(phys.name.clone(), eth);
                }
                ConfigItem::Bond(bond) => {
                    let bond_cfg = self.convert_bond(bond);
                    v2.bonds.insert(bond.name.clone(), bond_cfg);
                }
                ConfigItem::Bridge(bridge) => {
                    let bridge_cfg = self.convert_bridge(bridge);
                    v2.bridges.insert(bridge.name.clone(), bridge_cfg);
                }
                ConfigItem::Vlan(vlan) => {
                    let vlan_cfg = self.convert_vlan(vlan);
                    v2.vlans.insert(vlan.name.clone(), vlan_cfg);
                }
                ConfigItem::Nameserver(ns) => {
                    global_dns.extend(ns.address.clone());
                    global_search.extend(ns.search.clone());
                }
                ConfigItem::Route(_) => {
                    // Global routes are handled per-interface in v2
                    // We'll skip these for now as they need to be attached to an interface
                }
            }
        }

        // Apply global DNS to all interfaces that don't have their own
        if !global_dns.is_empty() || !global_search.is_empty() {
            let global_ns = NameserverConfig {
                addresses: global_dns,
                search: global_search,
            };

            for eth in v2.ethernets.values_mut() {
                if eth.common.nameservers.addresses.is_empty() {
                    eth.common.nameservers = global_ns.clone();
                }
            }
        }

        v2
    }

    fn convert_physical(&self, phys: &PhysicalConfig) -> EthernetConfig {
        let mut common = InterfaceCommon {
            mtu: phys.mtu,
            wakeonlan: phys.wakeonlan,
            ..Default::default()
        };

        // Process subnets
        self.apply_subnets(&mut common, &phys.subnets);

        EthernetConfig {
            common,
            match_config: phys.mac_address.as_ref().map(|mac| MatchConfig {
                macaddress: Some(mac.clone()),
                ..Default::default()
            }),
        }
    }

    fn convert_bond(&self, bond: &BondConfigV1) -> BondConfig {
        let mut common = InterfaceCommon {
            mtu: bond.mtu,
            macaddress: bond.mac_address.clone(),
            ..Default::default()
        };

        self.apply_subnets(&mut common, &bond.subnets);

        BondConfig {
            common,
            interfaces: bond.bond_interfaces.clone(),
            parameters: Some(BondParameters {
                mode: bond.bond_mode.clone(),
                mii_monitor_interval: bond.bond_miimon,
                transmit_hash_policy: bond.bond_xmit_hash_policy.clone(),
                ..Default::default()
            }),
        }
    }

    fn convert_bridge(&self, bridge: &BridgeConfigV1) -> BridgeConfig {
        let mut common = InterfaceCommon {
            mtu: bridge.mtu,
            ..Default::default()
        };

        self.apply_subnets(&mut common, &bridge.subnets);

        BridgeConfig {
            common,
            interfaces: bridge.bridge_interfaces.clone(),
            parameters: Some(super::BridgeParameters {
                stp: bridge.bridge_stp,
                forward_delay: bridge.bridge_fd,
                ..Default::default()
            }),
        }
    }

    fn convert_vlan(&self, vlan: &VlanConfigV1) -> VlanConfig {
        let mut common = InterfaceCommon {
            mtu: vlan.mtu,
            ..Default::default()
        };

        self.apply_subnets(&mut common, &vlan.subnets);

        VlanConfig {
            common,
            id: vlan.vlan_id,
            link: vlan.vlan_link.clone(),
        }
    }

    fn apply_subnets(&self, common: &mut InterfaceCommon, subnets: &[SubnetConfig]) {
        for subnet in subnets {
            match subnet.subnet_type.as_str() {
                "dhcp" | "dhcp4" => {
                    common.dhcp4 = Some(true);
                }
                "dhcp6" => {
                    common.dhcp6 = Some(true);
                }
                "static" | "static4" | "static6" => {
                    if let Some(addr) = &subnet.address {
                        let cidr = if let Some(mask) = &subnet.netmask {
                            format!("{}/{}", addr, netmask_to_prefix(mask))
                        } else {
                            addr.clone()
                        };
                        common.addresses.push(cidr);
                    }

                    if let Some(gw) = &subnet.gateway {
                        // Determine if IPv4 or IPv6
                        if gw.contains(':') {
                            common.gateway6 = Some(gw.clone());
                        } else {
                            common.gateway4 = Some(gw.clone());
                        }
                    }
                }
                "ipv6_slaac" | "ipv6_dhcpv6-stateless" => {
                    common.accept_ra = Some(true);
                }
                "ipv6_dhcpv6-stateful" => {
                    common.dhcp6 = Some(true);
                }
                _ => {}
            }

            // DNS
            if !subnet.dns_nameservers.is_empty() {
                common
                    .nameservers
                    .addresses
                    .extend(subnet.dns_nameservers.clone());
            }
            if !subnet.dns_search.is_empty() {
                common.nameservers.search.extend(subnet.dns_search.clone());
            }

            // Routes
            for route in &subnet.routes {
                let dest = route
                    .destination
                    .clone()
                    .or_else(|| {
                        route.network.as_ref().map(|net| {
                            if let Some(mask) = &route.netmask {
                                format!("{}/{}", net, netmask_to_prefix(mask))
                            } else {
                                net.clone()
                            }
                        })
                    })
                    .unwrap_or_else(|| "default".to_string());

                common.routes.push(RouteConfig {
                    to: dest,
                    via: route.gateway.clone(),
                    metric: route.metric,
                    ..Default::default()
                });
            }
        }
    }
}

/// Convert netmask to CIDR prefix length
fn netmask_to_prefix(netmask: &str) -> u8 {
    // Handle CIDR notation directly
    if let Ok(prefix) = netmask.parse::<u8>() {
        return prefix;
    }

    // Convert dotted-decimal netmask to prefix
    let octets: Vec<u8> = netmask.split('.').filter_map(|s| s.parse().ok()).collect();

    if octets.len() != 4 {
        return 24; // Default to /24
    }

    let mut prefix = 0u8;
    for octet in octets {
        prefix += octet.count_ones() as u8;
    }
    prefix
}

/// Detect and parse network config (v1 or v2)
pub fn parse_network_config(yaml: &str) -> Result<NetworkConfig, serde_yaml::Error> {
    // Try to detect version
    #[derive(Deserialize)]
    struct VersionCheck {
        version: Option<u8>,
        network: Option<Box<VersionCheck>>,
    }

    let check: VersionCheck = serde_yaml::from_str(yaml)?;
    let version = check
        .version
        .or_else(|| check.network.as_ref().and_then(|n| n.version))
        .unwrap_or(2);

    if version == 1 {
        let v1 = NetworkConfigV1::from_yaml(yaml)?;
        Ok(v1.to_v2())
    } else {
        NetworkConfig::from_yaml(yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_v1_physical() {
        let yaml = r#"
version: 1
config:
  - type: physical
    name: eth0
    mac_address: "00:11:22:33:44:55"
    subnets:
      - type: dhcp4
"#;
        let v1 = NetworkConfigV1::from_yaml(yaml).unwrap();
        assert_eq!(v1.version, 1);
        assert_eq!(v1.config.len(), 1);

        let v2 = v1.to_v2();
        assert!(v2.ethernets.contains_key("eth0"));
        assert_eq!(v2.ethernets["eth0"].common.dhcp4, Some(true));
    }

    #[test]
    fn test_parse_v1_static() {
        let yaml = r#"
version: 1
config:
  - type: physical
    name: eth0
    subnets:
      - type: static
        address: 192.168.1.10
        netmask: 255.255.255.0
        gateway: 192.168.1.1
        dns_nameservers:
          - 8.8.8.8
"#;
        let v1 = NetworkConfigV1::from_yaml(yaml).unwrap();
        let v2 = v1.to_v2();

        let eth0 = &v2.ethernets["eth0"];
        assert_eq!(eth0.common.addresses, vec!["192.168.1.10/24"]);
        assert_eq!(eth0.common.gateway4, Some("192.168.1.1".to_string()));
        assert_eq!(eth0.common.nameservers.addresses, vec!["8.8.8.8"]);
    }

    #[test]
    fn test_parse_v1_bond() {
        let yaml = r#"
version: 1
config:
  - type: physical
    name: eth0
  - type: physical
    name: eth1
  - type: bond
    name: bond0
    bond_interfaces:
      - eth0
      - eth1
    bond_mode: 802.3ad
    subnets:
      - type: dhcp4
"#;
        let v1 = NetworkConfigV1::from_yaml(yaml).unwrap();
        let v2 = v1.to_v2();

        assert!(v2.bonds.contains_key("bond0"));
        let bond = &v2.bonds["bond0"];
        assert_eq!(bond.interfaces, vec!["eth0", "eth1"]);
        assert_eq!(
            bond.parameters.as_ref().unwrap().mode,
            Some("802.3ad".to_string())
        );
    }

    #[test]
    fn test_netmask_to_prefix() {
        assert_eq!(netmask_to_prefix("255.255.255.0"), 24);
        assert_eq!(netmask_to_prefix("255.255.0.0"), 16);
        assert_eq!(netmask_to_prefix("255.0.0.0"), 8);
        assert_eq!(netmask_to_prefix("255.255.255.128"), 25);
        assert_eq!(netmask_to_prefix("24"), 24);
    }

    #[test]
    fn test_auto_detect_version() {
        let v1_yaml = r#"
version: 1
config:
  - type: physical
    name: eth0
    subnets:
      - type: dhcp4
"#;
        let v2_yaml = r#"
version: 2
ethernets:
  eth0:
    dhcp4: true
"#;

        let config1 = parse_network_config(v1_yaml).unwrap();
        let config2 = parse_network_config(v2_yaml).unwrap();

        // Both should result in equivalent v2 configs
        assert!(config1.ethernets.contains_key("eth0"));
        assert!(config2.ethernets.contains_key("eth0"));
    }
}
