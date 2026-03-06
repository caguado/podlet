## ADDED Requirements

### Requirement: x-pods top-level key is parsed into typed structs
The system SHALL implement `XPodsHandler` which, in `build_context`, extracts the `x-pods` key from the compose top-level extensions and deserializes it via `serde_yaml::from_value` into a map of pod names to `PodDefinition` structs. Each `PodDefinition` contains:
- `networks: IndexMap<String, PodNetworkAttachment>` where `PodNetworkAttachment` has `ipv4_address: Option<Ipv4Addr>`.
- `x_podman: Option<XPodmanOnPod>` with `userns: Option<String>`.
- `x_systemd: Option<XSystemdMap>` — a two-level INI-section map (`IndexMap<String, IndexMap<String, SystemdDirectiveValue>>`).

The `x_systemd` field is processed as follows:
- `Unit.*` directives are applied to `ResolvedPod.unit` via `apply_unit_directives` (shared with `XSystemdHandler`).
- `Install.WantedBy` is extracted separately into `ResolvedPod.wanted_by` and later propagated to every member container's `[Install]` section.

The parsed pod definitions SHALL be stored in `ExtensionContext::pods` as `ResolvedPod` entries.

#### Scenario: Valid x-pods map is parsed without error
- **WHEN** the compose file contains a valid `x-pods` map with one pod definition including `networks`, `x-podman.userns`, and `x-systemd` (using the INI-section map format)
- **THEN** `ExtensionContext::pods` contains one entry with all fields populated

#### Scenario: x-pods with no x-podman or x-systemd is valid
- **WHEN** a pod definition has `networks` but no `x-podman` or `x-systemd` keys
- **THEN** `ResolvedPod.unit` is `None` and `ResolvedPod.wanted_by` is empty

#### Scenario: x-systemd.Unit.Requires and After populate ResolvedPod.unit
- **WHEN** a pod definition has `x-systemd.Unit.Requires: [local-fs.target]` and `x-systemd.Unit.After: [local-fs.target]`
- **THEN** `ResolvedPod.unit` is `Some` and contains `Requires=local-fs.target` and `After=local-fs.target`

#### Scenario: x-systemd.Install.WantedBy is extracted into wanted_by
- **WHEN** a pod definition has `x-systemd.Install.WantedBy: [default.target]`
- **THEN** `ResolvedPod.wanted_by` equals `["default.target"]`

#### Scenario: Malformed x-pods value returns an error
- **WHEN** `x-pods` contains a value that does not match the expected schema (e.g., a string instead of a map)
- **THEN** `build_context` returns an error with a message identifying `x-pods` as the source

---

### Requirement: x-pods and --pod flag cannot both be used
The system SHALL return an error during the `podlet compose` pre-flight check if both `--pod` is passed on the CLI and `x-pods` is present in the compose file's extensions.

#### Scenario: Conflict is detected early
- **WHEN** `podlet compose --pod` is run on a compose file that contains `x-pods`
- **THEN** an error is returned before any quadlet files are generated, with a message explaining the conflict

---

### Requirement: Pod names from x-pods drive service renaming
When `x-pods` is present, services that declare `x-pod.name: <pod-name>` SHALL have their generated container file name prefixed with `<pod-name>-`, consistent with the existing `--pod` behaviour.

#### Scenario: Service is renamed with pod prefix
- **WHEN** a service named `prometheus` declares `x-pod.name: observability`
- **THEN** the generated container quadlet file is named `observability-prometheus.container`
