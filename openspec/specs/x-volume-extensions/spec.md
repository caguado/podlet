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

### Requirement: x-systemd on a volume uses an INI-section map and populates the corresponding quadlet sections
The system SHALL implement `XSystemdHandler::handle_volume` which extracts `x-systemd` from the volume's extensions and deserializes it as an `XSystemdMap` (a two-level map of INI section name → directive name → `SystemdDirectiveValue`). Each directive is applied to the matching section of the generated `quadlet::File`:

- `Unit.*` directives are applied to `file.unit` via `apply_unit_directives`. Recognised keys: `Requires`, `After`, `Wants`, `Before`, `BindsTo`. Unknown keys are silently ignored.
- `Install.*` directives are applied to `file.install` via `apply_install_directives`. Recognised keys: `WantedBy`, `RequiredBy`. Unknown keys are silently ignored.

A `SystemdDirectiveValue` is either a scalar string or a YAML sequence of strings. Sequences are stored as multiple entries in the corresponding `Vec<String>` field and joined by a space when serialized to INI.

If the file has no `[Unit]` section yet, one SHALL be created when any `Unit` directive is present.

#### Scenario: Requires= and After= are emitted from x-systemd on a volume
- **WHEN** a volume has `x-systemd.Unit.Requires: [local-fs.target, observability-landing-network.service]` and matching `After`
- **THEN** the generated `.volume` file contains `Requires=local-fs.target observability-landing-network.service` and `After=local-fs.target observability-landing-network.service` in its `[Unit]` section

#### Scenario: Volume with no x-systemd has no [Unit] section
- **WHEN** a volume has no `x-systemd` key (and no other source of unit dependencies)
- **THEN** the generated `.volume` file has no `[Unit]` section
