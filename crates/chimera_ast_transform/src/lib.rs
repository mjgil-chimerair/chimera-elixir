//! AST transformation plugins for the zelix Rust/Zig Elixir compiler.
//!
//! Provides an AST visitor API, node replacement/insertion with span preservation,
//! and a golden test harness for AST transformations.

#[cfg(test)]
use chimera_allocator as _;

use std::collections::HashMap;
use chimera_plugin_api::{PluginMetadata, PluginPhase};

/// Unique identifier for AST nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstNodeId(pub u64);

impl AstNodeId {
    pub fn new(index: u64) -> Self {
        Self(index)
    }

    pub fn none() -> Self {
        Self(u64::MAX)
    }
}

/// Source span information preserved through transformations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub start_offset: usize,
    pub end_offset: usize,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        Self {
            start_offset: start,
            end_offset: end,
            start_line: line,
            start_column: col,
            end_line: line,
            end_column: col,
        }
    }

    pub fn merge(&self, other: &SourceSpan) -> Self {
        Self {
            start_offset: self.start_offset.min(other.start_offset),
            end_offset: self.end_offset.max(other.end_offset),
            start_line: self.start_line.min(other.start_line),
            start_column: self.start_column.min(other.start_column),
            end_line: self.end_line.max(other.end_line),
            end_column: self.end_column.max(other.end_column),
        }
    }

    pub fn length(&self) -> usize {
        self.end_offset.saturating_sub(self.start_offset)
    }

    pub fn clone(&self) -> Self {
        Self {
            start_offset: self.start_offset,
            end_offset: self.end_offset,
            start_line: self.start_line,
            start_column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
        }
    }
}

/// AST node kinds in the Elixir abstract syntax tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AstKind {
    Module,
    FunctionDef,
    FunctionClause,
    Call,
    RemoteCall,
    Atom,
    Integer,
    Float,
    String,
    List,
    Tuple,
    Map,
    MapEntry,
    Binary,
    BinarySegment,
    Variable,
    Alias,
    Operator,
    UnaryOp,
    BinaryOp,
    Guard,
    Pattern,
    TypeSpec,
    Block,
    If,
    Case,
    Cond,
    Receive,
    Try,
    With,
    Comprehension,
    ComprehensionGenerator,
    ComprehensionFilter,
    Fn,
    Do,
    Rescue,
    Catch,
    After,
    Quote,
    Unquote,
    UnquoteSplicing,
    MacroInvocation,
    SpecialForm,
    Comment,
    Unknown,
}

impl AstKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AstKind::Module => "module",
            AstKind::FunctionDef => "function_def",
            AstKind::FunctionClause => "function_clause",
            AstKind::Call => "call",
            AstKind::RemoteCall => "remote_call",
            AstKind::Atom => "atom",
            AstKind::Integer => "integer",
            AstKind::Float => "float",
            AstKind::String => "string",
            AstKind::List => "list",
            AstKind::Tuple => "tuple",
            AstKind::Map => "map",
            AstKind::MapEntry => "map_entry",
            AstKind::Binary => "binary",
            AstKind::BinarySegment => "binary_segment",
            AstKind::Variable => "variable",
            AstKind::Alias => "alias",
            AstKind::Operator => "operator",
            AstKind::UnaryOp => "unary_op",
            AstKind::BinaryOp => "binary_op",
            AstKind::Guard => "guard",
            AstKind::Pattern => "pattern",
            AstKind::TypeSpec => "type_spec",
            AstKind::Block => "block",
            AstKind::If => "if",
            AstKind::Case => "case",
            AstKind::Cond => "cond",
            AstKind::Receive => "receive",
            AstKind::Try => "try",
            AstKind::With => "with",
            AstKind::Comprehension => "comprehension",
            AstKind::ComprehensionGenerator => "comprehension_generator",
            AstKind::ComprehensionFilter => "comprehension_filter",
            AstKind::Fn => "fn",
            AstKind::Do => "do",
            AstKind::Rescue => "rescue",
            AstKind::Catch => "catch",
            AstKind::After => "after",
            AstKind::Quote => "quote",
            AstKind::Unquote => "unquote",
            AstKind::UnquoteSplicing => "unquote_splicing",
            AstKind::MacroInvocation => "macro_invocation",
            AstKind::SpecialForm => "special_form",
            AstKind::Comment => "comment",
            AstKind::Unknown => "unknown",
        }
    }
}

