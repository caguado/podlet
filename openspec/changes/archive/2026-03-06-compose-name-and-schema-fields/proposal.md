## Why

The compose specification defines `name` fields on networks, volumes, and other entities to allow specifying a custom runtime name that differs from the compose map key. Podlet currently rejects these fields with "not supported" errors, forcing users to rename their compose keys or lose the intended resource names when generating Quadlet files. The compose spec also has several other fields (`network.attachable`, `service.domain_name`) that have direct or near-direct Quadlet/Podman equivalents yet are either silently dropped or blocked. Additionally, the top-level compose `name` (project name) is unused outside of `--pod` mode, even though it could auto-populate the systemd `Description=` to make generated units more readable.

> **Note on `description`:** The compose specification schema does not define a `description` field on any entity (service, network, volume). The systemd `[Unit] Description=` field is already settable via the `--description` CLI flag. What IS missing is *automatic population* of `Description=` from compose data.

## What Changes

- **`network.name`**: Remove the "not supported" rejection; add `NetworkName=` field to `quadlet::Network` and map compose `network.name` to it during conversion.
- **`volume.name`**: Remove the "not supported" rejection; add `VolumeName=` field to `quadlet::Volume` and map compose `volume.name` to it during conversion.
- **Top-level compose `name` → `Description=`**: When converting a compose file, if `--description` is not set, auto-populate `Description=` on each generated unit file using the pattern `"<service/network/volume> for pod <pod_name>"`, where `pod_name` comes from the top-level compose `name` field (or the explicit pod name when `--pod` is used).
- **`network.attachable`**: Remove the rejection; pass as `--attachable` via `PodmanArgs=` on the generated `.network` file.
- **`service.domain_name`**: Move from `Unsupported` to `PodmanArgs` (pass as `--domainname=VALUE`).

## Capabilities

### New Capabilities
- `network-name`: Support `name` field on compose networks, mapping to `NetworkName=` in `.network` Quadlet files.
- `volume-name`: Support `name` field on compose volumes, mapping to `VolumeName=` in `.volume` Quadlet files.
- `compose-project-description`: Auto-populate systemd `Description=` using `"<entity> for pod <name>"` from the top-level compose `name` when no explicit `--description` is provided.
- `network-attachable`: Support `attachable: true` on compose networks, emitting `--attachable` via `PodmanArgs=`.
- `service-domain-name`: Support `domain_name` on compose services, passing it as `--domainname=VALUE` in `PodmanArgs=`.

### Modified Capabilities
<!-- No existing specs have requirement changes from this proposal -->

## Impact

- `src/quadlet/network.rs`: Add `network_name: Option<String>` field; update `TryFrom<compose_spec::Network>` to map `name` instead of rejecting it; handle `attachable` via `PodmanArgs`.
- `src/quadlet/volume.rs`: Add `volume_name: Option<String>` field; update `TryFrom<compose_spec::Volume>` to map `name` instead of rejecting it.
- `src/cli/compose.rs`: Auto-populate `Description=` from compose project `name` when absent.
- `src/cli/container/compose.rs`: Move `domain_name` from `Unsupported` to `PodmanArgs`.
- `src/cli/container/podman.rs`: Add `domain_name` to the PodmanArgs builder.
- Downgrade logic: `NetworkName=` and `VolumeName=` may need minimum Podman version guards (check when these were added to Quadlet).
