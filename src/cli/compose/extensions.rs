mod x_pod_service;
mod x_podman;
mod x_pods;
mod x_systemd;

use std::collections::HashSet;

use color_eyre::eyre::Result;
use compose_spec::Extensions;
use indexmap::IndexMap;

use crate::quadlet::{self, Unit};

/// Shared context built from the top-level compose extensions during the context phase.
pub struct ExtensionContext {
    /// Pods resolved from `x-pods`, keyed by pod name.
    pub pods: IndexMap<String, ResolvedPod>,
}

/// A pod resolved from the `x-pods` extension, holding all data needed to build a `.pod` quadlet
/// file and to dispatch services/containers into the pod.
pub struct ResolvedPod {
    /// Pod name (e.g., "observability").
    pub name: String,
    /// The `[Pod]` section data.
    pub pod: quadlet::Pod,
    /// Optional `[Unit]` section (requires, after, etc.).
    pub unit: Option<Unit>,
    /// `WantedBy=` targets propagated from `x-systemd.wanted-by`.
    pub wanted_by: Vec<String>,
}

/// Trait for typed compose extension processors.
///
/// Each implementation handles one or more `x-*` keys.  Default no-op implementations are provided
/// for every method so that a handler only needs to override the scopes it cares about.
pub trait ComposeExtensionHandler {
    /// The extension keys this handler processes (e.g., `&["x-pods"]`).
    fn handled_keys(&self) -> &[&'static str];

    /// Called once with the full compose document to build shared context.
    ///
    /// Handlers populate [`ExtensionContext`] here (e.g., resolve pod definitions).
    fn build_context(
        &self,
        _compose: &compose_spec::Compose,
        _context: &mut ExtensionContext,
    ) -> Result<()> {
        Ok(())
    }

    /// Returns additional quadlet files to emit (e.g., `.pod` files).
    fn compose_files(
        &self,
        _context: &ExtensionContext,
        _install: Option<&quadlet::Install>,
    ) -> Result<Vec<quadlet::File>> {
        Ok(Vec::new())
    }

    /// Called for each service with its extension map.
    fn handle_service(
        &self,
        _service_name: &str,
        _extensions: &Extensions,
        _context: &mut ExtensionContext,
        _file: &mut quadlet::File,
    ) -> Result<()> {
        Ok(())
    }

    /// Called for each network with its extension map.
    fn handle_network(
        &self,
        _network_name: &str,
        _extensions: &Extensions,
        _context: &mut ExtensionContext,
        _file: &mut quadlet::File,
    ) -> Result<()> {
        Ok(())
    }

    /// Called for each volume with its extension map.
    fn handle_volume(
        &self,
        _volume_name: &str,
        _extensions: &Extensions,
        _context: &mut ExtensionContext,
        _file: &mut quadlet::File,
    ) -> Result<()> {
        Ok(())
    }
}

/// Registry that dispatches compose extensions to registered [`ComposeExtensionHandler`]s.
pub struct ExtensionRegistry {
    handlers: Vec<Box<dyn ComposeExtensionHandler>>,
    disabled_keys: HashSet<String>,
}

impl ExtensionRegistry {
    /// Build the registry with all default handlers registered.
    ///
    /// Keys in `disabled_keys` will be treated as known (suppressing "unknown extension" warnings)
    /// but their handlers will be skipped.
    #[must_use]
    pub fn new(disabled_keys: HashSet<String>) -> Self {
        Self {
            handlers: vec![
                Box::new(x_pods::XPodsHandler),
                Box::new(x_pod_service::XPodServiceHandler),
                Box::new(x_podman::XPodmanHandler),
                Box::new(x_systemd::XSystemdHandler),
            ],
            disabled_keys,
        }
    }

    /// Returns `true` if the given extension key is disabled for a given handler.
    fn is_disabled(&self, key: &str) -> bool {
        self.disabled_keys.contains(key)
    }

    /// Returns `true` if all of a handler's keys are disabled.
    fn handler_disabled(&self, handler: &dyn ComposeExtensionHandler) -> bool {
        handler.handled_keys().iter().all(|k| self.is_disabled(k))
    }

    /// Run the context-build phase: calls `build_context` on each non-disabled handler.
    ///
    /// # Errors
    ///
    /// Returns an error if any handler's `build_context` fails.
    pub fn build_context(&self, compose: &compose_spec::Compose) -> Result<ExtensionContext> {
        let mut context = ExtensionContext {
            pods: IndexMap::new(),
        };
        for handler in &self.handlers {
            if !self.handler_disabled(handler.as_ref()) {
                handler.build_context(compose, &mut context)?;
            }
        }
        Ok(context)
    }

    /// Collect extra quadlet files (e.g., pod files) from all non-disabled handlers.
    ///
    /// # Errors
    ///
    /// Returns an error if any handler's `compose_files` fails.
    pub fn compose_files(
        &self,
        context: &ExtensionContext,
        install: Option<&quadlet::Install>,
    ) -> Result<Vec<quadlet::File>> {
        let mut files = Vec::new();
        for handler in &self.handlers {
            if !self.handler_disabled(handler.as_ref()) {
                files.extend(handler.compose_files(context, install)?);
            }
        }
        Ok(files)
    }