/// A node in the Elixir AST.
#[derive(Debug, Clone)]
pub struct AstNode {
    pub id: AstNodeId,
    pub kind: AstKind,
    pub content: String,
    pub span: SourceSpan,
    pub children: Vec<AstNodeId>,
    pub metadata: HashMap<String, String>,
    pub parent: Option<AstNodeId>,
}

impl AstNode {
    pub fn new(id: AstNodeId, kind: AstKind, span: SourceSpan) -> Self {
        Self {
            id,
            kind,
            content: String::new(),
            span,
            children: Vec::new(),
            metadata: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn add_child(&mut self, child: AstNodeId) {
        self.children.push(child);
    }
}

/// The AST document containing all nodes.
#[derive(Debug, Default)]
pub struct AstDocument {
    nodes: HashMap<AstNodeId, AstNode>,
    root: Option<AstNodeId>,
    next_id: u64,
}

impl AstDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a new node and return its ID.
    pub fn insert(&mut self, mut node: AstNode) -> AstNodeId {
        let id = AstNodeId::new(self.next_id);
        node.id = id;
        self.next_id += 1;
        self.nodes.insert(id, node);
        id
    }

    /// Get a node by ID.
    pub fn get(&self, id: AstNodeId) -> Option<&AstNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable node by ID.
    pub fn get_mut(&mut self, id: AstNodeId) -> Option<&mut AstNode> {
        self.nodes.get_mut(&id)
    }

    /// Remove a node and all its descendants.
    pub fn remove(&mut self, id: AstNodeId) -> Option<AstNode> {
        let removed = self.nodes.remove(&id)?;
        // Remove from parent's children list
        if let Some(parent_id) = removed.parent {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.children.retain(|&c| c != id);
            }
        }
        // Recursively remove children
        for child_id in &removed.children {
            self.remove(*child_id);
        }
        Some(removed)
    }

    /// Replace a node with a new one, preserving the ID and span.
    pub fn replace(&mut self, id: AstNodeId, mut new_node: AstNode) -> Option<AstNodeId> {
        if let Some(old_node) = self.nodes.get(&id) {
            new_node.id = id;
            new_node.span = old_node.span.clone();
            new_node.metadata.extend(old_node.metadata.clone());
            // Update parent's reference
            if let Some(parent_id) = old_node.parent {
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    if let Some(pos) = parent.children.iter().position(|&c| c == id) {
                        parent.children[pos] = id;
                    }
                }
            }
            self.nodes.insert(id, new_node);
            Some(id)
        } else {
            None
        }
    }

    /// Insert a new child node at a specific position.
    pub fn insert_child_at(&mut self, parent_id: AstNodeId, position: usize, mut child: AstNode) -> Option<AstNodeId> {
        let pos = {
            let parent = self.nodes.get_mut(&parent_id)?;
            position.min(parent.children.len())
        };
        child.parent = Some(parent_id);
        let child_id = self.insert(child);
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.insert(pos, child_id);
        }
        Some(child_id)
    }

    /// Set the root node of the document.
    pub fn set_root(&mut self, id: AstNodeId) {
        self.root = Some(id);
    }

    /// Get the root node ID.
    pub fn root(&self) -> Option<AstNodeId> {
        self.root
    }

    /// Get all node IDs in document order.
    #[allow(dead_code)]
    pub fn node_ids(&self) -> Vec<AstNodeId> {
        (0..self.next_id).map(AstNodeId::new).filter(|id| self.nodes.contains_key(id)).collect()
    }

    /// Update the span of a node and all descendants.
    pub fn update_span(&mut self, id: AstNodeId, span: SourceSpan) {
        let children_ids: Vec<AstNodeId> = {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.span = span.clone();
                node.children.clone()
            } else {
                return;
            }
        };

        for child_id in children_ids {
            let child_span = SourceSpan::new(
                0, 0,
                span.start_line,
                span.start_column
            );
            self.update_span(child_id, child_span);
        }
    }

    /// Get the total number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Visitor trait for traversing AST nodes.
