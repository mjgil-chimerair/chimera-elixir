//! Plugin API for the zelix Rust/Zig Elixir compiler.
//!
//! Provides plugin discovery, lifecycle hooks, phase pass-through,
//! and isolation for custom compiler extensions.

#![allow(dead_code)]

#[cfg(test)]
use chimera_allocator as _;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Lifecycle phase when the plugin is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginPhase {
    BeforeLexer,
    AfterLexer,
    BeforeParser,
    AfterParser,
    BeforeSemantic,
    AfterSemantic,
    BeforeLowering,
    AfterLowering,
    BeforeBeam,
    AfterBeam,
    BeforeEmit,
    AfterEmit,
}

impl PluginPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            PluginPhase::BeforeLexer => "before_lexer",
            PluginPhase::AfterLexer => "after_lexer",
            PluginPhase::BeforeParser => "before_parser",
            PluginPhase::AfterParser => "after_parser",
            PluginPhase::BeforeSemantic => "before_semantic",
            PluginPhase::AfterSemantic => "after_semantic",
            PluginPhase::BeforeLowering => "before_lowering",
            PluginPhase::AfterLowering => "after_lowering",
            PluginPhase::BeforeBeam => "before_beam",
            PluginPhase::AfterBeam => "after_beam",
            PluginPhase::BeforeEmit => "before_emit",
            PluginPhase::AfterEmit => "after_emit",
        }
    }
}

/// Severity level for diagnostics and lint rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
    Critical,
}

/// Metadata about a loaded plugin.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub lifecycle_phase: PluginPhase,
    pub api_version: u32,
}

/// Result returned by plugin execution.
#[derive(Debug)]
pub struct PluginResult {
    pub success: bool,
    pub modified: bool,
    pub error_message: Option<String>,
}

impl PluginResult {
    pub fn success() -> Self {
        Self {
            success: true,
            modified: false,
            error_message: None,
        }
    }

    pub fn modified() -> Self {
        Self {
            success: true,
            modified: true,
            error_message: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            modified: false,
            error_message: Some(message.into()),
        }
    }
}

/// Plugin instance with its state.
pub trait PluginInstance: Send + Sync {
    fn execute(&self, ctx: &mut PluginContext) -> PluginResult;
    fn metadata(&self) -> &PluginMetadata;
}

/// Plugin interface that all plugins must implement.
pub trait Plugin: Send + Sync {
    fn init(&self, config: &[u8]) -> Result<Box<dyn PluginInstance>, PluginError>;

    fn metadata(&self) -> &PluginMetadata;
    fn create_instance(&self) -> Result<Box<dyn PluginInstance>, PluginError>;
}

/// Errors that can occur during plugin operations.
#[derive(Debug)]
pub enum PluginError {
    NotFound(String),
    Disabled(String),
    AlreadyLoaded(String),
    InitFailed(String),
    ExecutionFailed(String),
    InvalidConfig,
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::NotFound(name) => write!(f, "plugin not found: {}", name),
            PluginError::Disabled(name) => write!(f, "plugin is disabled: {}", name),
            PluginError::AlreadyLoaded(name) => write!(f, "plugin already loaded: {}", name),
            PluginError::InitFailed(msg) => write!(f, "plugin initialization failed: {}", msg),
            PluginError::ExecutionFailed(msg) => write!(f, "plugin execution failed: {}", msg),
            PluginError::InvalidConfig => write!(f, "invalid plugin configuration"),
        }
    }
}

impl std::error::Error for PluginError {}

/// Custom lint rule registration for plugins.
/// Allows plugins to register their own lint rules through the plugin API.
pub struct CustomLintRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub severity: Severity,
    pub check_fn: Box<dyn Fn(u32, &str, &str) -> Vec<LintFindingSimple> + Send + Sync>,
}

impl CustomLintRule {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        severity: Severity,
        check_fn: impl Fn(u32, &str, &str) -> Vec<LintFindingSimple> + 'static + Send + Sync,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            category: category.into(),
            severity,
            check_fn: Box::new(check_fn),
        }
    }

    pub fn check(&self, source_id: u32, file_path: &str, source: &str) -> Vec<LintFindingSimple> {
        (self.check_fn)(source_id, file_path, source)
    }
}

/// Simple lint finding for plugin-defined rules.
#[derive(Debug, Clone)]
pub struct LintFindingSimple {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub note: Option<String>,
    pub hint: Option<String>,
}

