//! systemd-networkd renderer
//!
//! Generates .network, .netdev, and .link files for systemd-networkd.

use super::{RenderedFile, Renderer, RendererType};
use crate::CloudInitError;
use crate::network::{
    BondConfig, BridgeConfig, EthernetConfig, InterfaceCommon, NetworkConfig, VlanConfig,
};
use std::fmt::Write;
use std::path::Path;

/// systemd-networkd renderer
pub struct NetworkdRenderer;

impl NetworkdRenderer {
    pub fn new() -> Self {
        Self
    }

    fn render_ethernet(
        &self,
        name: &str,
        config: &EthernetConfig,
        priority: u32,
    ) -> Vec<RenderedFile> {
        let mut files = Vec::new();

        // Create .network file
        let network_content =
            self.render_network_section(name, &config.common, &config.match_config);
        files.push(RenderedFile {
            path: format!("{:02}-{}.network", priority, name),
            content: network_content,
            mode: 0o644,
        });

        // Create .link file if we have match config
        if let Some(match_config) = &config.match_config
            && (match_config.macaddress.is_some() || match_config.driver.is_some())
        {
            let link_content = self.render_link_section(name, match_config, &config.common);
            files.push(RenderedFile {
                path: format!("{:02}-{}.link", priority, name),
                content: link_content,
                mode: 0o644,
            });
        }

        files
    }

    fn render_bond(&self, name: &str, config: &BondConfig, priority: u32) -> Vec<RenderedFile> {
        let mut files = Vec::new();

        // Create .netdev for the bond
        let mut netdev = String::new();
        writeln!(netdev, "[NetDev]").unwrap();
        writeln!(netdev, "Name={}", name).unwrap();
        writeln!(netdev, "Kind=bond").unwrap();
        writeln!(netdev).unwrap();

        if let Some(params) = &config.parameters {
            writeln!(netdev, "[Bond]").unwrap();
            if let Some(mode) = &params.mode {
                writeln!(netdev, "Mode={}", self.bond_mode_to_networkd(mode)).unwrap();
            }
            if let Some(interval) = params.mii_monitor_interval {
                writeln!(netdev, "MIIMonitorSec={}ms", interval).unwrap();
            }
            if let Some(primary) = &params.primary {
                writeln!(netdev, "PrimaryReselectPolicy={}", primary).unwrap();
            }
            if let Some(policy) = &params.transmit_hash_policy {
                writeln!(netdev, "TransmitHashPolicy={}", policy).unwrap();
            }
            if let Some(rate) = &params.lacp_rate {
                writeln!(netdev, "LACPTransmitRate={}", rate).unwrap();
            }
        }

        files.push(RenderedFile {
            path: format!("{:02}-{}.netdev", priority, name),
            content: netdev,
            mode: 0o644,
        });

        // Create .network for the bond interface
        let network_content = self.render_network_section(name, &config.common, &None);
        files.push(RenderedFile {
            path: format!("{:02}-{}.network", priority, name),
            content: network_content,
            mode: 0o644,
        });

        // Create .network files for member interfaces
        for (i, member) in config.interfaces.iter().enumerate() {
            let mut member_network = String::new();
            writeln!(member_network, "[Match]").unwrap();
            writeln!(member_network, "Name={}", member).unwrap();
            writeln!(member_network).unwrap();
            writeln!(member_network, "[Network]").unwrap();
            writeln!(member_network, "Bond={}", name).unwrap();

            files.push(RenderedFile {
                path: format!("{:02}-{}-{}.network", priority + 1, name, i),
                content: member_network,
                mode: 0o644,
            });
        }

        files
    }

    fn render_bridge(&self, name: &str, config: &BridgeConfig, priority: u32) -> Vec<RenderedFile> {
        let mut files = Vec::new();

        // Create .netdev for the bridge
        let mut netdev = String::new();
        writeln!(netdev, "[NetDev]").unwrap();
        writeln!(netdev, "Name={}", name).unwrap();
        writeln!(netdev, "Kind=bridge").unwrap();
        writeln!(netdev).unwrap();

        if let Some(params) = &config.parameters {
            writeln!(netdev, "[Bridge]").unwrap();
            if let Some(stp) = params.stp {
                writeln!(netdev, "STP={}", if stp { "yes" } else { "no" }).unwrap();
            }
            if let Some(fd) = params.forward_delay {
                writeln!(netdev, "ForwardDelaySec={}", fd).unwrap();
            }
            if let Some(hello) = params.hello_time {
                writeln!(netdev, "HelloTimeSec={}", hello).unwrap();
            }
            if let Some(age) = params.max_age {
                writeln!(netdev, "MaxAgeSec={}", age).unwrap();
            }
            if let Some(prio) = params.priority {
                writeln!(netdev, "Priority={}", prio).unwrap();
            }
        }

        files.push(RenderedFile {
            path: format!("{:02}-{}.netdev", priority, name),
            content: netdev,
            mode: 0o644,
        });

        // Create .network for the bridge interface
        let network_content = self.render_network_section(name, &config.common, &None);
        files.push(RenderedFile {
            path: format!("{:02}-{}.network", priority, name),
            content: network_content,
            mode: 0o644,
        });

        // Create .network files for member interfaces
        for (i, member) in config.interfaces.iter().enumerate() {
            let mut member_network = String::new();
            writeln!(member_network, "[Match]").unwrap();
            writeln!(member_network, "Name={}", member).unwrap();
            writeln!(member_network).unwrap();
            writeln!(member_network, "[Network]").unwrap();
            writeln!(member_network, "Bridge={}", name).unwrap();

            files.push(RenderedFile {
                path: format!("{:02}-{}-{}.network", priority + 1, name, i),
                content: member_network,
                mode: 0o644,
            });
        }

        files
    }