    /// Dispatch service extensions to all relevant non-disabled handlers.
    ///
    /// # Errors
    ///
    /// Returns an error if any handler returns an error.
    pub fn apply_service(
        &self,
        service_name: &str,
        extensions: &Extensions,
        context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        for handler in &self.handlers {
            let skip = handler
                .handled_keys()
                .iter()
                .all(|k| self.is_disabled(k) || !extensions.contains_key(*k));
            if !skip {
                handler.handle_service(service_name, extensions, context, file)?;
            }
        }
        Ok(())
    }

    /// Dispatch network extensions to all relevant non-disabled handlers.
    ///
    /// # Errors
    ///
    /// Returns an error if any handler returns an error.
    pub fn apply_network(
        &self,
        network_name: &str,
        extensions: &Extensions,
        context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        for handler in &self.handlers {
            let skip = handler
                .handled_keys()
                .iter()
                .all(|k| self.is_disabled(k) || !extensions.contains_key(*k));
            if !skip {
                handler.handle_network(network_name, extensions, context, file)?;
            }
        }
        Ok(())
    }

    /// Dispatch volume extensions to all relevant non-disabled handlers.
    ///
    /// # Errors
    ///
    /// Returns an error if any handler returns an error.
    pub fn apply_volume(
        &self,
        volume_name: &str,
        extensions: &Extensions,
        context: &mut ExtensionContext,
        file: &mut quadlet::File,
    ) -> Result<()> {
        for handler in &self.handlers {
            let skip = handler
                .handled_keys()
                .iter()
                .all(|k| self.is_disabled(k) || !extensions.contains_key(*k));
            if !skip {
                handler.handle_volume(volume_name, extensions, context, file)?;
            }
        }
        Ok(())
    }

    /// Warn to stderr for any extension key not handled by any registered handler (and not
    /// disabled).
    pub fn warn_unknown(&self, scope: &str, extensions: &Extensions) {
        let all_known: HashSet<&str> = self
            .handlers
            .iter()
            .flat_map(|h| h.handled_keys().iter().copied())
            .chain(self.disabled_keys.iter().map(String::as_str))
            .collect();

        for key in extensions.keys() {
            if !all_known.contains(key.as_str()) {
                eprintln!(
                    "warning: compose extension `{}` in `{scope}` is not supported and will be ignored",
                    key.as_str()
                );
            }
        }
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new(HashSet::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadlet::{self, Globals};

    struct AppendHandler {
        tag: &'static str,
    }

    impl ComposeExtensionHandler for AppendHandler {
        fn handled_keys(&self) -> &[&'static str] {
            &["x-test"]
        }

        fn handle_service(
            &self,
            _service_name: &str,
            _extensions: &Extensions,
            _context: &mut ExtensionContext,
            file: &mut quadlet::File,
        ) -> Result<()> {
            if let quadlet::Resource::Container(container) = &mut file.resource {
                let existing = container.container_name.get_or_insert_with(String::new);
                existing.push_str(self.tag);
            }
            Ok(())
        }
    }

    fn make_registry_with_two_handlers() -> (ExtensionRegistry, ExtensionContext) {
        let mut registry = ExtensionRegistry::new(HashSet::new());
        // Replace default handlers with two test handlers.
        registry.handlers = vec![
            Box::new(AppendHandler { tag: "A" }),
            Box::new(AppendHandler { tag: "B" }),
        ];
        let context = ExtensionContext {
            pods: IndexMap::new(),
        };
        (registry, context)
    }

    #[test]
    fn registry_two_handlers_both_called_in_order() {
        let (registry, mut context) = make_registry_with_two_handlers();

        let mut file = quadlet::File {
            name: "test".into(),
            unit: Default::default(),
            resource: quadlet::Container {
                image: "img".into(),
                container_name: Some(String::new()),
                ..Default::default()
            }
            .into(),
            globals: Globals::default(),
            quadlet: Default::default(),
            service: Default::default(),
            install: Default::default(),
        };

        // Build a fake extensions map with "x-test" present.
        let mut extensions = Extensions::new();
        extensions.insert(
            "x-test".parse().expect("valid key"),
            serde_yaml::Value::Null,
        );

        registry
            .apply_service("svc", &extensions, &mut context, &mut file)
            .expect("apply_service");

        // Both handlers should have appended their tag in order.
        assert!(matches!(&file.resource, quadlet::Resource::Container(_)));
        if let quadlet::Resource::Container(c) = &file.resource {
            assert_eq!(c.container_name.as_deref(), Some("AB"));
        }
    }

    #[test]
    fn warn_unknown_emits_warning_for_unrecognized_key() {
        // warn_unknown writes to stderr; we just call it to verify no panic.
        let registry = ExtensionRegistry::default();
        let mut extensions = Extensions::new();
        extensions.insert(
            "x-totally-unknown".parse().expect("valid key"),
            serde_yaml::Value::Null,
        );
        // Should not panic.
        registry.warn_unknown("test-scope", &extensions);
    }
}
