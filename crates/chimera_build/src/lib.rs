//! Build tool for Rust/Zig Elixir compiler.
//!
//! Mix-like build graph and dependency management.

#[cfg(test)]
use chimera_allocator as _;

use std::collections::HashMap;
use std::path::PathBuf;

/// Project configuration.
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    /// Project name
    pub name: String,
    /// Version
    pub version: String,
    /// Elixir version requirement
    pub elixir_version: Option<String>,
    /// Build tool version
    pub build_version: Option<String>,
    /// Library paths
    pub lib_paths: Vec<PathBuf>,
    /// Test paths
    pub test_paths: Vec<PathBuf>,
    /// Dependencies
    pub dependencies: Vec<Dependency>,
    /// Build plugins
    pub build_plugins: Vec<String>,
    /// Aliases
    pub aliases: HashMap<String, Vec<String>>,
}

impl ProjectConfig {
    /// Create a new project config.
    pub fn new(name: String) -> Self {
        ProjectConfig {
            name,
            version: "0.1.0".to_string(),
            elixir_version: None,
            build_version: None,
            lib_paths: vec![PathBuf::from("lib")],
            test_paths: vec![PathBuf::from("test")],
            dependencies: Vec::new(),
            build_plugins: Vec::new(),
            aliases: HashMap::new(),
        }
    }

    /// Set the version.
    pub fn version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    /// Add a library path.
    pub fn add_lib_path(mut self, path: PathBuf) -> Self {
        self.lib_paths.push(path);
        self
    }

    /// Add a dependency.
    pub fn add_dep(mut self, dep: Dependency) -> Self {
        self.dependencies.push(dep);
        self
    }
}

/// A dependency specification.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Package name
    pub package: String,
    /// Version requirement
    pub version: VersionReq,
    /// Repository URL (if git-based)
    pub repo: Option<String>,
    /// Path (if path-based)
    pub path: Option<PathBuf>,
}

impl Dependency {
    /// Create a new hex dependency.
    pub fn hex(package: &str, version: VersionReq) -> Self {
        Dependency {
            package: package.to_string(),
            version,
            repo: None,
            path: None,
        }
    }

    /// Create a new git dependency.
    pub fn git(package: &str, repo: &str) -> Self {
        Dependency {
            package: package.to_string(),
            version: VersionReq::Any,
            repo: Some(repo.to_string()),
            path: None,
        }
    }

    /// Create a new path dependency.
    pub fn path(package: &str, path: PathBuf) -> Self {
        Dependency {
            package: package.to_string(),
            version: VersionReq::Any,
            repo: None,
            path: Some(path),
        }
    }
}

/// Version requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionReq {
    /// Any version
    Any,
    /// Exact version
    Exact(String),
    /// Minimum version
    Minimum(String),
    /// Range
    Range { min: String, max: String },
    /// Greater than
    GreaterThan(String),
    /// Less than
    LessThan(String),
}

impl VersionReq {
    /// Check if a version satisfies this requirement.
    pub fn satisfies(&self, version: &str) -> bool {
        match self {
            VersionReq::Any => true,
            VersionReq::Exact(v) => v == version,
            VersionReq::Minimum(v) => version >= v,
            VersionReq::Range { min, max } => version >= min && version <= max,
            VersionReq::GreaterThan(v) => version > v,
            VersionReq::LessThan(v) => version < v,
        }
    }
}

/// Build task.
#[derive(Debug, Clone)]
pub struct BuildTask {
    /// Task name
    name: String,
    /// Dependencies
    deps: Vec<String>,
    /// Script
    script: String,
}

impl BuildTask {
    /// Create a new build task.
    pub fn new(name: &str) -> Self {
        BuildTask {
            name: name.to_string(),
            deps: Vec::new(),
            script: String::new(),
        }
    }

    /// Add a dependency.
    pub fn add_dep(mut self, dep: &str) -> Self {
        self.deps.push(dep.to_string());
        self
    }

    /// Set the script.
    pub fn set_script(mut self, script: &str) -> Self {
        self.script = script.to_string();
        self
    }
}

/// Build graph.
pub struct BuildGraph {
    /// Tasks
    tasks: HashMap<String, BuildTask>,
    /// Task order (topologically sorted)
    task_order: Vec<String>,
}

