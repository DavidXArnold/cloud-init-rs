//! NetworkManager renderer
//!
//! Generates .nmconnection files for NetworkManager.

use super::{RenderedFile, Renderer, RendererType};
use crate::CloudInitError;
use crate::network::{EthernetConfig, InterfaceCommon, NetworkConfig};
use std::fmt::Write;
use std::path::Path;
use uuid::Uuid;

/// NetworkManager renderer
pub struct NetworkManagerRenderer;

impl NetworkManagerRenderer {
    pub fn new() -> Self {
        Self
    }

    fn render_ethernet(&self, name: &str, config: &EthernetConfig) -> RenderedFile {
        let uuid = Uuid::new_v4();
        let mut content = String::new();

        // [connection] section
        writeln!(content, "[connection]").unwrap();
        writeln!(content, "id={}", name).unwrap();
        writeln!(content, "uuid={}", uuid).unwrap();
        writeln!(content, "type=ethernet").unwrap();
        writeln!(content, "interface-name={}", name).unwrap();
        writeln!(content).unwrap();

        // [ethernet] section
        writeln!(content, "[ethernet]").unwrap();
        if let Some(match_config) = &config.match_config
            && let Some(mac) = &match_config.macaddress
        {
            writeln!(content, "mac-address={}", mac).unwrap();
        }
        if let Some(mtu) = config.common.mtu {
            writeln!(content, "mtu={}", mtu).unwrap();
        }
        if let Some(wol) = config.common.wakeonlan {
            writeln!(content, "wake-on-lan={}", if wol { 64 } else { 0 }).unwrap();
        }
        writeln!(content).unwrap();

        // IPv4 section
        self.write_ipv4_section(&mut content, &config.common);

        // IPv6 section
        self.write_ipv6_section(&mut content, &config.common);

        RenderedFile {
            path: format!("{}.nmconnection", name),
            content,
            mode: 0o600, // NetworkManager requires 0600
        }
    }

    fn write_ipv4_section(&self, content: &mut String, common: &InterfaceCommon) {
        writeln!(content, "[ipv4]").unwrap();

        if common.dhcp4 == Some(true) {
            writeln!(content, "method=auto").unwrap();
        } else if !common.addresses.is_empty() {
            writeln!(content, "method=manual").unwrap();

            // Filter IPv4 addresses
            let ipv4_addrs: Vec<_> = common
                .addresses
                .iter()
                .filter(|a| !a.contains(':'))
                .collect();

            for (i, addr) in ipv4_addrs.iter().enumerate() {
                writeln!(content, "address{}={}", i + 1, addr).unwrap();
            }
        } else {
            writeln!(content, "method=disabled").unwrap();
        }

        if let Some(gw) = &common.gateway4 {
            writeln!(content, "gateway={}", gw).unwrap();
        }

        // DNS servers (IPv4 only)
        let ipv4_dns: Vec<_> = common
            .nameservers
            .addresses
            .iter()
            .filter(|d| !d.contains(':'))
            .map(|s| s.as_str())
            .collect();

        if !ipv4_dns.is_empty() {
            writeln!(content, "dns={}", ipv4_dns.join(";")).unwrap();
        }

        if !common.nameservers.search.is_empty() {
            writeln!(
                content,
                "dns-search={}",
                common.nameservers.search.join(";")
            )
            .unwrap();
        }

        // Routes
        for (i, route) in common.routes.iter().enumerate() {
            if route.to.contains(':') {
                continue; // Skip IPv6 routes
            }
            let mut route_str = route.to.clone();
            if let Some(via) = &route.via {
                route_str = format!("{},{}", route_str, via);
            }
            if let Some(metric) = route.metric {
                route_str = format!("{},{}", route_str, metric);
            }
            writeln!(content, "route{}={}", i + 1, route_str).unwrap();
        }

        writeln!(content).unwrap();
    }

    fn write_ipv6_section(&self, content: &mut String, common: &InterfaceCommon) {
        writeln!(content, "[ipv6]").unwrap();

        if common.dhcp6 == Some(true) {
            writeln!(content, "method=auto").unwrap();
        } else if common.accept_ra == Some(true) {
            writeln!(content, "method=auto").unwrap();
            writeln!(content, "addr-gen-mode=eui64").unwrap();
        } else {
            // Check for IPv6 static addresses
            let ipv6_addrs: Vec<_> = common
                .addresses
                .iter()
                .filter(|a| a.contains(':'))
                .collect();

            if !ipv6_addrs.is_empty() {
                writeln!(content, "method=manual").unwrap();
                for (i, addr) in ipv6_addrs.iter().enumerate() {
                    writeln!(content, "address{}={}", i + 1, addr).unwrap();
                }
            } else {
                writeln!(content, "method=ignore").unwrap();
            }
        }

        if let Some(gw) = &common.gateway6 {
            writeln!(content, "gateway={}", gw).unwrap();
        }

        // DNS servers (IPv6 only)
        let ipv6_dns: Vec<_> = common
            .nameservers
            .addresses
            .iter()
            .filter(|d| d.contains(':'))
            .map(|s| s.as_str())
            .collect();

        if !ipv6_dns.is_empty() {
            writeln!(content, "dns={}", ipv6_dns.join(";")).unwrap();
        }

        writeln!(content).unwrap();
    }
}

impl Default for NetworkManagerRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for NetworkManagerRenderer {
    fn render(
        &self,
        config: &NetworkConfig,
        _output_dir: &Path,
    ) -> Result<Vec<RenderedFile>, CloudInitError> {
        let mut files = Vec::new();

        // Render ethernets
        for (name, eth_config) in &config.ethernets {
            files.push(self.render_ethernet(name, eth_config));
        }

        // TODO: Implement bonds, bridges, VLANs for NetworkManager
        // These require additional connection types and more complex configuration

        Ok(files)
    }

    fn renderer_type(&self) -> RendererType {
        RendererType::NetworkManager
    }

    fn is_available(&self) -> bool {
        Path::new("/usr/sbin/NetworkManager").exists() || Path::new("/usr/bin/nmcli").exists()
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

        let renderer = NetworkManagerRenderer::new();
        let files = renderer.render(&config, Path::new("/tmp")).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].path.ends_with(".nmconnection"));
        assert!(files[0].content.contains("method=auto"));
        assert_eq!(files[0].mode, 0o600);
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

        let renderer = NetworkManagerRenderer::new();
        let files = renderer.render(&config, Path::new("/tmp")).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("method=manual"));
        assert!(files[0].content.contains("address1=192.168.1.10/24"));
        assert!(files[0].content.contains("gateway=192.168.1.1"));
        assert!(files[0].content.contains("dns=8.8.8.8"));
    }
}
