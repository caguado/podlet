use color_eyre::eyre::{Result, WrapErr};
use compose_spec::Extensions;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::quadlet::{self, Unit};

use super::{ComposeExtensionHandler, ExtensionContext};

/// Extension key processed by this handler.
const KEY: &str = "x-systemd";

/// A systemd directive value: either a scalar string or a sequence of strings.
///
/// When rendered to INI, sequence values are joined with a single space.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub(crate) enum SystemdDirectiveValue {
    Single(String),
    List(Vec<String>),
}

impl SystemdDirectiveValue {
    /// Convert into a list of string values.
    pub(crate) fn into_values(self) -> Vec<String> {
        match self {
            SystemdDirectiveValue::Single(s) => vec![s],
            SystemdDirectiveValue::List(v) => v,
        }
    }
}

/// Map of INI section names to directive maps.
///
/// Top-level keys are section names (e.g. `"Unit"`, `"Install"`).
/// Inner keys are directive names (e.g. `"Requires"`, `"After"`, `"WantedBy"`).
pub(crate) type XSystemdMap = IndexMap<String, IndexMap<String, SystemdDirectiveValue>>;

/// Handler for the `x-systemd` extension at network and volume scope.
///
/// Applies directives from each INI section to the corresponding section of the generated quadlet.
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
        apply_systemd_sections(KEY, network_name, "network", extensions, file)
    }

    fn handle_volume(
        &self,
        volume_name: &str,
        extensions: &Extensions,
        _context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        apply_systemd_sections(KEY, volume_name, "volume", extensions, file)
    }
}

/// Deserialize `x-systemd` and apply its INI section directives to the quadlet file.
fn apply_systemd_sections(
    key: &str,
    resource_name: &str,
    resource_kind: &str,
    extensions: &Extensions,
    file: &mut quadlet::File,
) -> Result<()> {
    let Some(value) = extensions.get(key) else {
        return Ok(());
    };

    let sections: XSystemdMap = serde_yaml::from_value(value.clone()).wrap_err_with(|| {
        format!("error deserializing `x-systemd` for {resource_kind} `{resource_name}`")
    })?;

    apply_unit_directives(&sections, &mut file.unit);
    apply_install_directives(&sections, file);

    Ok(())
}

/// Apply directives from the `Unit` section of an [`XSystemdMap`] to a [`Unit`].
///
/// Recognised directives: `Requires`, `After`, `Wants`, `Before`, `BindsTo`.
/// Unknown directives are silently ignored.
pub(crate) fn apply_unit_directives(sections: &XSystemdMap, unit: &mut Unit) {
    let Some(unit_section) = sections.get("Unit") else {
        return;
    };
    if unit_section.is_empty() {
        return;
    }
    let u = unit;
    for (directive, value) in unit_section {
        let values = value.clone().into_values();
        match directive.as_str() {
            "Requires" => {
                for v in values {
                    u.add_requires(v);
                }
            }
            "After" => {
                for v in values {
                    u.add_after(v);
                }
            }
            "Wants" => {
                for v in values {
                    u.add_wants(v);
                }
            }
            "Before" => {
                for v in values {
                    u.add_before(v);
                }
            }
            "BindsTo" => {
                for v in values {
                    u.add_binds_to(v);
                }
            }
            _ => {} // ignore unknown directives
        }
    }
}

/// Apply directives from the `Install` section of an [`XSystemdMap`] to `file.install`.
///
/// Recognised directives: `WantedBy`, `RequiredBy`.
/// Unknown directives are silently ignored.
fn apply_install_directives(sections: &XSystemdMap, file: &mut quadlet::File) {
    let Some(install_section) = sections.get("Install") else {
        return;
    };
    if install_section.is_empty() {
        return;
    }
    let install = &mut file.install;
    for (directive, value) in install_section {
        let values = value.clone().into_values();
        match directive.as_str() {
            "WantedBy" => {
                for v in values {
                    if !install.wanted_by.contains(&v) {
                        install.wanted_by.push(v);
                    }
                }
            }
            "RequiredBy" => {
                for v in values {
                    if !install.required_by.contains(&v) {
                        install.required_by.push(v);
                    }
                }
            }
            _ => {}
        }
    }
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

    /// Build an Extensions map with an `x-systemd` value using the section-map format.
    fn make_ext(
        unit_directives: &[(&str, &[&str])],
        install_directives: &[(&str, &[&str])],
    ) -> Extensions {
        let mut sections = serde_yaml::Mapping::new();

        if !unit_directives.is_empty() {
            let mut unit_map = serde_yaml::Mapping::new();
            for (directive, values) in unit_directives {
                let seq: Vec<_> = values
                    .iter()
                    .map(|s| serde_yaml::Value::String((*s).to_owned()))
                    .collect();
                unit_map.insert(
                    serde_yaml::Value::String((*directive).to_owned()),
                    serde_yaml::Value::Sequence(seq),
                );
            }
            sections.insert(
                serde_yaml::Value::String("Unit".into()),
                serde_yaml::Value::Mapping(unit_map),
            );
        }

        if !install_directives.is_empty() {
            let mut install_map = serde_yaml::Mapping::new();
            for (directive, values) in install_directives {
                let seq: Vec<_> = values
                    .iter()
                    .map(|s| serde_yaml::Value::String((*s).to_owned()))
                    .collect();
                install_map.insert(
                    serde_yaml::Value::String((*directive).to_owned()),
                    serde_yaml::Value::Sequence(seq),
                );
            }
            sections.insert(
                serde_yaml::Value::String("Install".into()),
                serde_yaml::Value::Mapping(install_map),
            );
        }

        let mut ext = Extensions::new();
        ext.insert(
            "x-systemd".parse().expect("valid key"),
            serde_yaml::Value::Mapping(sections),
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
        let ext = make_ext(
            &[
                ("Requires", &["openvswitch.service"]),
                ("After", &["openvswitch.service"]),
            ],
            &[],
        );
        handler
            .handle_network("net", &ext, &mut ctx, &mut file)
            .expect("handle");
        let unit = file.unit.clone();
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
        let ext = make_ext(
            &[
                ("Requires", &["local-fs.target"]),
                ("After", &["local-fs.target"]),
            ],
            &[],
        );
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
        let ext = make_ext(&[("Requires", &["something.service"])], &[]);
        handler
            .handle_network("net", &ext, &mut ctx, &mut file)
            .expect("handle");
        assert!(
            !file.unit.is_empty(),
            "unit section should have been populated"
        );
    }

    #[test]
    fn x_systemd_install_section_sets_wanted_by() {
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
        let ext = make_ext(&[], &[("WantedBy", &["multi-user.target"])]);
        handler
            .handle_network("net", &ext, &mut ctx, &mut file)
            .expect("handle");
        assert!(
            file.install
                .wanted_by
                .contains(&"multi-user.target".to_owned()),
            "install section should contain WantedBy"
        );
    }

    #[test]
    fn x_systemd_sequence_values_each_pushed() {
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
        let ext = make_ext(
            &[
                ("Requires", &["local-fs.target", "other-network.service"]),
                ("After", &["local-fs.target", "other-network.service"]),
            ],
            &[],
        );
        handler
            .handle_volume("vol", &ext, &mut ctx, &mut file)
            .expect("handle");
        let unit = file.unit.clone();
        let output = crate::serde::quadlet::to_string_join_all(&unit).expect("serialize");
        assert!(
            output.contains("Requires=local-fs.target other-network.service"),
            "{output}"
        );
        assert!(
            output.contains("After=local-fs.target other-network.service"),
            "{output}"
        );
    }
}