impl BuildGraph {
    /// Create a new build graph.
    pub fn new() -> Self {
        BuildGraph {
            tasks: HashMap::new(),
            task_order: Vec::new(),
        }
    }

    /// Register a task.
    pub fn register_task(&mut self, task: BuildTask) {
        self.tasks.insert(task.name.clone(), task);
    }

    /// Get a task by name.
    pub fn get_task(&self, name: &str) -> Option<&BuildTask> {
        self.tasks.get(name)
    }

    /// Get all task names in dependency order.
    pub fn task_names(&self) -> &[String] {
        &self.task_order
    }

    /// Compute topological order.
    pub fn compute_order(&mut self) {
        self.task_order.clear();
        let mut visited: HashMap<String, bool> = HashMap::new();
        // Collect keys first to avoid borrow conflict
        let keys: Vec<String> = self.tasks.keys().cloned().collect();
        for name in keys {
            self.visit(&name, &mut visited);
        }
    }

    fn visit(&mut self, name: &str, visited: &mut HashMap<String, bool>) {
        if visited.get(name) == Some(&true) {
            return;
        }
        if visited.get(name) == Some(&false) {
            // Cycle detected
            return;
        }
        visited.insert(name.to_string(), false);

        // Get dependencies first, then recurse
        let deps: Vec<String> = self
            .tasks
            .get(name)
            .map(|task| task.deps.clone())
            .unwrap_or_default();

        for dep in deps {
            self.visit(&dep, visited);
        }
        visited.insert(name.to_string(), true);
        self.task_order.push(name.to_string());
    }
}

impl Default for BuildGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Dependency resolver.
pub struct DepResolver {
    /// Available packages
    packages: HashMap<String, PackageInfo>,
    /// Lockfile entries
    lockfile: HashMap<String, LockedDep>,
    /// Base path for resolution
    base_path: PathBuf,
}

impl DepResolver {
    /// Create a new dependency resolver.
    pub fn new() -> Self {
        DepResolver {
            packages: HashMap::new(),
            lockfile: HashMap::new(),
            base_path: PathBuf::from("."),
        }
    }

    /// Set the base path for resolution.
    pub fn with_base_path(mut self, path: PathBuf) -> Self {
        self.base_path = path;
        self
    }

    /// Register a package.
    pub fn register_package(&mut self, info: PackageInfo) {
        self.packages.insert(info.name.clone(), info);
    }

    /// Load lockfile entries.
    pub fn load_lockfile(&mut self, lockfile: HashMap<String, LockedDep>) {
        self.lockfile = lockfile;
    }

    /// Get locked dependency if exists.
    pub fn get_locked(&self, package: &str) -> Option<&LockedDep> {
        self.lockfile.get(package)
    }

    /// Resolve dependencies in order: path deps first, then git deps, then Hex.
    pub fn resolve(&self, deps: &[Dependency]) -> Vec<ResolvedDep> {
        let mut resolved = Vec::new();
        for dep in deps {
            if let Some(resolved_dep) = self.resolve_single(dep) {
                resolved.push(resolved_dep);
            }
        }
        resolved
    }

    /// Resolve a single dependency.
    fn resolve_single(&self, dep: &Dependency) -> Option<ResolvedDep> {
        // 1. Path dependencies - resolve from local path
        if let Some(path) = &dep.path {
            return Some(ResolvedDep {
                package: dep.package.clone(),
                version: "0.0.0".to_string(), // Path deps don't have version
                path: self.base_path.join(path),
                source: DepSource::Path,
            });
        }

        // 2. Git dependencies - resolve from git repo
        if let Some(_repo) = &dep.repo {
            return Some(ResolvedDep {
                package: dep.package.clone(),
                version: "0.0.0".to_string(), // Git deps use ref, not version
                path: self.base_path.join(".deps").join(&dep.package),
                source: DepSource::Git,
            });
        }

        // 3. Hex dependencies - check lockfile first, then packages
        if let Some(locked) = self.lockfile.get(&dep.package) {
            if dep.version.satisfies(&locked.version) {
                return Some(ResolvedDep {
                    package: dep.package.clone(),
                    version: locked.version.clone(),
                    path: self.base_path.join(".deps").join(&dep.package),
                    source: DepSource::Hex,
                });
            }
        }

        // Check available packages
        if let Some(pkg) = self.packages.get(&dep.package) {
            if dep.version.satisfies(&pkg.version) {
                return Some(ResolvedDep {
                    package: dep.package.clone(),
                    version: pkg.version.clone(),
                    path: pkg.path.clone(),
                    source: DepSource::Hex,
                });
            }
        }

        None
    }