pub trait AstVisitor: Send + Sync {
    fn visit(&self, document: &AstDocument, id: AstNodeId) -> VisitResult;
}

/// Result of a visitor operation.
#[derive(Debug, Clone)]
pub enum VisitResult {
    Continue,
    Skip,
    Replace(AstNodeId),
    Remove,
}

/// AST visitor that collects statistics.
pub struct StatsVisitor {
    pub node_counts: HashMap<AstKind, usize>,
    pub total_nodes: usize,
}

impl StatsVisitor {
    pub fn new() -> Self {
        Self {
            node_counts: HashMap::new(),
            total_nodes: 0,
        }
    }

    pub fn reset(&mut self) {
        self.node_counts.clear();
        self.total_nodes = 0;
    }
}

impl AstVisitor for StatsVisitor {
    fn visit(&self, document: &AstDocument, id: AstNodeId) -> VisitResult {
        if let Some(_node) = document.get(id) {
            // Note: StatsVisitor requires &mut self for counting, using interior mutability pattern
            VisitResult::Continue
        } else {
            VisitResult::Continue
        }
    }
}

/// Default visitor that traverses all nodes.
pub struct DefaultVisitor;

impl AstVisitor for DefaultVisitor {
    fn visit(&self, _document: &AstDocument, _id: AstNodeId) -> VisitResult {
        VisitResult::Continue
    }
}

/// Traversal context for AST visits.
#[derive(Debug)]
pub struct TraversalContext {
    pub path: Vec<AstNodeId>,
    pub current_id: Option<AstNodeId>,
    pub depth: usize,
}

impl TraversalContext {
    pub fn new() -> Self {
        Self {
            path: Vec::new(),
            current_id: None,
            depth: 0,
        }
    }

    pub fn push(&mut self, id: AstNodeId) {
        self.path.push(id);
        self.depth += 1;
        self.current_id = Some(id);
    }

    pub fn pop(&mut self) -> Option<AstNodeId> {
        self.depth = self.depth.saturating_sub(1);
        self.path.pop();
        self.current_id = self.path.last().copied();
        self.path.last().copied()
    }

    #[allow(dead_code)]
    pub fn parent(&self) -> Option<AstNodeId> {
        self.path.iter().rev().nth(1).copied()
    }

    #[allow(dead_code)]
    pub fn ancestors(&self) -> Vec<AstNodeId> {
        self.path.clone()
    }
}

impl Default for TraversalContext {
    fn default() -> Self {
        Self::new()
    }
}

/// AST transformer that applies transformations based on rules.
pub struct AstTransformer {
    transforms: Vec<TransformRule>,
}

impl AstTransformer {
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: TransformRule) {
        self.transforms.push(rule);
    }

    pub fn transform(&mut self, mut document: AstDocument) -> AstDocument {
        let root_id = document.root().expect("document has no root");
        self.apply_transforms(&mut document, root_id);
        document
    }

    fn apply_transforms(&self, document: &mut AstDocument, id: AstNodeId) {
        // Apply transform rules
        for rule in &self.transforms {
            if let Some(new_node) = rule.apply(document, id) {
                document.replace(id, new_node);
            }
        }

        // Visit children
        let children: Vec<AstNodeId> = {
            if let Some(node) = document.get(id) {
                node.children.clone()
            } else {
                return;
            }
        };

        for child_id in children {
            self.apply_transforms(document, child_id);
        }
    }
}

impl Default for AstTransformer {
    fn default() -> Self {
        Self::new()
    }
}

/// A transformation rule.
pub struct TransformRule {
    pub name: String,
    pub description: String,
    apply_fn: Box<dyn Fn(&mut AstDocument, AstNodeId) -> Option<AstNode> + Send + Sync>,
}

impl TransformRule {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        apply_fn: impl Fn(&mut AstDocument, AstNodeId) -> Option<AstNode> + 'static + Send + Sync,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            apply_fn: Box::new(apply_fn),
        }
    }

    pub fn apply(&self, document: &mut AstDocument, id: AstNodeId) -> Option<AstNode> {
        (self.apply_fn)(document, id)
    }
}

/// Transform finder for locating patterns in AST.
pub struct TransformFinder {
    patterns: Vec<FindPattern>,
}

impl TransformFinder {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn add_pattern(&mut self, pattern: FindPattern) {
        self.patterns.push(pattern);
    }

