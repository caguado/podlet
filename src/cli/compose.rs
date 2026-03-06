pub mod extensions;

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, IsTerminal},
    iter, mem,
    path::{Path, PathBuf},
};

use clap::Args;
use color_eyre::{
    Help,
    eyre::{OptionExt, WrapErr, bail, ensure, eyre},
};
use compose_spec::{
    Extensions, Identifier, Network, Networks, Options, Resource, Service, Volumes,
    service::Command,
};
use indexmap::IndexMap;

use crate::quadlet::{self, GenericSections, Globals, container::volume::Source};

use super::{Build, Container, File, GlobalArgs, k8s};

use self::extensions::ExtensionRegistry;

/// Converts a [`Command`] into a [`Vec<String>`], splitting the [`String`](Command::String) variant
/// as a shell would.
///
/// # Errors
///
/// Returns an error if, while splitting the string variant, the command ends while in a quote or
/// has a trailing unescaped '\\'.
pub fn command_try_into_vec(command: Command) -> color_eyre::Result<Vec<String>> {
    match command {
        Command::String(command) => shlex::split(&command)
            .ok_or_else(|| eyre!("invalid command: `{command}`"))
            .suggestion(
                "In the command, make sure quotes are closed properly and there are no \
                    trailing \\. Alternatively, use an array instead of a string.",
            ),
        Command::List(command) => Ok(command),
    }
}

/// [`Args`] for the `podlet compose` subcommand.
#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct Compose {
    /// Create a `.pod` file and link it with each `.container` file.
    ///
    /// The top-level `name` field in the compose file is required when using this option.
    /// It is used for the name of the pod and in the filenames of the created files.
    ///
    /// Each container becomes a part of the pod and is renamed to "{pod}-{container}".
    ///
    /// Published ports are taken from each container and applied to the pod.
    #[arg(long, conflicts_with = "kube")]
    pub pod: bool,

    /// Create a Kubernetes YAML file for a pod instead of separate containers
    ///
    /// A `.kube` file using the generated Kubernetes YAML file is also created.
    ///
    /// The top-level `name` field in the compose file is required when using this option.
    /// It is used for the name of the pod and in the filenames of the created files.
    #[arg(long, conflicts_with = "pod")]
    pub kube: bool,

    /// Disable a compose extension by key (repeatable).
    ///
    /// The extension will be treated as known (no warning) but its handler will be skipped.
    #[arg(long = "disable-extension", value_name = "KEY")]
    pub disable_extension: Vec<String>,

    /// The compose file to convert
    ///
    /// If `-` or not provided and stdin is not a terminal,
    /// the compose file will be read from stdin.
    ///
    /// If not provided, and stdin is a terminal, Podlet will look for (in order)
    /// `compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml`,
    /// `podman-compose.yaml`, and `podman-compose.yml`,
    /// in the current working directory.
    #[allow(clippy::struct_field_names)]
    pub compose_file: Option<PathBuf>,
}

