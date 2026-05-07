//! Module dependency graph and cross-reference analysis.
//!
//! Provides infrastructure for building and analyzing module dependency
//! graphs to support cross-module function resolution and error reporting.

use chimera_source::SourceFileId;
use chimera_term::Atom;
use std::collections::{HashMap, HashSet, VecDeque};

/// A module node in the dependency graph.
#[derive(Debug, Clone)]
pub struct ModuleNode {
    /// Module name
    pub name: Atom,
    /// Source file containing the module
    pub file_id: SourceFileId,
    /// Functions defined in this module (name -> arity list)
    pub functions: HashMap<Atom, Vec<u8>>,
    /// Types defined in this module
    pub types: HashSet<Atom>,
    /// Imports/requires from other modules
    pub dependencies: HashSet<Atom>,
    /// Whether this is an external (natively compiled) module
    pub is_external: bool,
}

impl ModuleNode {
    pub fn new(name: Atom, file_id: SourceFileId) -> Self {
        ModuleNode {
            name,
            file_id,
            functions: HashMap::new(),
            types: HashSet::new(),
            dependencies: HashSet::new(),
            is_external: false,
        }
    }

    /// Add a function definition to this module.
    pub fn add_function(&mut self, name: Atom, arity: u8) {
        self.functions.entry(name).or_default().push(arity);
    }

    /// Add a type definition to this module.
    pub fn add_type(&mut self, name: Atom) {
        self.types.insert(name);
    }

    /// Add a dependency on another module.
    pub fn add_dependency(&mut self, module: Atom) {
        self.dependencies.insert(module);
    }

    /// Check if this module defines a function with given name and arity.
    pub fn has_function(&self, name: Atom, arity: u8) -> bool {
        self.functions
            .get(&name)
            .map(|arities| arities.contains(&arity))
            .unwrap_or(false)
    }

    /// Check if this module defines a type.
    pub fn has_type(&self, name: Atom) -> bool {
        self.types.contains(&name)
    }
}

/// An edge in the module dependency graph.
#[derive(Debug, Clone, PartialEq)]
pub enum DependencyKind {
    /// Regular import
    Import,
    /// Require (compile-time dependency)
    Require,
    /// Use (behavior implementation)
    Use,
    /// Implementation of a behavior/callback
    CallbackImpl,
}

#[derive(Debug, Clone)]
pub struct DependencyEdge {
    pub from: Atom,
    pub to: Atom,
    pub kind: DependencyKind,
}

