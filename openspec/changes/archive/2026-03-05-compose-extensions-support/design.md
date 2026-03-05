## Context

Podlet converts Compose files to Quadlet systemd units. The `compose_spec` crate already preserves all `x-*` extension keys in `extensions: Extensions` (`IndexMap<ExtensionKey, YamlValue>`) on `Compose`, `Service`, `Network`, `Volume`, and several nested types. Podlet has ~30 call sites that unconditionally reject any non-empty `extensions` map with `ensure!(extensions.is_empty(), "compose extensions are not supported")`.

A Compose extension schema (`x-pods`, `x-pod`, `x-podman`, `x-systemd`) exists in `../podman-compose-spec/` and needs to be consumed by podlet to generate fully-configured Quadlet files without needing manual CLI flags per resource.

The change must also be extensible — new `x-*` keys added in the future should be easy to plug in without touching the core conversion logic.

## Goals / Non-Goals

**Goals:**
- Define a `ComposeExtensionHandler` trait that decouples extension logic from core conversion code.
- Implement a registry that dispatches extension processing and detects unrecognized keys.
- Implement four concrete handlers: `XPodsHandler`, `XPodServiceHandler`, `XPodmanHandler`, `XSystemdHandler`.
- Add `CgroupsMode=` to `quadlet::Container` and `UserNS=` to `quadlet::Pod`.
- Add `network_attachments: IndexMap<String, PodNetworkOptions>` to `quadlet::Pod` for named network attachments with options.
- Add `--disable-extension <KEY>` to `podlet compose` for per-handler opt-out.
- Replace all `ensure!(extensions.is_empty(), ...)` guards with registry dispatch.

**Non-Goals:**
- Modifying `compose_spec_rs` — all extension values are already preserved.
- Supporting `x-*` extensions in the Kubernetes YAML path (`podlet compose --kube`); those call sites keep their existing guards for now.
- Runtime-loadable plugins (dynamic dispatch at startup via dynamic libraries).
- Validating extension values against the JSON schema at runtime.

## Decisions

### Decision 1: Trait + registry over ad-hoc match arms

**Chosen:** Define a `ComposeExtensionHandler` trait with default no-op implementations for each scope (compose-level, service, network, volume). A `ExtensionRegistry` dispatches to all registered handlers, collects extra quadlet files, and validates that every extension key in the file is handled by at least one registered handler.

**Alternative considered:** A large `match extension_key { "x-podman" => ..., "x-systemd" => ... }` block inside `compose.rs`. Simple, but requires editing core conversion code to add each new extension. Rejected because the goal is extensibility.

**Alternative considered:** Visitor pattern applied to the compose tree. More powerful but overkill given the shallow extension structure; the trait approach is simpler and sufficient.

---

### Decision 2: Two-phase processing (context build → per-resource apply)