impl Compose {
    /// Attempt to convert the `compose_file` into [`File`]s.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an error:
    ///
    /// - Reading/deserializing the compose file.
    /// - Converting the compose file to Kubernetes YAML.
    /// - Converting the compose file to Quadlet files.
    pub fn try_into_files(self, sections: GenericSections) -> color_eyre::Result<Vec<File>> {
        let Self {
            pod,
            kube,
            disable_extension,
            compose_file,
        } = self;

        let mut options = compose_spec::Compose::options();
        options.apply_merge(true);
        let compose = read_from_file_or_stdin(compose_file.as_deref(), &options)
            .wrap_err("error reading compose file")?;
        compose
            .validate_all()
            .wrap_err("error validating compose file")?;

        if kube {
            let mut k8s_file = k8s::File::try_from(compose)
                .wrap_err("error converting compose file into Kubernetes YAML")?;

            let GenericSections {
                unit,
                quadlet,
                install,
            } = sections;
            let kube =
                quadlet::Kube::new(PathBuf::from(format!("{}-kube.yaml", k8s_file.name)).into());
            let quadlet_file = quadlet::File {
                name: k8s_file.name.clone(),
                unit,
                resource: kube.into(),
                globals: Globals::default(),
                quadlet,
                service: quadlet::Service::default(),
                install,
            };

            k8s_file.name.push_str("-kube");
            Ok(vec![quadlet_file.into(), k8s_file.into()])
        } else {
            let compose_spec::Compose {
                version: _,
                name,
                include,
                services,
                networks,
                volumes,
                configs,
                secrets,
                extensions,
            } = compose;

            // Build the extension registry.
            let disabled_keys: HashSet<String> = disable_extension.into_iter().collect();
            let registry = ExtensionRegistry::new(disabled_keys);

            // Pre-flight check: x-pods and --pod are mutually exclusive.
            let has_x_pods = extensions.contains_key("x-pods");
            ensure!(
                !(pod && has_x_pods),
                "`--pod` and `x-pods` extension cannot be used together; \
                 use one or the other to define pod topology"
            );

            let name: Option<String> = name.map(Into::into);
            let pod_name = pod
                .then(|| {
                    name.clone()
                        .ok_or_eyre("`name` is required when using `--pod`")
                })
                .transpose()?;
            let compose_name = name;

            ensure!(include.is_empty(), "`include` is not supported");
            ensure!(configs.is_empty(), "`configs` is not supported");
            ensure!(
                secrets.values().all(Resource::is_external),
                "only external `secrets` are supported",
            );

            // Warn about unrecognized top-level extensions.
            registry.warn_unknown("compose", &extensions);

            parts_try_into_files(
                services,
                networks,
                volumes,
                extensions,
                pod_name,
                sections,
                compose_name,
                &registry,
            )
            .wrap_err("error converting compose file into Quadlet files")
        }
    }
}

/// Read and deserialize a [`compose_spec::Compose`] from a file at the given [`Path`], stdin, or a
/// list of default files.
///
/// If the path is '-', or stdin is not a terminal, the compose file is deserialized from stdin.
/// If a path is not provided, the files `compose.yaml`, `compose.yml`, `docker-compose.yaml`,
/// `docker-compose.yml`, `podman-compose.yaml`, and `podman-compose.yml` are, in order, looked for
///  in the current directory.
///
/// # Errors
///
/// Returns an error if:
///
/// - There was an error opening the given file.
/// - Stdin was selected and stdin is a terminal.
/// - No path was given and none of the default files could be opened.
/// - There was an error deserializing [`compose_spec::Compose`].
fn read_from_file_or_stdin(
    path: Option<&Path>,
    options: &Options,
) -> color_eyre::Result<compose_spec::Compose> {
    let (compose_file, path) = if let Some(path) = path {
        if path.as_os_str() == "-" {
            return read_from_stdin(options);
        }
        let compose_file = fs::File::open(path)
            .wrap_err("could not open provided compose file")
            .suggestion("make sure you have the proper permissions for the given file")?;
        (compose_file, path)
    } else {
        const FILE_NAMES: [&str; 6] = [
            "compose.yaml",
            "compose.yml",
            "docker-compose.yaml",
            "docker-compose.yml",
            "podman-compose.yaml",
            "podman-compose.yml",
        ];

        if !io::stdin().is_terminal() {
            return read_from_stdin(options);
        }

        let mut result = None;
        for file_name in FILE_NAMES {
            if let Ok(compose_file) = fs::File::open(file_name) {
                result = Some((compose_file, file_name.as_ref()));
                break;
            }
        }

        result.ok_or_eyre(
            "a compose file was not provided and none of \
                `compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml`, \
                `podman-compose.yaml`, or `podman-compose.yml` exist in the current directory or \
                could not be read",
        )?
    };

    options
        .from_yaml_reader(compose_file)
        .wrap_err_with(|| format!("File `{}` is not a valid compose file", path.display()))
}