    fn render_vlan(&self, name: &str, config: &VlanConfig, priority: u32) -> Vec<RenderedFile> {
        let mut files = Vec::new();

        // Create .netdev for the VLAN
        let mut netdev = String::new();
        writeln!(netdev, "[NetDev]").unwrap();
        writeln!(netdev, "Name={}", name).unwrap();
        writeln!(netdev, "Kind=vlan").unwrap();
        writeln!(netdev).unwrap();
        writeln!(netdev, "[VLAN]").unwrap();
        writeln!(netdev, "Id={}", config.id).unwrap();

        files.push(RenderedFile {
            path: format!("{:02}-{}.netdev", priority, name),
            content: netdev,
            mode: 0o644,
        });

        // Create .network for the VLAN interface
        let network_content = self.render_network_section(name, &config.common, &None);
        files.push(RenderedFile {
            path: format!("{:02}-{}.network", priority, name),
            content: network_content,
            mode: 0o644,
        });

        // Add VLAN reference to parent interface
        let mut parent_network = String::new();
        writeln!(parent_network, "[Match]").unwrap();
        writeln!(parent_network, "Name={}", config.link).unwrap();
        writeln!(parent_network).unwrap();
        writeln!(parent_network, "[Network]").unwrap();
        writeln!(parent_network, "VLAN={}", name).unwrap();

        files.push(RenderedFile {
            path: format!("{:02}-{}-vlan.network", priority + 1, config.link),
            content: parent_network,
            mode: 0o644,
        });

        files
    }

    fn render_network_section(
        &self,
        name: &str,
        common: &InterfaceCommon,
        match_config: &Option<crate::network::MatchConfig>,
    ) -> String {
        let mut content = String::new();

        // [Match] section
        writeln!(content, "[Match]").unwrap();
        if let Some(mc) = match_config {
            if let Some(mac) = &mc.macaddress {
                writeln!(content, "MACAddress={}", mac).unwrap();
            } else if let Some(drv) = &mc.driver {
                writeln!(content, "Driver={}", drv).unwrap();
            } else if let Some(n) = &mc.name {
                writeln!(content, "Name={}", n).unwrap();
            } else {
                writeln!(content, "Name={}", name).unwrap();
            }
        } else {
            writeln!(content, "Name={}", name).unwrap();
        }
        writeln!(content).unwrap();

        // [Network] section
        writeln!(content, "[Network]").unwrap();

        if common.dhcp4 == Some(true) && common.dhcp6 == Some(true) {
            writeln!(content, "DHCP=yes").unwrap();
        } else if common.dhcp4 == Some(true) {
            writeln!(content, "DHCP=ipv4").unwrap();
        } else if common.dhcp6 == Some(true) {
            writeln!(content, "DHCP=ipv6").unwrap();
        }

        // Static addresses
        for addr in &common.addresses {
            writeln!(content, "Address={}", addr).unwrap();
        }

        // Gateways
        if let Some(gw) = &common.gateway4 {
            writeln!(content, "Gateway={}", gw).unwrap();
        }
        if let Some(gw) = &common.gateway6 {
            writeln!(content, "Gateway={}", gw).unwrap();
        }

        // DNS
        for dns in &common.nameservers.addresses {
            writeln!(content, "DNS={}", dns).unwrap();
        }
        for domain in &common.nameservers.search {
            writeln!(content, "Domains={}", domain).unwrap();
        }

        // IPv6 RA
        if let Some(accept_ra) = common.accept_ra {
            writeln!(
                content,
                "IPv6AcceptRA={}",
                if accept_ra { "yes" } else { "no" }
            )
            .unwrap();
        }

        // [Link] section for MTU
        if common.mtu.is_some() || common.macaddress.is_some() || common.wakeonlan.is_some() {
            writeln!(content).unwrap();
            writeln!(content, "[Link]").unwrap();
            if let Some(mtu) = common.mtu {
                writeln!(content, "MTUBytes={}", mtu).unwrap();
            }
            if let Some(mac) = &common.macaddress {
                writeln!(content, "MACAddress={}", mac).unwrap();
            }
            if let Some(wol) = common.wakeonlan {
                writeln!(content, "WakeOnLan={}", if wol { "magic" } else { "off" }).unwrap();
            }
        }

        // [Route] sections
        for route in &common.routes {
            writeln!(content).unwrap();
            writeln!(content, "[Route]").unwrap();
            writeln!(content, "Destination={}", route.to).unwrap();
            if let Some(via) = &route.via {
                writeln!(content, "Gateway={}", via).unwrap();
            }
            if let Some(metric) = route.metric {
                writeln!(content, "Metric={}", metric).unwrap();
            }
            if let Some(table) = route.table {
                writeln!(content, "Table={}", table).unwrap();
            }
        }

        // [RoutingPolicyRule] sections
        for rule in &common.routing_policy {
            writeln!(content).unwrap();
            writeln!(content, "[RoutingPolicyRule]").unwrap();
            if let Some(from) = &rule.from {
                writeln!(content, "From={}", from).unwrap();
            }
            if let Some(to) = &rule.to {
                writeln!(content, "To={}", to).unwrap();
            }
            if let Some(table) = rule.table {
                writeln!(content, "Table={}", table).unwrap();
            }
            if let Some(prio) = rule.priority {
                writeln!(content, "Priority={}", prio).unwrap();
            }
        }

        content
    }