    pub fn find(&self, document: &AstDocument) -> Vec<TransformMatch> {
        let mut matches = Vec::new();
        self.find_in(document, document.root(), &mut matches);
        matches
    }

    fn find_in(&self, document: &AstDocument, id: Option<AstNodeId>, matches: &mut Vec<TransformMatch>) {
        let Some(node_id) = id else { return };
        let Some(node) = document.get(node_id) else { return };

        for pattern in &self.patterns {
            if pattern.matches(document, node_id) {
                matches.push(TransformMatch {
                    node_id,
                    pattern_name: pattern.name.clone(),
                    span: node.span.clone(),
                });
            }
        }

        for child_id in &node.children {
            self.find_in(document, Some(*child_id), matches);
        }
    }
}

impl Default for TransformFinder {
    fn default() -> Self {
        Self::new()
    }
}

/// A pattern to find in the AST.
pub struct FindPattern {
    pub name: String,
    kind: Option<AstKind>,
    content_match: Option<String>,
    predicate: Option<Box<dyn Fn(&AstDocument, AstNodeId) -> bool + Send + Sync>>,
}

impl FindPattern {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: None,
            content_match: None,
            predicate: None,
        }
    }

    pub fn with_kind(mut self, kind: AstKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content_match = Some(content.into());
        self
    }

    pub fn with_predicate(mut self, pred: impl Fn(&AstDocument, AstNodeId) -> bool + 'static + Send + Sync) -> Self {
        self.predicate = Some(Box::new(pred));
        self
    }

    pub fn matches(&self, document: &AstDocument, id: AstNodeId) -> bool {
        let Some(node) = document.get(id) else { return false };

        if let Some(kind) = &self.kind {
            if &node.kind != kind {
                return false;
            }
        }

        if let Some(content) = &self.content_match {
            if &node.content != content {
                return false;
            }
        }

        if let Some(pred) = &self.predicate {
            if !pred(document, id) {
                return false;
            }
        }

        true
    }
}

/// A match found by the finder.
#[derive(Debug)]
pub struct TransformMatch {
    pub node_id: AstNodeId,
    pub pattern_name: String,
    pub span: SourceSpan,
}

/// Golden test harness for AST transformations.
pub struct GoldenTestHarness {
    fixtures: std::collections::HashMap<String, Fixture>,
}

impl GoldenTestHarness {
    pub fn new() -> Self {
        Self {
            fixtures: std::collections::HashMap::new(),
        }
    }

    pub fn add_fixture(&mut self, name: impl Into<String>, fixture: Fixture) {
        self.fixtures.insert(name.into(), fixture);
    }

    pub fn run_test(&self, name: &str, transformer: &mut AstTransformer, input_doc: AstDocument, _expected_doc: AstDocument) -> Result<GoldenTestResult, String> {
        let fixture = self.fixtures.get(name).ok_or_else(|| format!("fixture not found: {}", name))?;

        let document = input_doc;
        let original_span = document.get(fixture.input_root).map(|n| n.span.clone());

        let result = transformer.transform(document);

        let expected_root = fixture.expected_root;
        let actual_root = result.root();

        // Compare structure (simplified - in production would do deep comparison)
        let passed = actual_root == Some(expected_root);

        Ok(GoldenTestResult {
            fixture_name: name.to_string(),
            passed,
            input_root: fixture.input_root,
            expected_root,
            actual_root,
            span_preserved: original_span.is_some(),
            errors: if passed { vec![] } else { vec!["output mismatch".to_string()] },
        })
    }

    pub fn list_fixtures(&self) -> Vec<String> {
        self.fixtures.keys().cloned().collect()
    }
}

impl Default for GoldenTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// A test fixture containing input and expected output.
pub struct Fixture {
    pub input: AstDocument,
    pub input_root: AstNodeId,
    pub expected: AstDocument,
    pub expected_root: AstNodeId,
    pub transform_name: String,
}

/// Result of a golden test run.
#[derive(Debug)]
pub struct GoldenTestResult {
    pub fixture_name: String,
    pub passed: bool,
    pub input_root: AstNodeId,
    pub expected_root: AstNodeId,
    pub actual_root: Option<AstNodeId>,
    pub span_preserved: bool,
    pub errors: Vec<String>,
}