/// Read and deserialize [`compose_spec::Compose`] from stdin.
///
/// # Errors
///
/// Returns an error if stdin is a terminal or there was an error deserializing.
fn read_from_stdin(options: &Options) -> color_eyre::Result<compose_spec::Compose> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        bail!("cannot read compose from stdin, stdin is a terminal");
    }

    options
        .from_yaml_reader(stdin)
        .wrap_err("data from stdin is not a valid compose file")
}

/// Build a systemd `Description=` string for a compose entity.
fn make_description(entity_type: &str, project_name: &str) -> String {
    format!("{entity_type} for pod {project_name}")
}

/// Attempt to convert [`Service`]s, [`Networks`], and [`Volumes`] into [`File`]s.
///
/// # Errors
///
/// Returns an error if a [`Service`], [`Network`], or [`Volume`](compose_spec::Volume) could not be
/// converted into a [`quadlet::File`].
#[allow(clippy::too_many_arguments)]
fn parts_try_into_files(
    services: IndexMap<Identifier, Service>,
    networks: Networks,
    volumes: Volumes,
    compose_extensions: Extensions,
    pod_name: Option<String>,
    sections: GenericSections,
    compose_name: Option<String>,
    registry: &ExtensionRegistry,
) -> color_eyre::Result<Vec<File>> {
    // Build the extension context from top-level extensions.
    let compose = compose_spec::Compose {
        extensions: compose_extensions,
        ..Default::default()
    };
    let mut context = registry
        .build_context(&compose)
        .wrap_err("error building compose extension context")?;

    // Get a map of volumes to whether the volume has options associated with it for use in
    // converting a service into a Quadlet file. Extra volume options must be specified in a
    // separate Quadlet file which is referenced from the container Quadlet file.
    let volume_has_options = volumes
        .iter()
        .map(|(name, volume)| {
            let has_options = volume
                .as_ref()
                .and_then(Resource::as_compose)
                .is_some_and(|volume| !volume.is_empty());
            (name.clone(), has_options)
        })
        .collect();

    // Process each resource kind separately to avoid overlapping mutable borrows of context.
    let mut pod_ports = Vec::new();
    let mut files: Vec<File> = services_try_into_quadlet_files(
        services,
        &sections,
        &volume_has_options,
        pod_name.as_deref(),
        compose_name.as_deref(),
        &mut pod_ports,
        &mut context,
        registry,
    )
    .map(|r| r.map(Into::into))
    .collect::<Result<_, _>>()?;

    let network_files: Vec<File> = networks_try_into_quadlet_files(
        networks,
        &sections,
        compose_name.as_deref(),
        &mut context,
        registry,
    )
    .map(|r| r.map(Into::into))
    .collect::<Result<_, _>>()?;
    files.extend(network_files);

    let volume_files: Vec<File> = volumes_try_into_quadlet_files(
        volumes,
        &sections,
        compose_name.as_deref(),
        &mut context,
        registry,
    )
    .map(|r| r.map(Into::into))
    .collect::<Result<_, _>>()?;
    files.extend(volume_files);

    if let Some(name) = pod_name {
        let pod = quadlet::Pod {
            publish_port: pod_ports,
            ..quadlet::Pod::default()
        };
        let pod = quadlet::File {
            name,
            unit: sections.unit.clone(),
            resource: pod.into(),
            globals: Globals::default(),
            quadlet: sections.quadlet,
            service: quadlet::Service::default(),
            install: sections.install.clone(),
        };
        files.push(pod.into());
    }

    // Append any extra files from extension handlers (e.g., pod files from x-pods).
    let extra = registry
        .compose_files(&context, Some(&sections.install))
        .wrap_err("error generating extension compose files")?;
    for f in extra {
        files.push(f.into());
    }

    Ok(files)
}