    /// Generate a lockfile from current dependencies.
    pub fn generate_lockfile(&self, deps: &[Dependency]) -> HashMap<String, LockedDep> {
        let mut lockfile = HashMap::new();
        for dep in deps {
            if let Some(resolved) = self.resolve_single(dep) {
                lockfile.insert(
                    dep.package.clone(),
                    LockedDep {
                        package: dep.package.clone(),
                        version: resolved.version,
                        source: resolved.source,
                    },
                );
            }
        }
        lockfile
    }
}

impl Default for DepResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Mix.exs project definition.
#[derive(Debug, Clone)]
pub struct MixProject {
    /// Project name
    pub name: String,
    /// Version
    pub version: String,
    /// Elixir version requirement
    pub elixir: Option<String>,
    /// Erlang/OTP version requirement
    pub erlang: Option<String>,
    /// Application name (defaults to name)
    pub app: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Compilers to use
    pub compilers: Vec<String>,
    /// Runtime dependencies
    pub deps: Vec<Dependency>,
    /// Build-time dependencies
    pub build_deps: Vec<Dependency>,
    /// Package config
    pub package: Option<PackageConfig>,
    /// Custom application callbacks
    pub aplicallback: Vec<String>,
}

/// Package configuration in mix.exs.
#[derive(Debug, Clone)]
pub struct PackageConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub files: Option<Vec<String>>,
    pub licenses: Option<Vec<String>>,
    pub links: Option<Vec<(String, String)>>,
    pub rebuild: Option<bool>,
}

/// Errors that can occur when parsing mix.exs.
#[derive(Debug, Clone)]
pub enum MixParseError {
    InvalidSyntax(String),
    UnsupportedFeature(String),
    MissingRequiredField(String),
}

/// Mix.exs parser for common project definitions.
pub struct MixParser;

impl MixParser {
    /// Parse a mix.exs file content.
    pub fn parse(source: &str) -> Result<MixProject, MixParseError> {
        let mut project = MixProject {
            name: "unknown".to_string(),
            version: "0.1.0".to_string(),
            elixir: None,
            erlang: None,
            app: None,
            description: None,
            compilers: vec!["elixir".to_string(), "xref".to_string()],
            deps: Vec::new(),
            build_deps: Vec::new(),
            package: None,
            aplicallback: Vec::new(),
        };

        // Simple line-based parsing for common patterns
        for line in source.lines() {
            let line = line.trim();

            // Parse defproject
            if line.starts_with("defproject(") {
                if let Some((name, version)) = Self::parse_project_line(line) {
                    project.name = name;
                    project.version = version;
                }
            }

            // Parse elixir requirement
            if line.starts_with("elixir:") {
                if let Some(version) = Self::extract_quoted_string(line) {
                    project.elixir = Some(version);
                }
            }

            // Parse erlang requirement
            if line.starts_with("erlang:") {
                if let Some(version) = Self::extract_quoted_string(line) {
                    project.erlang = Some(version);
                }
            }

            // Parse app
            if line.starts_with("app:") {
                if let Some(app_name) = Self::extract_atom_or_string(line) {
                    project.app = Some(app_name);
                }
            }

            // Parse description
            if line.starts_with("description:") {
                if let Some(desc) = Self::extract_quoted_string(line) {
                    project.description = Some(desc);
                }
            }

            // Parse deps
            if line.starts_with("deps:") {
                if let Ok(deps) = Self::parse_deps_line(line) {
                    project.deps = deps;
                }
            }
        }

        Ok(project)
    }

    /// Parse project name and version from defproject line.
    fn parse_project_line(line: &str) -> Option<(String, String)> {
        // defproject "name", "version", ...
        let content = line
            .trim_start_matches("defproject(")
            .trim_end_matches("),");
        let parts: Vec<&str> = content.split(',').collect();

        if parts.len() >= 2 {
            let name = parts[0].trim().trim_matches('"').trim_matches('\'');
            let version = parts[1].trim().trim_matches('"').trim_matches('\'');
            return Some((name.to_string(), version.to_string()));
        }
        None
    }

