## ADDED Requirements

### Requirement: x-podman.ownership on a volume sets User= and Group= in the volume quadlet
The system SHALL implement `XPodmanHandler::handle_volume` which extracts `x-podman` from the volume's extensions and, if `ownership` is present, sets `volume.user` and `volume.group` on the `quadlet::Volume` inside the file. The values SHALL be serialized as decimal integer strings (e.g., `"506120"`).

The existing `user: Option<String>` and `group: Option<String>` fields on `quadlet::Volume` SHALL be used; no new fields are needed.

#### Scenario: User= and Group= are emitted when x-podman.ownership is set
- **WHEN** a volume has `x-podman.ownership.user: 506120` and `x-podman.ownership.group: 506120`
- **THEN** the generated `.volume` file contains `User=506120` and `Group=506120` in the `[Volume]` section

#### Scenario: Partial ownership (user only) is not valid per schema
- **WHEN** `x-podman.ownership` is present but missing `group`
- **THEN** `handle_volume` returns an error indicating `group` is required

---

### Requirement: x-systemd on a volume populates [Unit] Requires= and After=
The system SHALL implement `XSystemdHandler::handle_volume` which extracts `x-systemd` from the volume's extensions and appends its `requires` and `after` lists to the `unit.requires` and `unit.after` fields of the `quadlet::File`.

If the file has no `[Unit]` section yet, one SHALL be created.

#### Scenario: Requires= and After= are emitted from x-systemd on a volume
- **WHEN** a volume has `x-systemd.requires: [local-fs.target, observability-landing-network.service]` and matching `after`
- **THEN** the generated `.volume` file contains all entries under `Requires=` and `After=` in its `[Unit]` section

#### Scenario: Volume with no x-systemd has no [Unit] section
- **WHEN** a volume has no `x-systemd` key (and no other source of unit dependencies)
- **THEN** the generated `.volume` file has no `[Unit]` section
