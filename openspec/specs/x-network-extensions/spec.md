## ADDED Requirements

### Requirement: x-podman.disable-dns on a network sets DisableDNS= in the network quadlet
The system SHALL implement `XPodmanHandler::handle_network` which extracts `x-podman` from the network's extensions and, if `disable-dns: true` is set, sets `network.disable_dns = true` on the `quadlet::Network` inside the file.

The existing `disable_dns` field on `quadlet::Network` SHALL be used; no new field is needed.

#### Scenario: DisableDNS is emitted when x-podman.disable-dns is true
- **WHEN** a network has `x-podman.disable-dns: true`
- **THEN** the generated `.network` file contains `DisableDNS=true` in the `[Network]` section

#### Scenario: No DisableDNS when x-podman.disable-dns is false or absent
- **WHEN** a network has no `x-podman` key, or `x-podman.disable-dns` is `false`
- **THEN** the generated `.network` file does not contain a `DisableDNS=` line

---

### Requirement: x-systemd on a network uses an INI-section map and populates the corresponding quadlet sections
The system SHALL implement `XSystemdHandler::handle_network` which extracts `x-systemd` from the network's extensions and deserializes it as an `XSystemdMap` (a two-level map of INI section name → directive name → `SystemdDirectiveValue`). Each directive is applied to the matching section of the generated `quadlet::File`:

- `Unit.*` directives are applied to `file.unit` via `apply_unit_directives`. Recognised keys: `Requires`, `After`, `Wants`, `Before`, `BindsTo`. Unknown keys are silently ignored.
- `Install.*` directives are applied to `file.install` via `apply_install_directives`. Recognised keys: `WantedBy`, `RequiredBy`. Unknown keys are silently ignored.

A `SystemdDirectiveValue` is either a scalar string or a YAML sequence of strings. Sequences are stored as multiple entries in the corresponding `Vec<String>` field and joined by a space when serialized to INI.

If the file has no `[Unit]` section yet, one SHALL be created when any `Unit` directive is present.

#### Scenario: Requires= and After= are emitted from x-systemd
- **WHEN** a network has `x-systemd.Unit.Requires: [openvswitch.service]` and `x-systemd.Unit.After: [openvswitch.service]`
- **THEN** the generated `.network` file has `Requires=openvswitch.service` and `After=openvswitch.service` in its `[Unit]` section

#### Scenario: x-systemd with only After is valid
- **WHEN** a network has `x-systemd.Unit.After: [openvswitch.service]` but no `Requires`
- **THEN** the `.network` file has `After=openvswitch.service` and no `Requires=` line

#### Scenario: Sequence directive values are joined with a space
- **WHEN** a network has `x-systemd.Unit.Requires: [a.service, b.service]`
- **THEN** the `.network` file contains `Requires=a.service b.service` in the `[Unit]` section

#### Scenario: Install.WantedBy is written to [Install]
- **WHEN** a network has `x-systemd.Install.WantedBy: [multi-user.target]`
- **THEN** the `.network` file contains `WantedBy=multi-user.target` in its `[Install]` section
