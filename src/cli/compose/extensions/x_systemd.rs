use color_eyre::eyre::{Result, WrapErr};
use compose_spec::Extensions;
use serde::Deserialize;

use crate::quadlet;

use super::{ComposeExtensionHandler, ExtensionContext};

/// Extension key processed by this handler.
const KEY: &str = "x-systemd";

/// Handler for the `x-systemd` extension at network and volume scope.
///
/// Appends `Requires=` and `After=` entries to the `[Unit]` section of the generated quadlet file.
pub struct XSystemdHandler;

impl ComposeExtensionHandler for XSystemdHandler {
    fn handled_keys(&self) -> &[&'static str] {
        &[KEY]
    }

    fn handle_network(
        &self,
        network_name: &str,
        extensions: &Extensions,
        _context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        apply_systemd_deps(KEY, network_name, "network", extensions, file)
    }

    fn handle_volume(
        &self,
        volume_name: &str,
        extensions: &Extensions,
        _context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        apply_systemd_deps(KEY, volume_name, "volume", extensions, file)
    }
}

/// Deserialize `x-systemd` and append its `requires`/`after` to the file's unit section.
fn apply_systemd_deps(
    key: &str,
    resource_name: &str,
    resource_kind: &str,
    extensions: &Extensions,
    file: &mut quadlet::File,
) -> Result<()> {
    let Some(value) = extensions.get(key) else {
        return Ok(());
    };

    let x_systemd: XSystemdBase = serde_yaml::from_value(value.clone()).wrap_err_with(|| {
        format!("error deserializing `x-systemd` for {resource_kind} `{resource_name}`")
    })?;

    if x_systemd.requires.is_empty() && x_systemd.after.is_empty() {
        return Ok(());
    }

    let unit = &mut file.unit;
    for req in x_systemd.requires {
        unit.add_requires(req);
    }
    for after in x_systemd.after {
        unit.add_after(after);
    }

    Ok(())
}

/// Base `x-systemd` structure for network and volume scope.
#[derive(Deserialize, Default)]
struct XSystemdBase {
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    after: Vec<String>,
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

    fn make_ext(requires: &[&str], after: &[&str]) -> Extensions {
        let requires_list: Vec<_> = requires
            .iter()
            .map(|s| serde_yaml::Value::String((*s).to_owned()))
            .collect();
        let after_list: Vec<_> = after
            .iter()
            .map(|s| serde_yaml::Value::String((*s).to_owned()))
            .collect();
        let mut map = serde_yaml::Mapping::new();
        if !requires.is_empty() {
            map.insert(
                serde_yaml::Value::String("requires".into()),
                serde_yaml::Value::Sequence(requires_list),
            );
        }
        if !after.is_empty() {
            map.insert(
                serde_yaml::Value::String("after".into()),
                serde_yaml::Value::Sequence(after_list),
            );
        }
        let mut ext = Extensions::new();
        ext.insert(
            "x-systemd".parse().expect("valid key"),
            serde_yaml::Value::Mapping(map),
        );
        ext
    }

    #[test]
    fn x_systemd_network_adds_requires_and_after() {
        let handler = XSystemdHandler;
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
        let ext = make_ext(&["openvswitch.service"], &["openvswitch.service"]);
        handler
            .handle_network("net", &ext, &mut ctx, &mut file)
            .expect("handle");
        let unit = file.unit.clone();
        // Serialize to verify requires/after are present.
        let output = crate::serde::quadlet::to_string_join_all(&unit).expect("serialize");
        assert!(output.contains("Requires=openvswitch.service"), "{output}");
        assert!(output.contains("After=openvswitch.service"), "{output}");
    }

    #[test]
    fn x_systemd_volume_adds_requires_and_after() {
        let handler = XSystemdHandler;
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
        let ext = make_ext(&["local-fs.target"], &["local-fs.target"]);
        handler
            .handle_volume("vol", &ext, &mut ctx, &mut file)
            .expect("handle");
        let unit = file.unit.clone();
        let output = crate::serde::quadlet::to_string_join_all(&unit).expect("serialize");
        assert!(output.contains("Requires=local-fs.target"), "{output}");
        assert!(output.contains("After=local-fs.target"), "{output}");
    }

    #[test]
    fn x_systemd_creates_unit_section_when_absent() {
        let handler = XSystemdHandler;
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
        assert!(file.unit.is_empty(), "unit should start empty");
        let ext = make_ext(&["something.service"], &[]);
        handler
            .handle_network("net", &ext, &mut ctx, &mut file)
            .expect("handle");
        assert!(
            !file.unit.is_empty(),
            "unit section should have been populated"
        );
    }
}
