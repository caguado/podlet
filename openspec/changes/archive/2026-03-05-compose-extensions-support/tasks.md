## 1. Quadlet Struct Extensions

- [x] 1.1 Add `cgroups_mode: Option<String>` with `#[serde(rename = "CgroupsMode", skip_serializing_if = "Option::is_none")]` to `quadlet::Container` in `src/quadlet/container.rs`
- [x] 1.2 Add `user_ns: Option<String>` with `#[serde(rename = "UserNS", skip_serializing_if = "Option::is_none")]` to `quadlet::Pod` in `src/quadlet/pod.rs`
- [x] 1.3 Define `PodNetworkOptions { ip: Option<Ipv4Addr> }` in `src/quadlet/pod.rs`
- [x] 1.4 Add `network_attachments: IndexMap<String, PodNetworkOptions>` to `quadlet::Pod` with a `serialize_with` function that emits `Network=<name>.network[:ip=<addr>]` entries; skip when empty
- [x] 1.5 Add unit test: `pod_network_attachments_serializes_with_ip` — verify `Network=obs.network:ip=10.0.0.1`
- [x] 1.6 Add unit test: `pod_network_attachments_serializes_without_ip` — verify `Network=obs.network`
- [x] 1.7 Add unit test: `pod_user_ns_serializes` — verify `UserNS=auto:...`
- [x] 1.8 Add unit test: `container_cgroups_mode_serializes` — verify `CgroupsMode=enabled`

## 2. Extension Trait and Registry Infrastructure

- [x] 2.1 Create `src/cli/compose/extensions.rs` with `ExtensionContext`, `ResolvedPod`, `ComposeExtensionHandler` trait, and `ExtensionRegistry` struct
- [x] 2.2 Implement `ComposeExtensionHandler` trait with all methods having default no-op implementations (only `handled_keys` has no default)
- [x] 2.3 Implement `ExtensionRegistry::build_context` — calls `handler.build_context` for each registered handler
- [x] 2.4 Implement `ExtensionRegistry::compose_files` — collects and flattens `compose_files` from all handlers
- [x] 2.5 Implement `ExtensionRegistry::apply_service`, `apply_network`, `apply_volume` — dispatch to all handlers in order
- [x] 2.6 Implement `ExtensionRegistry::warn_unknown` — warns to stderr for keys not in any handler's `handled_keys()`
- [x] 2.7 Add `--disable-extension <KEY>` (repeatable) to `Compose` args in `src/cli/compose.rs`; pass the disabled-key set to the registry at construction time so matching handlers skip their methods
- [x] 2.8 Add unit test: registry with two handlers both modify the same file scope — verify both are called in order
- [x] 2.9 Add unit test: `warn_unknown` emits warning for unrecognized key

## 3. XPodsHandler

- [x] 3.1 Create `src/cli/compose/extensions/x_pods.rs` with private serde structs (`PodDefinition`, `PodNetworkAttachment`, `XPodmanOnPod`, `XSystemdOnPod`) mirroring the JSON schema
- [x] 3.2 Implement `XPodsHandler::build_context` — deserialize `x-pods` from compose extensions via `serde_yaml::from_value`, populate `ExtensionContext::pods`
- [x] 3.3 Implement `XPodsHandler::compose_files` — for each `ResolvedPod`, build a `quadlet::File` with `Pod` resource, unit, and install sections
- [x] 3.4 Add unit test: `x_pods_handler_build_context_full` — full pod definition with all fields
- [x] 3.5 Add unit test: `x_pods_handler_compose_files_generates_pod_quadlet` — verify all pod quadlet fields

## 4. XPodServiceHandler

- [x] 4.1 Create `src/cli/compose/extensions/x_pod_service.rs` with private serde struct `XPodService { name: String }`
- [x] 4.2 Implement `XPodServiceHandler::handle_service` — extract `x-pod`, look up pod in context, set `Pod=`, prefix file name, propagate `WantedBy=`, move ports to pod
- [x] 4.3 Return error when `x-pod.name` references a pod not in `ExtensionContext::pods`
- [x] 4.4 Add unit test: `x_pod_service_assigns_pod_and_prefixes_name`
- [x] 4.5 Add unit test: `x_pod_service_propagates_wanted_by`
- [x] 4.6 Add unit test: `x_pod_service_unknown_pod_returns_error`