    fn render_link_section(
        &self,
        _name: &str,
        match_config: &crate::network::MatchConfig,
        common: &InterfaceCommon,
    ) -> String {
        let mut content = String::new();

        writeln!(content, "[Match]").unwrap();
        if let Some(mac) = &match_config.macaddress {
            writeln!(content, "MACAddress={}", mac).unwrap();
        }
        if let Some(drv) = &match_config.driver {
            writeln!(content, "Driver={}", drv).unwrap();
        }
        writeln!(content).unwrap();

        writeln!(content, "[Link]").unwrap();
        if let Some(set_name) = &common.set_name {
            writeln!(content, "Name={}", set_name).unwrap();
        }
        if let Some(mtu) = common.mtu {
            writeln!(content, "MTUBytes={}", mtu).unwrap();
        }
        if let Some(wol) = common.wakeonlan {
            writeln!(content, "WakeOnLan={}", if wol { "magic" } else { "off" }).unwrap();
        }

        content
    }

    fn bond_mode_to_networkd<'a>(&self, mode: &'a str) -> &'a str {
        match mode {
            "balance-rr" | "0" => "balance-rr",
            "active-backup" | "1" => "active-backup",
            "balance-xor" | "2" => "balance-xor",
            "broadcast" | "3" => "broadcast",
            "802.3ad" | "4" => "802.3ad",
            "balance-tlb" | "5" => "balance-tlb",
            "balance-alb" | "6" => "balance-alb",
            _ => mode,
        }
    }
}

impl Default for NetworkdRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer for NetworkdRenderer {
    fn render(
        &self,
        config: &NetworkConfig,
        _output_dir: &Path,
    ) -> Result<Vec<RenderedFile>, CloudInitError> {
        let mut files = Vec::new();
        let mut priority = 10u32;

        // Render ethernets
        for (name, eth_config) in &config.ethernets {
            files.extend(self.render_ethernet(name, eth_config, priority));
            priority += 10;
        }

        // Render bonds
        for (name, bond_config) in &config.bonds {
            files.extend(self.render_bond(name, bond_config, priority));
            priority += 10;
        }

        // Render bridges
        for (name, bridge_config) in &config.bridges {
            files.extend(self.render_bridge(name, bridge_config, priority));
            priority += 10;
        }

        // Render VLANs
        for (name, vlan_config) in &config.vlans {
            files.extend(self.render_vlan(name, vlan_config, priority));
            priority += 10;
        }

        Ok(files)
    }

    fn renderer_type(&self) -> RendererType {
        RendererType::Networkd
    }

    fn is_available(&self) -> bool {
        Path::new("/lib/systemd/systemd-networkd").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::NameserverConfig;
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

        let renderer = NetworkdRenderer::new();
        let files = renderer.render(&config, Path::new("/tmp")).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].path.ends_with(".network"));
        assert!(files[0].content.contains("DHCP=ipv4"));
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

        let renderer = NetworkdRenderer::new();
        let files = renderer.render(&config, Path::new("/tmp")).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].content.contains("Address=192.168.1.10/24"));
        assert!(files[0].content.contains("Gateway=192.168.1.1"));
        assert!(files[0].content.contains("DNS=8.8.8.8"));
    }
}
