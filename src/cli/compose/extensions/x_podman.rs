use color_eyre::eyre::{Result, WrapErr, bail};
use compose_spec::Extensions;
use serde::Deserialize;

use crate::quadlet;

use super::{ComposeExtensionHandler, ExtensionContext};

/// Extension key processed by this handler.
const KEY: &str = "x-podman";

/// Handler for the `x-podman` extension at service, network, and volume scope.
pub struct XPodmanHandler;

impl ComposeExtensionHandler for XPodmanHandler {
    fn handled_keys(&self) -> &[&'static str] {
        &[KEY]
    }

    fn handle_service(
        &self,
        service_name: &str,
        extensions: &Extensions,
        _context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        let Some(value) = extensions.get(KEY) else {
            return Ok(());
        };

        let x_podman: XPodmanService =
            serde_yaml::from_value(value.clone()).wrap_err_with(|| {
                format!("error deserializing `x-podman` for service `{service_name}`")
            })?;

        if let quadlet::Resource::Container(container) = &mut file.resource {
            container.cgroups_mode = x_podman.cgroups;
        }

        Ok(())
    }

    fn handle_network(
        &self,
        network_name: &str,
        extensions: &Extensions,
        _context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        let Some(value) = extensions.get(KEY) else {
            return Ok(());
        };

        let x_podman: XPodmanNetwork =
            serde_yaml::from_value(value.clone()).wrap_err_with(|| {
                format!("error deserializing `x-podman` for network `{network_name}`")
            })?;

        if let quadlet::Resource::Network(network) = &mut file.resource {
            if let Some(disable_dns) = x_podman.disable_dns {
                network.disable_dns = disable_dns;
            }
        }

        Ok(())
    }

    fn handle_volume(
        &self,
        volume_name: &str,
        extensions: &Extensions,
        _context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        let Some(value) = extensions.get(KEY) else {
            return Ok(());
        };

        let x_podman: XPodmanVolume =
            serde_yaml::from_value(value.clone()).wrap_err_with(|| {
                format!("error deserializing `x-podman` for volume `{volume_name}`")
            })?;

        if let Some(ownership) = x_podman.ownership {
            match (ownership.user, ownership.group) {
                (Some(user), Some(group)) => {
                    if let quadlet::Resource::Volume(volume) = &mut file.resource {
                        volume.user = Some(user.to_string());
                        volume.group = Some(group.to_string());
                    }
                }
                (None, None) => {}
                _ => {
                    bail!(
                        "`x-podman.ownership` for volume `{volume_name}` must specify both \
                         `user` and `group`, or neither"
                    );
                }
            }
        }

        Ok(())
    }
}

/// `x-podman` at service scope.
#[derive(Deserialize)]
struct XPodmanService {
    cgroups: Option<String>,
}

/// `x-podman` at network scope.
#[derive(Deserialize)]
struct XPodmanNetwork {
    #[serde(rename = "disable-dns")]
    disable_dns: Option<bool>,
}

/// `x-podman` at volume scope.
#[derive(Deserialize)]
struct XPodmanVolume {
    ownership: Option<XPodmanOwnership>,
}

/// Ownership options for a volume.
#[derive(Deserialize)]
struct XPodmanOwnership {
    user: Option<u32>,
    group: Option<u32>,
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::{cli::compose::extensions::ExtensionContext, quadlet::Globals};

    fn empty_context() -> ExtensionContext {
        ExtensionContext {
            pods: IndexMap::new(),
        }
    }

    fn make_ext(key: &str, value: serde_yaml::Value) -> Extensions {
        let mut ext = Extensions::new();
        ext.insert(key.parse().expect("valid key"), value);
        ext
    }

    #[test]
    fn x_podman_cgroups_sets_cgroups_mode() {
        let handler = XPodmanHandler;
        let mut ctx = empty_context();
        let mut file = quadlet::File {
            name: "svc".into(),
            unit: Default::default(),
            resource: quadlet::Container {
                image: "img".into(),
                ..Default::default()
            }
            .into(),
            globals: Globals::default(),
            quadlet: Default::default(),
            service: Default::default(),
            install: Default::default(),
        };
        let ext = make_ext(
            "x-podman",
            serde_yaml::from_str("cgroups: enabled").expect("yaml"),
        );
        handler
            .handle_service("svc", &ext, &mut ctx, &mut file)
            .expect("handle");
        assert!(matches!(&file.resource, quadlet::Resource::Container(_)));
        if let quadlet::Resource::Container(c) = &file.resource {
            assert_eq!(c.cgroups_mode.as_deref(), Some("enabled"));
        }
    }

    #[test]
    fn x_podman_disable_dns_sets_disable_dns() {
        let handler = XPodmanHandler;
        let mut ctx = empty_context();
        let mut file = quadlet::File {
            name: "net".into(),
            unit: Default::default(),
            resource: quadlet::Network::default().into(),
            globals: Globals::default(),
            quadlet: Default::default(),
            service: Default::default(),
            install: Default::default(),
        };
        let ext = make_ext(
            "x-podman",
            serde_yaml::from_str("disable-dns: true").expect("yaml"),
        );
        handler
            .handle_network("net", &ext, &mut ctx, &mut file)
            .expect("handle");
        assert!(matches!(&file.resource, quadlet::Resource::Network(_)));
        if let quadlet::Resource::Network(n) = &file.resource {
            assert!(n.disable_dns);
        }
    }

    #[test]
    fn x_podman_ownership_sets_user_and_group() {
        let handler = XPodmanHandler;
        let mut ctx = empty_context();
        let mut file = quadlet::File {
            name: "vol".into(),
            unit: Default::default(),
            resource: quadlet::Volume::default().into(),
            globals: Globals::default(),
            quadlet: Default::default(),
            service: Default::default(),
            install: Default::default(),
        };
        let ext = make_ext(
            "x-podman",
            serde_yaml::from_str("ownership:\n  user: 1000\n  group: 1000").expect("yaml"),
        );
        handler
            .handle_volume("vol", &ext, &mut ctx, &mut file)
            .expect("handle");
        assert!(matches!(&file.resource, quadlet::Resource::Volume(_)));
        if let quadlet::Resource::Volume(v) = &file.resource {
            assert_eq!(v.user.as_deref(), Some("1000"));
            assert_eq!(v.group.as_deref(), Some("1000"));
        }
    }

    #[test]
    fn x_podman_ownership_missing_group_returns_error() {
        let handler = XPodmanHandler;
        let mut ctx = empty_context();
        let mut file = quadlet::File {
            name: "vol".into(),
            unit: Default::default(),
            resource: quadlet::Volume::default().into(),
            globals: Globals::default(),
            quadlet: Default::default(),
            service: Default::default(),
            install: Default::default(),
        };
        let ext = make_ext(
            "x-podman",
            serde_yaml::from_str("ownership:\n  user: 1000").expect("yaml"),
        );
        let result = handler.handle_volume("vol", &ext, &mut ctx, &mut file);
        assert!(result.is_err());
    }
}