## 5. XPodmanHandler

- [x] 5.1 Create `src/cli/compose/extensions/x_podman.rs` with private serde structs for service, network, and volume scopes
- [x] 5.2 Implement `XPodmanHandler::handle_service` — extract `x-podman.cgroups`, set `container.cgroups_mode`
- [x] 5.3 Implement `XPodmanHandler::handle_network` — extract `x-podman.disable-dns`, set `network.disable_dns`
- [x] 5.4 Implement `XPodmanHandler::handle_volume` — extract `x-podman.ownership`, set `volume.user` and `volume.group`; return error if only one of user/group is present
- [x] 5.5 Add unit test: `x_podman_cgroups_sets_cgroups_mode`
- [x] 5.6 Add unit test: `x_podman_disable_dns_sets_disable_dns`
- [x] 5.7 Add unit test: `x_podman_ownership_sets_user_and_group`
- [x] 5.8 Add unit test: `x_podman_ownership_missing_group_returns_error`

## 6. XSystemdHandler

- [x] 6.1 Create `src/cli/compose/extensions/x_systemd.rs` with private serde struct `XSystemdBase { requires: Vec<String>, after: Vec<String> }`
- [x] 6.2 Implement `XSystemdHandler::handle_network` — append `requires` and `after` to file's unit section; create unit if absent
- [x] 6.3 Implement `XSystemdHandler::handle_volume` — same as network
- [x] 6.4 Add unit test: `x_systemd_network_adds_requires_and_after`
- [x] 6.5 Add unit test: `x_systemd_volume_adds_requires_and_after`
- [x] 6.6 Add unit test: `x_systemd_creates_unit_section_when_absent`

## 7. Compose Conversion Integration

- [x] 7.1 In `src/cli/compose.rs`, add pre-flight check: error if both `--pod` flag and `x-pods` extension are present
- [x] 7.2 Thread `ExtensionRegistry` through `parts_try_into_files`, `services_try_into_quadlet_files`, `networks_try_into_quadlet_files`, `volumes_try_into_quadlet_files`
- [x] 7.3 Replace `ensure!(extensions.is_empty(), "compose extensions are not supported")` in `src/cli/compose.rs` with `registry.warn_unknown("compose", &extensions)`
- [x] 7.4 Replace extension guard in `src/quadlet/network.rs::TryFrom<compose_spec::Network>` — pass extensions through to caller for registry dispatch instead of rejecting them; caller applies `registry.apply_network`
- [x] 7.5 Replace extension guard in `src/quadlet/volume.rs::TryFrom<compose_spec::Volume>` — same pattern as network
- [x] 7.6 Replace extension guard in `src/cli/container/compose.rs` — extract `x-pod` and `x-podman` before conversion, apply via registry after file is created
- [x] 7.7 Replace all remaining `"compose extensions are not supported"` guards in container/quadlet.rs, k8s/service.rs, build.rs, and other locations with `warn_unknown` calls (kube path stays rejected for now per non-goals)
- [x] 7.8 Append pod quadlet files from `registry.compose_files()` to the output in `parts_try_into_files`
- [x] 7.9 Wire `ExtensionRegistry::default()` into `Compose::try_into_files` and pass `--disable-extension` keys from CLI args

## 8. End-to-End Integration Test

- [x] 8.1 Add integration test `compose_extensions_observability` that reads `../podman-compose-spec/examples/compose.yaml` and asserts the five generated quadlet files match the contents of the `.pod`, `.container` (×2), `.network`, and `.volume` files in the same directory
- [x] 8.2 Add integration test `compose_no_extensions_unchanged` — verify a compose file with no `x-*` keys produces identical output before and after this change
- [x] 8.3 Add integration test `compose_disable_extension_x_podman` — verify `--disable-extension x-podman` produces quadlets without `CgroupsMode=` or `DisableDNS=`
