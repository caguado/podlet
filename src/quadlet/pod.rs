use std::{
    fmt::Write as _,
    net::{Ipv4Addr, Ipv6Addr},
    path::PathBuf,
};

use indexmap::IndexMap;
use serde::{Serialize, Serializer};

use super::{
    Downgrade, DowngradeError, HostPaths, PodmanVersion, ResourceKind,
    container::{Dns, Volume},
};

/// Network attachment options for a pod.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PodNetworkOptions {
    /// Static IPv4 address for this network attachment.
    pub ip: Option<Ipv4Addr>,
}

/// Serialize `network_attachments` as repeated `Network=<name>.network[:ip=<addr>]` entries.
fn serialize_network_attachments<S: Serializer>(
    attachments: &IndexMap<String, PodNetworkOptions>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let strings: Vec<String> = attachments
        .iter()
        .map(|(name, opts)| {
            let mut s = format!("{name}.network");
            if let Some(ip) = opts.ip {
                let _ = write!(s, ":ip={ip}");
            }
            s
        })
        .collect();
    strings.serialize(serializer)
}

/// Options for the \[Pod\] section of a `.pod` Quadlet file.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Pod {
    /// Add host-to-IP mapping to `/etc/hosts`.
    pub add_host: Vec<String>,

    /// Set network-scoped DNS resolver/nameserver for containers in this pod.
    #[serde(rename = "DNS")]
    pub dns: Dns,

    /// Set custom DNS options.
    #[serde(rename = "DNSOption")]
    pub dns_option: Vec<String>,

    /// Set custom DNS search domains.
    #[serde(rename = "DNSSearch")]
    pub dns_search: Vec<String>,

    /// GID map for the user namespace.
    #[serde(rename = "GIDMap")]
    pub gid_map: Vec<String>,

    /// Specify a static IPv4 address for the pod.
    #[serde(rename = "IP")]
    pub ip: Option<Ipv4Addr>,

    /// Specify a static IPv6 address for the pod.
    #[serde(rename = "IP6")]
    pub ip6: Option<Ipv6Addr>,

    /// Specify a custom network for the pod.
    pub network: Vec<String>,

    /// Named network attachments with options (e.g., static IP).
    ///
    /// Serialized as `Network=<name>.network[:ip=<addr>]` entries.
    #[serde(
        rename = "Network",
        serialize_with = "serialize_network_attachments",
        skip_serializing_if = "IndexMap::is_empty"
    )]
    pub network_attachments: IndexMap<String, PodNetworkOptions>,

    /// Add a network-scoped alias for the pod.
    pub network_alias: Vec<String>,

    /// A list of arguments passed directly to the end of the `podman pod create` command in the
    /// generated file.
    pub podman_args: Option<String>,

    /// The name of the Podman pod.
    ///
    /// If not set, the default value is `systemd-%N`.
    #[allow(clippy::struct_field_names)]
    pub pod_name: Option<String>,

    /// Exposes a port, or a range of ports, from the pod to the host.
    pub publish_port: Vec<String>,

    /// Create the pod in a new user namespace using the map with name in the `/etc/subgid` file.
    #[serde(rename = "SubGIDMap")]
    pub sub_gid_map: Option<String>,

    /// Create the pod in a new user namespace using the map with name in the `/etc/subuid` file.
    #[serde(rename = "SubUIDMap")]
    pub sub_uid_map: Option<String>,

    /// UID map for the user namespace.
    #[serde(rename = "UIDMap")]
    pub uid_map: Vec<String>,

    /// Set the user namespace mode for the pod.
    #[serde(rename = "UserNS")]
    pub user_ns: Option<String>,

    /// Mount a volume in the pod.
    pub volume: Vec<Volume>,
}

impl HostPaths for Pod {
    fn host_paths(&mut self) -> impl Iterator<Item = &mut PathBuf> {
        self.volume.host_paths()
    }
}

