## ADDED Requirements

### Requirement: quadlet::Pod gains UserNS= and structured network attachment fields
The system SHALL add the following to `quadlet::Pod`:
- `pub user_ns: Option<String>` with `#[serde(rename = "UserNS", skip_serializing_if = "Option::is_none")]`.
- `pub network_attachments: IndexMap<String, PodNetworkOptions>` serialized via a custom `serialize_with` function that emits one `Network=<name>.network:ip=<addr>` entry per map entry (or `Network=<name>.network` if ip is `None`). The field is skipped when the map is empty.

`PodNetworkOptions` SHALL be defined as:
```rust
pub struct PodNetworkOptions {
    pub ip: Option<Ipv4Addr>,
}
```

The existing `network: Vec<String>` field SHALL be retained for raw mode strings from the CLI.

#### Scenario: UserNS= is emitted when set
- **WHEN** `quadlet::Pod.user_ns` is `Some("auto:uidmapping=0:505120:1024,gidmapping=0:505120:1024")`
- **THEN** the serialized pod file contains `UserNS=auto:uidmapping=0:505120:1024,gidmapping=0:505120:1024`

#### Scenario: Network= with IP suffix is emitted for named network attachments
- **WHEN** `quadlet::Pod.network_attachments` contains `("observability-landing", PodNetworkOptions { ip: Some("100.64.49.10") })`
- **THEN** the serialized pod file contains `Network=observability-landing.network:ip=100.64.49.10`

#### Scenario: Network= without IP is emitted when ip is None
- **WHEN** `network_attachments` contains `("my-net", PodNetworkOptions { ip: None })`
- **THEN** the serialized pod file contains `Network=my-net.network`

#### Scenario: Existing raw network Vec<String> still works
- **WHEN** `quadlet::Pod.network` contains `["host"]` and `network_attachments` is empty
- **THEN** the serialized pod file contains `Network=host` with no regression

---

### Requirement: XPodsHandler generates a .pod quadlet file per pod definition
The system SHALL implement `XPodsHandler::compose_files` which, for each `ResolvedPod` in `ExtensionContext::pods`, produces a `quadlet::File` with:
- `name`: the pod name.
- `resource`: `quadlet::Resource::Pod(...)` with `pod_name = Some(pod_name)`, `user_ns` set from `ResolvedPod.user_ns`, `network_attachments` built from `ResolvedPod.networks`.
- `unit`: `ResolvedPod.unit` — populated by `apply_unit_directives` from the `Unit` section of `x-systemd` — set to `Some(Unit { ... })` if any Unit directives were present.
- `install`: `Some(Install { wanted_by: resolved_pod.systemd_wanted_by, ... })` if `wanted_by` is non-empty.

#### Scenario: Full pod quadlet is generated from x-pods definition
- **WHEN** `x-pods` defines an `observability` pod with networks, userns, and systemd fields
- **THEN** the generated `observability.pod` file matches the expected quadlet content (PodName=, Network=, UserNS=, [Unit] Requires=/After=, [Install] WantedBy=)

#### Scenario: Pod with only a name and no options generates a minimal quadlet
- **WHEN** a pod definition has no `x-podman`, no `x-systemd`, and no `networks`
- **THEN** the `.pod` file is generated with only `PodName=<name>` and no [Unit] or [Install] sections