impl LintFindingSimple {
    pub fn new(message: impl Into<String>, line: usize, column: usize, offset: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
            offset,
            note: None,
            hint: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Registry for custom lint rules from plugins.
pub struct CustomLintRegistry {
    rules: std::sync::RwLock<Vec<CustomLintRule>>,
}

impl CustomLintRegistry {
    pub fn new() -> Self {
        Self {
            rules: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Register a custom lint rule from a plugin.
    pub fn register(&self, rule: CustomLintRule) -> Result<(), String> {
        let mut rules = self
            .rules
            .write()
            .map_err(|e| format!("lock poisoned: {}", e))?;
        rules.push(rule);
        Ok(())
    }

    /// Unregister all rules from a plugin by prefix.
    pub fn unregister_plugin(&self, plugin_name: &str) {
        let mut rules = self.rules.write().unwrap();
        rules.retain(|r| !r.id.starts_with(&format!("{}_", plugin_name)));
    }

    /// Get all registered rule IDs.
    pub fn rule_ids(&self) -> Vec<String> {
        let rules = self.rules.read().unwrap();
        rules.iter().map(|r| r.id.clone()).collect()
    }

    /// Check source with all registered custom rules.
    pub fn check_all(
        &self,
        source_id: u32,
        file_path: &str,
        source: &str,
    ) -> Vec<LintFindingSimple> {
        let rules = self.rules.read().unwrap();
        let mut findings = Vec::new();
        for rule in rules.iter() {
            findings.extend(rule.check(source_id, file_path, source));
        }
        findings
    }

    /// Get count of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.read().unwrap().len()
    }

    /// Clear all rules.
    pub fn clear(&self) {
        self.rules.write().unwrap().clear();
    }
}

impl Default for CustomLintRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// AST transform pass registered by plugins.
pub struct TransformPass {
    pub id: String,
    pub name: String,
    pub description: String,
    pub phase: PluginPhase,
    transform_fn: Box<dyn Fn(AstDocumentSimple) -> AstDocumentSimple + Send + Sync>,
}

impl TransformPass {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        phase: PluginPhase,
        transform_fn: impl Fn(AstDocumentSimple) -> AstDocumentSimple + 'static + Send + Sync,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            phase,
            transform_fn: Box::new(transform_fn),
        }
    }

    pub fn execute(&self, document: AstDocumentSimple) -> AstDocumentSimple {
        (self.transform_fn)(document)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn phase(&self) -> PluginPhase {
        self.phase
    }
}

/// Simple AST document representation for transform passes.
#[derive(Debug, Clone, Default)]
pub struct AstDocumentSimple {
    pub source_id: u32,
    pub nodes: Vec<AstNodeSimple>,
    pub root_id: Option<u64>,
}

impl AstDocumentSimple {
    pub fn new(source_id: u32) -> Self {
        Self {
            source_id,
            nodes: Vec::new(),
            root_id: None,
        }
    }

    pub fn with_nodes(mut self, nodes: Vec<AstNodeSimple>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn set_root(&mut self, id: u64) {
        self.root_id = Some(id);
    }
}

/// Simple AST node representation.
#[derive(Debug, Clone)]
pub struct AstNodeSimple {
    pub id: u64,
    pub kind: String,
    pub content: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub children: Vec<u64>,
}

impl AstNodeSimple {
    pub fn new(
        id: u64,
        kind: impl Into<String>,
        content: impl Into<String>,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            id,
            kind: kind.into(),
            content: content.into(),
            start_offset: start,
            end_offset: end,
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<u64>) -> Self {
        self.children = children;
        self
    }
}

/// Registry for AST transform passes from plugins.
pub struct TransformPassRegistry {
    passes: std::sync::RwLock<Vec<TransformPass>>,
}

impl TransformPassRegistry {
    pub fn new() -> Self {
        Self {
            passes: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Register a transform pass from a plugin.
    pub fn register(&self, pass: TransformPass) -> Result<(), String> {
        let mut passes = self
            .passes
            .write()
            .map_err(|e| format!("lock poisoned: {}", e))?;
        passes.push(pass);
        Ok(())
    }

    /// Unregister all passes from a plugin by prefix.
    pub fn unregister_plugin(&self, plugin_name: &str) {
        let mut passes = self.passes.write().unwrap();
        passes.retain(|p| !p.id.starts_with(&format!("{}_", plugin_name)));
    }

    /// Get pass IDs for a specific phase (metadata only, no cloning of fn).
    pub fn get_pass_ids_for_phase(&self, phase: PluginPhase) -> Vec<String> {
        let passes = self.passes.read().unwrap();
        passes
            .iter()
            .filter(|p| p.phase == phase)
            .map(|p| p.id.clone())
            .collect()
    }

    /// Check if a pass with given ID exists.
    pub fn has_pass(&self, id: &str) -> bool {
        let passes = self.passes.read().unwrap();
        passes.iter().any(|p| p.id == id)
    }

    /// Get count of registered passes.
    pub fn pass_count(&self) -> usize {
        self.passes.read().unwrap().len()
    }

    /// Clear all passes.
    pub fn clear(&self) {
        self.passes.write().unwrap().clear();
    }

    /// Execute all passes for a phase on a document.
    pub fn execute_phase(
        &self,
        phase: PluginPhase,
        document: AstDocumentSimple,
    ) -> AstDocumentSimple {
        let passes = self.passes.read().unwrap();
        let mut result = document;
        for p in passes.iter() {
            if p.phase == phase {
                result = (p.transform_fn)(result);
            }
        }
        result
    }

    /// Get all registered pass IDs.
    pub fn pass_ids(&self) -> Vec<String> {
        let passes = self.passes.read().unwrap();
        passes.iter().map(|p| p.id.clone()).collect()
    }
}

impl Default for TransformPassRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Context passed to plugins during execution.
#[derive(Debug)]
pub struct PluginContext<'a> {
    pub source_id: u32,
    pub metadata: HashMap<String, String>,
    pub compilation: Option<&'a CompilationContext>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> PluginContext<'a> {
    pub fn new() -> Self {
        Self {
            source_id: 0,
            metadata: HashMap::new(),
            compilation: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_source_id(mut self, source_id: u32) -> Self {
        self.source_id = source_id;
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for PluginContext<'static> {
    fn default() -> Self {
        Self::new()
    }
}

/// Compilation context shared with plugins.
#[derive(Debug)]
pub struct CompilationContext {
    sources: HashMap<u32, String>,
    options: CompileOptions,
}

impl CompilationContext {
    pub fn new(sources: HashMap<u32, String>, options: CompileOptions) -> Self {
        Self { sources, options }
    }

    pub fn get_source(&self, source_id: u32) -> Option<&str> {
        self.sources.get(&source_id).map(|s| s.as_str())
    }
}

/// Compile options.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub target: Option<String>,
    pub optimize: bool,
    pub debug: bool,
    pub warnings_as_errors: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            target: None,
            optimize: false,
            debug: false,
            warnings_as_errors: false,
        }
    }
}

/// Plugin manager handles discovery, loading, and execution of plugins.
pub struct PluginManager {
    plugins: RwLock<Vec<Arc<dyn Plugin>>>,
    instances: RwLock<HashMap<String, Arc<RwLock<Box<dyn PluginInstance>>>>>,
    disabled: RwLock<HashMap<String, ()>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(Vec::new()),
            instances: RwLock::new(HashMap::new()),
            disabled: RwLock::new(HashMap::new()),
        }
    }

    /// Register a plugin with the manager.
    pub fn register(&self, plugin: Arc<dyn Plugin>) -> Result<(), PluginError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|_| PluginError::InitFailed("lock poisoned".into()))?;
        plugins.push(plugin);
        Ok(())
    }

    /// Load a plugin instance by name.
    pub fn load(&self, name: &str) -> Result<Arc<RwLock<Box<dyn PluginInstance>>>, PluginError> {
        // Check if disabled
        if self.disabled.read().unwrap().contains_key(name) {
            return Err(PluginError::Disabled(name.into()));
        }

        // Check if already loaded
        if let Some(instance) = self.instances.read().unwrap().get(name) {
            return Ok(instance.clone());
        }

        // Find plugin
        let plugin = {
            let plugins = self.plugins.read().unwrap();
            plugins.iter().find(|p| p.metadata().name == name).cloned()
        };

        let plugin = plugin.ok_or_else(|| PluginError::NotFound(name.into()))?;

        // Create instance
        let instance = plugin.create_instance()?;
        let wrapped: Arc<RwLock<Box<dyn PluginInstance>>> = Arc::new(RwLock::new(instance));

        self.instances
            .write()
            .unwrap()
            .insert(name.into(), wrapped.clone());
        Ok(wrapped)
    }

    /// Unload a plugin instance by name.
    pub fn unload(&self, name: &str) {
        self.instances.write().unwrap().remove(name);
    }

    /// Disable a plugin by name.
    pub fn disable(&self, name: &str) -> Result<(), PluginError> {
        self.disabled.write().unwrap().insert(name.into(), ());
        self.unload(name);
        Ok(())
    }

    /// Enable a previously disabled plugin.
    pub fn enable(&self, name: &str) {
        self.disabled.write().unwrap().remove(name);
    }

    /// Execute plugins for a specific phase.
    pub fn execute_phase(
        &self,
        phase: PluginPhase,
        ctx: &mut PluginContext,
    ) -> Result<(), PluginError> {
        let plugins: Vec<Arc<dyn Plugin>> = self
            .plugins
            .read()
            .unwrap()
            .iter()
            .filter(|p| {
                p.metadata().lifecycle_phase == phase
                    && !self
                        .disabled
                        .read()
                        .unwrap()
                        .contains_key(&p.metadata().name)
            })
            .cloned()
            .collect();

        for plugin in plugins {
            let instance = self.load(&plugin.metadata().name)?;
            let instance_lock = instance
                .write()
                .map_err(|_| PluginError::ExecutionFailed("lock poisoned".into()))?;
            let _ = instance_lock.execute(ctx);
        }

        Ok(())
    }

    /// Get list of registered plugin names.
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins
            .read()
            .unwrap()
            .iter()
            .map(|p| p.metadata().name.clone())
            .collect()
    }