/// Attempt to convert Compose [`Service`]s into [`quadlet::File`]s.
///
/// `volume_has_options` should be a map from volume [`Identifier`]s to whether the volume has any
/// options set. It is used to determine whether to link to a [`quadlet::Volume`] in the created
/// [`quadlet::Container`].
///
/// If `pod_name` is [`Some`] and a service has any published ports, they are taken from the
/// created [`quadlet::Container`] and added to `pod_ports`.
///
/// # Errors
///
/// Returns an error if there was an error [adding](Unit::add_dependency()) a service
/// [`Dependency`](compose_spec::service::Dependency) to the [`Unit`], converting the
/// [`Build`](compose_spec::service::Build) section into a [`quadlet::Build`] file, or converting
/// the [`Service`] into a [`quadlet::Container`] file.
#[allow(clippy::too_many_arguments)]
fn services_try_into_quadlet_files<'a>(
    services: IndexMap<Identifier, Service>,
    sections @ GenericSections {
        unit,
        quadlet,
        install,
    }: &'a GenericSections,
    volume_has_options: &'a HashMap<Identifier, bool>,
    pod_name: Option<&'a str>,
    compose_name: Option<&'a str>,
    pod_ports: &'a mut Vec<String>,
    context: &'a mut extensions::ExtensionContext,
    registry: &'a ExtensionRegistry,
) -> impl Iterator<Item = color_eyre::Result<quadlet::File>> + 'a {
    services.into_iter().flat_map(move |(name, mut service)| {
        if service.image.is_some() && service.build.is_some() {
            return iter::once(Err(eyre!(
                "error converting service `{name}`: `image` and `build` cannot both be set"
            )))
            .chain(None);
        }

        let build = service.build.take().map(|build| {
            let build = Build::try_from(build.into_long()).wrap_err_with(|| {
                format!(
                    "error converting `build` for service `{name}` into a Quadlet `.build` file"
                )
            })?;
            let image = format!("{}.build", build.name()).try_into()?;
            service.image = Some(image);
            Ok(quadlet::File {
                name: build.name().to_owned(),
                unit: unit.clone(),
                resource: build.into(),
                globals: Globals::default(),
                quadlet: *quadlet,
                service: quadlet::Service::default(),
                install: install.clone(),
            })
        });
        if let Some(result @ Err(_)) = build {
            return iter::once(result).chain(None);
        }

        let container = service_try_into_quadlet_file(
            service,
            name,
            sections.clone(),
            volume_has_options,
            pod_name,
            compose_name,
            pod_ports,
            context,
            registry,
        );

        iter::once(container).chain(build)
    })
}