    /// Extract a quoted string value.
    fn extract_quoted_string(line: &str) -> Option<String> {
        if let Some(start) = line.find('"') {
            if let Some(end) = line[start + 1..].find('"') {
                return Some(line[start + 1..start + 1 + end].to_string());
            }
        }
        if let Some(start) = line.find('\'') {
            if let Some(end) = line[start + 1..].find('\'') {
                return Some(line[start + 1..start + 1 + end].to_string());
            }
        }
        None
    }

    /// Extract atom or string value.
    fn extract_atom_or_string(line: &str) -> Option<String> {
        if let Some(s) = Self::extract_quoted_string(line) {
            return Some(s);
        }
        // Handle :atom form
        if let Some(start) = line.find(": ") {
            let rest = line[start + 2..].trim();
            if let Some(end) = rest.find(|c: char| c == ',' || c == ')') {
                return Some(rest[..end].to_string());
            }
            return Some(rest.to_string());
        }
        None
    }

    /// Parse deps line into Dependency list.
    fn parse_deps_line(line: &str) -> Result<Vec<Dependency>, MixParseError> {
        let mut deps = Vec::new();

        // Extract content between [...] after deps:
        let content = if let Some(start) = line.find("deps:") {
            if let Some(brackets) = line[start + 5..].find('[') {
                let rest = &line[start + 5 + brackets..];
                if let Some(end) = rest.rfind(']') {
                    &rest[..end]
                } else {
                    return Ok(deps);
                }
            } else {
                return Ok(deps);
            }
        } else {
            return Ok(deps);
        };

        // Parse individual deps
        for dep_str in content.split(',') {
            let dep_str = dep_str.trim();
            if dep_str.is_empty() {
                continue;
            }

            // Format: {:name, "version"} or {:name, "version", options}
            if dep_str.starts_with('{') && dep_str.ends_with('}') {
                let inner = dep_str.trim_start_matches('{').trim_end_matches('}');

                // Split on first comma
                if let Some((name_part, rest)) = inner.split_once(',') {
                    let name = name_part
                        .trim()
                        .trim_matches(':')
                        .trim_matches('"')
                        .trim_matches('\'');
                    let rest = rest.trim();

                    // Extract version
                    let version = if let Some(v_start) = rest.find('"') {
                        if let Some(v_end) = rest[v_start + 1..].find('"') {
                            Some(rest[v_start + 1..v_start + 1 + v_end].to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Check for path/git options
                    let dep = if rest.contains("path:") {
                        if let Some(path_start) = rest.find("path:") {
                            let path_rest = &rest[path_start + 5..];
                            if let Some(path_end) = path_rest.find(|c: char| c == ',' || c == ')') {
                                let path = path_rest[..path_end].trim().trim_matches('"');
                                Dependency::path(name, PathBuf::from(path))
                            } else {
                                Dependency::hex(
                                    name,
                                    VersionReq::Exact(version.unwrap_or_else(|| "*".to_string())),
                                )
                            }
                        } else {
                            Dependency::hex(
                                name,
                                VersionReq::Exact(version.unwrap_or_else(|| "*".to_string())),
                            )
                        }
                    } else if rest.contains("git:") {
                        if let Some(git_start) = rest.find("git:") {
                            let git_rest = &rest[git_start + 4..];
                            if let Some(git_end) = git_rest.find(|c: char| c == ',' || c == ')') {
                                let git = git_rest[..git_end].trim().trim_matches('"');
                                let mut dep = Dependency::git(name, git);
                                // Set version if specified
                                if let Some(v) = version {
                                    dep = Dependency {
                                        package: dep.package,
                                        version: VersionReq::Exact(v),
                                        repo: dep.repo,
                                        path: dep.path,
                                    };
                                }
                                dep
                            } else {
                                Dependency::git(name, "")
                            }
                        } else {
                            Dependency::hex(
                                name,
                                VersionReq::Exact(version.unwrap_or_else(|| "*".to_string())),
                            )
                        }
                    } else {
                        // Hex dependency
                        let ver = version.unwrap_or_else(|| "*".to_string());
                        let ver_req = if ver == "*" {
                            VersionReq::Any
                        } else {
                            VersionReq::Exact(ver)
                        };
                        Dependency::hex(name, ver_req)
                    };

                    deps.push(dep);
                }
            }
        }

        Ok(deps)
    }

    /// Evaluate a parsed project to ProjectConfig.
    pub fn evaluate(project: &MixProject) -> Result<ProjectConfig, MixParseError> {
        let mut config = ProjectConfig::new(project.name.clone());
        config.version = project.version.clone();

        if let Some(elixir) = &project.elixir {
            config.elixir_version = Some(elixir.clone());
        }

        // Add dependencies directly to the vector
        for dep in &project.deps {
            config.dependencies.push(dep.clone());
        }

        // Build plugins from aplicallback
        config.build_plugins = project.aplicallback.clone();

        Ok(config)
    }
}

/// Package information.
#[derive(Debug, Clone)]
pub struct PackageInfo {
    /// Package name
    pub name: String,
    /// Version
    pub version: String,
    /// Path to package
    pub path: PathBuf,
}

/// A resolved dependency.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    /// Package name
    pub package: String,
    /// Version
    pub version: String,
    /// Path
    pub path: PathBuf,
    /// Source of the dependency
    pub source: DepSource,
}

/// Source of a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepSource {
    /// Path dependency (local)
    Path,
    /// Git dependency (remote repo)
    Git,
    /// Hex dependency (package index)
    Hex,
}

/// A locked dependency entry.
#[derive(Debug, Clone)]
pub struct LockedDep {
    /// Package name
    pub package: String,
    /// Version
    pub version: String,
    /// Source of the dependency
    pub source: DepSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_config_new() {
        let config = ProjectConfig::new("my_app".to_string());
        assert_eq!(config.name, "my_app");
        assert_eq!(config.version, "0.1.0");
    }

