## ADDED Requirements

### Requirement: x-pod service extension assigns the container to a pod
The system SHALL implement `XPodServiceHandler` which, in `handle_service`, extracts `x-pod` from the service's extensions, deserializes it to `XPodService { name: String }`, and:
1. Sets `container.pod = Some(format!("{}.pod", pod_name))`.
2. Prefixes the quadlet file name with `<pod_name>-`.
3. Looks up `pod_name` in `ExtensionContext::pods` and propagates `systemd_wanted_by` to the file's `install.wanted_by`.
4. Removes published ports from the container and registers them on the pod (same as the existing `--pod` behavior).

If `x-pod.name` does not match any key in `ExtensionContext::pods`, the system SHALL return an error.

#### Scenario: Container is correctly assigned to its pod
- **WHEN** service `grafana` has `x-pod.name: observability` and `observability` is defined in `x-pods`
- **THEN** the generated `.container` file contains `Pod=observability.pod` and the file is named `observability-grafana.container`

#### Scenario: WantedBy is propagated from pod definition to container
- **WHEN** the `observability` pod has `x-systemd.wanted-by: [default.target]`
- **THEN** the `grafana` container quadlet has `WantedBy=default.target` in its `[Install]` section

#### Scenario: Unknown pod name in x-pod returns an error
- **WHEN** a service declares `x-pod.name: nonexistent-pod` but no such pod is defined in `x-pods`
- **THEN** `handle_service` returns an error naming the missing pod

---

### Requirement: x-podman.cgroups on a service sets CgroupsMode= in the container quadlet
The system SHALL implement `XPodmanHandler::handle_service` which extracts `x-podman` from the service's extensions and, if `cgroups` is present, sets `container.cgroups_mode = Some(value)` on the `quadlet::Container` inside the file.

The `cgroups_mode` field SHALL be added to `quadlet::Container` as `pub cgroups_mode: Option<String>` with `#[serde(rename = "CgroupsMode", skip_serializing_if = "Option::is_none")]`.

#### Scenario: CgroupsMode is emitted when x-podman.cgroups is set
- **WHEN** a service has `x-podman.cgroups: enabled`
- **THEN** the generated `.container` file contains `CgroupsMode=enabled` in the `[Container]` section

#### Scenario: No CgroupsMode when x-podman is absent
- **WHEN** a service has no `x-podman` key
- **THEN** the generated `.container` file does not contain a `CgroupsMode=` line