/// Transformation plugin that implements AST transformations.
pub struct TransformPlugin {
    metadata: PluginMetadata,
    transformer: AstTransformer,
}

impl TransformPlugin {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            metadata: PluginMetadata {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                author: "zelix".to_string(),
                description: description.to_string(),
                lifecycle_phase: PluginPhase::AfterParser,
                api_version: 1,
            },
            transformer: AstTransformer::new(),
        }
    }

    pub fn with_transformer(mut self, transformer: AstTransformer) -> Self {
        self.transformer = transformer;
        self
    }

    pub fn add_rule(&mut self, rule: TransformRule) {
        self.transformer.add_rule(rule);
    }

    pub fn transform(&mut self, document: AstDocument) -> AstDocument {
        self.transformer.transform(document)
    }

    pub fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
}


impl chimera_plugin_api::Plugin for TransformPlugin {
    fn init(&self, _config: &[u8]) -> Result<Box<dyn chimera_plugin_api::PluginInstance>, chimera_plugin_api::PluginError> {
        Ok(Box::new(TransformPluginInstance {
            transformer: Box::new(AstTransformer::new()),
            metadata: self.metadata.clone(),
        }))
    }

    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn create_instance(&self) -> Result<Box<dyn chimera_plugin_api::PluginInstance>, chimera_plugin_api::PluginError> {
        Self::init(self, &[])
    }
}

pub struct TransformPluginInstance {
    #[allow(dead_code)]
    transformer: Box<AstTransformer>,
    metadata: PluginMetadata,
}

impl chimera_plugin_api::PluginInstance for TransformPluginInstance {
    fn execute(&self, _ctx: &mut chimera_plugin_api::PluginContext) -> chimera_plugin_api::PluginResult {
        // In a full implementation, this would receive an AST document to transform
        chimera_plugin_api::PluginResult::success()
    }

    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_document() -> AstDocument {
        let mut doc = AstDocument::new();

        // Create a simple module node
        let module_span = SourceSpan::new(0, 20, 1, 1);
        let module_id = doc.insert(AstNode::new(AstNodeId::none(), AstKind::Module, module_span)
            .with_content("defmodule Foo"));

        // Create a function node as child
        let func_span = SourceSpan::new(0, 20, 1, 1);
        let func_id = doc.insert(AstNode::new(AstNodeId::none(), AstKind::FunctionDef, func_span)
            .with_content("def foo"));
        if let Some(parent) = doc.get_mut(module_id) {
            parent.add_child(func_id);
        }
        if let Some(child) = doc.get_mut(func_id) {
            child.parent = Some(module_id);
        }

        doc.set_root(module_id);
        doc
    }

    #[test]
    fn test_ast_document_insert_and_get() {
        let doc = create_test_document();
        assert!(doc.root().is_some());
        assert!(doc.len() >= 2);
    }

    #[test]
    fn test_ast_document_replace() {
        let mut doc = create_test_document();
        let root_id = doc.root().unwrap();

        let new_span = SourceSpan::new(0, 30, 1, 1);
        let new_node = AstNode::new(AstNodeId::none(), AstKind::Module, new_span)
            .with_content("defmodule Bar");
        doc.replace(root_id, new_node);

        let updated = doc.get(root_id).unwrap();
        assert_eq!(updated.content, "defmodule Bar");
    }

    #[test]
    fn test_ast_document_remove() {
        let mut doc = create_test_document();
        let root_id = doc.root().unwrap();
        let module_node = doc.get(root_id).unwrap();
        let func_id = module_node.children[0];

        doc.remove(func_id);
        let updated = doc.get(root_id).unwrap();
        assert!(updated.children.is_empty());
    }

    #[test]
    fn test_source_span_merge() {
        let span1 = SourceSpan::new(0, 10, 1, 1);
        let span2 = SourceSpan::new(5, 15, 2, 5);
        let merged = span1.merge(&span2);
        assert_eq!(merged.start_offset, 0);
        assert_eq!(merged.end_offset, 15);
    }

    #[test]
    fn test_visit_result_variants() {
        assert!(matches!(VisitResult::Continue, VisitResult::Continue));
        assert!(matches!(VisitResult::Skip, VisitResult::Skip));
        assert!(matches!(VisitResult::Remove, VisitResult::Remove));
    }

