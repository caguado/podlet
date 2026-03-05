use std::mem;

use color_eyre::eyre::{Result, WrapErr};
use compose_spec::Extensions;
use serde::Deserialize;

use crate::quadlet;

use super::{ComposeExtensionHandler, ExtensionContext};

/// Extension key processed by this handler.
const KEY: &str = "x-pod";

/// Handler for the per-service `x-pod` extension.
///
/// Assigns containers to pods defined in `x-pods`, sets `Pod=`, prefixes the file name, propagates
/// `WantedBy=`, and moves published ports to the pod.
pub struct XPodServiceHandler;

impl ComposeExtensionHandler for XPodServiceHandler {
    fn handled_keys(&self) -> &[&'static str] {
        &[KEY]
    }

    fn handle_service(
        &self,
        service_name: &str,
        extensions: &Extensions,
        context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        let Some(value) = extensions.get(KEY) else {
            return Ok(());
        };

        let x_pod: XPodService = serde_yaml::from_value(value.clone()).wrap_err_with(|| {
            format!("error deserializing `x-pod` for service `{service_name}`")
        })?;

        let pod_name = &x_pod.name;

        let resolved = context.pods.get_mut(pod_name).ok_or_else(|| {
            color_eyre::eyre::eyre!(
                "service `{service_name}` references pod `{pod_name}` via `x-pod` \
                 but `{pod_name}` is not defined in `x-pods`"
            )
        })?;

        // Set Pod= on the container.
        if let quadlet::Resource::Container(container) = &mut file.resource {
            container.pod = Some(format!("{pod_name}.pod"));

            // Move published ports from container to pod.
            let ports = mem::take(&mut container.publish_port);
            resolved.pod.publish_port.extend(ports);
        }

        // Prefix the file name with the pod name.
        file.name = format!("{pod_name}-{service_name}");

        // Propagate WantedBy= from the pod definition.
        for target in &resolved.wanted_by {
            if !file.install.wanted_by.contains(target) {
                file.install.wanted_by.push(target.clone());
            }
        }

        Ok(())
    }
}

/// Deserialization struct for the `x-pod` service extension.
#[derive(Deserialize)]
struct XPodService {
    name: String,
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::*;
    use crate::{
        cli::compose::extensions::ResolvedPod,
        quadlet::{Globals, Pod},
    };

    fn make_file(service_name: &str) -> quadlet::File {
        quadlet::File {
            name: service_name.to_owned(),
            unit: Default::default(),
            resource: quadlet::Container {
                image: "img".into(),
                publish_port: vec!["8080:80".to_owned()],
                ..Default::default()
            }
            .into(),
            globals: Globals::default(),
            quadlet: Default::default(),
            service: Default::default(),
            install: Default::default(),
        }
    }

    fn make_context(pod_name: &str, wanted_by: Vec<String>) -> ExtensionContext {
        let mut pods = IndexMap::new();
        pods.insert(
            pod_name.to_owned(),
            ResolvedPod {
                name: pod_name.to_owned(),
                pod: Pod {
                    pod_name: Some(pod_name.to_owned()),
                    ..Default::default()
                },
                unit: None,
                wanted_by,
            },
        );
        ExtensionContext { pods }
    }

    fn make_extensions(pod_name: &str) -> Extensions {
        let mut ext = Extensions::new();
        ext.insert(
            "x-pod".parse().expect("valid"),
            serde_yaml::to_value(serde_yaml::Mapping::from_iter([(
                serde_yaml::Value::String("name".into()),
                serde_yaml::Value::String(pod_name.to_owned()),
            )]))
            .expect("value"),
        );
        ext
    }

    #[test]
    fn x_pod_service_assigns_pod_and_prefixes_name() {
        let handler = XPodServiceHandler;
        let mut context = make_context("mypod", Vec::new());
        let mut file = make_file("myservice");
        let extensions = make_extensions("mypod");

        handler
            .handle_service("myservice", &extensions, &mut context, &mut file)
            .expect("handle_service");

        assert_eq!(file.name, "mypod-myservice");
        assert!(matches!(&file.resource, quadlet::Resource::Container(_)));
        if let quadlet::Resource::Container(c) = &file.resource {
            assert_eq!(c.pod.as_deref(), Some("mypod.pod"));
            assert!(c.publish_port.is_empty(), "ports should have been moved");
        }
        // Port was moved to pod.
        assert_eq!(
            context
                .pods
                .get("mypod")
                .expect("Pod section has named pod")
                .pod
                .publish_port,
            ["8080:80"]
        );
    }

    #[test]
    fn x_pod_service_propagates_wanted_by() {
        let handler = XPodServiceHandler;
        let mut context = make_context("mypod", vec!["default.target".to_owned()]);
        let mut file = make_file("svc");
        let extensions = make_extensions("mypod");

        handler
            .handle_service("svc", &extensions, &mut context, &mut file)
            .expect("handle_service");

        assert!(
            file.install
                .wanted_by
                .contains(&"default.target".to_owned())
        );
    }

    #[test]
    fn x_pod_service_unknown_pod_returns_error() {
        let handler = XPodServiceHandler;
        let mut context = make_context("otherpod", Vec::new());
        let mut file = make_file("svc");
        let extensions = make_extensions("nonexistent");

        let result = handler.handle_service("svc", &extensions, &mut context, &mut file);
        assert!(result.is_err());
    }
}