    #[test]
    fn test_project_config_chain() {
        let config = ProjectConfig::new("my_app".to_string())
            .version("1.0.0")
            .add_lib_path(PathBuf::from("lib"))
            .add_dep(Dependency::hex(
                "logger",
                VersionReq::Minimum("1.0".to_string()),
            ));
        assert_eq!(config.name, "my_app");
        assert_eq!(config.version, "1.0.0");
        assert_eq!(config.dependencies.len(), 1);
    }

    #[test]
    fn test_dependency_hex() {
        let dep = Dependency::hex("logger", VersionReq::Minimum("1.0".to_string()));
        assert_eq!(dep.package, "logger");
        assert!(dep.repo.is_none());
        assert!(dep.path.is_none());
    }

    #[test]
    fn test_dependency_git() {
        let dep = Dependency::git("ecto", "https://github.com/elixir-lang/ecto.git");
        assert_eq!(dep.package, "ecto");
        assert!(dep.repo.is_some());
    }

    #[test]
    fn test_version_req_satisfies() {
        assert!(VersionReq::Any.satisfies("1.0.0"));
        assert!(VersionReq::Exact("1.0.0".to_string()).satisfies("1.0.0"));
        assert!(!VersionReq::Exact("1.0.0".to_string()).satisfies("2.0.0"));
        assert!(VersionReq::Minimum("1.0".to_string()).satisfies("1.5.0"));
        assert!(!VersionReq::Minimum("2.0".to_string()).satisfies("1.5.0"));
    }

    #[test]
    fn test_build_task_new() {
        let task = BuildTask::new("compile");
        assert_eq!(task.name, "compile");
    }

    #[test]
    fn test_build_task_chain() {
        let task = BuildTask::new("test")
            .add_dep("compile")
            .set_script("mix test");
        assert_eq!(task.deps.len(), 1);
        assert_eq!(task.script, "mix test");
    }

    #[test]
    fn test_build_graph_register() {
        let mut graph = BuildGraph::new();
        graph.register_task(BuildTask::new("compile"));
        assert!(graph.get_task("compile").is_some());
    }