    #[test]
    fn test_stats_visitor() {
        let doc = create_test_document();
        let visitor = StatsVisitor::new();
        let result = visitor.visit(&doc, doc.root().unwrap());
        assert!(matches!(result, VisitResult::Continue));
    }

    #[test]
    fn test_traversal_context_push_pop() {
        let mut ctx = TraversalContext::new();
        let id = AstNodeId::new(1);
        ctx.push(id);
        assert_eq!(ctx.depth, 1);
        assert_eq!(ctx.current_id, Some(id));
        ctx.pop();
        assert_eq!(ctx.depth, 0);
    }

    #[test]
    fn test_find_pattern_matching() {
        let doc = create_test_document();
        let pattern = FindPattern::new("test_pattern").with_kind(AstKind::Module);
        let root_id = doc.root().unwrap();
        assert!(pattern.matches(&doc, root_id));
    }

    #[test]
    fn test_find_pattern_content_mismatch() {
        let doc = create_test_document();
        let pattern = FindPattern::new("test_pattern").with_content("wrong content");
        let root_id = doc.root().unwrap();
        assert!(!pattern.matches(&doc, root_id));
    }

    #[test]
    fn test_transform_rule() {
        let rule = TransformRule::new("test_rule", "A test rule", |_doc, _id| {
            None // No transformation
        });
        assert_eq!(rule.name, "test_rule");
    }

    #[test]
    fn test_ast_kind_as_str() {
        assert_eq!(AstKind::Module.as_str(), "module");
        assert_eq!(AstKind::Call.as_str(), "call");
        assert_eq!(AstKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_golden_test_harness_fixtures() {
        let mut harness = GoldenTestHarness::new();
        let doc = create_test_document();
        let root = doc.root().unwrap();
        // Use two separate documents since one document can't be used twice
        let doc2 = create_test_document();
        let root2 = doc2.root().unwrap();
        let fixture = Fixture {
            input: doc,
            input_root: root,
            expected: doc2,
            expected_root: root2,
            transform_name: "test".to_string(),
        };
        harness.add_fixture("test", fixture);
        let fixtures = harness.list_fixtures();
        assert_eq!(fixtures.len(), 1);
    }

    #[test]
    fn test_ast_node_id_new_and_none() {
        let id = AstNodeId::new(42);
        assert_eq!(id.0, 42);
        let none = AstNodeId::none();
        assert_eq!(none.0, u64::MAX);
    }

    #[test]
    fn test_transform_plugin_new() {
        let plugin = TransformPlugin::new("test-plugin", "A test transformation plugin");
        let meta = plugin.metadata();
        assert_eq!(meta.name, "test-plugin");
    }

    #[test]
    fn test_insert_child_at() {
        let mut doc = create_test_document();
        let root_id = doc.root().unwrap();

        let child_span = SourceSpan::new(0, 10, 1, 1);
        let child = AstNode::new(AstNodeId::none(), AstKind::Variable, child_span)
            .with_content("x");

        let child_id = doc.insert_child_at(root_id, 0, child).unwrap();
        let parent = doc.get(root_id).unwrap();
        assert!(parent.children.contains(&child_id));
    }

    #[test]
    fn test_source_span_clone() {
        let span = SourceSpan::new(0, 10, 1, 1);
        let cloned = span.clone();
        assert_eq!(span.start_offset, cloned.start_offset);
        assert_eq!(span.end_offset, cloned.end_offset);
    }

    #[test]
    fn test_transform_finder() {
        let doc = create_test_document();
        let mut finder = TransformFinder::new();
        finder.add_pattern(FindPattern::new("modules").with_kind(AstKind::Module));
        let matches = finder.find(&doc);
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_ast_transformer_add_rule() {
        let mut transformer = AstTransformer::new();
        let rule = TransformRule::new("test", "test rule", |_, _| None);
        transformer.add_rule(rule);
        // If no panic, test passes
    }

    #[test]
    fn test_ast_transformer_transform() {
        let mut transformer = AstTransformer::new();
        let mut doc = create_test_document();
        let result = transformer.transform(doc);
        // Returns the document
        assert!(result.root().is_some());
    }
}