**Chosen:** Processing is split into two phases:
1. **Context phase**: The registry calls `build_context()` on each handler with the top-level `Compose` struct. Handlers populate a shared `ExtensionContext` (e.g., `XPodsHandler` populates `ExtensionContext::pods` with `ResolvedPod` entries, keyed by pod name).
2. **Apply phase**: The registry calls `handle_service` / `handle_network` / `handle_volume` with the immutable `&ExtensionContext`. Handlers read from the context to do cross-resource work (e.g., `XPodServiceHandler` looks up the pod's `wanted_by` list from context when assigning a container to a pod).

**Alternative considered:** Single-pass with mutable shared state. Would require either locking or careful ordering. Rejected to keep handlers stateless and order-independent within each phase.

**Alternative considered:** Passing the full `Compose` to every per-resource handler. Rejected as too broad; handlers should only see what they need.

---

### Decision 3: `XPodsHandler` supersedes `--pod` flag for extension-driven files

**Chosen:** When `x-pods` is present in the compose file's top-level extensions, pod topology is inferred from `x-pods` rather than the `--pod` flag. The `--pod` flag continues to work as-is for compose files without `x-pods`. If both are present, return an error (ambiguous).

**Alternative considered:** Require the user to always pass `--pod` even when `x-pods` is present. Rejected because the entire point of the extension is to make the compose file self-describing.

---

### Decision 4: Unknown extensions warn rather than error

**Chosen:** After dispatching known extensions, the registry logs a warning for any unrecognized `x-*` key rather than returning an error. This makes podlet forward-compatible with future extension keys that users may add before podlet supports them.

**Alternative considered:** Error on unknown extensions. Strict but breaks existing files when new keys are added. Rejected.

---

### Decision 5: Deserialize extension values with `serde_yaml::from_value`

**Chosen:** Each handler defines private typed structs mirroring the JSON schema definitions and deserializes them from `YamlValue` via `serde_yaml::from_value`. This gives strongly-typed access without changing compose_spec_rs.

**Alternative considered:** Match on `YamlValue` manually. Verbose and error-prone. Rejected.

---

### Decision 6: `CgroupsMode=` is a new field on `quadlet::Container`

**Chosen:** Add `pub cgroups_mode: Option<String>` with `#[serde(rename = "CgroupsMode")]` to `quadlet::Container`. The `x-podman` handler sets this from the `cgroups` extension value. The existing `--cgroups` CLI arg in `podman.rs` remains as a `PodmanArgs` passthrough for direct CLI use; `CgroupsMode=` is the Quadlet-native form used in generated files.

---

### Decision 7: Named network attachments in `quadlet::Pod` use a structured map field

**Chosen:** Add a new `network_attachments: IndexMap<String, PodNetworkOptions>` field to `quadlet::Pod`, separate from the existing `network: Vec<String>` (which handles raw mode strings like `host`, `slirp4netns`, or `bridge:...` from the `--network` CLI flag). `PodNetworkOptions` holds typed per-network options (initially `ip: Option<Ipv4Addr>`). The field is serialized with a custom `serialize_with` function that converts the map to a sequence of `Network=<name>.network:ip=<addr>` strings, which the quadlet serializer then emits as repeated `Network=` lines — the same wire format used by `Vec<String>`. This avoids collisions between the two fields since they serialize to the same key name through different mechanisms.

**Why separate fields rather than merging into `Vec<String>` with formatted strings:** String concatenation (`"name.network:ip=10.0.0.1"`) is opaque and untestable. The map form mirrors the extension schema structure, is directly deserializable from `x-pods.networks`, and allows future options (`mac=`, `interface=`) without format string changes.

**Why not change `Vec<String>` to `Vec<PodNetworkEntry>` (a struct with name + options map):** The existing `Vec<String>` field is set by the CLI path via `From<Create>` for `quadlet::Pod` with raw user-supplied strings. Changing its type would require converting free-form mode strings into a structured type, which is lossy and fragile (e.g., `bridge:interface=eth0,mtu=1500` is not a named network). Keeping them as separate fields is cleaner.

**Constraint:** The quadlet serializer's `SerializeMap = Impossible<(), Error>` means `IndexMap<K,V>` cannot be serialized natively. The `serialize_with` function works around this by converting the map to a `Vec<String>` before serialization, producing the correct `Network=` lines.

---

### Decision 8: Registry is enabled by default; individual handlers are configurable via CLI

**Chosen:** All four handlers are registered by default. A new `--disable-extension <KEY>` CLI option (repeatable) on `podlet compose` allows disabling specific extension keys (e.g., `--disable-extension x-podman`). This covers the case where a user wants to ignore certain extensions in their compose file without removing them.

**Alternative considered:** Always-on with no opt-out. Simpler but inflexible. Rejected given user preference for configurability.

## Risks / Trade-offs

- [Risk] Extension parsing errors (bad YAML shape) produce opaque serde errors → Mitigation: wrap each `from_value` call with `wrap_err_with(|| format!("error in extension `{key}` for ..."))`.
- [Risk] `x-pods` and `--pod` both present → Mitigation: explicit error in compose pre-flight check.
- [Risk] Handler order matters if two handlers modify the same quadlet field → Mitigation: document that handlers are applied in registration order and each handler owns distinct fields (no two handlers write the same quadlet field).
- [Risk] `WantedBy=` propagation logic is split across `XPodsHandler` (builds context) and `XPodServiceHandler` (applies it) — if the pod name in `x-pod` doesn't match any key in `x-pods`, we should error clearly → Mitigation: validate pod membership references against the context in `XPodServiceHandler.handle_service`.

## Migration Plan

All changes are additive. Existing compose files without any `x-*` keys continue to work exactly as before (handlers are all no-ops when `extensions.is_empty()`). Files that previously failed with "compose extensions are not supported" will now succeed.

The `--pod` flag is unchanged; files that use it without `x-pods` continue to work.

## Open Questions

- None outstanding. Resolved: unknown extensions warn to stderr; registry is default-on with per-handler opt-out via `--disable-extension`.