/// A module dependency graph for cross-reference analysis.
#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    /// Module name -> ModuleNode
    modules: HashMap<Atom, ModuleNode>,
    /// Dependency edges
    edges: Vec<DependencyEdge>,
    /// External modules (natively compiled, not in graph)
    external_modules: HashSet<Atom>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        ModuleGraph::default()
    }

    /// Add a module to the graph.
    pub fn add_module(&mut self, node: ModuleNode) {
        self.modules.insert(node.name.clone(), node);
    }

    /// Get a module by name.
    pub fn get_module(&self, name: Atom) -> Option<&ModuleNode> {
        self.modules.get(&name)
    }

    /// Get a module by name (mutable).
    pub fn get_module_mut(&mut self, name: Atom) -> Option<&mut ModuleNode> {
        self.modules.get_mut(&name)
    }

    /// Add a dependency edge between modules.
    pub fn add_dependency(&mut self, from: Atom, to: Atom, kind: DependencyKind) {
        self.edges.push(DependencyEdge {
            from: from.clone(),
            to: to.clone(),
            kind: kind.clone(),
        });
        // Update the module's dependency list
        if let Some(module) = self.modules.get_mut(&from) {
            module.add_dependency(to);
        }
    }

    /// Mark a module as external (natively compiled).
    pub fn mark_external(&mut self, name: Atom) {
        self.external_modules.insert(name);
    }

    /// Check if a module is external.
    pub fn is_external(&self, name: Atom) -> bool {
        self.external_modules.contains(&name)
    }

    /// Get all modules that depend on a given module.
    pub fn dependents(&self, name: Atom) -> Vec<Atom> {
        self.edges
            .iter()
            .filter(|e| e.to == name)
            .map(|e| e.from.clone())
            .collect()
    }

    /// Get all modules that a given module depends on.
    pub fn dependencies(&self, name: Atom) -> Vec<Atom> {
        self.edges
            .iter()
            .filter(|e| e.from == name)
            .map(|e| e.to.clone())
            .collect()
    }

    /// Check if adding an edge would create a cycle.
    pub fn would_cycle(&self, from: Atom, to: Atom) -> bool {
        // Simple cycle detection: check if there's already a path from to to from
        self.has_path(to, from)
    }

    /// Find any cycle in the graph.
    /// Returns the nodes involved in the cycle, if any.
    pub fn find_cycle(&self) -> Option<Vec<Atom>> {
        let mut visited: HashSet<Atom> = HashSet::new();
        let mut in_stack: HashSet<Atom> = HashSet::new();
        let mut stack: Vec<Atom> = Vec::new();

        for module in self.modules.keys() {
            if !visited.contains(module) {
                if let Some(cycle) = self.find_cycle_from(module, &mut visited, &mut in_stack, &mut stack) {
                    return Some(cycle);
                }
            }
        }
        None
    }

    fn find_cycle_from(
        &self,
        module: &Atom,
        visited: &mut HashSet<Atom>,
        in_stack: &mut HashSet<Atom>,
        stack: &mut Vec<Atom>,
    ) -> Option<Vec<Atom>> {
        visited.insert(module.clone());
        in_stack.insert(module.clone());
        stack.push(module.clone());

        for dep in self.dependencies(module.clone()) {
            if !visited.contains(&dep) {
                if let Some(cycle) = self.find_cycle_from(&dep, visited, in_stack, stack) {
                    return Some(cycle);
                }
            } else if in_stack.contains(&dep) {
                // Found a cycle - extract it from the stack
                let cycle_start = stack.iter().position(|m| *m == dep).unwrap();
                return Some(stack[cycle_start..].to_vec());
            }
        }

        stack.pop();
        in_stack.remove(&module.clone());
        None
    }

    /// Check if there's a path from one module to another.
    fn has_path(&self, from: Atom, to: Atom) -> bool {
        // Quick check: same module is not a path
        if from == to {
            return false;
        }
        let mut visited = HashSet::new();
        let mut queue = vec![from];

        while let Some(current) = queue.pop() {
            if current == to {
                return true;
            }
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            queue.extend(self.dependencies(current));
        }
        false
    }

    /// Get all modules in the graph.
    pub fn modules(&self) -> Vec<&ModuleNode> {
        self.modules.values().collect()
    }

    /// Get the number of modules in the graph.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Get topological sort of modules (dependencies first).
    pub fn topological_sort(&self) -> Result<Vec<Atom>, CycleError> {
        let mut in_degree: HashMap<Atom, usize> = HashMap::new();
        let mut graph: HashMap<Atom, Vec<Atom>> = HashMap::new();

        // Initialize
        for module in self.modules.keys() {
            in_degree.insert(module.clone(), 0);
            graph.insert(module.clone(), Vec::new());
        }

        // Build graph and calculate in-degrees
        for edge in &self.edges {
            if let Some(deps) = graph.get_mut(&edge.from) {
                deps.push(edge.to.clone());
            }
            if let Some(degree) = in_degree.get_mut(&edge.to) {
                *degree += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<Atom> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(n, _)| n.clone())
            .collect();
        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());
            if let Some(deps) = graph.get(&node) {
                for dep in deps {
                    if let Some(degree) = in_degree.get_mut(dep) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.modules.len() {
            // Cycle detected - find the cycle
            let remaining: Vec<Atom> = self.modules.keys()
                .filter(|m| !result.contains(m))
                .cloned()
                .collect();
            Err(CycleError { nodes: remaining })
        } else {
            Ok(result)
        }
    }
}

/// Error returned when a cycle is detected in the dependency graph.
#[derive(Debug, Clone, PartialEq)]
pub struct CycleError {
    /// Nodes involved in the cycle
    pub nodes: Vec<Atom>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cycle detected in dependency graph")
    }
}

impl std::error::Error for CycleError {}

/// Function reference for cross-module calls.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionRef {
    /// Module name (if remote call)
    pub module: Option<Atom>,
    /// Function name
    pub name: Atom,
    /// Function arity
    pub arity: u8,
}

impl FunctionRef {
    /// Create a local function reference.
    pub fn local(name: Atom, arity: u8) -> Self {
        FunctionRef {
            module: None,
            name,
            arity,
        }
    }

    /// Create a remote function reference.
    pub fn remote(module: Atom, name: Atom, arity: u8) -> Self {
        FunctionRef {
            module: Some(module),
            name,
            arity,
        }
    }