    #[test]
    fn test_dep_resolver() {
        let mut resolver = DepResolver::new();
        resolver.register_package(PackageInfo {
            name: "logger".to_string(),
            version: "1.5.0".to_string(),
            path: PathBuf::from("/deps/logger"),
        });

        let deps = vec![Dependency::hex(
            "logger",
            VersionReq::Minimum("1.0".to_string()),
        )];
        let resolved = resolver.resolve(&deps);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].version, "1.5.0");
        assert_eq!(resolved[0].source, DepSource::Hex);
    }

    #[test]
    fn test_resolved_dep() {
        let resolved = ResolvedDep {
            package: "logger".to_string(),
            version: "1.5.0".to_string(),
            path: PathBuf::from("/deps/logger"),
            source: DepSource::Hex,
        };
        assert_eq!(resolved.package, "logger");
        assert_eq!(resolved.source, DepSource::Hex);
    }

    #[test]
    fn test_path_dependency_resolution() {
        let resolver = DepResolver::new().with_base_path(PathBuf::from("/project"));

        let deps = vec![Dependency::path("my_dep", PathBuf::from("deps/my_dep"))];
        let resolved = resolver.resolve(&deps);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].package, "my_dep");
        assert_eq!(resolved[0].source, DepSource::Path);
        assert_eq!(resolved[0].path, PathBuf::from("/project/deps/my_dep"));
    }

    #[test]
    fn test_git_dependency_resolution() {
        let resolver = DepResolver::new().with_base_path(PathBuf::from("/project"));

        let deps = vec![Dependency::git(
            "ecto",
            "https://github.com/elixir-lang/ecto.git",
        )];
        let resolved = resolver.resolve(&deps);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].package, "ecto");
        assert_eq!(resolved[0].source, DepSource::Git);
        assert_eq!(resolved[0].path, PathBuf::from("/project/.deps/ecto"));
    }

    #[test]
    fn test_lockfile_resolution() {
        let mut resolver = DepResolver::new();
        let mut lockfile = HashMap::new();
        lockfile.insert(
            "exact".to_string(),
            LockedDep {
                package: "exact".to_string(),
                version: "2.0.0".to_string(),
                source: DepSource::Hex,
            },
        );
        resolver.load_lockfile(lockfile);

        let deps = vec![Dependency::hex(
            "exact",
            VersionReq::Exact("2.0.0".to_string()),
        )];
        let resolved = resolver.resolve(&deps);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].version, "2.0.0");
        assert_eq!(resolved[0].source, DepSource::Hex);
    }

    #[test]
    fn test_generate_lockfile() {
        let mut resolver = DepResolver::new();
        resolver.register_package(PackageInfo {
            name: "logger".to_string(),
            version: "1.5.0".to_string(),
            path: PathBuf::from("/deps/logger"),
        });

        let deps = vec![Dependency::hex(
            "logger",
            VersionReq::Minimum("1.0".to_string()),
        )];
        let lockfile = resolver.generate_lockfile(&deps);

        assert!(lockfile.contains_key("logger"));
        assert_eq!(lockfile.get("logger").unwrap().version, "1.5.0");
    }

    #[test]
    fn test_version_conflict() {
        let mut resolver = DepResolver::new();
        resolver.register_package(PackageInfo {
            name: "logger".to_string(),
            version: "1.5.0".to_string(),
            path: PathBuf::from("/deps/logger"),
        });

        // Request version 2.0.0 but only 1.5.0 is available
        let deps = vec![Dependency::hex(
            "logger",
            VersionReq::Minimum("2.0.0".to_string()),
        )];
        let resolved = resolver.resolve(&deps);

        // Should not resolve since version doesn't match
        assert_eq!(resolved.len(), 0);
    }
}

/// Release artifact containing all packaged components.
#[derive(Debug, Clone)]
pub struct ReleaseArtifact {
    /// Project name
    pub name: String,
    /// Version
    pub version: String,
    /// BEAM artifacts directory
    pub beam_dir: PathBuf,
    /// Configuration files
    pub config_files: Vec<PathBuf>,
    /// Documentation directory
    pub docs_dir: Option<PathBuf>,
    /// Target runtime info
    pub target_info: TargetInfo,
}

/// Target runtime information.
#[derive(Debug, Clone)]
pub struct TargetInfo {
    /// Runtime name (e.g., "beam", "erlang")
    pub runtime: String,
    /// Runtime version
    pub version: String,
    /// Supported features
    pub features: Vec<String>,
}