    /// Get list of loaded plugin names.
    pub fn loaded_plugins(&self) -> Vec<String> {
        self.instances.read().unwrap().keys().cloned().collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for plugin discovery.
#[derive(Debug)]
pub struct PluginConfig {
    pub plugin_dirs: Vec<String>,
    pub env_prefix: String,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            plugin_dirs: Vec::new(),
            env_prefix: "ZELIX_PLUGIN_".into(),
        }
    }
}

/// Plugin discovery result.
#[derive(Debug)]
pub struct DiscoveredPlugin {
    pub name: String,
    pub path: String,
    pub metadata: PluginMetadata,
}

/// Discover plugins from file system and environment.
pub struct PluginDiscovery {
    config: PluginConfig,
}

impl PluginDiscovery {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }

    /// Discover plugins from a directory.
    pub fn discover_from_dir(&self, dir: &str) -> Vec<DiscoveredPlugin> {
        let mut plugins = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return plugins;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Check for chimera_plugin.toml or plugin.json
                let metadata_path = path.join("chimera_plugin.toml");
                let json_path = path.join("plugin.json");

                if metadata_path.exists() {
                    if let Some(plugin) = self.load_plugin_from_toml(&metadata_path) {
                        plugins.push(plugin);
                    }
                } else if json_path.exists() {
                    if let Some(plugin) = self.load_plugin_from_json(&json_path) {
                        plugins.push(plugin);
                    }
                }
            }
        }
        plugins
    }

    /// Discover plugins from environment variables.
    pub fn discover_from_env(&self) -> Vec<DiscoveredPlugin> {
        let mut plugins = Vec::new();
        for (key, value) in std::env::vars() {
            if key.starts_with(&self.config.env_prefix) && key.len() > self.config.env_prefix.len()
            {
                let name = key[self.config.env_prefix.len()..].to_lowercase();
                // Environment variable contains path to plugin
                let path = std::path::Path::new(&value);
                if path.exists() {
                    if let Some(plugin) = self.discover_plugin_at_path(&name, path) {
                        plugins.push(plugin);
                    }
                }
            }
        }
        plugins
    }

    fn load_plugin_from_toml(&self, path: &std::path::Path) -> Option<DiscoveredPlugin> {
        let content = std::fs::read_to_string(path).ok()?;
        let name = path.parent()?.file_name()?.to_str()?.to_string();

        // Simple TOML parsing for plugin metadata
        // In production, use a TOML crate
        let mut metadata = PluginMetadata {
            name: name.clone(),
            version: "0.1.0".to_string(),
            author: "unknown".to_string(),
            description: String::new(),
            lifecycle_phase: PluginPhase::AfterParser,
            api_version: 1,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("name = ") {
                metadata.name = line.strip_prefix("name = ")?.trim_matches('"').to_string();
            } else if line.starts_with("version = ") {
                metadata.version = line
                    .strip_prefix("version = ")?
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("author = ") {
                metadata.author = line
                    .strip_prefix("author = ")?
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("description = ") {
                metadata.description = line
                    .strip_prefix("description = ")?
                    .trim_matches('"')
                    .to_string();
            }
        }

        Some(DiscoveredPlugin {
            name,
            path: path.to_str()?.to_string(),
            metadata,
        })
    }

    fn load_plugin_from_json(&self, path: &std::path::Path) -> Option<DiscoveredPlugin> {
        let content = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let name = json.get("name")?.as_str()?.to_string();
        let path_str = path.to_str()?.to_string();

        Some(DiscoveredPlugin {
            name: name.clone(),
            path: path_str,
            metadata: PluginMetadata {
                name,
                version: json
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.1.0")
                    .to_string(),
                author: json
                    .get("author")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                description: json
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                lifecycle_phase: PluginPhase::AfterParser,
                api_version: json
                    .get("api_version")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u32,
            },
        })
    }

    fn discover_plugin_at_path(
        &self,
        name: &str,
        path: &std::path::Path,
    ) -> Option<DiscoveredPlugin> {
        let metadata_path = path.join("chimera_plugin.toml");
        let json_path = path.join("plugin.json");

        if metadata_path.exists() {
            Some(DiscoveredPlugin {
                name: name.to_string(),
                path: metadata_path.to_str()?.to_string(),
                metadata: PluginMetadata {
                    name: name.to_string(),
                    version: "0.1.0".to_string(),
                    author: "unknown".to_string(),
                    description: "Discovered from environment".to_string(),
                    lifecycle_phase: PluginPhase::AfterParser,
                    api_version: 1,
                },
            })
        } else if json_path.exists() {
            self.load_plugin_from_json(&json_path).map(|mut p| {
                p.name = name.to_string();
                p
            })
        } else {
            None
        }
    }

    /// Run full discovery across all configured sources.
    pub fn discover_all(&self) -> Vec<DiscoveredPlugin> {
        let mut plugins = Vec::new();

        // Discover from directories
        for dir in &self.config.plugin_dirs {
            plugins.extend(self.discover_from_dir(dir));
        }

        // Discover from environment
        plugins.extend(self.discover_from_env());

        plugins
    }
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self::new(PluginConfig::default())
    }
}

