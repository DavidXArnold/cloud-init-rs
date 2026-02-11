//! Debian ENI (Ethernet Network Interfaces) renderer
//!
//! Generates /etc/network/interfaces format configuration.

use super::{RenderedFile, Renderer, RendererType};
use crate::CloudInitError;
use crate::network::{EthernetConfig, NetworkConfig};
use std::fmt::Write;
use std::path::Path;

/// Debian ENI renderer
pub struct EniRenderer;

impl EniRenderer {
    pub fn new() -> Self {
        Self
    }

    fn render_interface(&self, name: &str, config: &EthernetConfig) -> String {
        let mut content = String::new();

        // Determine the interface configuration method
        if config.common.dhcp4 == Some(true) {
            writeln!(content, "auto {}", name).unwrap();
            writeln!(content, "iface {} inet dhcp", name).unwrap();
        } else if !config.common.addresses.is_empty() {
            // Static configuration
            let ipv4_addrs: Vec<_> = config
                .common
                .addresses
                .iter()
                .filter(|a| !a.contains(':'))
                .collect();

            if !ipv4_addrs.is_empty() {
                writeln!(content, "auto {}", name).unwrap();
                writeln!(content, "iface {} inet static", name).unwrap();

                // Parse first address for primary config
                if let Some(addr) = ipv4_addrs.first() {
                    let (ip, mask) = self.parse_cidr(addr);
                    writeln!(content, "    address {}", ip).unwrap();
                    writeln!(content, "    netmask {}", mask).unwrap();
                }

                if let Some(gw) = &config.common.gateway4 {
                    writeln!(content, "    gateway {}", gw).unwrap();
                }

                // DNS
                if !config.common.nameservers.addresses.is_empty() {
                    writeln!(
                        content,
                        "    dns-nameservers {}",
                        config.common.nameservers.addresses.join(" ")
                    )
                    .unwrap();
                }

                if !config.common.nameservers.search.is_empty() {
                    writeln!(
                        content,
                        "    dns-search {}",
                        config.common.nameservers.search.join(" ")
                    )
                    .unwrap();
                }

                // Additional addresses
                for addr in ipv4_addrs.iter().skip(1) {
                    let (ip, mask) = self.parse_cidr(addr);
                    writeln!(content, "    up ip addr add {}/{} dev {}", ip, mask, name).unwrap();
                }
            }
        } else {
            // Manual mode (no auto-config)
            writeln!(content, "auto {}", name).unwrap();
            writeln!(content, "iface {} inet manual", name).unwrap();
        }

        // MTU
        if let Some(mtu) = config.common.mtu {
            writeln!(content, "    mtu {}", mtu).unwrap();
        }

        // WoL
        if config.common.wakeonlan == Some(true) {
            writeln!(content, "    ethernet-wol g").unwrap();
        }

        // Routes
        for route in &config.common.routes {
            if route.to.contains(':') {
                continue; // Skip IPv6 routes
            }
            let mut route_cmd = format!("    up ip route add {}", route.to);
            if let Some(via) = &route.via {
                route_cmd = format!("{} via {}", route_cmd, via);
            }
            if let Some(metric) = route.metric {
                route_cmd = format!("{} metric {}", route_cmd, metric);
            }
            writeln!(content, "{}", route_cmd).unwrap();
        }

        // IPv6 configuration
        if config.common.dhcp6 == Some(true) {
            writeln!(content).unwrap();
            writeln!(content, "iface {} inet6 dhcp", name).unwrap();
        } else if config.common.accept_ra == Some(true) {
            writeln!(content).unwrap();
            writeln!(content, "iface {} inet6 auto", name).unwrap();
        } else {
            let ipv6_addrs: Vec<_> = config
                .common
                .addresses
                .iter()
                .filter(|a| a.contains(':'))
                .collect();

            if !ipv6_addrs.is_empty() {
                writeln!(content).unwrap();
                writeln!(content, "iface {} inet6 static", name).unwrap();

                if let Some(addr) = ipv6_addrs.first() {
                    writeln!(content, "    address {}", addr).unwrap();
                }

                if let Some(gw) = &config.common.gateway6 {
                    writeln!(content, "    gateway {}", gw).unwrap();
                }
            }
        }

        content
    }

