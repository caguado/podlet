## ADDED Requirements

### Requirement: Services without x-pod are unaffected when x-pods is present
When `x-pods` is present in the compose file, services that do NOT declare `x-pod` SHALL be converted to standalone container quadlets without `Pod=` set, preserving current behaviour.

#### Scenario: Service without x-pod is a standalone container
- **WHEN** `x-pods` is present but service `sidecar` has no `x-pod` key
- **THEN** the generated `sidecar.container` file has no `Pod=` line and its name is not prefixed

---

### Requirement: WantedBy in container [Install] comes from the pod definition
Container quadlets assigned to a pod via `x-pod` SHALL inherit their `[Install] WantedBy=` exclusively from the pod's `x-systemd.wanted-by` list. Any `--install`/`--wanted-by` values passed on the CLI SHALL be ignored for pod-member containers when `x-pod` is present.

#### Scenario: WantedBy from pod overrides CLI install options
- **WHEN** `--wanted-by multi-user.target` is passed on CLI but the pod has `x-systemd.wanted-by: [default.target]`
- **THEN** the container quadlet has `WantedBy=default.target`, not `WantedBy=multi-user.target`

---

### Requirement: Published ports are moved from container to pod
When a service assigned to a pod (via `x-pod`) has `ports:` defined, those ports SHALL be moved to the pod's `publish_port` list and removed from the container quadlet, matching the existing `--pod` behaviour.

#### Scenario: Port from service is emitted on pod, not on container
- **WHEN** a service has `ports: ["8080:80"]` and is assigned to a pod via `x-pod`
- **THEN** the `.pod` file contains `PublishPort=8080:80` and the `.container` file has no `PublishPort=` line

---

### Requirement: Compose file with x-pods but no matching x-pod services produces pod quadlets only
If `x-pods` defines pods but no service declares `x-pod`, pod quadlets SHALL still be generated (empty pods). A warning SHALL be emitted for each pod that has no member containers.

#### Scenario: Empty pod generates a quadlet with a warning
- **WHEN** `x-pods` defines a pod `my-pod` but no service has `x-pod.name: my-pod`
- **THEN** `my-pod.pod` is generated and a warning is printed indicating the pod has no member containers