/// Attempt to convert a compose [`Service`] into a [`quadlet::File`].
///
/// `volume_has_options` should be a map from volume [`Identifier`]s to whether the volume has any
/// options set. It is used to determine whether to link to a [`quadlet::Volume`] in the created
/// [`quadlet::Container`].
///
/// If `pod_name` is [`Some`] and the `service` has any published ports, they are taken from the
/// created [`quadlet::Container`] and added to `pod_ports`.
///
/// # Errors
///
/// Returns an error if there was an error [adding](Unit::add_dependency()) a service
/// [`Dependency`](compose_spec::service::Dependency) to the [`Unit`] or converting the [`Service`]
/// into a [`quadlet::Container`].
#[allow(clippy::too_many_arguments)]
fn service_try_into_quadlet_file(
    mut service: Service,
    name: Identifier,
    GenericSections {
        mut unit,
        quadlet,
        install,
    }: GenericSections,
    volume_has_options: &HashMap<Identifier, bool>,
    pod_name: Option<&str>,
    compose_name: Option<&str>,
    pod_ports: &mut Vec<String>,
    context: &mut extensions::ExtensionContext,
    registry: &ExtensionRegistry,
) -> color_eyre::Result<quadlet::File> {
    // Add any service dependencies to the [Unit] section of the Quadlet file.
    let dependencies = mem::take(&mut service.depends_on).into_long();
    if !dependencies.is_empty() {
        for (ident, dependency) in dependencies {
            unit.add_dependency(
                pod_name.map_or_else(
                    || ident.to_string(),
                    |pod_name| format!("{pod_name}-{ident}"),
                ),
                dependency,
            )
            .wrap_err_with(|| {
                format!("error adding dependency on `{ident}` to service `{name}`")
            })?;
        }
    }

    let global_args = GlobalArgs::from_compose(&mut service);

    let restart = service.restart;

    // Extract service extensions before conversion.
    let service_extensions = mem::take(&mut service.extensions);
    // Warn about unrecognized service-level extensions.
    registry.warn_unknown(&format!("service `{name}`"), &service_extensions);

    let mut container = Container::try_from(service)
        .map(quadlet::Container::from)
        .wrap_err_with(|| format!("error converting service `{name}` into a Quadlet container"))?;

    // For each named volume, check to see if it has any options set.
    // If it does, add `.volume` to the source to link this `.container` file to the generated
    // `.volume` file.
    for volume in &mut container.volume {
        if let Some(Source::NamedVolume(source)) = &mut volume.source {
            let volume_has_options = volume_has_options
                .get(source.as_str())
                .copied()
                .unwrap_or_default();
            if volume_has_options {
                source.push_str(".volume");
            }
        }
    }

    let name = if let Some(pod_name) = pod_name {
        container.pod = Some(format!("{pod_name}.pod"));
        pod_ports.extend(mem::take(&mut container.publish_port));
        format!("{pod_name}-{name}")
    } else {
        name.into()
    };

    let mut file = quadlet::File {
        name,
        unit,
        resource: container.into(),
        globals: global_args.into(),
        quadlet,
        service: restart.map(Into::into).unwrap_or_default(),
        install,
    };

    // Auto-populate Description= from the compose project name if not already set.
    if let Some(project) = compose_name {
        file.unit
            .set_description_if_absent(make_description("container", project));
    }

    // Apply service extensions via the registry.
    if !service_extensions.is_empty() {
        registry
            .apply_service(&file.name.clone(), &service_extensions, context, &mut file)
            .wrap_err_with(|| format!("error applying extensions for service `{}`", file.name))?;
    }

    Ok(file)
}

/// Attempt to convert compose [`Networks`] into an [`Iterator`] of [`quadlet::File`]s.
///
/// # Errors
///
/// The [`Iterator`] returns an [`Err`] if a [`Network`] could not be converted into a
/// [`quadlet::Network`].
fn networks_try_into_quadlet_files<'a>(
    networks: Networks,
    GenericSections {
        unit,
        quadlet,
        install,
    }: &'a GenericSections,
    compose_name: Option<&'a str>,
    context: &'a mut extensions::ExtensionContext,
    registry: &'a ExtensionRegistry,
) -> impl Iterator<Item = color_eyre::Result<quadlet::File>> + 'a {
    networks.into_iter().map(move |(name, network)| {
        let network = match network {
            Some(Resource::Compose(network)) => network,
            None => Network::default(),
            Some(Resource::External { .. }) => {
                bail!("external networks (`{name}`) are not supported");
            }
        };

        // Extract network extensions before conversion.
        let net_extensions = network.extensions.clone();
        // Warn about unrecognized network-level extensions.
        registry.warn_unknown(&format!("network `{name}`"), &net_extensions);

        let network = quadlet::Network::try_from(network).wrap_err_with(|| {
            format!("error converting network `{name}` into a Quadlet network")
        })?;

        let file_name = name.as_str().to_owned();
        let mut file = quadlet::File {
            name: file_name.clone(),
            unit: unit.clone(),
            resource: network.into(),
            globals: Globals::default(),
            quadlet: *quadlet,
            service: quadlet::Service::default(),
            install: install.clone(),
        };

        // Auto-populate Description= from the compose project name if not already set.
        if let Some(project) = compose_name {
            file.unit
                .set_description_if_absent(make_description("network", project));
        }

        // Apply network extensions via the registry.
        if !net_extensions.is_empty() {
            registry
                .apply_network(&file_name, &net_extensions, context, &mut file)
                .wrap_err_with(|| format!("error applying extensions for network `{name}`"))?;
        }

        Ok(file)
    })
}