    /// Check if this is a local call.
    pub fn is_local(&self) -> bool {
        self.module.is_none()
    }

    /// Check if this is a remote call.
    pub fn is_remote(&self) -> bool {
        self.module.is_some()
    }
}

/// Result of resolving a function reference.
#[derive(Debug, Clone)]
pub enum FunctionResolution {
    /// Function found and resolved
    Resolved {
        module: Atom,
        file_id: SourceFileId,
    },
    /// Function not found in any known module
    NotFound {
        tried_modules: Vec<Atom>,
    },
    /// Function exists but arity doesn't match
    ArityMismatch {
        module: Atom,
        available_arity: u8,
        requested_arity: u8,
    },
    /// Function is local to a module but called remotely from different context
    LocalOnly {
        module: Atom,
    },
}

/// Cross-reference resolver for function calls.
pub struct XrefResolver {
    graph: ModuleGraph,
}

impl XrefResolver {
    pub fn new(graph: ModuleGraph) -> Self {
        XrefResolver { graph }
    }

    /// Resolve a function reference to a concrete module.
    pub fn resolve(&self, func: &FunctionRef) -> FunctionResolution {
        match &func.module {
            Some(module) => self.resolve_remote(module, &func.name, func.arity),
            None => FunctionResolution::NotFound {
                tried_modules: Vec::new(),
            },
        }
    }

    fn resolve_remote(&self, module: &Atom, name: &Atom, arity: u8) -> FunctionResolution {
        // Check if module exists
        if let Some(node) = self.graph.get_module(module.clone()) {
            // Check if function exists in module
            if let Some(arities) = node.functions.get(name) {
                if arities.contains(&arity) {
                    return FunctionResolution::Resolved {
                        module: module.clone(),
                        file_id: node.file_id,
                    };
                } else {
                    // Function exists but wrong arity
                    return FunctionResolution::ArityMismatch {
                        module: module.clone(),
                        available_arity: arities[0], // Just report first available
                        requested_arity: arity,
                    };
                }
            }
            // Function not in module
            return FunctionResolution::NotFound {
                tried_modules: vec![module.clone()],
            };
        }

        // Module not found at all
        FunctionResolution::NotFound {
            tried_modules: vec![module.clone()],
        }
    }

    /// Get the graph for analysis.
    pub fn graph(&self) -> &ModuleGraph {
        &self.graph
    }

    /// Resolve a local function call within a module.
    pub fn resolve_local(&self, module: &Atom, name: &Atom, arity: u8) -> FunctionResolution {
        if let Some(node) = self.graph.get_module(module.clone()) {
            if node.has_function(name.clone(), arity) {
                return FunctionResolution::Resolved {
                    module: module.clone(),
                    file_id: node.file_id,
                };
            }
            // Function not defined in this module
            return FunctionResolution::LocalOnly {
                module: module.clone(),
            };
        }
        FunctionResolution::NotFound {
            tried_modules: vec![module.clone()],
        }
    }

    /// Check for undefined functions in the graph.
    /// Returns list of (module, function, arity) tuples for calls that don't resolve.
    pub fn check_undefined_functions<'a>(
        &'a self,
        calls: &'a [(Atom, FunctionRef)],
    ) -> Vec<(&'a Atom, &'a FunctionRef)> {
        let mut undefined = Vec::new();
        for (module, func_ref) in calls {
            let resolution = self.resolve(func_ref);
            match resolution {
                FunctionResolution::NotFound { .. } |
                FunctionResolution::ArityMismatch { .. } |
                FunctionResolution::LocalOnly { .. } => {
                    undefined.push((module, func_ref));
                }
                FunctionResolution::Resolved { .. } => {}
            }
        }
        undefined
    }

    /// Get all undefined functions for a given module.
    pub fn undefined_in_module(&self, _module: &Atom) -> Vec<(&FunctionRef, &str)> {
        // This would need access to call sites - placeholder for integration
        Vec::new()
    }
}

/// Build a module graph from parsed modules.
/// This is a placeholder that will be connected to the actual AST parsing.
pub struct ModuleGraphBuilder {
    graph: ModuleGraph,
}

impl ModuleGraphBuilder {
    pub fn new() -> Self {
        ModuleGraphBuilder {
            graph: ModuleGraph::new(),
        }
    }

    /// Add a module definition to the graph.
    pub fn add_module(&mut self, name: Atom, file_id: SourceFileId) -> &mut ModuleNode {
        let node = ModuleNode::new(name.clone(), file_id);
        self.graph.add_module(node);
        self.graph.get_module_mut(name).unwrap()
    }

