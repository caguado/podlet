## ADDED Requirements

### Requirement: ComposeExtensionHandler trait exists
The system SHALL define a `ComposeExtensionHandler` trait in `src/cli/compose/extensions.rs` with the following method signatures and default no-op implementations:
- `fn handled_keys(&self) -> &[&'static str]` — the `x-*` keys this handler processes (no default; must be implemented).
- `fn build_context(&self, extensions: &Extensions, context: &mut ExtensionContext) -> color_eyre::Result<()>` — default `Ok(())`.
- `fn compose_files(&self, context: &ExtensionContext, install: Option<&quadlet::Install>) -> color_eyre::Result<Vec<quadlet::File>>` — default `Ok(vec![])`.
- `fn handle_service(&self, name: &Identifier, extensions: &Extensions, context: &ExtensionContext, file: &mut quadlet::File) -> color_eyre::Result<()>` — default `Ok(())`.
- `fn handle_network(&self, name: &Identifier, extensions: &Extensions, file: &mut quadlet::File) -> color_eyre::Result<()>` — default `Ok(())`.
- `fn handle_volume(&self, name: &Identifier, extensions: &Extensions, file: &mut quadlet::File) -> color_eyre::Result<()>` — default `Ok(())`.

#### Scenario: Handler only implements the scopes it cares about
- **WHEN** a handler only implements `handle_network` and leaves all other methods at their default
- **THEN** calling `handle_service` or `handle_volume` on that handler produces `Ok(())` without panicking

---

### Requirement: ExtensionContext carries cross-resource data
The system SHALL define an `ExtensionContext` struct with at minimum:
- `pods: IndexMap<String, ResolvedPod>` — pod definitions keyed by pod name, populated by `XPodsHandler::build_context`.

`ResolvedPod` SHALL contain:
- `networks: IndexMap<String, Option<Ipv4Addr>>` — network name → optional static IP.
- `user_ns: Option<String>`.
- `systemd_requires: Vec<String>`.
- `systemd_after: Vec<String>`.
- `systemd_wanted_by: Vec<String>`.

#### Scenario: Context is empty when no extensions are present
- **WHEN** `build_context` is called with an empty `Extensions` map
- **THEN** `ExtensionContext::pods` is empty and no error is returned

---

### Requirement: ExtensionRegistry dispatches to all registered handlers
The system SHALL define an `ExtensionRegistry` struct that holds `Vec<Box<dyn ComposeExtensionHandler>>` and exposes:
- `fn new(handlers: Vec<Box<dyn ComposeExtensionHandler>>) -> Self`
- `fn build_context(&self, compose: &compose_spec::Compose) -> color_eyre::Result<ExtensionContext>` — calls `build_context` on each handler with the top-level extensions.
- `fn compose_files(&self, context: &ExtensionContext, install: Option<&quadlet::Install>) -> color_eyre::Result<Vec<quadlet::File>>` — collects and flattens files from all handlers.
- `fn apply_service(...)`, `fn apply_network(...)`, `fn apply_volume(...)` — dispatch to all registered handlers in order.
- `fn warn_unknown(&self, scope: &str, extensions: &Extensions)` — emits a warning to stderr for any key in `extensions` not covered by any handler's `handled_keys()`.

#### Scenario: Registry dispatches to every registered handler
- **WHEN** two handlers are registered and both implement `handle_network`
- **THEN** both handlers are called in registration order when `apply_network` is invoked

#### Scenario: Registry warns on unknown extension keys
- **WHEN** a network has an extension key `x-unknown` that no handler declares in `handled_keys()`
- **THEN** a warning is printed to stderr and no error is returned

---

### Requirement: Default registry registers all four handlers with all enabled
The system SHALL provide a `ExtensionRegistry::default()` constructor that registers `XPodsHandler`, `XPodServiceHandler`, `XPodmanHandler`, and `XSystemdHandler` with all handlers active.

#### Scenario: Default registry handles all known extension keys
- **WHEN** `ExtensionRegistry::default()` is constructed
- **THEN** the set of all `handled_keys()` across registered handlers includes `x-pods`, `x-pod`, `x-podman`, and `x-systemd`

---

### Requirement: Handlers can be disabled via --disable-extension CLI flag
The `podlet compose` subcommand SHALL accept a repeatable `--disable-extension <KEY>` option. When a key is disabled, the corresponding handler's methods SHALL be skipped (treated as no-ops) for that invocation.

#### Scenario: Disabling a handler prevents it from modifying quadlet output
- **WHEN** `--disable-extension x-podman` is passed
- **THEN** `x-podman` values in the compose file are ignored and no `CgroupsMode=` or `DisableDNS=` is emitted in any quadlet

#### Scenario: Disabling one handler does not affect others
- **WHEN** `--disable-extension x-podman` is passed and `x-systemd` is present on a network
- **THEN** the network quadlet still has `Requires=` and `After=` from `x-systemd`
