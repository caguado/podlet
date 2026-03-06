use std::net::Ipv4Addr;

use color_eyre::eyre::{Result, WrapErr};
use indexmap::IndexMap;
use serde::Deserialize;

use crate::quadlet::{self, Unit, pod::PodNetworkOptions};

use super::{
    ComposeExtensionHandler, ExtensionContext, ResolvedPod,
    x_systemd::{XSystemdMap, apply_unit_directives},
};

/// Extension key processed by this handler.
const KEY: &str = "x-pods";

/// Handler for the top-level `x-pods` extension.
///
/// Parses each pod definition and populates [`ExtensionContext::pods`].
pub struct XPodsHandler;

impl ComposeExtensionHandler for XPodsHandler {
    fn handled_keys(&self) -> &[&'static str] {
        &[KEY]
    }

    fn build_context(
        &self,
        compose: &compose_spec::Compose,
        context: &mut ExtensionContext,
    ) -> Result<()> {
        let Some(value) = compose.extensions.get(KEY) else {
            return Ok(());
        };

        let pods: IndexMap<String, PodDefinition> =
            serde_yaml::from_value(value.clone()).wrap_err("error deserializing `x-pods`")?;

        for (name, definition) in pods {
            let mut pod = quadlet::Pod {
                pod_name: Some(name.clone()),
                ..Default::default()
            };

            // Populate network attachments.
            for (net_name, attachment) in definition.networks {
                pod.network_attachments.insert(
                    net_name,
                    PodNetworkOptions {
                        ip: attachment.ipv4_address,
                    },
                );
            }

            // Apply x-podman options (userns).
            if let Some(x_podman) = definition.x_podman {
                pod.user_ns = x_podman.userns;
            }

            // Build [Unit] section and extract Install.WantedBy from x-systemd.
            let (unit, wanted_by) = if let Some(x_systemd) = definition.x_systemd {
                let mut unit = Unit::default();
                apply_unit_directives(&x_systemd, &mut unit);
                let unit = (!unit.is_empty()).then_some(unit);

                let wanted_by = x_systemd
                    .get("Install")
                    .and_then(|s| s.get("WantedBy"))
                    .map(|v| v.clone().into_values())
                    .unwrap_or_default();

                (unit, wanted_by)
            } else {
                (None, Vec::new())
            };

            context.pods.insert(
                name.clone(),
                ResolvedPod {
                    name,
                    pod,
                    unit,
                    wanted_by,
                },
            );
        }

        Ok(())
    }

    fn compose_files(
        &self,
        context: &ExtensionContext,
        install: Option<&quadlet::Install>,
    ) -> Result<Vec<quadlet::File>> {
        let mut files = Vec::new();

        for resolved in context.pods.values() {
            let mut file_install = install.cloned().unwrap_or_default();
            for target in &resolved.wanted_by {
                if !file_install.wanted_by.contains(target) {
                    file_install.wanted_by.push(target.clone());
                }
            }

            files.push(quadlet::File {
                name: resolved.name.clone(),
                unit: resolved.unit.clone().unwrap_or_default(),
                resource: resolved.pod.clone().into(),
                globals: quadlet::Globals::default(),
                quadlet: quadlet::Quadlet::default(),
                service: quadlet::Service::default(),
                install: file_install,
            });
        }

        Ok(files)
    }
}

/// Private deserialization struct mirroring the pod definition schema.
#[derive(Deserialize, Default)]
struct PodDefinition {
    #[serde(default)]
    networks: IndexMap<String, PodNetworkAttachment>,
    #[serde(rename = "x-podman", default)]
    x_podman: Option<XPodmanOnPod>,
    #[serde(rename = "x-systemd", default)]
    x_systemd: Option<XSystemdMap>,
}

/// Network attachment with optional static IP.
#[derive(Deserialize, Default)]
struct PodNetworkAttachment {
    ipv4_address: Option<Ipv4Addr>,
}

/// Pod-level `x-podman` options.
#[derive(Deserialize)]
struct XPodmanOnPod {
    userns: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::compose::extensions::ExtensionRegistry;

    fn make_compose_with_xpods(yaml: &str) -> compose_spec::Compose {
        let mut opts = compose_spec::Compose::options();
        opts.apply_merge(true);
        opts.from_yaml_str(yaml).expect("valid compose yaml")
    }

    #[test]
    fn x_pods_handler_build_context_full() {
        let yaml = r#"
services: {}
x-pods:
  mypod:
    networks:
      mynet:
        ipv4_address: 10.0.0.5
    x-podman:
      userns: "auto:uidmapping=0:1000:1024"
    x-systemd:
      Unit:
        Requires: [local-fs.target]
        After: [local-fs.target]
      Install:
        WantedBy: [default.target]
"#;
        let compose = make_compose_with_xpods(yaml);
        let registry = ExtensionRegistry::default();
        let context = registry.build_context(&compose).expect("build_context");

        let pod = context.pods.get("mypod").expect("pod in context");
        assert_eq!(pod.name, "mypod");
        assert_eq!(pod.pod.pod_name.as_deref(), Some("mypod"));
        assert!(
            pod.pod
                .network_attachments
                .get("mynet")
                .is_some_and(|opts| opts.ip == Some("10.0.0.5".parse().expect("Valid IP address")))
        );
        assert_eq!(
            pod.pod.user_ns.as_deref(),
            Some("auto:uidmapping=0:1000:1024")
        );
        assert!(pod.unit.is_some());
        assert_eq!(pod.wanted_by, ["default.target"]);
    }

    #[test]
    fn x_pods_handler_compose_files_generates_pod_quadlet() {
        let yaml = "
services: {}
x-pods:
  testpod:
    x-systemd:
      Install:
        WantedBy: [multi-user.target]
";
        let compose = make_compose_with_xpods(yaml);
        let registry = ExtensionRegistry::default();
        let context = registry.build_context(&compose).expect("build_context");
        let files = registry
            .compose_files(&context, None)
            .expect("compose_files");
        assert_eq!(files.len(), 1);
        let file = files.first().expect("Missing compose_files in registry");
        assert_eq!(file.name, "testpod");
        assert!(matches!(file.resource, quadlet::Resource::Pod(_)));
        assert!(
            file.install
                .wanted_by
                .contains(&"multi-user.target".to_owned())
        );
    }
}