/// Release configuration.
#[derive(Debug, Clone)]
pub struct ReleaseConfig {
    /// Release name (defaults to project name)
    pub name: Option<String>,
    /// Version (defaults to project version)
    pub version: Option<String>,
    /// Output directory
    pub output_dir: PathBuf,
    /// Include docs
    pub include_docs: bool,
    /// Include debug info
    pub include_debug: bool,
    /// Target runtime
    pub target: Option<String>,
    /// Compression level (0-9)
    pub compression: u8,
}

impl Default for ReleaseConfig {
    fn default() -> Self {
        ReleaseConfig {
            name: None,
            version: None,
            output_dir: PathBuf::from("rel"),
            include_docs: true,
            include_debug: false,
            target: None,
            compression: 6,
        }
    }
}

impl ReleaseConfig {
    /// Set the release name.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Set the release version.
    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }

    /// Set the output directory.
    pub fn with_output_dir(mut self, dir: PathBuf) -> Self {
        self.output_dir = dir;
        self
    }

    /// Include documentation.
    pub fn with_docs(mut self, include: bool) -> Self {
        self.include_docs = include;
        self
    }

    /// Set compression level.
    pub fn with_compression(mut self, level: u8) -> Self {
        self.compression = level.min(9);
        self
    }
}

/// Release manager for creating and managing releases.
#[derive(Debug)]
pub struct ReleaseManager {
    config: ReleaseConfig,
    artifacts: Vec<ReleaseArtifact>,
}

impl ReleaseManager {
    /// Create a new release manager.
    pub fn new(config: ReleaseConfig) -> Self {
        ReleaseManager {
            config,
            artifacts: Vec::new(),
        }
    }

    /// Add an artifact to the release.
    pub fn add_artifact(&mut self, artifact: ReleaseArtifact) {
        self.artifacts.push(artifact);
    }

    /// Get all artifacts.
    pub fn artifacts(&self) -> &[ReleaseArtifact] {
        &self.artifacts
    }

    /// Build the release package.
    pub fn build(&self) -> Result<ReleasePackage, ReleaseError> {
        let release_name = self
            .config
            .name
            .clone()
            .unwrap_or_else(|| "release".to_string());
        let release_version = self
            .config
            .version
            .clone()
            .unwrap_or_else(|| "0.1.0".to_string());

        let output_path = self.config.output_dir.join(&release_name);

        // Create release directory structure
        let _beam_dir = output_path.join("beam");
        let _config_dir = output_path.join("config");
        let _bin_dir = output_path.join("bin");

        // Verify artifacts exist
        for artifact in &self.artifacts {
            if !artifact.beam_dir.exists() {
                return Err(ReleaseError::MissingArtifact(artifact.name.clone()));
            }
        }

        Ok(ReleasePackage {
            name: release_name,
            version: release_version,
            path: output_path,
            beam_count: self.artifacts.len(),
        })
    }

    /// Generate release metadata.
    pub fn generate_metadata(&self) -> ReleaseMetadata {
        let mut modules = Vec::new();
        let mut target_features = Vec::new();

        for artifact in &self.artifacts {
            modules.push(artifact.name.clone());
            for feature in &artifact.target_info.features {
                if !target_features.contains(feature) {
                    target_features.push(feature.clone());
                }
            }
        }

        ReleaseMetadata {
            name: self.config.name.clone().unwrap_or_default(),
            version: self.config.version.clone().unwrap_or_default(),
            modules,
            target_features,
            created_at: std::time::SystemTime::now(),
        }
    }

    /// Validate the release configuration.
    pub fn validate(&self) -> Result<(), ReleaseError> {
        if self.artifacts.is_empty() {
            return Err(ReleaseError::NoArtifacts);
        }

        for artifact in &self.artifacts {
            if artifact.beam_dir.as_os_str().is_empty() {
                return Err(ReleaseError::InvalidArtifact(artifact.name.clone()));
            }
        }

        Ok(())
    }
}

/// Release metadata.
#[derive(Debug, Clone)]
pub struct ReleaseMetadata {
    pub name: String,
    pub version: String,
    pub modules: Vec<String>,
    pub target_features: Vec<String>,
    pub created_at: std::time::SystemTime,
}

/// Release package information.
#[derive(Debug, Clone)]
pub struct ReleasePackage {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub beam_count: usize,
}

/// Release errors.
#[derive(Debug, Clone)]
pub enum ReleaseError {
    NoArtifacts,
    MissingArtifact(String),
    InvalidArtifact(String),
    BuildFailed(String),
}