/// Attempt to convert compose [`Volumes`] into an [`Iterator`] of [`quadlet::File`]s.
///
/// [`Volume`](compose_spec::Volume)s which are [empty](compose_spec::Volume::is_empty()) are
/// filtered out as they do not need a `.volume` Quadlet file to define extra options.
///
/// # Errors
///
/// The [`Iterator`] returns an [`Err`] if a [`Volume`](compose_spec::Volume) could not be converted
/// to a [`quadlet::Volume`].
fn volumes_try_into_quadlet_files<'a>(
    volumes: Volumes,
    GenericSections {
        unit,
        quadlet,
        install,
    }: &'a GenericSections,
    compose_name: Option<&'a str>,
    context: &'a mut extensions::ExtensionContext,
    registry: &'a ExtensionRegistry,
) -> impl Iterator<Item = color_eyre::Result<quadlet::File>> + 'a {
    volumes.into_iter().filter_map(move |(name, volume)| {
        volume.and_then(|volume| match volume {
            Resource::Compose(volume) => {
                let vol_extensions = volume.extensions.clone();
                // Warn about unrecognized volume-level extensions.
                registry.warn_unknown(&format!("volume `{name}`"), &vol_extensions);

                // A volume with only extensions is not empty per `is_empty()`, but should not
                // produce a quadlet file if extensions are the only content.
                let has_non_extension_options = {
                    let mut v2 = volume.clone();
                    v2.extensions = IndexMap::default();
                    !v2.is_empty()
                };

                if !has_non_extension_options && vol_extensions.is_empty() {
                    return None;
                }

                Some((|| -> color_eyre::Result<quadlet::File> {
                    let quadlet_volume = quadlet::Volume::try_from(volume).wrap_err_with(|| {
                        format!("error converting volume `{name}` into a Quadlet volume")
                    })?;

                    let file_name = name.as_str().to_owned();
                    let mut file = quadlet::File {
                        name: file_name.clone(),
                        unit: unit.clone(),
                        resource: quadlet_volume.into(),
                        globals: Globals::default(),
                        quadlet: *quadlet,
                        service: quadlet::Service::default(),
                        install: install.clone(),
                    };

                    // Auto-populate Description= from the compose project name if not already set.
                    if let Some(project) = compose_name {
                        file.unit
                            .set_description_if_absent(make_description("volume", project));
                    }

                    // Apply volume extensions via the registry.
                    if !vol_extensions.is_empty() {
                        registry
                            .apply_volume(&file_name, &vol_extensions, context, &mut file)
                            .wrap_err_with(|| {
                                format!("error applying extensions for volume `{name}`")
                            })?;
                    }

                    Ok(file)
                })())
            }
            Resource::External { .. } => {
                Some(Err(eyre!("external volumes (`{name}`) are not supported")))
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadlet::{self, JoinOption};

    /// Convert a compose YAML string and return a map of file name → serialized quadlet text.
    fn convert_compose_yaml(
        yaml: &str,
        compose_args: Compose,
        install: Option<quadlet::Install>,
    ) -> color_eyre::Result<HashMap<String, String>> {
        let mut options = compose_spec::Compose::options();
        options.apply_merge(true);
        let compose = options.from_yaml_str(yaml).wrap_err("parse")?;
        compose.validate_all().wrap_err("validate")?;

        let compose_spec::Compose {
            version: _,
            name,
            include,
            services,
            networks,
            volumes,
            configs,
            secrets,
            extensions,
        } = compose;

        let disabled_keys: HashSet<String> = compose_args.disable_extension.into_iter().collect();
        let registry = ExtensionRegistry::new(disabled_keys);

        let compose_name: Option<String> = name.map(Into::into);
        ensure!(include.is_empty(), "`include` is not supported");
        ensure!(configs.is_empty(), "`configs` is not supported");
        ensure!(
            secrets.values().all(Resource::is_external),
            "only external `secrets` are supported"
        );

        let sections = GenericSections {
            unit: quadlet::Unit::default(),
            quadlet: quadlet::Quadlet::default(),
            install: install.unwrap_or_default(),
        };
        let files = parts_try_into_files(
            services,
            networks,
            volumes,
            extensions,
            None,
            sections,
            compose_name,
            &registry,
        )?;

        let join_opts = JoinOption::all_set();
        let mut result = HashMap::new();
        for file in files {
            if let File::Quadlet(qf) = &file {
                let text = qf.serialize_to_quadlet(&join_opts).wrap_err("serialize")?;
                result.insert(qf.name.clone(), text);
            }
        }
        Ok(result)
    }

    #[test]
    fn compose_extensions_observability() -> color_eyre::Result<()> {
        let yaml = fs::read_to_string("../podman-compose-spec/examples/compose.yaml")
            .expect("podman-compose-spec examples must be present");

        let install = Some(quadlet::Install {
            wanted_by: vec!["default.target".to_owned()],
            required_by: Vec::new(),
        });
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };

        let files = convert_compose_yaml(&yaml, compose_args, install)?;

        // Verify 6 files were generated.
        assert_eq!(
            files.len(),
            6,
            "expected 6 files, got: {:?}",
            files.keys().collect::<Vec<_>>()
        );

        // --- observability.pod ---
        let pod = files.get("observability").expect("observability pod file");
        assert!(pod.contains("PodName=observability"), "{pod}");
        assert!(
            pod.contains("Network=observability-landing.network:ip=100.64.49.10"),
            "{pod}"
        );
        assert!(
            pod.contains("UserNS=auto:uidmapping=0:505120:1024,gidmapping=0:505120:1024"),
            "{pod}"
        );
        assert!(pod.contains("Requires=local-fs.target"), "{pod}");
        assert!(pod.contains("After=local-fs.target"), "{pod}");
        assert!(pod.contains("WantedBy=default.target"), "{pod}");

        // --- observability-grafana.container ---
        let grafana = files
            .get("observability-grafana")
            .expect("grafana container file");
        assert!(grafana.contains("Pod=observability.pod"), "{grafana}");
        assert!(
            grafana.contains("ContainerName=observability-grafana"),
            "{grafana}"
        );
        assert!(grafana.contains("Image=grafana/grafana:12.2"), "{grafana}");
        assert!(grafana.contains("CgroupsMode=enabled"), "{grafana}");
        assert!(grafana.contains("WantedBy=default.target"), "{grafana}");

        // --- observability-prometheus.container ---
        let prometheus = files
            .get("observability-prometheus")
            .expect("prometheus container file");
        assert!(prometheus.contains("Pod=observability.pod"), "{prometheus}");
        assert!(
            prometheus.contains("ContainerName=observability-prometheus"),
            "{prometheus}"
        );
        assert!(
            prometheus.contains("Image=prom/prometheus:v3.7.3"),
            "{prometheus}"
        );
        assert!(prometheus.contains("CgroupsMode=enabled"), "{prometheus}");
        assert!(
            prometheus.contains("WantedBy=default.target"),
            "{prometheus}"
        );
        // prometheus has user/group from compose spec
        assert!(prometheus.contains("User=1000"), "{prometheus}");
        assert!(prometheus.contains("Group=1000"), "{prometheus}");

        // --- observability-landing.network ---
        let network = files
            .get("observability-landing")
            .expect("observability-landing network file");
        assert!(network.contains("DisableDNS=true"), "{network}");
        assert!(network.contains("Driver=pond-netns"), "{network}");
        assert!(network.contains("Internal=true"), "{network}");
        assert!(network.contains("Subnet=100.64.49.0/24"), "{network}");
        assert!(
            network.contains("Requires=openvswitch.service"),
            "{network}"
        );
        assert!(network.contains("After=openvswitch.service"), "{network}");

        // --- observability-prometheus-data.volume ---
        let volume = files
            .get("observability-prometheus-data")
            .expect("observability-prometheus-data volume file");
        assert!(volume.contains("User=506120"), "{volume}");
        assert!(volume.contains("Group=506120"), "{volume}");
        assert!(
            volume.contains("Requires=local-fs.target observability-landing-network.service"),
            "{volume}"
        );
        assert!(
            volume.contains("After=local-fs.target observability-landing-network.service"),
            "{volume}"
        );

        Ok(())
    }

    #[test]
    fn compose_no_extensions_unchanged() -> color_eyre::Result<()> {
        let yaml = r#"
services:
  web:
    image: nginx:latest
    ports:
      - "8080:80"
networks:
  default:
    driver: bridge
"#;
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        // Should produce 1 container file; network with only driver=bridge produces a file.
        assert!(files.contains_key("web"), "web container missing");
        let web = files.get("web").expect("Missing web service definition");
        assert!(web.contains("Image=nginx:latest"), "{web}");
        Ok(())
    }

    #[test]
    fn compose_disable_extension_x_podman() -> color_eyre::Result<()> {
        let yaml = "
services:
  app:
    image: myapp:latest
    x-podman:
      cgroups: enabled
";
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: vec!["x-podman".to_owned()],
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        assert!(files.contains_key("app"), "app container missing");
        let app = files.get("app").expect("Missing app service definition");
        // CgroupsMode should NOT be set since x-podman is disabled.
        assert!(!app.contains("CgroupsMode="), "{app}");
        Ok(())
    }

    #[test]
    fn compose_project_name_auto_populates_description() -> color_eyre::Result<()> {
        let yaml = "
name: myapp
services:
  web:
    image: nginx:latest
networks:
  frontend:
    driver: bridge
";
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        let web = files.get("web").expect("web container missing");
        assert!(
            web.contains("Description=container for pod myapp"),
            "expected description in container: {web}"
        );
        let network = files.get("frontend").expect("frontend network missing");
        assert!(
            network.contains("Description=network for pod myapp"),
            "expected description in network: {network}"
        );
        Ok(())
    }

    #[test]
    fn compose_no_project_name_no_description() -> color_eyre::Result<()> {
        let yaml = "
services:
  web:
    image: nginx:latest
";
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        let web = files.get("web").expect("web container missing");
        assert!(
            !web.contains("Description="),
            "unexpected Description= when no compose name: {web}"
        );
        Ok(())
    }

    #[test]
    fn compose_service_domain_name_emits_podman_args() -> color_eyre::Result<()> {
        let yaml = "
services:
  app:
    image: myapp:latest
    domainname: example.local
";
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        let app = files.get("app").expect("app container missing");
        assert!(
            app.contains("--domainname example.local"),
            "expected --domainname in PodmanArgs: {app}"
        );
        Ok(())
    }

    #[test]
    fn compose_service_without_domain_name_omits_flag() -> color_eyre::Result<()> {
        let yaml = "
services:
  app:
    image: myapp:latest
";
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        let app = files.get("app").expect("app container missing");
        assert!(
            !app.contains("--domainname"),
            "unexpected --domainname in output: {app}"
        );
        Ok(())
    }

    #[test]
    fn compose_volume_name_uses_compose_key_as_file_and_volume_name_in_quadlet()
    -> color_eyre::Result<()> {
        let yaml = "
services:
  app:
    image: myapp:latest
    volumes:
      - data:/data
volumes:
  data:
    name: app-persistent-data
    driver: local
";
        let compose_args = Compose {
            pod: false,
            kube: false,
            disable_extension: Vec::new(),
            compose_file: None,
        };
        let files = convert_compose_yaml(yaml, compose_args, None)?;
        // Container still references volume by compose key
        let app = files.get("app").expect("app container missing");
        assert!(
            app.contains("Volume=data.volume:/data"),
            "container should reference volume by compose key: {app}"
        );
        // Volume file has VolumeName= with the custom name
        let volume = files.get("data").expect("data volume file missing");
        assert!(
            volume.contains("VolumeName=app-persistent-data"),
            "volume file should have VolumeName=: {volume}"
        );
        Ok(())
    }
}