/// Hook phase for compile-time callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookPhase {
    BeforeCompile,
    AfterCompile,
    BeforeModuleCompile,
    AfterModuleCompile,
}

impl HookPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookPhase::BeforeCompile => "before_compile",
            HookPhase::AfterCompile => "after_compile",
            HookPhase::BeforeModuleCompile => "before_module_compile",
            HookPhase::AfterModuleCompile => "after_module_compile",
        }
    }
}

/// A compile-time hook callback.
pub struct HookCallback {
    pub phase: HookPhase,
    pub callback: Box<dyn Fn(&HookContext) -> HookResult + Send + Sync>,
    pub plugin_name: String,
}

impl HookCallback {
    pub fn new(
        phase: HookPhase,
        plugin_name: impl Into<String>,
        callback: impl Fn(&HookContext) -> HookResult + 'static + Send + Sync,
    ) -> Self {
        Self {
            phase,
            callback: Box::new(callback),
            plugin_name: plugin_name.into(),
        }
    }

    pub fn execute(&self, ctx: &HookContext) -> HookResult {
        (self.callback)(ctx)
    }
}

/// Context passed to hook callbacks.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub source_id: u32,
    pub module_name: Option<String>,
    pub file_path: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl HookContext {
    pub fn new() -> Self {
        Self {
            source_id: 0,
            module_name: None,
            file_path: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_source_id(mut self, source_id: u32) -> Self {
        self.source_id = source_id;
        self
    }

    pub fn with_module_name(mut self, name: impl Into<String>) -> Self {
        self.module_name = Some(name.into());
        self
    }

    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for HookContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a hook execution.
#[derive(Debug, Clone)]
pub struct HookResult {
    pub success: bool,
    pub abort: bool,
    pub error_message: Option<String>,
}

impl HookResult {
    pub fn success() -> Self {
        Self {
            success: true,
            abort: false,
            error_message: None,
        }
    }

    pub fn aborted(reason: impl Into<String>) -> Self {
        Self {
            success: false,
            abort: true,
            error_message: Some(reason.into()),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            success: false,
            abort: false,
            error_message: Some(message.into()),
        }
    }
}

/// System for managing and executing compile-time hooks.
pub struct HookSystem {
    hooks: std::sync::RwLock<std::collections::HashMap<HookPhase, Vec<HookCallback>>>,
}

impl HookSystem {
    pub fn new() -> Self {
        Self {
            hooks: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Register a hook callback for a specific phase.
    pub fn register(&self, callback: HookCallback) -> Result<(), String> {
        let mut hooks = self
            .hooks
            .write()
            .map_err(|e| format!("lock poisoned: {}", e))?;
        hooks.entry(callback.phase).or_default().push(callback);
        Ok(())
    }

    /// Unregister all hooks for a plugin.
    pub fn unregister_plugin(&self, plugin_name: &str) {
        let mut hooks = self.hooks.write().unwrap();
        for callbacks in hooks.values_mut() {
            callbacks.retain(|c| c.plugin_name != plugin_name);
        }
    }

    /// Execute all hooks for a specific phase.
    pub fn execute_phase(&self, phase: HookPhase, ctx: &HookContext) -> Vec<HookResult> {
        let hooks = self.hooks.read().unwrap();
        let callbacks = hooks.get(&phase);

        match callbacks {
            Some(callbacks) => callbacks.iter().map(|cb| cb.execute(ctx)).collect(),
            None => Vec::new(),
        }
    }

    /// Execute all hooks for a phase and abort on first failure if configured.
    pub fn execute_phase_strict(
        &self,
        phase: HookPhase,
        ctx: &HookContext,
    ) -> Result<Vec<HookResult>, String> {
        let results = self.execute_phase(phase, ctx);
        for result in &results {
            if !result.success && result.abort {
                return Err(result
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "hook aborted".into()));
            }
        }
        Ok(results)
    }

    /// Get count of registered hooks for a phase.
    pub fn hook_count(&self, phase: HookPhase) -> usize {
        let hooks = self.hooks.read().unwrap();
        hooks.get(&phase).map(|c| c.len()).unwrap_or(0)
    }

    /// Get total hook count.
    pub fn total_hooks(&self) -> usize {
        let hooks = self.hooks.read().unwrap();
        hooks.values().map(|c| c.len()).sum()
    }

    /// Clear all hooks.
    pub fn clear(&self) {
        let mut hooks = self.hooks.write().unwrap();
        hooks.clear();
    }
}

impl Default for HookSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_config_default() {
        let config = PluginConfig::default();
        assert_eq!(config.env_prefix, "ZELIX_PLUGIN_");
        assert!(config.plugin_dirs.is_empty());
    }

    #[test]
    fn test_plugin_discovery_new() {
        let discovery = PluginDiscovery::new(PluginConfig::default());
        let plugins = discovery.discover_all();
        assert!(plugins.is_empty()); // No dirs configured
    }

    #[test]
    fn test_discovered_plugin_debug() {
        let plugin = DiscoveredPlugin {
            name: "test-plugin".to_string(),
            path: "/path/to/plugin".to_string(),
            metadata: PluginMetadata {
                name: "test-plugin".to_string(),
                version: "0.1.0".to_string(),
                author: "test".to_string(),
                description: "Test plugin".to_string(),
                lifecycle_phase: PluginPhase::AfterParser,
                api_version: 1,
            },
        };
        let debug_str = format!("{:?}", plugin);
        assert!(debug_str.contains("test-plugin"));
    }

    #[test]
    fn test_plugin_config_with_dirs() {
        let config = PluginConfig {
            plugin_dirs: vec![
                "~/.rzx/plugins".to_string(),
                "/usr/local/lib/rzx/plugins".to_string(),
            ],
            env_prefix: "RZ_".to_string(),
        };
        assert_eq!(config.plugin_dirs.len(), 2);
        assert_eq!(config.env_prefix, "RZ_");
    }

    #[test]
    fn test_hook_phase_as_str() {
        assert_eq!(HookPhase::BeforeCompile.as_str(), "before_compile");
        assert_eq!(HookPhase::AfterCompile.as_str(), "after_compile");
        assert_eq!(
            HookPhase::BeforeModuleCompile.as_str(),
            "before_module_compile"
        );
        assert_eq!(
            HookPhase::AfterModuleCompile.as_str(),
            "after_module_compile"
        );
    }

    #[test]
    fn test_hook_context_new() {
        let ctx = HookContext::new();
        assert_eq!(ctx.source_id, 0);
        assert!(ctx.module_name.is_none());
        assert!(ctx.file_path.is_none());
    }

    #[test]
    fn test_hook_context_builder() {
        let ctx = HookContext::new()
            .with_source_id(42)
            .with_module_name("MyModule")
            .with_file_path("/path/to/file.ex");
        assert_eq!(ctx.source_id, 42);
        assert_eq!(ctx.module_name.as_deref(), Some("MyModule"));
        assert_eq!(ctx.file_path.as_deref(), Some("/path/to/file.ex"));
    }

    #[test]
    fn test_hook_result_success() {
        let result = HookResult::success();
        assert!(result.success);
        assert!(!result.abort);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_hook_result_aborted() {
        let result = HookResult::aborted("module not found");
        assert!(!result.success);
        assert!(result.abort);
        assert_eq!(result.error_message.as_deref(), Some("module not found"));
    }

    #[test]
    fn test_hook_result_failed() {
        let result = HookResult::failed("compilation error");
        assert!(!result.success);
        assert!(!result.abort);
        assert_eq!(result.error_message.as_deref(), Some("compilation error"));
    }

    #[test]
    fn test_hook_system_new() {
        let system = HookSystem::new();
        assert_eq!(system.total_hooks(), 0);
    }

    #[test]
    fn test_hook_system_register() {
        let system = HookSystem::new();
        let callback = HookCallback::new(HookPhase::BeforeCompile, "test-plugin", |_| {
            HookResult::success()
        });
        system.register(callback).unwrap();
        assert_eq!(system.hook_count(HookPhase::BeforeCompile), 1);
    }

    #[test]
    fn test_hook_system_execute() {
        let system = HookSystem::new();
        let callback = HookCallback::new(HookPhase::BeforeCompile, "test-plugin", |_| {
            HookResult::success()
        });
        system.register(callback).unwrap();

        let ctx = HookContext::new().with_source_id(1);
        let results = system.execute_phase(HookPhase::BeforeCompile, &ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[test]
    fn test_hook_system_unregister_plugin() {
        let system = HookSystem::new();
        let callback = HookCallback::new(HookPhase::BeforeCompile, "test-plugin", |_| {
            HookResult::success()
        });
        system.register(callback).unwrap();
        assert_eq!(system.total_hooks(), 1);

        system.unregister_plugin("test-plugin");
        assert_eq!(system.total_hooks(), 0);
    }

    #[test]
    fn test_hook_system_clear() {
        let system = HookSystem::new();
        let callback = HookCallback::new(HookPhase::BeforeCompile, "test-plugin", |_| {
            HookResult::success()
        });
        system.register(callback).unwrap();
        system.clear();
        assert_eq!(system.total_hooks(), 0);
    }

    #[test]
    fn test_hook_system_execute_no_hooks() {
        let system = HookSystem::new();
        let ctx = HookContext::new();
        let results = system.execute_phase(HookPhase::AfterCompile, &ctx);
        assert!(results.is_empty());
    }

    #[test]
    fn test_hook_system_multiple_callbacks() {
        let system = HookSystem::new();
        let callback1 = HookCallback::new(HookPhase::BeforeCompile, "plugin1", |_| {
            HookResult::success()
        });
        let callback2 = HookCallback::new(HookPhase::BeforeCompile, "plugin2", |_| {
            HookResult::success()
        });
        system.register(callback1).unwrap();
        system.register(callback2).unwrap();

        let ctx = HookContext::new();
        let results = system.execute_phase(HookPhase::BeforeCompile, &ctx);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_custom_lint_rule_new() {
        let rule = CustomLintRule::new(
            "PLUGIN_001",
            "Test Rule",
            "A test rule",
            "best_practice",
            Severity::Warning,
            |_source_id, _file_path, _source| Vec::new(),
        );
        assert_eq!(rule.id, "PLUGIN_001");
        assert_eq!(rule.name, "Test Rule");
    }

    #[test]
    fn test_custom_lint_rule_check() {
        let rule = CustomLintRule::new(
            "PLUGIN_001",
            "Test Rule",
            "A test rule",
            "best_practice",
            Severity::Warning,
            |_source_id, _file_path, _source| vec![LintFindingSimple::new("Found issue", 1, 0, 0)],
        );
        let findings = rule.check(1, "test.ex", "source");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].message, "Found issue");
    }

    #[test]
    fn test_lint_finding_simple_new() {
        let finding = LintFindingSimple::new("Test message", 10, 5, 100);
        assert_eq!(finding.line, 10);
        assert_eq!(finding.column, 5);
        assert_eq!(finding.offset, 100);
        assert_eq!(finding.message, "Test message");
    }

    #[test]
    fn test_lint_finding_simple_with_note_and_hint() {
        let finding = LintFindingSimple::new("Test", 1, 0, 0)
            .with_note("This is a note")
            .with_hint("This is a hint");
        assert_eq!(finding.note.as_deref(), Some("This is a note"));
        assert_eq!(finding.hint.as_deref(), Some("This is a hint"));
    }

    #[test]
    fn test_custom_lint_registry_new() {
        let registry = CustomLintRegistry::new();
        assert_eq!(registry.rule_count(), 0);
    }

    #[test]
    fn test_custom_lint_registry_register() {
        let registry = CustomLintRegistry::new();
        let rule = CustomLintRule::new(
            "plugin_RULE001",
            "Test",
            "Test rule",
            "code_style",
            Severity::Warning,
            |_, _, _| Vec::new(),
        );
        registry.register(rule).unwrap();
        assert_eq!(registry.rule_count(), 1);
    }

    #[test]
    fn test_custom_lint_registry_check_all() {
        let registry = CustomLintRegistry::new();
        let rule1 = CustomLintRule::new(
            "plugin_RULE001",
            "Rule 1",
            "First rule",
            "code_style",
            Severity::Warning,
            |_, _, _| vec![LintFindingSimple::new("Finding 1", 1, 0, 0)],
        );
        let rule2 = CustomLintRule::new(
            "plugin_RULE002",
            "Rule 2",
            "Second rule",
            "code_style",
            Severity::Warning,
            |_, _, _| vec![LintFindingSimple::new("Finding 2", 2, 0, 10)],
        );
        registry.register(rule1).unwrap();
        registry.register(rule2).unwrap();

        let findings = registry.check_all(1, "test.ex", "source code");
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn test_custom_lint_registry_unregister_plugin() {
        let registry = CustomLintRegistry::new();
        let rule = CustomLintRule::new(
            "myplugin_RULE001",
            "Test",
            "Test rule",
            "code_style",
            Severity::Warning,
            |_, _, _| Vec::new(),
        );
        registry.register(rule).unwrap();
        assert_eq!(registry.rule_count(), 1);

        registry.unregister_plugin("myplugin");
        assert_eq!(registry.rule_count(), 0);
    }

    #[test]
    fn test_custom_lint_registry_clear() {
        let registry = CustomLintRegistry::new();
        let rule = CustomLintRule::new(
            "plugin_RULE001",
            "Test",
            "Test rule",
            "code_style",
            Severity::Warning,
            |_, _, _| Vec::new(),
        );
        registry.register(rule).unwrap();
        assert_eq!(registry.rule_count(), 1);

        registry.clear();
        assert_eq!(registry.rule_count(), 0);
    }

    #[test]
    fn test_transform_pass_new() {
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        assert_eq!(pass.id, "plugin_PASS001");
        assert_eq!(pass.phase, PluginPhase::AfterParser);
    }

    #[test]
    fn test_transform_pass_execute() {
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |mut doc: AstDocumentSimple| {
                doc.source_id = 42;
                doc
            },
        );
        let input = AstDocumentSimple::new(1);
        let output = pass.execute(input);
        assert_eq!(output.source_id, 42);
    }

    #[test]
    fn test_transform_pass_registry_new() {
        let registry = TransformPassRegistry::new();
        assert_eq!(registry.pass_count(), 0);
    }

    #[test]
    fn test_transform_pass_registry_register() {
        let registry = TransformPassRegistry::new();
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        registry.register(pass).unwrap();
        assert_eq!(registry.pass_count(), 1);
    }

    #[test]
    fn test_transform_pass_registry_execute_phase() {
        let registry = TransformPassRegistry::new();
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |mut doc: AstDocumentSimple| {
                doc.source_id = 100;
                doc
            },
        );
        registry.register(pass).unwrap();

        let input = AstDocumentSimple::new(1);
        let output = registry.execute_phase(PluginPhase::AfterParser, input);
        assert_eq!(output.source_id, 100);
    }

    #[test]
    fn test_transform_pass_registry_execute_phase_no_passes() {
        let registry = TransformPassRegistry::new();
        let input = AstDocumentSimple::new(1);
        let output = registry.execute_phase(PluginPhase::AfterParser, input);
        assert_eq!(output.source_id, 1); // Unchanged
    }

    #[test]
    fn test_transform_pass_registry_pass_ids() {
        let registry = TransformPassRegistry::new();
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        registry.register(pass).unwrap();

        let ids = registry.pass_ids();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], "plugin_PASS001");
    }

    #[test]
    fn test_transform_pass_registry_has_pass() {
        let registry = TransformPassRegistry::new();
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        registry.register(pass).unwrap();

        assert!(registry.has_pass("plugin_PASS001"));
        assert!(!registry.has_pass("nonexistent"));
    }

    #[test]
    fn test_transform_pass_registry_unregister_plugin() {
        let registry = TransformPassRegistry::new();
        let pass = TransformPass::new(
            "myplugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        registry.register(pass).unwrap();
        assert_eq!(registry.pass_count(), 1);

        registry.unregister_plugin("myplugin");
        assert_eq!(registry.pass_count(), 0);
    }

    #[test]
    fn test_transform_pass_registry_clear() {
        let registry = TransformPassRegistry::new();
        let pass = TransformPass::new(
            "plugin_PASS001",
            "Test Pass",
            "A test transform pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        registry.register(pass).unwrap();
        assert_eq!(registry.pass_count(), 1);

        registry.clear();
        assert_eq!(registry.pass_count(), 0);
    }

    #[test]
    fn test_transform_pass_registry_get_pass_ids_for_phase() {
        let registry = TransformPassRegistry::new();

        let pass1 = TransformPass::new(
            "plugin_PASS001",
            "Pass 1",
            "First pass",
            PluginPhase::AfterParser,
            |doc| doc,
        );
        let pass2 = TransformPass::new(
            "plugin_PASS002",
            "Pass 2",
            "Second pass",
            PluginPhase::AfterSemantic,
            |doc| doc,
        );
        registry.register(pass1).unwrap();
        registry.register(pass2).unwrap();

        let parser_passes = registry.get_pass_ids_for_phase(PluginPhase::AfterParser);
        assert_eq!(parser_passes.len(), 1);
        assert_eq!(parser_passes[0], "plugin_PASS001");
    }

    #[test]
    fn test_ast_document_simple_new() {
        let doc = AstDocumentSimple::new(42);
        assert_eq!(doc.source_id, 42);
        assert!(doc.root_id.is_none());
        assert!(doc.nodes.is_empty());
    }

    #[test]
    fn test_ast_document_simple_with_nodes() {
        let nodes = vec![
            AstNodeSimple::new(1, "module", "defmodule Foo", 0, 14),
            AstNodeSimple::new(2, "function", "def foo", 15, 23),
        ];
        let doc = AstDocumentSimple::new(1).with_nodes(nodes);
        assert_eq!(doc.nodes.len(), 2);
    }

    #[test]
    fn test_ast_document_simple_set_root() {
        let mut doc = AstDocumentSimple::new(1);
        doc.set_root(42);
        assert_eq!(doc.root_id, Some(42));
    }

    #[test]
    fn test_ast_node_simple_new() {
        let node = AstNodeSimple::new(1, "module", "defmodule Foo", 0, 14);
        assert_eq!(node.id, 1);
        assert_eq!(node.kind, "module");
        assert_eq!(node.start_offset, 0);
        assert_eq!(node.end_offset, 14);
    }

    #[test]
    fn test_ast_node_simple_with_children() {
        let node =
            AstNodeSimple::new(1, "module", "defmodule Foo", 0, 14).with_children(vec![2, 3, 4]);
        assert_eq!(node.children.len(), 3);
    }
}
