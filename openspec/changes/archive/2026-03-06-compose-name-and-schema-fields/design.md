## Context

Podlet converts compose files to Podman Quadlet unit files. The compose specification allows several entity-level fields that Quadlet has direct equivalents for, but Podlet either rejects them (`name` on networks/volumes, `attachable` on networks) or silently discards them (`domain_name` on services). Additionally, the top-level compose `name` (project name) is only used when `--pod` is active, leaving generated units with no human-readable description when opened in a text editor or systemd tooling.

The key file paths involved:
- `src/quadlet/network.rs` — `quadlet::Network` struct and `TryFrom<compose_spec::Network>`
- `src/quadlet/volume.rs` — `quadlet::Volume` struct and `TryFrom<compose_spec::Volume>`
- `src/cli/compose.rs` — `parts_try_into_files`, `service_try_into_quadlet_file`
- `src/cli/container/compose.rs` — compose Service → PodmanArgs split
- `src/cli/container/podman.rs` — PodmanArgs → `podman run` flags

## Goals / Non-Goals

**Goals:**
- Map compose `network.name` → `NetworkName=` in `.network` Quadlet files.
- Map compose `volume.name` → `VolumeName=` in `.volume` Quadlet files.
- Map compose `network.attachable: true` → `PodmanArgs=--attachable`.
- Map compose `service.domain_name` → `PodmanArgs=--domainname=VALUE`.
- Auto-populate `Description=` on generated units when the top-level compose `name` is set and no `--description` was passed, using pattern `"<entity> for pod <name>"`.

**Non-Goals:**
- Adding Quadlet `NetworkName=`/`VolumeName=` to the CLI `podlet network`/`podlet volume` subcommands (only the compose path).
- Renaming generated `.network`/`.volume` files to match the custom `name` (the compose key remains the file name; the quadlet option sets the runtime name).
- Supporting `attachable` via a dedicated quadlet field (no such field exists; `PodmanArgs=` is the correct vehicle).

## Decisions

### D1: `network_name` / `volume_name` as first-class quadlet struct fields

Add `network_name: Option<String>` and `volume_name: Option<String>` directly to `quadlet::Network` and `quadlet::Volume` respectively, following the existing `container_name` pattern. Alternative of passing via `PodmanArgs=--name` was rejected: explicit fields are verifiable, downgradeable, and match the Quadlet spec's own `NetworkName=`/`VolumeName=` options.

**Minimum version**: Both `NetworkName=` and `VolumeName=` were introduced with Quadlet network/volume support in Podman 4.5. No additional downgrade guard is needed beyond what already exists.

### D2: `network.attachable` via `PodmanArgs=`

There is no dedicated `Attachable=` quadlet network field, so the flag is appended to `PodmanArgs=` when `attachable: true`. The existing `Network::push_arg` helper only handles `--flag value` pairs; a new `push_flag` variant (or inline string append) is needed for bare flags. We add a `push_flag(&str)` helper on `quadlet::Network` to keep the pattern consistent.

### D3: `domain_name` moves to `PodmanArgs`, not `QuadletOptions`

No Quadlet `DomainName=` option exists. `domain_name` is moved from `Unsupported` to `PodmanArgs` in `compose::PodmanArgs` and rendered as `--domainname=VALUE`. This matches how other non-quadlet options (cgroup, ipc, uts, etc.) are handled.

### D4: Description auto-population is conditional and non-overriding

`Description=` is only set when:
1. The top-level compose `name` field is present.
2. The caller has not already set a description (i.e., `unit.description.is_none()` or `unit` is `None`).

The function `parts_try_into_files` already receives `unit: Option<Unit>` and `pod_name: Option<String>`. The compose `name` is extracted upstream; we thread it into `parts_try_into_files` as `Option<String>` alongside `pod_name`. Entity type strings: `"container"`, `"network"`, `"volume"`. Pattern: `"<entity> for pod <name>"`.

## Risks / Trade-offs

- **`VolumeName=` breaks the `.volume` reference assumption**: When a compose volume has a custom `name`, its Podman runtime name differs from the compose key (and thus from the `.volume` file stem). Containers referencing the volume as `mykey.volume:/path` still work because Quadlet resolves the reference through the `.volume` file — Podman uses whatever `VolumeName=` says at runtime. No change to how containers reference volumes is needed. [Low risk]
- **`NetworkName=` similarly**: Same reasoning applies. Containers use `mykey.network` as the quadlet reference; the actual network name at runtime is whatever `NetworkName=` says. [Low risk]
- **Description auto-population is additive**: If a user passes `--description`, it takes precedence (no override). If compose `name` is absent, no description is auto-set. [No risk]
- **`--domainname` availability**: `--domainname` has been in Podman since early versions; no version guard needed.

## Open Questions

- Should `network_name` and `volume_name` also be exposed in the CLI `podlet network create` and `podlet volume create` subcommands? (Deferred — not in scope of this change.)
- Should the description pattern be configurable (e.g., via a future `--description-pattern` flag)? (Deferred — hardcode for now.)
