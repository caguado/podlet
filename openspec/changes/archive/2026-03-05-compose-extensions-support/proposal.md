## Why

Podlet converts Compose files into Quadlet units, but every compose extension key (`x-*`) is universally rejected across ~30 call sites with `"compose extensions are not supported"`. There is no mechanism to handle extensions at all — not in podlet, not in compose_spec_rs. A Compose extension schema (`x-pods`, `x-pod`, `x-podman`, `x-systemd`) has been designed to express pod topology, Podman runtime options, and systemd unit dependencies; to support it (and any future extensions), podlet needs both a generic extension plugin/trait system and the concrete implementations for these four keys.

## What Changes

- Introduce a `ComposeExtensionHandler` trait in podlet that allows typed extension processors to be registered and called at each conversion scope (top-level, service, network, volume).
- Replace all `ensure!(extensions.is_empty(), "compose extensions are not supported")` guards with calls that pass unknown extensions through the registered handler chain, only erroring on truly unrecognised keys.
- Implement four concrete handlers as the first users of the trait:
  - **`XPodsHandler`**: parses top-level `x-pods` map; generates `.pod` quadlets with `PodName=`, `Network=<name>:ip=<addr>`, `UserNS=`, `[Unit]` dependencies, and `[Install] WantedBy=`.
  - **`XPodServiceHandler`**: parses per-service `x-pod.name`; sets `Pod=` in `.container` quadlets and inherits `WantedBy=` from the pod definition.
  - **`XPodmanHandler`**: parses `x-podman` at service scope (`cgroups` → `CgroupsMode=`), network scope (`disable-dns` → `DisableDNS=`), and volume scope (`ownership.user/group` → `User=`/`Group=`).
  - **`XSystemdHandler`**: parses `x-systemd` at network and volume scope (`requires`, `after` → `[Unit]` entries); at pod scope also handles `wanted-by` → `[Install] WantedBy=` propagated to member containers.
- Extend `quadlet::Pod` with `user_ns: Option<String>` and change `network` entries to support `name:ip=<addr>` notation.
- No changes to compose_spec_rs are required; extension values are already preserved in the `extensions: Extensions` field of each compose type and can be extracted via `serde_yaml::from_value`.

## Capabilities

### New Capabilities

- `compose-extension-trait`: A `ComposeExtensionHandler` trait and handler registry/dispatch infrastructure in podlet that replaces the blanket extension rejection.
- `x-pods-parsing`: Typed deserialization and `.pod` quadlet generation from the top-level `x-pods` map (pod network attachments, userns, systemd unit deps, wanted-by).
- `x-service-extensions`: Per-service `x-pod` (pod membership) and `x-podman` (`cgroups`) extension parsing and quadlet application.
- `x-network-extensions`: Per-network `x-podman` (`disable-dns`) and `x-systemd` (`requires`, `after`) extension parsing and quadlet application.
- `x-volume-extensions`: Per-volume `x-podman` (`ownership.user/group`) and `x-systemd` (`requires`, `after`) extension parsing and quadlet application.
- `pod-quadlet-generation`: Full `.pod` quadlet generation from extension-driven pod definitions, including all `[Unit]`, `[Pod]`, and `[Install]` fields.
- `container-pod-assignment`: Assign containers to pods and propagate `WantedBy=` from pod definitions to member container quadlets.

### Modified Capabilities

## Impact

- **podlet `src/cli/compose.rs`**: Main compose-to-quadlet conversion; replace extension rejection, thread handler registry through conversion functions.
- **podlet `src/cli/container/compose.rs`**, **`src/cli/k8s/service.rs`**, **`src/quadlet/network.rs`**, **`src/quadlet/volume.rs`**, and a dozen other files: Replace `ensure!(extensions.is_empty(), ...)` with handler dispatch or allowlist checks.
- **podlet `src/quadlet/pod.rs`**: Add `user_ns` field; extend `network` to encode static IP suffix.
- **New files in podlet**: `src/cli/compose/extensions.rs` (trait + registry) and `src/cli/compose/extensions/` (one module per handler).
- **compose_spec_rs**: No changes needed; `extensions: Extensions` already preserves all `x-*` data.
