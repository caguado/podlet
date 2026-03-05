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

### Requirement: x-systemd on a network populates [Unit] Requires= and After=
The system SHALL implement `XSystemdHandler::handle_network` which extracts `x-systemd` from the network's extensions and appends its `requires` and `after` lists to the `unit.requires` and `unit.after` fields of the `quadlet::File`.

If the file has no `[Unit]` section yet, one SHALL be created.

#### Scenario: Requires= and After= are emitted from x-systemd
- **WHEN** a network has `x-systemd.requires: [openvswitch.service]` and `x-systemd.after: [openvswitch.service]`
- **THEN** the generated `.network` file has `Requires=openvswitch.service` and `After=openvswitch.service` in its `[Unit]` section

#### Scenario: x-systemd with only after= is valid
- **WHEN** a network has `x-systemd.after: [openvswitch.service]` but no `requires`
- **THEN** the `.network` file has `After=openvswitch.service` and no `Requires=` line

#### Scenario: Multiple units in requires are all emitted
- **WHEN** a network has `x-systemd.requires: [a.service, b.service]`
- **THEN** the `.network` file contains both `Requires=a.service` and `Requires=b.service`