    fn parse_cidr(&self, cidr: &str) -> (String, String) {
        let parts: Vec<&str> = cidr.split('/').collect();
        let ip = parts[0].to_string();
        let prefix = parts
            .get(1)
            .and_then(|p| p.parse::<u8>().ok())
            .unwrap_or(24);
        let mask = self.prefix_to_netmask(prefix);
        (ip, mask)
    }

    fn prefix_to_netmask(&self, prefix: u8) -> String {
        let mask: u32 = if prefix >= 32 {
            0xffffffff
        } else {
            0xffffffff << (32 - prefix)
        };
        format!(
            "{}.{}.{}.{}",
            (mask >> 24) & 0xff,
            (mask >> 16) & 0xff,
            (mask >> 8) & 0xff,
            mask & 0xff
        )
    }
}

impl Default for EniRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for EniRenderer {
    fn render(
        &self,
        config: &NetworkConfig,
        _output_dir: &Path,
    ) -> Result<Vec<RenderedFile>, CloudInitError> {
        let mut content = String::new();

        // Header
        writeln!(content, "# This file is generated by cloud-init").unwrap();
        writeln!(content, "# See interfaces(5) for file format").unwrap();
        writeln!(content).unwrap();

        // Loopback
        writeln!(content, "auto lo").unwrap();
        writeln!(content, "iface lo inet loopback").unwrap();
        writeln!(content).unwrap();

        // Render all ethernet interfaces
        for (name, eth_config) in &config.ethernets {
            content.push_str(&self.render_interface(name, eth_config));
            writeln!(content).unwrap();
        }

        // TODO: Implement bonds and bridges for ENI

        Ok(vec![RenderedFile {
            path: "interfaces".to_string(),
            content,
            mode: 0o644,
        }])
    }

    fn renderer_type(&self) -> RendererType {
        RendererType::Eni
    }

    fn is_available(&self) -> bool {
        Path::new("/etc/network/interfaces").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{InterfaceCommon, NameserverConfig};
    use std::collections::HashMap;

    #[test]
    fn test_render_dhcp() {
        let mut ethernets = HashMap::new();
        ethernets.insert(
            "eth0".to_string(),
            EthernetConfig {
                common: InterfaceCommon {
                    dhcp4: Some(true),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let config = NetworkConfig {
            version: 2,
            ethernets,
            ..Default::default()
        };

        let renderer = EniRenderer::new();
        let files = renderer.render(&config, Path::new("/tmp")).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "interfaces");
        assert!(files[0].content.contains("auto eth0"));
        assert!(files[0].content.contains("iface eth0 inet dhcp"));
    }

    #[test]
    fn test_render_static() {
        let mut ethernets = HashMap::new();
        ethernets.insert(
            "eth0".to_string(),
            EthernetConfig {
                common: InterfaceCommon {
                    addresses: vec!["192.168.1.10/24".to_string()],
                    gateway4: Some("192.168.1.1".to_string()),
                    nameservers: NameserverConfig {
                        addresses: vec!["8.8.8.8".to_string()],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let config = NetworkConfig {
            version: 2,
            ethernets,
            ..Default::default()
        };

        let renderer = EniRenderer::new();
        let files = renderer.render(&config, Path::new("/tmp")).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("iface eth0 inet static"));
        assert!(files[0].content.contains("address 192.168.1.10"));
        assert!(files[0].content.contains("netmask 255.255.255.0"));
        assert!(files[0].content.contains("gateway 192.168.1.1"));
        assert!(files[0].content.contains("dns-nameservers 8.8.8.8"));
    }

    #[test]
    fn test_prefix_to_netmask() {
        let renderer = EniRenderer::new();
        assert_eq!(renderer.prefix_to_netmask(24), "255.255.255.0");
        assert_eq!(renderer.prefix_to_netmask(16), "255.255.0.0");
        assert_eq!(renderer.prefix_to_netmask(8), "255.0.0.0");
        assert_eq!(renderer.prefix_to_netmask(25), "255.255.255.128");
        assert_eq!(renderer.prefix_to_netmask(32), "255.255.255.255");
    }
}
