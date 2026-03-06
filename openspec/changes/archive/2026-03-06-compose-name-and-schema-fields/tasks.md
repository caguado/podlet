## 1. network-name: `NetworkName=` support

- [x] 1.1 Add `network_name: Option<String>` field to `quadlet::Network` struct in `src/quadlet/network.rs` with `#[serde(skip_serializing_if = "Option::is_none")]`
- [x] 1.2 Remove `("name", name.is_none())` from `unsupported_options` in `TryFrom<compose_spec::Network>` and instead assign `network_name: name` in the constructed `Network`
- [x] 1.3 Add unit test: compose network with `name` field generates `NetworkName=` in output
- [x] 1.4 Add unit test: compose network without `name` field does not emit `NetworkName=`

## 2. volume-name: `VolumeName=` support

- [x] 2.1 Add `volume_name: Option<String>` field to `quadlet::Volume` struct in `src/quadlet/volume.rs` with `#[serde(skip_serializing_if = "Option::is_none")]`
- [x] 2.2 Remove `ensure!(name.is_none(), ...)` from `TryFrom<compose_spec::Volume>` and instead assign `volume_name: name` in the constructed `Volume`
- [x] 2.3 Add unit test: compose volume with `name` field generates `VolumeName=` in output
- [x] 2.4 Add unit test: compose volume without `name` field does not emit `VolumeName=`
- [x] 2.5 Add unit test: container referencing a named volume still uses the compose-key `.volume` reference

## 3. compose-project-description: auto-populate `Description=`

- [x] 3.1 Thread the top-level compose `name` into `parts_try_into_files` as a new `compose_name: Option<String>` parameter in `src/cli/compose.rs`
- [x] 3.2 Implement helper `fn make_description(entity_type: &str, name: &str) -> String` returning `"<entity_type> for pod <name>"`
- [x] 3.3 In `service_try_into_quadlet_file`, when `compose_name` is `Some` and `unit.description` is `None`, set `description` on the unit to `make_description("container", compose_name)`
- [x] 3.4 In `networks_try_into_quadlet_files`, when `compose_name` is `Some` and the file's unit description is absent, set it using `make_description("network", compose_name)`
- [x] 3.5 In `volumes_try_into_quadlet_files`, when `compose_name` is `Some` and the file's unit description is absent, set it using `make_description("volume", compose_name)`
- [x] 3.6 Ensure explicit `--description` from the caller is never overridden (check `unit.description.is_none()` before setting)
- [x] 3.7 Add unit test: compose file with top-level `name` and no `--description` produces `Description=container for pod <name>` on each container file
- [x] 3.8 Add unit test: explicit `--description` takes precedence over auto-populated value
- [x] 3.9 Add unit test: compose file without top-level `name` produces no `Description=`

## 4. network-attachable: `attachable` support

- [x] 4.1 Add `push_flag(&str)` helper method to `quadlet::Network` in `src/quadlet/network.rs` that appends `--<flag>` (no value) to `PodmanArgs=`
- [x] 4.2 Remove `("attachable", !attachable)` from `unsupported_options` in `TryFrom<compose_spec::Network>`
- [x] 4.3 After constructing the `Network`, if `attachable` is `true`, call `network.push_flag("attachable")`
- [x] 4.4 Add unit test: compose network with `attachable: true` produces `PodmanArgs=--attachable`
- [x] 4.5 Add unit test: compose network with `attachable: false` produces no `--attachable` in output

## 5. service-domain-name: `domain_name` support

- [x] 5.1 Move `domain_name` field from `Unsupported` to `PodmanArgs` in `src/cli/container/compose.rs`
- [x] 5.2 Add `domain_name: Option<Hostname>` to the `PodmanArgs` struct in `src/cli/container/compose.rs`
- [x] 5.3 In the `TryFrom<compose::PodmanArgs>` (or equivalent builder) in `src/cli/container/podman.rs`, render `domain_name` as `--domainname=<value>` appended to the PodmanArgs string
- [x] 5.4 Remove `domain_name` from `Unsupported::ensure_empty` checks
- [x] 5.5 Add unit test: compose service with `domain_name: example.local` produces `--domainname=example.local` in `PodmanArgs=`
- [x] 5.6 Add unit test: compose service without `domain_name` produces no `--domainname` flag

## 6. Verification

- [x] 6.1 Run the full test suite (`cargo test`) and confirm all existing tests pass
- [x] 6.2 Manually verify a compose file exercising all five features round-trips correctly
- [x] 6.3 Update docs if applicable (do not modify CHANGELOG)