    /// Add a function to a module.
    pub fn add_function(&mut self, module: Atom, name: Atom, arity: u8) {
        if let Some(node) = self.graph.get_module_mut(module) {
            node.add_function(name, arity);
        }
    }

    /// Add a type to a module.
    pub fn add_type(&mut self, module: Atom, name: Atom) {
        if let Some(node) = self.graph.get_module_mut(module) {
            node.add_type(name);
        }
    }

    /// Add a dependency between modules.
    pub fn add_dependency(&mut self, from: Atom, to: Atom, kind: DependencyKind) {
        self.graph.add_dependency(from, to, kind);
    }

    /// Mark a module as external.
    pub fn mark_external(&mut self, module: Atom) {
        self.graph.mark_external(module);
    }

    /// Finalize and return the graph.
    pub fn build(self) -> ModuleGraph {
        self.graph
    }
}

impl Default for ModuleGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_term::AtomTable;

    #[test]
    fn test_module_node_new() {
        let mut table = AtomTable::new();
        let name = table.intern("MyModule");
        let node = ModuleNode::new(name.clone(), SourceFileId::new(0));

        assert_eq!(node.name, name.clone());
        assert_eq!(node.functions.len(), 0);
        assert_eq!(node.types.len(), 0);
        assert_eq!(node.dependencies.len(), 0);
        assert!(!node.is_external);
    }

    #[test]
    fn test_module_node_add_function() {
        let mut table = AtomTable::new();
        let mut node = ModuleNode::new(table.intern("Test"), SourceFileId::new(0));

        node.add_function(table.intern("foo"), 2);
        node.add_function(table.intern("bar"), 1);
        node.add_function(table.intern("foo"), 3); // Overload

        assert!(node.has_function(table.intern("foo"), 2));
        assert!(node.has_function(table.intern("foo"), 3));
        assert!(node.has_function(table.intern("bar"), 1));
        assert!(!node.has_function(table.intern("foo"), 1));
    }

    #[test]
    fn test_module_node_add_type() {
        let mut table = AtomTable::new();
        let mut node = ModuleNode::new(table.intern("Test"), SourceFileId::new(0));

        node.add_type(table.intern("my_type"));

        assert!(node.has_type(table.intern("my_type")));
        assert!(!node.has_type(table.intern("other")));
    }

    #[test]
    fn test_module_graph_new() {
        let graph = ModuleGraph::new();
        assert_eq!(graph.module_count(), 0);
    }

    #[test]
    fn test_module_graph_add_module() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        let node = ModuleNode::new(table.intern("ModuleA"), SourceFileId::new(0));
        graph.add_module(node);

        assert_eq!(graph.module_count(), 1);
        assert!(graph.get_module(table.intern("ModuleA")).is_some());
    }

    #[test]
    fn test_module_graph_add_dependency() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        let node_a = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        let node_b = ModuleNode::new(table.intern("B"), SourceFileId::new(0));
        graph.add_module(node_a);
        graph.add_module(node_b);

        graph.add_dependency(table.intern("A"), table.intern("B"), DependencyKind::Import);

        let deps = graph.dependencies(table.intern("A"));
        assert!(deps.contains(&table.intern("B")));
    }

    #[test]
    fn test_module_graph_topological_sort() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        // Create: A -> B -> C
        let node_a = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        let node_b = ModuleNode::new(table.intern("B"), SourceFileId::new(0));
        let node_c = ModuleNode::new(table.intern("C"), SourceFileId::new(0));
        graph.add_module(node_a);
        graph.add_module(node_b);
        graph.add_module(node_c);

        graph.add_dependency(table.intern("C"), table.intern("B"), DependencyKind::Import);
        graph.add_dependency(table.intern("B"), table.intern("A"), DependencyKind::Import);

        let sorted = graph.topological_sort().unwrap();
        // C should come before B, B before A
        let c_idx = sorted.iter().position(|x| *x == table.intern("C")).unwrap();
        let b_idx = sorted.iter().position(|x| *x == table.intern("B")).unwrap();
        let a_idx = sorted.iter().position(|x| *x == table.intern("A")).unwrap();
        assert!(c_idx < b_idx);
        assert!(b_idx < a_idx);
    }

    #[test]
    fn test_function_ref_local() {
        let mut table = AtomTable::new();
        let func = FunctionRef::local(table.intern("my_func"), 2);

        assert!(func.is_local());
        assert!(!func.is_remote());
        assert_eq!(func.module, None);
        assert_eq!(func.name, table.intern("my_func"));
        assert_eq!(func.arity, 2);
    }

    #[test]
    fn test_function_ref_remote() {
        let mut table = AtomTable::new();
        let func = FunctionRef::remote(
            table.intern("Enum"),
            table.intern("map"),
            2,
        );

        assert!(func.is_remote());
        assert!(!func.is_local());
        assert_eq!(func.module, Some(table.intern("Enum")));
        assert_eq!(func.name, table.intern("map"));
        assert_eq!(func.arity, 2);
    }

    #[test]
    fn test_xref_resolver_local() {
        let mut table = AtomTable::new();
        let resolver = XrefResolver::new(ModuleGraph::new());

        let func = FunctionRef::local(table.intern("unknown"), 1);
        let result = resolver.resolve(&func);

        assert!(matches!(result, FunctionResolution::NotFound { .. }));
    }

    #[test]
    fn test_xref_resolver_remote_found() {
        let mut table = AtomTable::new();
        let mut builder = ModuleGraphBuilder::new();

        builder.add_module(table.intern("Enum"), SourceFileId::new(0));
        builder.add_function(table.intern("Enum"), table.intern("map"), 2);

        let graph = builder.build();
        let resolver = XrefResolver::new(graph);

        let func = FunctionRef::remote(table.intern("Enum"), table.intern("map"), 2);
        let result = resolver.resolve(&func);

        assert!(matches!(result, FunctionResolution::Resolved { .. }));
    }

    #[test]
    fn test_xref_resolver_remote_not_found() {
        let mut table = AtomTable::new();
        let mut builder = ModuleGraphBuilder::new();

        builder.add_module(table.intern("Enum"), SourceFileId::new(0));
        builder.add_function(table.intern("Enum"), table.intern("map"), 2);

        let graph = builder.build();
        let resolver = XrefResolver::new(graph);

        let func = FunctionRef::remote(table.intern("Enum"), table.intern("reduce"), 3);
        let result = resolver.resolve(&func);

        assert!(matches!(result, FunctionResolution::NotFound { .. }));
    }

    #[test]
    fn test_module_graph_builder() {
        let mut table = AtomTable::new();
        let mut builder = ModuleGraphBuilder::new();

        let node = builder.add_module(table.intern("MyModule"), SourceFileId::new(0));
        node.add_function(table.intern("my_func"), 2);
        node.add_type(table.intern("my_type"));

        builder.add_dependency(
            table.intern("MyModule"),
            table.intern("OtherModule"),
            DependencyKind::Import,
        );

        let graph = builder.build();

        let module = graph.get_module(table.intern("MyModule")).unwrap();
        assert!(module.has_function(table.intern("my_func"), 2));
        assert!(module.has_type(table.intern("my_type")));
    }

    #[test]
    fn test_dependency_kind_variants() {
        let mut table = AtomTable::new();

        // Just ensure we can create each kind
        let import = DependencyKind::Import;
        let require = DependencyKind::Require;
        let use_kind = DependencyKind::Use;
        let callback = DependencyKind::CallbackImpl;

        let mut graph = ModuleGraph::new();
        let node = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        graph.add_module(node);

        // Verify enum variants exist (just clone checks)
        let _ = import.clone();
        let _ = require.clone();
        let _ = use_kind.clone();
        let _ = callback.clone();
    }

    #[test]
    fn test_xref_resolver_resolve_local() {
        let mut table = AtomTable::new();
        let mut builder = ModuleGraphBuilder::new();

        builder.add_module(table.intern("MyModule"), SourceFileId::new(0));
        builder.add_function(table.intern("MyModule"), table.intern("my_func"), 2);

        let graph = builder.build();
        let resolver = XrefResolver::new(graph);

        let result = resolver.resolve_local(
            &table.intern("MyModule"),
            &table.intern("my_func"),
            2,
        );

        assert!(matches!(result, FunctionResolution::Resolved { .. }));
    }

    #[test]
    fn test_xref_resolver_local_only() {
        let mut table = AtomTable::new();
        let mut builder = ModuleGraphBuilder::new();

        builder.add_module(table.intern("MyModule"), SourceFileId::new(0));
        builder.add_function(table.intern("MyModule"), table.intern("my_func"), 2);

        let graph = builder.build();
        let resolver = XrefResolver::new(graph);

        // Call to Enum.map from MyModule - MyModule doesn't define map
        let result = resolver.resolve_local(
            &table.intern("MyModule"),
            &table.intern("map"),
            2,
        );

        assert!(matches!(result, FunctionResolution::LocalOnly { .. }));
    }

    #[test]
    fn test_xref_resolver_check_undefined_functions() {
        let mut table = AtomTable::new();
        let mut builder = ModuleGraphBuilder::new();

        builder.add_module(table.intern("Enum"), SourceFileId::new(0));
        builder.add_function(table.intern("Enum"), table.intern("map"), 2);

        let module = table.intern("MyModule");
        let calls = vec![
            (module.clone(), FunctionRef::remote(table.intern("Enum"), table.intern("map"), 2)),
            (module.clone(), FunctionRef::remote(table.intern("Enum"), table.intern("reduce"), 3)),
            (module.clone(), FunctionRef::remote(table.intern("NonExistent"), table.intern("func"), 1)),
        ];

        let graph = builder.build();
        let resolver = XrefResolver::new(graph);

        let undefined = resolver.check_undefined_functions(&calls);

        // 2 out of 3 are not defined (reduce doesn't exist in Enum, NonExistent doesn't exist)
        assert_eq!(undefined.len(), 2);
    }

    #[test]
    fn test_module_graph_find_cycle_none() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        // A -> B -> C (no cycle)
        let node_a = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        let node_b = ModuleNode::new(table.intern("B"), SourceFileId::new(0));
        let node_c = ModuleNode::new(table.intern("C"), SourceFileId::new(0));
        graph.add_module(node_a);
        graph.add_module(node_b);
        graph.add_module(node_c);

        graph.add_dependency(table.intern("A"), table.intern("B"), DependencyKind::Import);
        graph.add_dependency(table.intern("B"), table.intern("C"), DependencyKind::Import);

        assert!(graph.find_cycle().is_none());
    }

    #[test]
    fn test_module_graph_find_cycle_simple() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        // A -> B -> C -> A (cycle: A -> B -> C -> A)
        let node_a = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        let node_b = ModuleNode::new(table.intern("B"), SourceFileId::new(0));
        let node_c = ModuleNode::new(table.intern("C"), SourceFileId::new(0));
        graph.add_module(node_a);
        graph.add_module(node_b);
        graph.add_module(node_c);

        graph.add_dependency(table.intern("A"), table.intern("B"), DependencyKind::Import);
        graph.add_dependency(table.intern("B"), table.intern("C"), DependencyKind::Import);
        graph.add_dependency(table.intern("C"), table.intern("A"), DependencyKind::Import);

        let cycle = graph.find_cycle();
        assert!(cycle.is_some());
        let cycle = cycle.unwrap();
        // Cycle should contain A, B, C
        assert!(cycle.contains(&table.intern("A")));
        assert!(cycle.contains(&table.intern("B")));
        assert!(cycle.contains(&table.intern("C")));
    }

    #[test]
    fn test_module_graph_find_cycle_self() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        // A -> A (self cycle)
        let node_a = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        graph.add_module(node_a);

        graph.add_dependency(table.intern("A"), table.intern("A"), DependencyKind::Import);

        let cycle = graph.find_cycle();
        assert!(cycle.is_some());
        let cycle = cycle.unwrap();
        // Self cycle should at least contain A
        assert!(cycle.contains(&table.intern("A")));
    }

    #[test]
    fn test_module_graph_would_cycle() {
        let mut table = AtomTable::new();
        let mut graph = ModuleGraph::new();

        let node_a = ModuleNode::new(table.intern("A"), SourceFileId::new(0));
        let node_b = ModuleNode::new(table.intern("B"), SourceFileId::new(0));
        let node_c = ModuleNode::new(table.intern("C"), SourceFileId::new(0));
        graph.add_module(node_a);
        graph.add_module(node_b);
        graph.add_module(node_c);

        // A -> B shouldn't cycle
        assert!(!graph.would_cycle(table.intern("A"), table.intern("B")));

        // Add A -> B
        graph.add_dependency(table.intern("A"), table.intern("B"), DependencyKind::Import);

        // Adding B -> C shouldn't cycle
        assert!(!graph.would_cycle(table.intern("B"), table.intern("C")));
        graph.add_dependency(table.intern("B"), table.intern("C"), DependencyKind::Import);

        // Adding C -> A WOULD create cycle (A -> B -> C -> A)
        assert!(graph.would_cycle(table.intern("C"), table.intern("A")));
    }
}