impl Downgrade for Pod {
    fn downgrade(&mut self, version: PodmanVersion) -> Result<(), DowngradeError> {
        if version < PodmanVersion::V5_3 {
            self.remove_v5_3_options();
        }

        if version < PodmanVersion::V5_2 {
            for network_alias in std::mem::take(&mut self.network_alias) {
                self.push_arg("network-alias", &network_alias);
            }
        }

        if version < PodmanVersion::V5_0 {
            return Err(DowngradeError::Kind {
                kind: ResourceKind::Pod,
                supported_version: PodmanVersion::V5_0,
            });
        }

        Ok(())
    }
}

impl Pod {
    /// Remove Quadlet options added in Podman v5.3.0.
    fn remove_v5_3_options(&mut self) {
        for add_host in std::mem::take(&mut self.add_host) {
            self.push_arg("add-host", &add_host);
        }

        match std::mem::take(&mut self.dns) {
            Dns::None => self.push_arg("dns", "none"),
            Dns::Custom(ip_addrs) => {
                for ip_addr in ip_addrs {
                    self.push_arg("dns", &ip_addr.to_string());
                }
            }
        }

        for dns_option in std::mem::take(&mut self.dns_option) {
            self.push_arg("dns-option", &dns_option);
        }

        for dns_search in std::mem::take(&mut self.dns_search) {
            self.push_arg("dns-search", &dns_search);
        }

        for gidmap in std::mem::take(&mut self.gid_map) {
            self.push_arg("gidmap", &gidmap);
        }

        if let Some(ip) = self.ip.take() {
            self.push_arg("ip", &ip.to_string());
        }

        if let Some(ip6) = self.ip6.take() {
            self.push_arg("ip6", &ip6.to_string());
        }

        if let Some(subgidname) = self.sub_gid_map.take() {
            self.push_arg("subgidname", &subgidname);
        }

        if let Some(subuidname) = self.sub_uid_map.take() {
            self.push_arg("subuidname", &subuidname);
        }

        for uidmap in std::mem::take(&mut self.uid_map) {
            self.push_arg("uidmap", &uidmap);
        }

        if let Some(userns) = self.user_ns.take() {
            self.push_arg("userns", &userns);
        }
    }

    /// Add `--{flag} {arg}` to `PodmanArgs=`.
    fn push_arg(&mut self, flag: &str, arg: &str) {
        let podman_args = self.podman_args.get_or_insert_with(String::new);
        if !podman_args.is_empty() {
            podman_args.push(' ');
        }
        podman_args.push_str("--");
        podman_args.push_str(flag);
        podman_args.push(' ');
        if arg.contains(char::is_whitespace) {
            podman_args.push('"');
            podman_args.push_str(arg);
            podman_args.push('"');
        } else {
            podman_args.push_str(arg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pod_network_attachments_serializes_with_ip() -> Result<(), crate::serde::quadlet::Error> {
        let mut pod = Pod::default();
        pod.network_attachments.insert(
            "obs".to_owned(),
            PodNetworkOptions {
                ip: Some("10.0.0.1".parse().expect("valid IP")),
            },
        );
        let output = crate::serde::quadlet::to_string_join_all(pod)?;
        assert!(
            output.contains("Network=obs.network:ip=10.0.0.1"),
            "{output}"
        );
        Ok(())
    }

    #[test]
    fn pod_network_attachments_serializes_without_ip() -> Result<(), crate::serde::quadlet::Error> {
        let mut pod = Pod::default();
        pod.network_attachments
            .insert("obs".to_owned(), PodNetworkOptions::default());
        let output = crate::serde::quadlet::to_string_join_all(pod)?;
        assert!(output.contains("Network=obs.network"), "{output}");
        assert!(!output.contains(":ip="), "{output}");
        Ok(())
    }

    #[test]
    fn pod_user_ns_serializes() -> Result<(), crate::serde::quadlet::Error> {
        let pod = Pod {
            user_ns: Some("auto:uidmapping=0:1000:1024".to_owned()),
            ..Pod::default()
        };
        let output = crate::serde::quadlet::to_string_join_all(pod)?;
        assert!(
            output.contains("UserNS=auto:uidmapping=0:1000:1024"),
            "{output}"
        );
        Ok(())
    }
}