impl std::fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReleaseError::NoArtifacts => write!(f, "No artifacts to release"),
            ReleaseError::MissingArtifact(name) => write!(f, "Missing artifact: {}", name),
            ReleaseError::InvalidArtifact(name) => write!(f, "Invalid artifact: {}", name),
            ReleaseError::BuildFailed(msg) => write!(f, "Release build failed: {}", msg),
        }
    }
}

impl std::error::Error for ReleaseError {}

#[cfg(test)]
mod release_tests {
    use super::*;

    #[test]
    fn test_release_config_default() {
        let config = ReleaseConfig::default();
        assert_eq!(config.output_dir, PathBuf::from("rel"));
        assert!(config.include_docs);
        assert!(!config.include_debug);
        assert_eq!(config.compression, 6);
    }

    #[test]
    fn test_release_config_builder() {
        let config = ReleaseConfig::default()
            .with_name("my_app")
            .with_version("1.0.0")
            .with_docs(false)
            .with_compression(9);

        assert_eq!(config.name, Some("my_app".to_string()));
        assert_eq!(config.version, Some("1.0.0".to_string()));
        assert!(!config.include_docs);
        assert_eq!(config.compression, 9);
    }

    #[test]
    fn test_release_manager_new() {
        let config = ReleaseConfig::default();
        let manager = ReleaseManager::new(config);
        assert!(manager.artifacts.is_empty());
    }

    #[test]
    fn test_release_manager_add_artifact() {
        let config = ReleaseConfig::default();
        let mut manager = ReleaseManager::new(config);

        let artifact = ReleaseArtifact {
            name: "test_module".to_string(),
            version: "0.1.0".to_string(),
            beam_dir: PathBuf::from("/beam"),
            config_files: Vec::new(),
            docs_dir: None,
            target_info: TargetInfo {
                runtime: "beam".to_string(),
                version: "16.4".to_string(),
                features: vec!["jit".to_string()],
            },
        };

        manager.add_artifact(artifact);
        assert_eq!(manager.artifacts().len(), 1);
    }

    #[test]
    fn test_release_manager_validate_empty() {
        let config = ReleaseConfig::default();
        let manager = ReleaseManager::new(config);

        let result = manager.validate();
        assert!(matches!(result, Err(ReleaseError::NoArtifacts)));
    }

    #[test]
    fn test_release_manager_metadata() {
        let config = ReleaseConfig::default().with_name("my_app");
        let mut manager = ReleaseManager::new(config);

        let artifact = ReleaseArtifact {
            name: "my_app".to_string(),
            version: "0.1.0".to_string(),
            beam_dir: PathBuf::from("/beam"),
            config_files: Vec::new(),
            docs_dir: None,
            target_info: TargetInfo {
                runtime: "beam".to_string(),
                version: "16.4".to_string(),
                features: vec!["jit".to_string()],
            },
        };

        manager.add_artifact(artifact);
        let metadata = manager.generate_metadata();

        assert_eq!(metadata.name, "my_app");
        assert!(metadata.modules.contains(&"my_app".to_string()));
        assert!(metadata.target_features.contains(&"jit".to_string()));
    }

    #[test]
    fn test_target_info() {
        let target = TargetInfo {
            runtime: "beam".to_string(),
            version: "16.4".to_string(),
            features: vec!["jit".to_string(), "unicode".to_string()],
        };

        assert_eq!(target.runtime, "beam");
        assert_eq!(target.version, "16.4");
        assert_eq!(target.features.len(), 2);
    }

    #[test]
    fn test_release_error_display() {
        let err = ReleaseError::NoArtifacts;
        assert_eq!(format!("{}", err), "No artifacts to release");

        let err = ReleaseError::MissingArtifact("foo".to_string());
        assert_eq!(format!("{}", err), "Missing artifact: foo");
    }

    #[test]
    fn test_release_package_info() {
        let pkg = ReleasePackage {
            name: "my_app".to_string(),
            version: "1.0.0".to_string(),
            path: PathBuf::from("/rel/my_app"),
            beam_count: 5,
        };

        assert_eq!(pkg.name, "my_app");
        assert_eq!(pkg.version, "1.0.0");
        assert_eq!(pkg.beam_count, 5);
    }
}
