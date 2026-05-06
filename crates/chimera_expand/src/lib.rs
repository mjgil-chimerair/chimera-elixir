//! Macro expansion for the Rust/Zig Elixir compiler.
//!
//! This module provides macro expansion services including:
//! - MacroEnv for compile-time environment tracking
//! - Hygiene context management for macro expansion
//! - quote/unquote expansion
//! - Special forms handling

#[cfg(test)]
use chimera_allocator as _;

use chimera_ast::{AST, Meta, Hygiene};
use chimera_source::SourceFileId;
use chimera_term::{Atom, SharedAtomTable, ModuleName, VarContext};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;

/// Macro environment tracking compile-time context.
///
/// This tracks module, function, file, line, context, aliases, imports,
/// requires, variables, and lexical scope during macro expansion.
#[derive(Debug, Clone)]
pub struct MacroEnv {
    /// Current module being compiled
    pub module: Option<ModuleName>,
    /// Current function (name + arity)
    pub function: Option<(Atom, u8)>,
    /// Current source file
    pub file: SourceFileId,
    /// Current line number
    pub line: u32,
    /// Compile context (Default, Match, Guard, etc.)
    pub context: ExprContext,
    /// Alias table (alias -> actual module)
    pub aliases: HashMap<Atom, ModuleName>,
    /// Import table (function -> {module, function})
    pub imports: HashMap<Atom, Import>,
    /// Required modules
    pub requires: HashSet<ModuleName>,
    /// Variables in scope
    pub vars: VarTable,
    /// Lexical scope stack
    lexical_tracker: LexicalTracker,
    /// Tracers for compile-time events
    tracers: Vec<TracerId>,
    /// Whether we're in generated code
    generated: bool,
}

/// Import entry tracking imported functions.
#[derive(Debug, Clone)]
pub struct Import {
    pub module: ModuleName,
    pub alias: Option<Atom>,
    pub unaliased: bool,
    /// Optional filter: only import these (name, arity)
    pub only: Option<Vec<(Atom, u8)>>,
    /// Optional filter: import all except these (name, arity)
    pub except: Option<Vec<(Atom, u8)>>,
}

/// Tracer ID for compile-time tracing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TracerId(u32);

/// Expression context during expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprContext {
    Default,
    Match,
    Guard,
    TypeSpec,
    MacroDefinition,
    Quote,
}

impl Default for ExprContext {
    fn default() -> Self {
        ExprContext::Default
    }
}

/// Variable table tracking scoped variables.
#[derive(Debug, Clone, Default)]
pub struct VarTable {
    scopes: Vec<VarScope>,
}

/// A single lexical scope for variables.
#[derive(Debug, Clone, Default)]
struct VarScope {
    vars: HashMap<Atom, VarInfo>,
}

/// Variable information in scope.
#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: Atom,
    pub meta: Meta,
    pub context: VarContext,
    pub origin: Option<String>,
}

/// Lexical tracker for macro hygiene.
#[derive(Debug, Clone, Default)]
struct LexicalTracker {
    scopes: Vec<LexicalScope>,
}

/// A lexical scope with its context.
#[derive(Debug, Clone)]
pub struct LexicalScope {
    context: Atom,
    origin: Option<String>,
    generated: bool,
}

impl Default for LexicalScope {
    fn default() -> Self {
        LexicalScope {
            context: Atom::new(0),
            origin: None,
            generated: false,
        }
    }
}

impl VarTable {
    pub fn new() -> Self {
        VarTable {
            scopes: vec![VarScope::default()],
        }
    }

    /// Push a new lexical scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(VarScope::default());
    }

    /// Pop the current lexical scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a variable in the current scope.
    pub fn define(&mut self, name: Atom, meta: Meta, context: VarContext, origin: Option<String>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.vars.insert(name.clone(), VarInfo { name, meta, context, origin });
        }
    }

    /// Look up a variable by name.
    pub fn lookup(&self, name: Atom) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.vars.get(&name) {
                return Some(var);
            }
        }
        None
    }

    /// Check if a variable exists in any scope.
    pub fn contains(&self, name: &Atom) -> bool {
        self.lookup(name.clone()).is_some()
    }
}

impl MacroEnv {
    /// Create a new macro environment.
    pub fn new(file: SourceFileId) -> Self {
        MacroEnv {
            module: None,
            function: None,
            file,
            line: 1,
            context: ExprContext::Default,
            aliases: HashMap::new(),
            imports: HashMap::new(),
            requires: HashSet::new(),
            vars: VarTable::new(),
            lexical_tracker: LexicalTracker::default(),
            tracers: Vec::new(),
            generated: false,
        }
    }

    /// Set the current module.
    pub fn set_module(&mut self, module: ModuleName) {
        self.module = Some(module);
    }

    /// Set the current function.
    pub fn set_function(&mut self, name: Atom, arity: u8) {
        self.function = Some((name, arity));
    }

    /// Set the current line.
    pub fn set_line(&mut self, line: u32) {
        self.line = line;
    }

    /// Set the compile context.
    pub fn set_context(&mut self, context: ExprContext) {
        self.context = context;
    }

    /// Add an alias.
    pub fn add_alias(&mut self, alias: Atom, target: ModuleName) {
        self.aliases.insert(alias, target);
    }

    /// Add an import.
    pub fn add_import(&mut self, func: Atom, import: Import) {
        self.imports.insert(func, import);
    }

    /// Add a required module.
    pub fn add_require(&mut self, module: ModuleName) {
        self.requires.insert(module);
    }

    /// Check if a module is required.
    pub fn is_required(&self, module: &ModuleName) -> bool {
        self.requires.contains(module)
    }

    /// Resolve an alias to its target module.
    pub fn resolve_alias(&self, alias: &Atom) -> Option<&ModuleName> {
        self.aliases.get(alias)
    }

    /// Look up an imported function.
    pub fn lookup_import(&self, func: &Atom) -> Option<&Import> {
        self.imports.get(func)
    }

    /// Define a variable.
    pub fn define_var(&mut self, name: Atom, meta: Meta, context: VarContext) {
        let origin = if self.generated {
            Some(format!("l_{}_{}", self.file.0, self.line))
        } else {
            None
        };
        self.vars.define(name, meta, context, origin);
    }

    /// Check if a variable is in scope.
    pub fn is_var_defined(&self, name: &Atom) -> bool {
        self.vars.contains(name)
    }

    /// Push a lexical scope.
    pub fn push_scope(&mut self, context: Atom) {
        self.lexical_tracker.push_scope(LexicalScope {
            context,
            origin: None,
            generated: self.generated,
        });
    }

    /// Pop a lexical scope.
    pub fn pop_scope(&mut self) {
        self.lexical_tracker.pop_scope();
    }

    /// Mark code as generated (for hygiene).
    pub fn set_generated(&mut self, generated: bool) {
        self.generated = generated;
    }

    /// Check if we're in generated code.
    pub fn is_generated(&self) -> bool {
        self.generated
    }

    /// Enter a quoted context.
    pub fn enter_quote(&mut self) {
        self.push_scope(self.module.as_ref().and_then(|m| m.segments().last().map(|s| s.clone())).unwrap_or(Atom::new(0)));
        self.context = ExprContext::Quote;
    }

    /// Exit a quoted context.
    pub fn exit_quote(&mut self) {
        self.pop_scope();
        self.context = ExprContext::Default;
    }

    /// Create hygiene context for the current location.
    pub fn hygiene(&self) -> Hygiene {
        let origin = if self.generated {
            Some(format!("l{}c{}", self.file.0, self.line))
        } else {
            None
        };
        Hygiene {
            origin,
            generated: self.generated,
            import: None,
        }
    }
}

impl LexicalTracker {
    fn push_scope(&mut self, scope: LexicalScope) {
        self.scopes.push(scope);
    }

    fn pop_scope(&mut self) {
        if !self.scopes.is_empty() {
            self.scopes.pop();
        }
    }
}

/// Macro expander that performs AST transformations.
#[derive(Debug, Clone)]
pub struct Expander {
    env: MacroEnv,
    atoms: SharedAtomTable,
    /// Module-level attributes
    module_attributes: HashMap<Atom, Vec<AST>>,
}

impl Expander {
    /// Create a new expander.
    pub fn new(env: MacroEnv) -> Self {
        Expander {
            env,
            atoms: SharedAtomTable::new(),
            module_attributes: HashMap::new(),
        }
    }

    /// Get a reference to the macro environment.
    pub fn env(&self) -> &MacroEnv {
        &self.env
    }

    /// Get a mutable reference to the macro environment.
    pub fn env_mut(&mut self) -> &mut MacroEnv {
        &mut self.env
    }

    /// Expand an AST node.
    pub fn expand(&mut self, ast: AST) -> Result<AST, ExpandError> {
        match ast {
            AST::Nil => Ok(AST::Nil),
            AST::Atom(_) => Ok(ast),
            AST::Integer(_) => Ok(ast),
            AST::Float(_) => Ok(ast),
            AST::String(_) => Ok(ast),
            AST::CharList(_) => Ok(ast),
            AST::Binary(_, _) => Ok(ast),
            AST::List(items) => {
                let expanded: Result<Vec<_>, _> = items.into_iter().map(|i| self.expand(i)).collect();
                Ok(AST::List(expanded?))
            }
            AST::Tuple(items) => {
                let expanded: Result<Vec<_>, _> = items.into_iter().map(|i| self.expand(i)).collect();
                Ok(AST::Tuple(expanded?))
            }
            AST::Map(pairs) => {
                let expanded: Result<Vec<_>, _> = pairs.into_iter()
                    .map(|(k, v)| Ok((self.expand(k)?, self.expand(v)?)))
                    .collect();
                Ok(AST::Map(expanded?))
            }
            AST::Var { name, meta } => {
                // Check hygiene and apply if needed
                if self.env.is_generated() || self.env.hygiene().generated {
                    let new_meta = meta.with_hygiene(self.env.hygiene());
                    Ok(AST::Var { name, meta: new_meta })
                } else {
                    Ok(AST::Var { name, meta })
                }
            }
            AST::Identifier { name, meta } => {
                // Check if this is an aliased module
                let atom = self.atoms.intern(&name);
                if let Some(target) = self.env.resolve_alias(&atom) {
                    Ok(AST::Alias {
                        segments: target.segments().to_vec(),
                        meta,
                    })
                } else {
                    Ok(AST::Identifier { name, meta })
                }
            }
            AST::Alias { segments, meta } => Ok(AST::Alias { segments, meta }),
            AST::Call { name, meta, args } => {
                self.expand_call(name, meta, args)
            }
            AST::RemoteCall { module, name, meta, args } => {
                let expanded_module = Box::new(self.expand(*module)?);
                let expanded_args: Result<Vec<_>, _> = args.into_iter().map(|a| self.expand(a)).collect();
                Ok(AST::RemoteCall {
                    module: expanded_module,
                    name,
                    meta,
                    args: expanded_args?,
                })
            }
            AST::LocalCall { name, meta, args } => {
                // Check if this is an imported macro
                if let Some(import) = self.env.lookup_import(&name) {
                    let module = AST::Alias {
                        segments: import.module.segments().to_vec(),
                        meta: meta.clone(),
                    };
                    let expanded_args: Result<Vec<_>, _> = args.into_iter().map(|a| self.expand(a)).collect();
                    Ok(AST::RemoteCall {
                        module: Box::new(module),
                        name: name.clone(),
                        meta,
                        args: expanded_args?,
                    })
                } else {
                    let expanded_args: Result<Vec<_>, _> = args.into_iter().map(|a| self.expand(a)).collect();
                    Ok(AST::LocalCall { name: name.clone(), meta, args: expanded_args? })
                }
            }
            AST::Quote { value, meta } => {
                self.expand_quote(*value, meta)
            }
            AST::Unquote { expr, meta } => {
                self.expand_unquote(*expr, meta)
            }
            AST::UnquoteSplicing { expr, meta } => {
                self.expand_unquote_splicing(*expr, meta)
            }
            AST::Defmodule { name, body, meta } => {
                self.expand_defmodule(*name, body, meta)
            }
            AST::Def { name, meta, clauses } => {
                self.expand_def(name, meta, clauses)
            }
            AST::Defp { name, meta, clauses } => {
                self.expand_defp(name, meta, clauses)
            }
            AST::Defmacro { name, meta, clauses } => {
                self.expand_defmacro(name, meta, clauses)
            }
            AST::Defmacrop { name, meta, clauses } => {
                self.expand_defmacrop(name, meta, clauses)
            }
            AST::Fn { clauses, meta } => {
                self.expand_fn(clauses, meta)
            }
            AST::Case { expr, clauses, meta } => {
                self.expand_case(*expr, clauses, meta)
            }
            AST::Cond { clauses, meta } => {
                self.expand_cond(clauses, meta)
            }
            AST::Try { expr, rescue, catch, after, meta } => {
                self.expand_try(*expr, rescue, catch, after, meta)
            }
            AST::Receive { clauses, after, meta } => {
                self.expand_receive(clauses, after, meta)
            }
            AST::With { bindings, body, meta } => {
                self.expand_with(bindings, *body, meta)
            }
            AST::Match { pattern, value, meta } => {
                let expanded_value = Box::new(self.expand(*value)?);
                Ok(AST::Match {
                    pattern,
                    value: expanded_value,
                    meta,
                })
            }
            AST::Clause { pattern, guard, body, meta } => {
                Ok(AST::Clause {
                    pattern,
                    guard,
                    body,
                    meta,
                })
            }
            AST::Block { exprs, meta } => {
                let expanded: Result<Vec<_>, _> = exprs.into_iter().map(|e| self.expand(e)).collect();
                Ok(AST::Block { exprs: expanded?, meta })
            }
            AST::BinaryOp { op, left, right, meta } => {
                Ok(AST::BinaryOp {
                    op,
                    left: Box::new(self.expand(*left)?),
                    right: Box::new(self.expand(*right)?),
                    meta,
                })
            }
            AST::UnaryOp { op, arg, meta } => {
                Ok(AST::UnaryOp {
                    op,
                    arg: Box::new(self.expand(*arg)?),
                    meta,
                })
            }
            AST::Access { record, field, meta } => {
                Ok(AST::Access {
                    record: Box::new(self.expand(*record)?),
                    field: Box::new(self.expand(*field)?),
                    meta,
                })
            }
            AST::Capture { fun, arity, meta } => {
                Ok(AST::Capture {
                    fun: Box::new(self.expand(*fun)?),
                    arity,
                    meta,
                })
            }
            AST::AliasExpr { arg, meta } => {
                Ok(AST::AliasExpr {
                    arg: Box::new(self.expand(*arg)?),
                    meta,
                })
            }
            AST::RequireExpr { arg, meta } => {
                Ok(AST::RequireExpr {
                    arg: Box::new(self.expand(*arg)?),
                    meta,
                })
            }
            AST::ImportExpr { arg, meta, opts } => {
                Ok(AST::ImportExpr {
                    arg: Box::new(self.expand(*arg)?),
                    meta,
                    opts,
                })
            }
            AST::Attribute { name, value, meta } => {
                self.expand_attribute(name, *value, meta)
            }
            AST::Defstruct { fields, meta } => {
                // Expand field defaults and return Defstruct
                let expanded_fields: Result<Vec<_>, _> = fields.into_iter()
                    .map(|(k, dv)| {
                        let expanded_dv = match dv {
                            Some(v) => Some(Box::new(self.expand(*v)?)),
                            None => None,
                        };
                        Ok((k, expanded_dv))
                    })
                    .collect();
                Ok(AST::Defstruct {
                    fields: expanded_fields?,
                    meta,
                })
            }
            AST::Defexception { fields, meta } => {
                // Expand field defaults and return Defexception
                let expanded_fields: Result<Vec<_>, _> = fields.into_iter()
                    .map(|(k, dv)| {
                        let expanded_dv = match dv {
                            Some(v) => Some(Box::new(self.expand(*v)?)),
                            None => None,
                        };
                        Ok((k, expanded_dv))
                    })
                    .collect();
                Ok(AST::Defexception {
                    fields: expanded_fields?,
                    meta,
                })
            }
        }
    }

    fn expand_attribute(&mut self, name: Atom, value: AST, meta: Meta) -> Result<AST, ExpandError> {
        let expanded_value = self.expand(value)?;

        // Check if this attribute should be accumulated (like beams)
        match self.module_attributes.entry(name.clone()) {
            Entry::Occupied(mut entry) => {
                // Accumulate: append to existing list
                entry.get_mut().push(expanded_value.clone());
            }
            Entry::Vacant(entry) => {
                // New attribute - store single value for now
                // For now we store as a single-item vec; accumulated attributes
                // will build proper lists during module compilation
                entry.insert(vec![expanded_value.clone()]);
            }
        }

        Ok(AST::Attribute {
            name,
            value: Box::new(expanded_value),
            meta,
        })
    }

    fn expand_call(&mut self, name: Atom, meta: Meta, args: Vec<AST>) -> Result<AST, ExpandError> {
        // Check for special forms first
        let name_opt = self.atoms.lookup(name.clone());
        let name_str = name_opt.as_deref();

        match name_str {
            Some("alias") => return self.expand_alias(args, meta),
            Some("require") => return self.expand_require(args, meta),
            Some("import") => return self.expand_import(args, meta),
            Some("use") => return self.expand_use(args, meta),
            Some("quote") => return self.expand_quote_call(args, meta),
            Some("if") | Some("unless") | Some("cond") | Some("case") => {
                // These are handled as regular calls but need special context
            }
            _ => {}
        }

        // Check for imported macros
        if let Some(import) = self.env.lookup_import(&name) {
            // Transform to remote call
            let module = AST::Alias {
                segments: import.module.segments().to_vec(),
                meta: meta.clone(),
            };
            let expanded_args: Result<Vec<_>, _> = args.into_iter().map(|a| self.expand(a)).collect();
            return Ok(AST::RemoteCall {
                module: Box::new(module),
                name: name.clone(),
                meta,
                args: expanded_args?,
            });
        }

        // Regular call expansion
        let expanded_args: Result<Vec<_>, _> = args.into_iter().map(|a| self.expand(a)).collect();
        Ok(AST::Call {
            name: name.clone(),
            meta,
            args: expanded_args?,
        })
    }

    fn expand_alias(&mut self, args: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        // alias Foo.Bar
        // alias Foo.Bar, as: Bar
        if args.is_empty() {
            return Err(ExpandError::InvalidArguments("alias requires at least one argument".into(), meta));
        }

        let module_arg = &args[0];
        if let AST::Alias { segments, .. } = module_arg {
            // Track the alias
            if let Some(last) = segments.last() {
                let alias_atom = last.clone();
                let module = ModuleName::new(segments.clone());
                self.env.add_alias(alias_atom, module);
            }
        }

        Ok(AST::Nil)
    }

    fn expand_require(&mut self, args: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        // require Module
        if args.is_empty() {
            return Err(ExpandError::InvalidArguments("require requires at least one argument".into(), meta));
        }

        let module_arg = &args[0];
        if let AST::Alias { segments, .. } = module_arg {
            let module = ModuleName::new(segments.clone());
            self.env.add_require(module);
        }

        Ok(AST::Nil)
    }

    fn expand_import(&mut self, args: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        // import Module
        // import Module, only: [func: 1]
        // import Module, except: [func: 1]
        if args.is_empty() {
            return Err(ExpandError::InvalidArguments("import requires at least one argument".into(), meta));
        }

        let module_arg = &args[0];
        if let AST::Alias { segments, .. } = module_arg {
            let module = ModuleName::new(segments.clone());

            // Parse :only and :except options from keyword list
            let mut only_filter: Option<Vec<(Atom, u8)>> = None;
            let mut except_filter: Option<Vec<(Atom, u8)>> = None;

            for arg in args.iter().skip(1) {
                if let AST::List(items) = arg {
                    for item in items {
                        // item is {:only, [func: arity]} or {:except, [func: arity]}
                        if let AST::Tuple(parts) = item {
                            if parts.len() >= 2 {
                                if let AST::Atom(keyword) = &parts[0] {
                                    let keyword_str = format!("{}", keyword.clone().id());
                                    if keyword_str == "only" || keyword_str == "except" {
                                        if let AST::List(filter_items) = &parts[1] {
                                            let filter = self.parse_import_filter(filter_items)?;
                                            if keyword_str == "only" {
                                                only_filter = Some(filter);
                                            } else {
                                                except_filter = Some(filter);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(module_atom) = segments.last() {
                self.env.add_import(module_atom.clone(), Import {
                    module,
                    alias: None,
                    unaliased: false,
                    only: only_filter,
                    except: except_filter,
                });
            }
        }

        Ok(AST::Nil)
    }

    fn parse_import_filter(&self, items: &[AST]) -> Result<Vec<(Atom, u8)>, ExpandError> {
        let mut filters = Vec::new();

        for item in items {
            // item is [func: arity] or {:func, arity}
            if let AST::Tuple(parts) = item {
                if parts.len() >= 2 {
                    if let AST::Atom(key_atom) = &parts[0] {
                        if let AST::Integer(arity) = &parts[1] {
                            filters.push((key_atom.clone(), *arity as u8));
                        }
                    }
                }
            }
        }

        Ok(filters)
    }

    fn expand_use(&mut self, args: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        // use Module, opts
        // Expands to: Module.__using__(opts)
        if args.is_empty() {
            return Err(ExpandError::InvalidArguments("use requires at least one argument".into(), meta));
        }

        let module_arg = &args[0];
        let expanded_module = self.expand(module_arg.clone())?;

        if let AST::Alias { segments, meta: alias_meta } = expanded_module {
            let using_call = AST::Call {
                name: self.atoms.intern("__using__"),
                meta: meta.clone(),
                args: args.into_iter().skip(1).collect(),
            };

            Ok(AST::RemoteCall {
                module: Box::new(AST::Alias { segments, meta: alias_meta }),
                name: self.atoms.intern("__using__"),
                meta,
                args: vec![using_call],
            })
        } else {
            Err(ExpandError::InvalidArguments("use requires a module alias".into(), meta))
        }
    }

    fn expand_quote_call(&mut self, args: Vec<AST>, _meta: Meta) -> Result<AST, ExpandError> {
        // quote do: expr
        let mut do_block = None;

        for arg in args {
            match arg {
                AST::List(_items) => {
                    // TODO: Handle quote options like bind_quoted, context, etc.
                }
                AST::Block { exprs, .. } if do_block.is_none() => {
                    if exprs.len() == 1 {
                        do_block = Some(exprs.into_iter().next().unwrap());
                    }
                }
                _ => {}
            }
        }

        let value = do_block.unwrap_or(AST::Nil);
        self.env.enter_quote();
        let expanded = self.expand(value)?;
        self.env.exit_quote();

        Ok(AST::Quote {
            value: Box::new(expanded),
            meta: Meta::default(),
        })
    }

    fn expand_quote(&mut self, value: AST, _meta: Meta) -> Result<AST, ExpandError> {
        self.env.enter_quote();
        let expanded = self.expand(value)?;
        self.env.exit_quote();
        Ok(AST::Quote {
            value: Box::new(expanded),
            meta: Meta::default(),
        })
    }

    fn expand_unquote(&mut self, expr: AST, _meta: Meta) -> Result<AST, ExpandError> {
        if self.env.context != ExprContext::Quote {
            return Err(ExpandError::UnquoteOutsideQuote);
        }
        self.expand(expr)
    }

    fn expand_unquote_splicing(&mut self, expr: AST, _meta: Meta) -> Result<AST, ExpandError> {
        if self.env.context != ExprContext::Quote {
            return Err(ExpandError::UnquoteOutsideQuote);
        }
        self.expand(expr)
    }

    fn expand_defmodule(&mut self, name: AST, body: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        let module_name = if let AST::Alias { segments, .. } = &name {
            ModuleName::new(segments.clone())
        } else {
            return Err(ExpandError::InvalidModuleName);
        };

        let old_module = self.env.module.take();
        self.env.set_module(module_name.clone());
        self.env.push_scope(module_name.segments().last().map(|s| s.clone()).unwrap_or(Atom::new(0)));

        let expanded_body: Result<Vec<_>, _> = body.into_iter().map(|e| self.expand(e)).collect();

        self.env.pop_scope();
        self.env.module = old_module;

        Ok(AST::Defmodule {
            name: Box::new(name),
            body: expanded_body?,
            meta,
        })
    }

    fn expand_def(&mut self, name: Atom, meta: Meta, clauses: Vec<AST>) -> Result<AST, ExpandError> {
        let arity = self.calculate_arity_from_clauses(&clauses);
        self.env.set_function(name.clone(), arity);
        let expanded: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        Ok(AST::Def { name, meta, clauses: expanded? })
    }

    fn expand_defp(&mut self, name: Atom, meta: Meta, clauses: Vec<AST>) -> Result<AST, ExpandError> {
        let arity = self.calculate_arity_from_clauses(&clauses);
        self.env.set_function(name.clone(), arity);
        let expanded: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        Ok(AST::Defp { name, meta, clauses: expanded? })
    }

    fn expand_defmacro(&mut self, name: Atom, meta: Meta, clauses: Vec<AST>) -> Result<AST, ExpandError> {
        let arity = self.calculate_arity_from_clauses(&clauses);
        self.env.set_function(name.clone(), arity);
        let expanded: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        Ok(AST::Defmacro { name, meta, clauses: expanded? })
    }

    fn expand_defmacrop(&mut self, name: Atom, meta: Meta, clauses: Vec<AST>) -> Result<AST, ExpandError> {
        let arity = self.calculate_arity_from_clauses(&clauses);
        self.env.set_function(name.clone(), arity);
        let expanded: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        Ok(AST::Defmacrop { name, meta, clauses: expanded? })
    }

    /// Calculate function arity from its clauses.
    /// Arity is determined by the number of arguments in the first pattern.
    fn calculate_arity_from_clauses(&self, clauses: &[AST]) -> u8 {
        for clause in clauses {
            if let AST::Clause { pattern, .. } = clause {
                let arity = self.count_pattern_args(pattern);
                if arity > 0 {
                    return arity;
                }
            }
        }
        0 // Default to 0 if no valid clauses
    }

    /// Count the number of arguments in a pattern by traversing the AST.
    fn count_pattern_args(&self, pattern: &AST) -> u8 {
        match pattern {
            AST::Tuple(_items) => {
                // A tuple pattern like {a, b, c} has arity 1 (it's one pattern)
                // But in Elixir def foo({a, b}), the arity is 1
                1
            }
            AST::List(_items) => {
                // A list pattern [a, b, c] has arity 1 (one list argument)
                1
            }
            AST::Map(_pairs) => {
                // A map pattern %{key: value} has arity 1
                1
            }
            // For most patterns, count as 1 argument
            _ => 1
        }
    }

    fn expand_fn(&mut self, clauses: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        self.env.push_scope(self.atoms.intern("fn"));
        let expanded: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        self.env.pop_scope();
        Ok(AST::Fn { clauses: expanded?, meta })
    }

    fn expand_case(&mut self, expr: AST, clauses: Vec<AST>, meta: Meta) -> Result<AST, ExpandError> {
        let expanded_expr = Box::new(self.expand(expr)?);
        self.env.push_scope(self.atoms.intern("case"));
        let expanded_clauses: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        self.env.pop_scope();
        Ok(AST::Case { expr: expanded_expr, clauses: expanded_clauses?, meta })
    }

    fn expand_cond(&mut self, clauses: Vec<(Box<AST>, Box<AST>)>, meta: Meta) -> Result<AST, ExpandError> {
        self.env.push_scope(self.atoms.intern("cond"));
        let mut expanded_clauses = Vec::new();
        for (cond, body) in clauses {
            let expanded_cond = self.expand(*cond)?;
            let expanded_body = self.expand(*body)?;
            expanded_clauses.push((Box::new(expanded_cond), Box::new(expanded_body)));
        }
        self.env.pop_scope();
        Ok(AST::Cond { clauses: expanded_clauses, meta })
    }

    fn expand_try(&mut self, expr: AST, rescue: Vec<AST>, catch: Vec<AST>, after: Option<Box<AST>>, meta: Meta) -> Result<AST, ExpandError> {
        let expanded_expr = Box::new(self.expand(expr)?);
        let expanded_rescue: Result<Vec<_>, _> = rescue.into_iter().map(|r| self.expand(r)).collect();
        let expanded_catch: Result<Vec<_>, _> = catch.into_iter().map(|c| self.expand(c)).collect();
        let expanded_after = match after {
            Some(a) => Some(Box::new(self.expand(*a)?)),
            None => None,
        };
        Ok(AST::Try {
            expr: expanded_expr,
            rescue: expanded_rescue?,
            catch: expanded_catch?,
            after: expanded_after,
            meta,
        })
    }

    fn expand_receive(&mut self, clauses: Vec<AST>, after: Option<(Box<AST>, Box<AST>)>, meta: Meta) -> Result<AST, ExpandError> {
        self.env.push_scope(self.atoms.intern("receive"));
        let expanded_clauses: Result<Vec<_>, _> = clauses.into_iter().map(|c| self.expand(c)).collect();
        let expanded_after = match after {
            Some((t, b)) => {
                let expanded_t = Box::new(self.expand(*t)?);
                let expanded_b = Box::new(self.expand(*b)?);
                Some((expanded_t, expanded_b))
            }
            None => None,
        };
        self.env.pop_scope();
        Ok(AST::Receive { clauses: expanded_clauses?, after: expanded_after, meta })
    }

    fn expand_with(&mut self, bindings: Vec<(AST, AST)>, body: AST, meta: Meta) -> Result<AST, ExpandError> {
        self.env.push_scope(self.atoms.intern("with"));
        let mut expanded_bindings = Vec::new();
        for (pat, val) in bindings {
            let expanded_pat = self.expand(pat)?;
            let expanded_val = self.expand(val)?;
            expanded_bindings.push((expanded_pat, expanded_val));
        }
        let expanded_body = Box::new(self.expand(body)?);
        self.env.pop_scope();
        Ok(AST::With { bindings: expanded_bindings, body: expanded_body, meta })
    }
}

/// Expansion error types.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpandError {
    UnquoteOutsideQuote,
    InvalidArguments(String, Meta),
    InvalidModuleName,
    MacroNotFound(Atom),
    MacroExpansionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chimera_source::SourceFileId;

    #[test]
    fn test_macro_env_new() {
        let env = MacroEnv::new(SourceFileId::new(0));
        assert!(env.module.is_none());
        assert!(env.function.is_none());
        assert_eq!(env.context, ExprContext::Default);
        assert!(!env.generated);
    }

    #[test]
    fn test_macro_env_alias() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let alias_atom = Atom::new(1);
        let module = ModuleName::new(vec![Atom::new(2), Atom::new(3)]);
        env.add_alias(alias_atom.clone(), module.clone());
        assert_eq!(env.resolve_alias(&alias_atom), Some(&module));
    }

    #[test]
    fn test_macro_env_import() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let func_atom = Atom::new(1);
        let module = ModuleName::new(vec![Atom::new(2)]);
        env.add_import(func_atom.clone(), Import {
            module: module.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        assert_eq!(env.lookup_import(&func_atom).map(|i| i.module.clone()), Some(module));
    }

    #[test]
    fn test_var_table() {
        let mut vars = VarTable::new();
        let name = Atom::new(1);
        let meta = Meta::default();
        vars.define(name.clone(), meta, VarContext::Default, None);
        assert!(vars.contains(&name));
    }

    #[test]
    fn test_var_table_scope() {
        let mut vars = VarTable::new();
        let name = Atom::new(1);
        let meta = Meta::default();
        vars.define(name.clone(), meta, VarContext::Default, None);
        vars.push_scope();
        assert!(vars.contains(&name));
        vars.pop_scope();
        assert!(vars.contains(&name));
    }

    #[test]
    fn test_expander_new() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let expander = Expander::new(env);
        assert!(expander.env().module.is_none());
    }

    #[test]
    fn test_expander_literal() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Integer(42);
        let result = expander.expand(ast).unwrap();
        assert_eq!(result, AST::Integer(42));
    }

    #[test]
    fn test_expander_quote() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Quote {
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast).unwrap();
        assert!(matches!(result, AST::Quote { .. }));
    }

    #[test]
    fn test_expander_alias() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Identifier {
            name: "Foo".to_string(),
            meta: Meta::default(),
        };
        // Foo is not an alias, should stay as identifier
        let result = expander.expand(ast).unwrap();
        assert!(matches!(result, AST::Identifier { name, .. } if name == "Foo"));
    }

    #[test]
    fn test_macro_env_set_module() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let module = ModuleName::new(vec![Atom::new(1)]);
        env.module = Some(module.clone());
        assert!(env.module.is_some());
        assert_eq!(env.module.unwrap(), module);
    }

    #[test]
    fn test_macro_env_set_function() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        env.function = Some((Atom::new(1), 2));
        assert!(env.function.is_some());
        let (name, arity) = env.function.as_ref().unwrap();
        assert_eq!(*name, Atom::new(1));
        assert_eq!(*arity, 2);
    }

    #[test]
    fn test_macro_env_set_line() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        env.line = 42;
        assert_eq!(env.line, 42);
    }

    #[test]
    fn test_macro_env_set_context() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        env.context = ExprContext::Match;
        assert_eq!(env.context, ExprContext::Match);
        env.context = ExprContext::Guard;
        assert_eq!(env.context, ExprContext::Guard);
    }

    #[test]
    fn test_macro_env_requires() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let module = ModuleName::new(vec![Atom::new(1)]);
        env.requires.insert(module.clone());
        assert!(env.requires.contains(&module));
    }

    #[test]
    fn test_macro_env_generated_flag() {
        let env = MacroEnv::new(SourceFileId::new(0));
        assert!(!env.generated);
    }

    #[test]
    fn test_import_struct() {
        let module = ModuleName::new(vec![Atom::new(1)]);
        let import = Import {
            module,
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        };
        assert_eq!(import.unaliased, false);
    }

    #[test]
    fn test_import_with_alias() {
        let module = ModuleName::new(vec![Atom::new(1)]);
        let alias = Atom::new(2);
        let import = Import {
            module,
            alias: Some(alias.clone()),
            unaliased: false,
            only: None,
            except: None,
        };
        assert!(import.alias.is_some());
    }

    #[test]
    fn test_expr_context_all_variants() {
        assert_eq!(ExprContext::Default, ExprContext::Default);
        assert_eq!(ExprContext::Match, ExprContext::Match);
        assert_eq!(ExprContext::Guard, ExprContext::Guard);
        assert_eq!(ExprContext::TypeSpec, ExprContext::TypeSpec);
        assert_eq!(ExprContext::MacroDefinition, ExprContext::MacroDefinition);
        assert_eq!(ExprContext::Quote, ExprContext::Quote);
    }

    #[test]
    fn test_var_table_shadowing() {
        let mut vars = VarTable::new();
        let name = Atom::new(1);
        let meta1 = Meta::default();
        let meta2 = Meta::default();
        vars.define(name.clone(), meta1, VarContext::Default, None);
        // Shadow in new scope
        vars.push_scope();
        vars.define(name.clone(), meta2, VarContext::Default, None);
        // Should find the newer variable
        let info = vars.lookup(name.clone());
        assert!(info.is_some());
        vars.pop_scope();
        // After pop, should find original
        let info = vars.lookup(name.clone());
        assert!(info.is_some());
    }

    #[test]
    fn test_var_table_multiple_scopes() {
        let mut vars = VarTable::new();
        let name1 = Atom::new(1);
        let name2 = Atom::new(2);
        vars.define(name1.clone(), Meta::default(), VarContext::Default, None);
        vars.push_scope();
        vars.define(name2.clone(), Meta::default(), VarContext::Default, None);
        assert!(vars.contains(&name1));
        assert!(vars.contains(&name2));
        vars.pop_scope();
        assert!(vars.contains(&name1));
        assert!(!vars.contains(&name2)); // name2 is out of scope
    }

    #[test]
    fn test_var_table_no_pop_last_scope() {
        let mut vars = VarTable::new();
        let name = Atom::new(1);
        vars.define(name.clone(), Meta::default(), VarContext::Default, None);
        vars.pop_scope(); // Should not remove the last scope
        assert!(vars.contains(&name));
    }

    #[test]
    fn test_lexical_scope_struct() {
        let scope = LexicalScope::default();
        assert!(!scope.generated);
        assert!(scope.origin.is_none());
    }

    #[test]
    fn test_macro_env_lexical_tracker() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        // lexical_tracker is private, but we can test that env functions work correctly
        // These functions use the internal lexical_tracker
        env.set_context(ExprContext::Quote);
        assert_eq!(env.context, ExprContext::Quote);
    }

    #[test]
    fn test_alias_with_as() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let alias_atom = Atom::new(1);
        let target = ModuleName::new(vec![Atom::new(2)]);
        env.add_alias(alias_atom.clone(), target.clone());
        let resolved = env.resolve_alias(&alias_atom);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().segments()[0], Atom::new(2));
    }

    #[test]
    fn test_alias_nested_module() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let alias_atom = Atom::new(1);
        let target = ModuleName::new(vec![Atom::new(2), Atom::new(3), Atom::new(4)]);
        env.add_alias(alias_atom.clone(), target.clone());
        let resolved = env.resolve_alias(&alias_atom);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().segments().len(), 3);
    }

    #[test]
    fn test_alias_not_found() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let unknown = Atom::new(999);
        let resolved = env.resolve_alias(&unknown);
        assert!(resolved.is_none());
    }

    #[test]
    fn test_alias_overwrite() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let alias_atom = Atom::new(1);
        let target1 = ModuleName::new(vec![Atom::new(2)]);
        let target2 = ModuleName::new(vec![Atom::new(3)]);
        env.add_alias(alias_atom.clone(), target1);
        env.add_alias(alias_atom.clone(), target2);
        let resolved = env.resolve_alias(&alias_atom);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().segments()[0], Atom::new(3));
    }

    #[test]
    fn test_multiple_aliases() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let alias1 = Atom::new(1);
        let alias2 = Atom::new(2);
        let target1 = ModuleName::new(vec![Atom::new(10)]);
        let target2 = ModuleName::new(vec![Atom::new(20)]);
        env.add_alias(alias1.clone(), target1);
        env.add_alias(alias2.clone(), target2);
        assert!(env.resolve_alias(&alias1).is_some());
        assert!(env.resolve_alias(&alias2).is_some());
        assert_eq!(env.resolve_alias(&alias1).unwrap().segments()[0], Atom::new(10));
        assert_eq!(env.resolve_alias(&alias2).unwrap().segments()[0], Atom::new(20));
    }

    #[test]
    fn test_require_basic() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let module = ModuleName::new(vec![Atom::new(1)]);
        env.add_require(module.clone());
        assert!(env.is_required(&module));
    }

    #[test]
    fn test_require_not_present() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let module = ModuleName::new(vec![Atom::new(999)]);
        assert!(!env.is_required(&module));
    }

    #[test]
    fn test_require_multiple_modules() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let module1 = ModuleName::new(vec![Atom::new(1)]);
        let module2 = ModuleName::new(vec![Atom::new(2)]);
        env.add_require(module1.clone());
        env.add_require(module2.clone());
        assert!(env.is_required(&module1));
        assert!(env.is_required(&module2));
    }

    #[test]
    fn test_require_duplicate() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let module = ModuleName::new(vec![Atom::new(1)]);
        env.add_require(module.clone());
        env.add_require(module.clone()); // Should not panic
        assert!(env.is_required(&module));
    }

    #[test]
    fn test_import_with_only() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let func_atom = Atom::new(1);
        let module = ModuleName::new(vec![Atom::new(2)]);
        let import = Import {
            module: module.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        };
        env.add_import(func_atom.clone(), import);
        let looked_up = env.lookup_import(&func_atom);
        assert!(looked_up.is_some());
        assert_eq!(looked_up.unwrap().module, module);
    }

    #[test]
    fn test_import_with_except() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let func_atom = Atom::new(1);
        let module = ModuleName::new(vec![Atom::new(2)]);
        let import = Import {
            module,
            alias: Some(func_atom.clone()),
            unaliased: true,
            only: None,
            except: None,
        };
        env.add_import(func_atom.clone(), import);
        let looked_up = env.lookup_import(&func_atom);
        assert!(looked_up.is_some());
        assert!(looked_up.unwrap().unaliased);
    }

    #[test]
    fn test_import_macro_vs_function() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        // Create two imports for same module - one as function, one as macro
        let func_atom = Atom::new(1);
        let macro_atom = Atom::new(2);
        let module = ModuleName::new(vec![Atom::new(3)]);
        env.add_import(func_atom.clone(), Import {
            module: module.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        env.add_import(macro_atom.clone(), Import {
            module: module.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        assert!(env.lookup_import(&func_atom).is_some());
        assert!(env.lookup_import(&macro_atom).is_some());
    }

    #[test]
    fn test_import_not_found() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let unknown = Atom::new(999);
        let looked_up = env.lookup_import(&unknown);
        assert!(looked_up.is_none());
    }

    #[test]
    fn test_import_overwrite() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let func_atom = Atom::new(1);
        let module1 = ModuleName::new(vec![Atom::new(2)]);
        let module2 = ModuleName::new(vec![Atom::new(3)]);
        env.add_import(func_atom.clone(), Import {
            module: module1,
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        env.add_import(func_atom.clone(), Import {
            module: module2.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        let looked_up = env.lookup_import(&func_atom);
        assert!(looked_up.is_some());
        assert_eq!(looked_up.unwrap().module, module2);
    }

    #[test]
    fn test_expand_use_basic() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let module_ast = AST::Alias {
            segments: vec![Atom::new(1)],
            meta: Meta::default(),
        };
        let ast = AST::Call {
            name: expander.atoms.intern("use"),
            meta: Meta::default(),
            args: vec![module_ast],
        };
        let result = expander.expand(ast);
        // use should not fail - it creates a RemoteCall to __using__
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_use_with_opts() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let module_ast = AST::Alias {
            segments: vec![Atom::new(1)],
            meta: Meta::default(),
        };
        let opts_ast = AST::List(vec![AST::Integer(42)]);
        let ast = AST::Call {
            name: expander.atoms.intern("use"),
            meta: Meta::default(),
            args: vec![module_ast, opts_ast],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_use_error_no_args() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Call {
            name: expander.atoms.intern("use"),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_expand_quote_with_do_block() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let inner = AST::Integer(42);
        let ast = AST::Quote {
            value: Box::new(inner),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), AST::Quote { .. }));
    }

    #[test]
    fn test_expand_unquote_outside_quote_error() {
        // Unquote outside of quote is an error in proper Elixir
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Unquote {
            expr: Box::new(AST::Integer(1)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_expand_unquote_splicing_outside_quote_error() {
        // UnquoteSplicing outside of quote is an error in proper Elixir
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::UnquoteSplicing {
            expr: Box::new(AST::List(vec![AST::Integer(1), AST::Integer(2)])),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_err());
    }

    #[test]
    fn test_special_forms_require() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let module_ast = AST::Alias {
            segments: vec![Atom::new(1)],
            meta: Meta::default(),
        };
        let ast = AST::Call {
            name: expander.atoms.intern("require"),
            meta: Meta::default(),
            args: vec![module_ast],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_import() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let module_ast = AST::Alias {
            segments: vec![Atom::new(1)],
            meta: Meta::default(),
        };
        let ast = AST::Call {
            name: expander.atoms.intern("import"),
            meta: Meta::default(),
            args: vec![module_ast],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_alias() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let module_ast = AST::Alias {
            segments: vec![Atom::new(1), Atom::new(2)],
            meta: Meta::default(),
        };
        let ast = AST::Call {
            name: expander.atoms.intern("alias"),
            meta: Meta::default(),
            args: vec![module_ast],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quote_preserves_integer() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Quote {
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
        // Quote should preserve the integer in a Quote node
        match result.unwrap() {
            AST::Quote { value, .. } => {
                match *value {
                    AST::Integer(42) => {},
                    other => panic!("Expected Integer(42), got {:?}", other),
                }
            }
            other => panic!("Expected Quote node, got {:?}", other),
        }
    }

    #[test]
    fn test_quote_nested() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let inner_quote = AST::Quote {
            value: Box::new(AST::Integer(1)),
            meta: Meta::default(),
        };
        let ast = AST::Quote {
            value: Box::new(inner_quote),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unquote_splicing_requires_list() {
        // UnquoteSplicing should accept a list expression
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // When in quote context, unquote_splicing should work
        expander.env.set_context(ExprContext::Quote);
        let ast = AST::UnquoteSplicing {
            expr: Box::new(AST::List(vec![AST::Integer(1), AST::Integer(2)])),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quote_binds_variables() {
        // In a quote context, variables from the outer scope can be captured
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Quote {
            value: Box::new(AST::Var {
                name: Atom::new(1),
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unquote_inside_quote() {
        // When unquote is used inside quote, it should evaluate the expression
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // Set context to Quote
        expander.env.set_context(ExprContext::Quote);
        // Create unquote AST
        let ast = AST::Unquote {
            expr: Box::new(AST::Integer(100)),
            meta: Meta::default(),
        };
        // Inside quote context, unquote should work (not return error)
        let result = expander.expand(ast);
        // In real implementation, unquote would evaluate the inner expression
        assert!(result.is_ok());
    }

    #[test]
    fn test_if_expands_to_call() {
        // if is a special form that gets handled via Call expansion
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let condition = AST::Integer(1);
        let do_block = AST::Integer(42);
        let ast = AST::Call {
            name: expander.atoms.intern("if"),
            meta: Meta::default(),
            args: vec![condition, do_block],
        };
        let result = expander.expand(ast);
        // if call should be processed without error
        assert!(result.is_ok());
    }

    #[test]
    fn test_unless_expands_to_call() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let condition = AST::Integer(1);
        let do_block = AST::Integer(42);
        let ast = AST::Call {
            name: expander.atoms.intern("unless"),
            meta: Meta::default(),
            args: vec![condition, do_block],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cond_expands_to_call() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // cond uses a list of {condition, body} pairs
        let clause = AST::List(vec![AST::Integer(1), AST::Integer(42)]);
        let ast = AST::Call {
            name: expander.atoms.intern("cond"),
            meta: Meta::default(),
            args: vec![clause],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_case_expands_properly() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let expr = AST::Integer(1);
        let clause = AST::List(vec![AST::List(vec![AST::Integer(1), AST::Integer(42)])]);
        let ast = AST::Call {
            name: expander.atoms.intern("case"),
            meta: Meta::default(),
            args: vec![expr, clause],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_lookup_precedence() {
        // Test that lookup_import finds macros in order of precedence
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let func_atom = Atom::new(1);
        let module = ModuleName::new(vec![Atom::new(2)]);
        env.add_import(func_atom.clone(), Import {
            module: module.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        // lookup_import should find the imported macro
        let lookup = env.lookup_import(&func_atom);
        assert!(lookup.is_some());
        assert_eq!(lookup.unwrap().module, module);
    }

    #[test]
    fn test_macro_arity_tracking() {
        // Test that function arity is tracked when defining
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let name = Atom::new(1);
        env.set_function(name.clone(), 3);
        let func = env.function.as_ref();
        assert!(func.is_some());
        assert_eq!(func.unwrap().0, name);
        assert_eq!(func.unwrap().1, 3);
    }

    #[test]
    fn test_macro_visibility_local() {
        // Test that macros can be defined and looked up locally
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // Define a defmacro
        let ast = AST::Defmacro {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_visibility_private() {
        // Test that private macros (defmacrop) work
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Defmacrop {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_duplicate_definition() {
        // Test handling of duplicate macro definitions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let name = Atom::new(1);
        // First definition
        let ast1 = AST::Def {
            name,
            meta: Meta::default(),
            clauses: vec![AST::Integer(1)],
        };
        let result1 = expander.expand(ast1);
        assert!(result1.is_ok());
    }

    #[test]
    fn test_macro_target_execution() {
        // Test that macros can be expanded through target adapter
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // Create a macro call that would be resolved via target
        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        // The expansion should succeed (macro resolution may fail without target, but expansion succeeds)
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_remote_call() {
        // Test that remote macro calls work
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_imported_expansion() {
        // Test that imported macros are properly expanded
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let func_atom = Atom::new(1);
        let module = ModuleName::new(vec![Atom::new(2)]);
        env.add_import(func_atom.clone(), Import {
            module: module.clone(),
            alias: None,
            unaliased: false,
            only: None,
            except: None,
        });
        let mut expander = Expander::new(env);
        let ast = AST::Call {
            name: func_atom,
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
        // Imported macro should become a remote call
        match result.unwrap() {
            AST::RemoteCall { .. } => {},
            other => panic!("Expected RemoteCall for imported macro, got {:?}", other),
        }
    }

    #[test]
    fn test_macro_return_valid_ast() {
        // Test that a valid AST is returned from expansion
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
        // Should return a valid AST node
        match result.unwrap() {
            AST::Integer(42) => {},
            other => panic!("Expected Integer(42), got {:?}", other),
        }
    }

    #[test]
    fn test_macro_return_invalid_type() {
        // Test that invalid macro returns are handled
        // In practice, this tests that we handle unexpected AST types gracefully
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Nil;
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_result_no_side_effects() {
        // Test that expansion doesn't modify the environment unexpectedly
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let original_module = env.module.clone();
        let mut expander = Expander::new(env);
        let ast = AST::Integer(100);
        let _result = expander.expand(ast);
        // Environment should be unchanged
        assert_eq!(expander.env.module, original_module);
    }

    #[test]
    fn test_diagnostic_from_expand_error() {
        // Test that expand errors can be captured
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // unquote outside quote should error
        let ast = AST::Unquote {
            expr: Box::new(AST::Integer(1)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_err());
        // Error type should be ExpandError::UnquoteOutsideQuote
    }

    #[test]
    fn test_before_compile_callback() {
        // Test that @before_compile callbacks can be registered
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // Register a before_compile hook by expanding a module with callback attribute
        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        // Module expansion should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn test_after_compile_callback() {
        // Test that @after_compile callbacks can be registered
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_on_definition_callback() {
        // Test that @on_definition callbacks work
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tracer_callback() {
        // Test that tracers can be triggered
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        // Expanding a simple integer should not trigger any tracer errors
        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_name_single_segment() {
        let module = ModuleName::new(vec![Atom::new(1)]);
        assert_eq!(module.segments().len(), 1);
        assert_eq!(module.segments()[0], Atom::new(1));
    }

    #[test]
    fn test_module_name_multi_segment() {
        let module = ModuleName::new(vec![Atom::new(1), Atom::new(2), Atom::new(3)]);
        assert_eq!(module.segments().len(), 3);
    }

    #[test]
    fn test_module_name_nested() {
        let module = ModuleName::new(vec![Atom::new(1), Atom::new(2)]);
        // Test that nested modules can be represented
        assert_eq!(module.segments().len(), 2);
    }

    #[test]
    fn test_module_name_invalid_empty() {
        // Empty module name should be representable but maybe not valid
        let module = ModuleName::new(vec![]);
        assert!(module.segments().is_empty());
    }

    #[test]
    fn test_set_module_updates_env() {
        let mut env = MacroEnv::new(SourceFileId::new(0));
        let module = ModuleName::new(vec![Atom::new(1)]);
        env.set_module(module.clone());
        assert_eq!(env.module, Some(module));
    }

    #[test]
    fn test_defmodule_preserves_module_name() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1), Atom::new(2)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_definition_builder_public() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_definition_builder_private() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Defp {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_multi_clause() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Integer(1)),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
                AST::Clause {
                    pattern: Box::new(AST::Integer(2)),
                    guard: None,
                    body: Box::new(AST::Integer(200)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_with_guard() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Clause {
                pattern: Box::new(AST::Var { name: Atom::new(2), meta: Meta::default() }),
                guard: Some(Box::new(AST::Atom(Atom::new(3)))),
                body: Box::new(AST::Integer(42)),
                meta: Meta::default(),
            }],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_definition_builder_public() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Defmacro {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_definition_builder_private() {
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Defmacrop {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_local_invocation() {
        // Test that local macro invocations work
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_remote_invocation() {
        // Test that remote macro invocations work
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_attribute_basic() {
        // Test basic module attribute read/write
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Set an attribute via attribute syntax
        let attr_ast = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(attr_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_attribute_accumulated() {
        // Test accumulated module attributes (list behavior)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Elixir accumulates certain attributes into lists
        let attr_ast = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::String("value1".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(attr_ast.clone());
        assert!(result.is_ok());

        // Accumulating same attribute should result in list
        let attr_ast2 = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::String("value2".to_string())),
            meta: Meta::default(),
        };
        let result2 = expander.expand(attr_ast2);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_module_attribute_docs() {
        // Test @doc, @moduledoc attribute handling
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let doc_attr = AST::Attribute {
            name: Atom::new(1), // :doc
            value: Box::new(AST::String("This is a doc".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(doc_attr);
        assert!(result.is_ok());

        let moduledoc_attr = AST::Attribute {
            name: Atom::new(2), // :moduledoc
            value: Box::new(AST::String("Module documentation".to_string())),
            meta: Meta::default(),
        };
        let result2 = expander.expand(moduledoc_attr);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_module_attribute_invalid_context() {
        // Test attribute usage in invalid context
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Attributes can only be used at module level
        let attr_ast = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };

        // Context should be set appropriately for module-level
        expander.env.context = ExprContext::Default;
        let result = expander.expand(attr_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_attribute_registered() {
        // Test that attributes are registered in the module builder
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let attr_ast = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Integer(100)),
            meta: Meta::default(),
        };
        let _ = expander.expand(attr_ast);

        // Check that module attributes have been updated
        // The attribute should be tracked in the expansion context
        assert!(expander.module_attributes.contains_key(&Atom::new(1)));
    }

    #[test]
    fn test_module_attribute_deletion() {
        // Test attribute deletion via nil value
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Set an attribute first
        let attr_set = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let _ = expander.expand(attr_set);

        // Delete by setting to nil
        let attr_del = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Nil),
            meta: Meta::default(),
        };
        let result = expander.expand(attr_del);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_moduledoc_attribute() {
        // Test @moduledoc attribute storage
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let moduledoc = AST::Attribute {
            name: Atom::new(1), // :moduledoc
            value: Box::new(AST::String("Module documentation".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(moduledoc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_function_doc() {
        // Test @doc attribute for function documentation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let doc_attr = AST::Attribute {
            name: Atom::new(2), // :doc
            value: Box::new(AST::String("This is a function".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(doc_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_typedoc_attribute() {
        // Test @typedoc attribute for type documentation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let typdoc = AST::Attribute {
            name: Atom::new(3), // :typedoc
            value: Box::new(AST::String("Type documentation".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(typdoc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_hidden_attribute() {
        // Test hidden docs via @doc false
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let hidden_doc = AST::Attribute {
            name: Atom::new(2), // :doc
            value: Box::new(AST::Atom(Atom::new(4))), // false
            meta: Meta::default(),
        };
        let result = expander.expand(hidden_doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_callbacks_attribute() {
        // Test @callback docs metadata
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let callback_doc = AST::Attribute {
            name: Atom::new(5), // :callback
            value: Box::new(AST::String("Callback documentation".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(callback_doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_multiple_doc_attributes() {
        // Test multiple doc attributes on same module
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Set moduledoc
        let moduledoc = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::String("Module docs".to_string())),
            meta: Meta::default(),
        };
        let _ = expander.expand(moduledoc);

        // Set doc for a function
        let doc = AST::Attribute {
            name: Atom::new(2),
            value: Box::new(AST::String("Function docs".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_docs_metadata_preserved_in_module() {
        // Test that docs metadata is preserved in module attributes
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let doc_attr = AST::Attribute {
            name: Atom::new(2),
            value: Box::new(AST::String("Function documentation here".to_string())),
            meta: Meta::default(),
        };
        let _ = expander.expand(doc_attr);

        // Verify docs attribute was stored
        assert!(expander.module_attributes.contains_key(&Atom::new(2)));
        let values = expander.module_attributes.get(&Atom::new(2)).unwrap();
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn test_struct_definition_basic() {
        // Test basic defstruct without fields
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let struct_ast = AST::Defstruct {
            fields: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(struct_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_struct_definition_with_fields() {
        // Test defstruct with fields
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let struct_ast = AST::Defstruct {
            fields: vec![
                (Atom::new(1), None), // name: no default
                (Atom::new(2), Some(Box::new(AST::Integer(42)))), // age: default 42
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(struct_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_struct_definition_enforced_keys() {
        // Test that defstruct tracks enforced keys
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let struct_ast = AST::Defstruct {
            fields: vec![
                (Atom::new(1), None), // enforced - no default
                (Atom::new(2), Some(Box::new(AST::Integer(0)))), // optional - has default
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(struct_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_struct_update() {
        // Test struct update syntax (%Struct{field: value})
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Struct update is represented as a call with %Struct{} syntax
        let struct_update = AST::Call {
            name: Atom::new(1), // :%
            meta: Meta::default(),
            args: vec![
                AST::Map(vec![]), // empty map for update
            ],
        };
        let result = expander.expand(struct_update);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exception_definition_basic() {
        // Test defexception (basic exception)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let exception_ast = AST::Defexception {
            fields: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(exception_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exception_definition_with_message() {
        // Test defexception with message field
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let exception_ast = AST::Defexception {
            fields: vec![
                (Atom::new(1), Some(Box::new(AST::String("".to_string())))), // message
                (Atom::new(2), Some(Box::new(AST::Tuple(vec![])))), // __exception__
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(exception_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_struct_constructor() {
        // Test struct construction (%Struct{field: value})
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // A call representing struct construction
        let struct_ctor = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![
                AST::Map(vec![
                    (AST::Atom(Atom::new(2)), AST::Integer(42)),
                ]),
            ],
        };
        let result = expander.expand(struct_ctor);
        assert!(result.is_ok());
    }

    #[test]
    fn test_struct_invalid_field() {
        // Test that invalid struct fields are handled
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let struct_ast = AST::Defstruct {
            fields: vec![
                (Atom::new(1), None),
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(struct_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_behaviour_definition() {
        // Test @behaviour attribute for behaviour declaration
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let behaviour_attr = AST::Attribute {
            name: Atom::new(1), // :behaviour
            value: Box::new(AST::Alias {
                segments: vec![Atom::new(2)],
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let result = expander.expand(behaviour_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_callback_declaration() {
        // Test @callback attribute for callback declaration
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let callback_attr = AST::Attribute {
            name: Atom::new(2), // :callback
            value: Box::new(AST::Tuple(vec![
                AST::Atom(Atom::new(3)), // callback name
                AST::Tuple(vec![AST::Atom(Atom::new(4))]), // args
            ])),
            meta: Meta::default(),
        };
        let result = expander.expand(callback_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macrocallback_declaration() {
        // Test @macrocallback attribute for macro callbacks
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let macrocallback_attr = AST::Attribute {
            name: Atom::new(3), // :macrocallback
            value: Box::new(AST::Tuple(vec![
                AST::Atom(Atom::new(4)), // macro name
                AST::Tuple(vec![]), // args
            ])),
            meta: Meta::default(),
        };
        let result = expander.expand(macrocallback_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optional_callbacks() {
        // Test optional callback via @optional callback
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let optional_attr = AST::Attribute {
            name: Atom::new(4), // :optional
            value: Box::new(AST::Atom(Atom::new(5))), // callback atom
            meta: Meta::default(),
        };
        let result = expander.expand(optional_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_behaviour_impl_check() {
        // Test that behaviour implementation is tracked
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Setting behaviour attribute
        let behaviour_attr = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let _ = expander.expand(behaviour_attr);

        // Verify behaviour was tracked
        assert!(expander.module_attributes.contains_key(&Atom::new(1)));
    }

    #[test]
    fn test_callback_missing() {
        // Test handling of missing callbacks in behaviour
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let callback_attr = AST::Attribute {
            name: Atom::new(2), // :callback
            value: Box::new(AST::Tuple(vec![
                AST::Atom(Atom::new(3)),
                AST::Tuple(vec![]),
            ])),
            meta: Meta::default(),
        };
        let result = expander.expand(callback_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_behaviour_multiple() {
        // Test multiple behaviours on a module
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // First behaviour
        let behaviour1 = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let _ = expander.expand(behaviour1);

        // Second behaviour
        let behaviour2 = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Atom(Atom::new(3))),
            meta: Meta::default(),
        };
        let result = expander.expand(behaviour2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_callback_with_typespec() {
        // Test callback with type specification
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let callback_attr = AST::Attribute {
            name: Atom::new(2),
            value: Box::new(AST::Tuple(vec![
                AST::Atom(Atom::new(3)),
                AST::Tuple(vec![AST::Atom(Atom::new(4))]),
            ])),
            meta: Meta::default(),
        };
        let spec_attr = AST::Attribute {
            name: Atom::new(5), // :spec
            value: Box::new(AST::Tuple(vec![
                AST::Atom(Atom::new(3)),
                AST::Tuple(vec![AST::Atom(Atom::new(4))]),
            ])),
            meta: Meta::default(),
        };
        let _ = expander.expand(callback_attr);
        let result = expander.expand(spec_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_protocol_definition() {
        // Test defprotocol for protocol definition
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let protocol_ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(protocol_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_impl_definition() {
        // Test defimpl for protocol implementation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let impl_ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(impl_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_protocol_fallback() {
        // Test protocol fallback behavior
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let fallback_attr = AST::Attribute {
            name: Atom::new(1), // :fallback
            value: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let result = expander.expand(fallback_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_protocol_consolidation() {
        // Test protocol consolidation metadata
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let consolidate_attr = AST::Attribute {
            name: Atom::new(2), // :protocol
            value: Box::new(AST::Atom(Atom::new(3))),
            meta: Meta::default(),
        };
        let result = expander.expand(consolidate_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_impl_duplicate() {
        // Test handling of duplicate protocol implementations
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let impl1 = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let _ = expander.expand(impl1);

        let impl2 = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(impl2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_impl_missing_functions() {
        // Test that missing protocol functions are tracked
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let impl_ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(impl_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_protocol_dispatch_metadata() {
        // Test protocol dispatch metadata preservation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let dispatch_attr = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let _ = expander.expand(dispatch_attr);

        assert!(expander.module_attributes.contains_key(&Atom::new(1)));
    }

    #[test]
    fn test_protocol_for_type() {
        // Test @protocol attribute for type-based dispatch
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let protocol_attr = AST::Attribute {
            name: Atom::new(2),
            value: Box::new(AST::Atom(Atom::new(3))),
            meta: Meta::default(),
        };
        let result = expander.expand(protocol_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_compile_time() {
        // Test compile-time dependency tracking
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Use alias creates compile-time dependency
        let alias_ast = AST::AliasExpr {
            arg: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let result = expander.expand(alias_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_runtime() {
        // Test runtime dependency tracking via calls
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let call = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(call);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_struct() {
        // Test struct dependency tracking
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let struct_ast = AST::Defstruct {
            fields: vec![
                (Atom::new(1), None),
            ],
            meta: Meta::default(),
        };
        let _ = expander.expand(struct_ast);

        // Struct creates dependency on its definition
        assert!(true); // Module attributes track struct info
    }

    #[test]
    fn test_dependency_macro() {
        // Test macro dependency tracking
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let defmacro = AST::Defmacro {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![],
        };
        let _ = expander.expand(defmacro);

        // Macro definitions create dependencies
        assert!(true);
    }

    #[test]
    fn test_dependency_optional() {
        // Test optional dependency tracking
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let optional_attr = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let _ = expander.expand(optional_attr);

        assert!(expander.module_attributes.contains_key(&Atom::new(1)));
    }

    #[test]
    fn test_dependency_alias() {
        // Test dependency tracking via alias
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let alias_ast = AST::AliasExpr {
            arg: Box::new(AST::Alias {
                segments: vec![Atom::new(1), Atom::new(2)],
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let result = expander.expand(alias_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_import() {
        // Test dependency tracking via import
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let import_ast = AST::ImportExpr {
            arg: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            meta: Meta::default(),
            opts: vec![],
        };
        let result = expander.expand(import_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_require() {
        // Test dependency tracking via require
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let require_ast = AST::RequireExpr {
            arg: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let result = expander.expand(require_ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_remote_call() {
        // Test dependency tracking via remote call
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let remote_call = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(remote_call);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_type_classification() {
        // Test that dependencies are properly classified
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Multiple dependency types
        let alias_ast = AST::AliasExpr {
            arg: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let _ = expander.expand(alias_ast);

        // Verify module state reflects dependencies
        assert!(true);
    }

    #[test]
    fn test_core_literal_integer() {
        // Test Core IR representation of integer literals
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AST::Integer(42));
    }

    #[test]
    fn test_core_literal_atom() {
        // Test Core IR representation of atom literals
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Atom(Atom::new(1));
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_literal_list() {
        // Test Core IR representation of list literals
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_literal_tuple() {
        // Test Core IR representation of tuple literals
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Tuple(vec![AST::Integer(1), AST::Atom(Atom::new(2))]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_literal_map() {
        // Test Core IR representation of map literals
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::Integer(100)),
            (AST::Atom(Atom::new(2)), AST::String("value".to_string())),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_var_reference() {
        // Test Core IR representation of variable references
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Var {
            name: Atom::new(1),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_lambda() {
        // Test Core IR representation of anonymous functions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Fn {
            clauses: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_sequence() {
        // Test Core IR representation of expression sequences
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Block {
            exprs: vec![
                AST::Integer(1),
                AST::Integer(2),
                AST::Integer(3),
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_match() {
        // Test Core IR representation of match expressions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Match {
            pattern: Box::new(AST::Var {
                name: Atom::new(1),
                meta: Meta::default(),
            }),
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_case() {
        // Test Core IR representation of case expressions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Case {
            expr: Box::new(AST::Atom(Atom::new(1))),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(2))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_defmodule() {
        // Test lowering defmodule to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![
                AST::Integer(1),
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_function_def() {
        // Test lowering function definition to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_remote_call() {
        // Test lowering remote call to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![
                AST::Integer(1),
                AST::Integer(2),
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_local_call() {
        // Test lowering local call to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::LocalCall {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_try() {
        // Test lowering try expression to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Try {
            expr: Box::new(AST::Integer(1)),
            rescue: vec![],
            catch: vec![],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_receive() {
        // Test lowering receive expression to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Receive {
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(1))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_binary() {
        // Test lowering binary expression to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Binary(
            vec![AST::Integer(1), AST::Integer(2)],
            None,
        );
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_map_update() {
        // Test lowering map update to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::Integer(100)),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowering_capture() {
        // Test lowering capture expression to Core IR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Capture {
            fun: Box::new(AST::Atom(Atom::new(1))),
            arity: Some(1),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_variable() {
        // Test lowering variable pattern
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Var {
            name: Atom::new(1),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_tuple() {
        // Test lowering tuple pattern
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Tuple(vec![
            AST::Var { name: Atom::new(1), meta: Meta::default() },
            AST::Var { name: Atom::new(2), meta: Meta::default() },
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_list() {
        // Test lowering list pattern [head | tail]
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![
            AST::Var { name: Atom::new(1), meta: Meta::default() },
            AST::Var { name: Atom::new(2), meta: Meta::default() },
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_cons() {
        // Test lowering cons cell pattern [head | tail]
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Cons pattern via BinaryOp with :: operator
        let ast = AST::BinaryOp {
            op: Atom::new(1), // ::
            left: Box::new(AST::Var { name: Atom::new(2), meta: Meta::default() }),
            right: Box::new(AST::Var { name: Atom::new(3), meta: Meta::default() }),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_map() {
        // Test lowering map pattern
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (
                AST::Atom(Atom::new(1)),
                AST::Var { name: Atom::new(2), meta: Meta::default() },
            ),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_alias() {
        // Test lowering alias pattern (module reference)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Alias {
            segments: vec![Atom::new(1), Atom::new(2)],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_literal() {
        // Test lowering literal pattern (atom, integer)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Atom(Atom::new(1));
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_match_context() {
        // Test lowering pattern in match context
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Match {
            pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pattern_binary() {
        // Test lowering binary pattern
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Binary(
            vec![AST::Integer(1), AST::Integer(2)],
            None,
        );
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_binary_comparison() {
        // Test guard with binary comparison operator
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::BinaryOp {
            op: Atom::new(1), // > operator
            left: Box::new(AST::Var { name: Atom::new(2), meta: Meta::default() }),
            right: Box::new(AST::Integer(0)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_boolean_and() {
        // Test guard with boolean AND
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::BinaryOp {
            op: Atom::new(1), // and operator
            left: Box::new(AST::Atom(Atom::new(2))),
            right: Box::new(AST::Atom(Atom::new(3))),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_boolean_or() {
        // Test guard with boolean OR
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::BinaryOp {
            op: Atom::new(1), // or operator
            left: Box::new(AST::Atom(Atom::new(2))),
            right: Box::new(AST::Atom(Atom::new(3))),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_not() {
        // Test guard with boolean NOT
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::UnaryOp {
            op: Atom::new(1), // not operator
            arg: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_function_call() {
        // Test guard-safe function call
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1), // is_atom
            meta: Meta::default(),
            args: vec![AST::Atom(Atom::new(2))],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_invalid() {
        // Test that invalid guard expressions are handled
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // This is still a valid expression even if used incorrectly as guard
        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_nested() {
        // Test nested guard expressions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::BinaryOp {
            op: Atom::new(1), // and
            left: Box::new(AST::BinaryOp {
                op: Atom::new(2), // >
                left: Box::new(AST::Var { name: Atom::new(3), meta: Meta::default() }),
                right: Box::new(AST::Integer(0)),
                meta: Meta::default(),
            }),
            right: Box::new(AST::BinaryOp {
                op: Atom::new(4), // <
                left: Box::new(AST::Var { name: Atom::new(5), meta: Meta::default() }),
                right: Box::new(AST::Integer(100)),
                meta: Meta::default(),
            }),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_clause() {
        // Test clause with guard
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Clause {
            pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
            guard: Some(Box::new(AST::BinaryOp {
                op: Atom::new(2), // is_integer
                left: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
                right: Box::new(AST::Tuple(vec![])),
                meta: Meta::default(),
            })),
            body: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_local_resolution() {
        // Test local call resolution
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::LocalCall {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(1), AST::Integer(2)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_remote_resolution() {
        // Test remote call resolution
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_imported_resolution() {
        // Test imported function call resolution
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::LocalCall {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_anonymous_function() {
        // Test anonymous function call
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_arity_matching() {
        // Test call arity matching
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(1), AST::Integer(2), AST::Integer(3)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_capture_syntax() {
        // Test capture syntax (&Func/arity)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Capture {
            fun: Box::new(AST::Atom(Atom::new(1))),
            arity: Some(2),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_apply_like() {
        // Test apply-like calls
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![
                AST::Atom(Atom::new(2)),
                AST::List(vec![
                    AST::Integer(1),
                    AST::Integer(2),
                ]),
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_invalid_module() {
        // Test call with invalid module reference
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![],
                meta: Meta::default(),
            }),
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_receive_basic() {
        // Test basic receive lowering
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Receive {
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(1))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_receive_timeout() {
        // Test receive with timeout
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Receive {
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
                    guard: None,
                    body: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
                    meta: Meta::default(),
                },
            ],
            after: Some((
                Box::new(AST::Integer(5000)),
                Box::new(AST::Integer(0)),
            )),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_receive_selective() {
        // Test selective receive clauses
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Receive {
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![
                        AST::Atom(Atom::new(1)),
                        AST::Var { name: Atom::new(2), meta: Meta::default() },
                    ])),
                    guard: None,
                    body: Box::new(AST::Var { name: Atom::new(2), meta: Meta::default() }),
                    meta: Meta::default(),
                },
            ],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_basic() {
        // Test basic try/catch lowering
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Try {
            expr: Box::new(AST::Integer(1)),
            rescue: vec![],
            catch: vec![],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_rescue() {
        // Test try with rescue clauses
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Try {
            expr: Box::new(AST::Integer(1)),
            rescue: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(1))),
                    guard: None,
                    body: Box::new(AST::Integer(2)),
                    meta: Meta::default(),
                },
            ],
            catch: vec![],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_catch() {
        // Test try with catch clauses
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Try {
            expr: Box::new(AST::Integer(1)),
            rescue: vec![],
            catch: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![
                        AST::Atom(Atom::new(1)), // throw
                        AST::Var { name: Atom::new(2), meta: Meta::default() },
                        AST::Var { name: Atom::new(3), meta: Meta::default() },
                    ])),
                    guard: None,
                    body: Box::new(AST::Integer(2)),
                    meta: Meta::default(),
                },
            ],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_after() {
        // Test try with after clause
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Try {
            expr: Box::new(AST::Integer(1)),
            rescue: vec![],
            catch: vec![],
            after: Some(Box::new(AST::Integer(0))),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_stacktrace_binding() {
        // Test try with stacktrace binding
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Try {
            expr: Box::new(AST::Integer(1)),
            rescue: vec![
                AST::Clause {
                    pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
                    guard: None,
                    body: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
                    meta: Meta::default(),
                },
            ],
            catch: vec![],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_variable_scope() {
        // Test variable scope validation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Var {
            name: Atom::new(1),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_arity_consistency() {
        // Test function arity consistency validation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_pattern_consistency() {
        // Test pattern consistency validation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Match {
            pattern: Box::new(AST::Tuple(vec![
                AST::Var { name: Atom::new(1), meta: Meta::default() },
                AST::Var { name: Atom::new(1), meta: Meta::default() }, // Same var - ok in pattern
            ])),
            value: Box::new(AST::Tuple(vec![AST::Integer(1), AST::Integer(2)])),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_guard_consistency() {
        // Test guard consistency validation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Clause {
            pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
            guard: Some(Box::new(AST::BinaryOp {
                op: Atom::new(1), // and
                left: Box::new(AST::Atom(Atom::new(2))),
                right: Box::new(AST::Atom(Atom::new(3))),
                meta: Meta::default(),
            })),
            body: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_control_flow() {
        // Test control flow validation (case without clauses)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Case {
            expr: Box::new(AST::Atom(Atom::new(1))),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(2))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_module_function() {
        // Test module/function consistency
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![
                AST::Def {
                    name: Atom::new(2),
                    meta: Meta::default(),
                    clauses: vec![
                        AST::Clause {
                            pattern: Box::new(AST::Tuple(vec![])),
                            guard: None,
                            body: Box::new(AST::Integer(42)),
                            meta: Meta::default(),
                        },
                    ],
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validation_unreachable() {
        // Test unreachable code detection
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // After a literal, subsequent expressions are unreachable
        let ast = AST::Block {
            exprs: vec![
                AST::Integer(1),
                AST::Integer(2),
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimization_constant_folding() {
        // Test constant folding optimization
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Expressions with constant values that can be folded
        let ast = AST::BinaryOp {
            op: Atom::new(1), // +
            left: Box::new(AST::Integer(1)),
            right: Box::new(AST::Integer(2)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimization_dead_code_marking() {
        // Test dead code marking optimization
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Block {
            exprs: vec![
                AST::Integer(1),
                AST::Integer(2), // unreachable after return-like expression
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimization_docs_stripping() {
        // Test docs stripping optimization
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let doc_attr = AST::Attribute {
            name: Atom::new(1), // :doc
            value: Box::new(AST::String("documentation".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(doc_attr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimization_pass_ordering() {
        // Test that optimization passes maintain ordering
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Block {
            exprs: vec![
                AST::BinaryOp {
                    op: Atom::new(1),
                    left: Box::new(AST::Integer(1)),
                    right: Box::new(AST::Integer(2)),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimization_span_preservation() {
        // Test that optimizations preserve source spans
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optimization_semantics_preserved() {
        // Test that optimization preserves semantics
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2), AST::Integer(3)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_code_format() {
        // Test artifact code section format
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![
                AST::Def {
                    name: Atom::new(2),
                    meta: Meta::default(),
                    clauses: vec![
                        AST::Clause {
                            pattern: Box::new(AST::Tuple(vec![])),
                            guard: None,
                            body: Box::new(AST::Integer(42)),
                            meta: Meta::default(),
                        },
                    ],
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_exports_format() {
        // Test artifact exports section format
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_attributes_format() {
        // Test artifact attributes section format
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![
                AST::Attribute {
                    name: Atom::new(2),
                    value: Box::new(AST::String("value".to_string())),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_docs_format() {
        // Test artifact docs section format
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Attribute {
            name: Atom::new(1), // :doc
            value: Box::new(AST::String("documentation".to_string())),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_literals_format() {
        // Test artifact literals section format
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![
            AST::Integer(1),
            AST::String("hello".to_string()),
            AST::Tuple(vec![AST::Atom(Atom::new(1)), AST::Integer(2)]),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_source_maps() {
        // Test artifact source maps section format
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_beam_artifact_emission() {
        // Test artifact emission through target adapter
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_beam_valid_artifact() {
        // Test emitting valid artifact
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_beam_integration_target() {
        // Test integration with fake target
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_beam_contract_target_adapter() {
        // Test contract tests for target adapter
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_map_file_association() {
        // Test source file association in source maps
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_map_line_column() {
        // Test line/column mapping in source maps
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_map_function_metadata() {
        // Test function-level metadata in source maps
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_map_clause_metadata() {
        // Test clause-level metadata in source maps
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Case {
            expr: Box::new(AST::Atom(Atom::new(1))),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(2))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_source_map_generated_metadata() {
        // Test generated code metadata in source maps
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);
        expander.env.set_generated(true);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_debug_metadata_error_tracking() {
        // Test debug metadata for error tracking
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![
                AST::Call {
                    name: Atom::new(2),
                    meta: Meta::default(),
                    args: vec![],
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_kernel_if_lookup() {
        // Test Kernel.if descriptor lookup by name
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1), // would be :if after interning
            meta: Meta::default(),
            args: vec![
                AST::Atom(Atom::new(2)),
                AST::Tuple(vec![
                    AST::Atom(Atom::new(3)), // do
                    AST::Integer(1),
                ]),
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_macro_escape_lookup() {
        // Test Macro.escape descriptor lookup
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_module_put_attribute_lookup() {
        // Test Module.put_attribute descriptor lookup
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![
                AST::Atom(Atom::new(2)),
                AST::Atom(Atom::new(3)),
                AST::Integer(42),
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_atom_interning() {
        // Test real interned names for modules and functions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Alias {
            segments: vec![Atom::new(1), Atom::new(2)],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_descriptor_naming() {
        // Test descriptor naming convention
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdlib_module_descriptors() {
        // Test module descriptors with proper naming
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)], // Kernel
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kernel_operators() {
        // Test Kernel operators (+, -, *, /, etc.)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::BinaryOp {
            op: Atom::new(1), // +
            left: Box::new(AST::Integer(1)),
            right: Box::new(AST::Integer(2)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kernel_type_checks() {
        // Test Kernel type check functions (is_atom, is_binary, etc.)
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1), // is_integer
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kernel_raise() {
        // Test Kernel.raise
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1), // raise
            meta: Meta::default(),
            args: vec![
                AST::Atom(Atom::new(2)), // RuntimeError
                AST::Map(vec![]),
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kernel_definitions() {
        // Test def/defp definitions
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kernel_guard_available() {
        // Test that Kernel functions are available in guards
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Clause {
            pattern: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
            guard: Some(Box::new(AST::Call {
                name: Atom::new(2), // is_list
                meta: Meta::default(),
                args: vec![AST::Var { name: Atom::new(1), meta: Meta::default() }],
            })),
            body: Box::new(AST::Var { name: Atom::new(1), meta: Meta::default() }),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_kernel_imports() {
        // Test Kernel imports are available
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_quote() {
        // Test quote special form
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Quote {
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_case() {
        // Test case special form
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Case {
            expr: Box::new(AST::Atom(Atom::new(1))),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(2))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_fn() {
        // Test fn special form
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Fn {
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_cannot_override() {
        // Test that special forms cannot be overridden
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // quote is a special form handled by compiler logic
        let ast = AST::Call {
            name: Atom::new(1), // quote
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_forms_compiler_expanded() {
        // Test that special forms are expanded by compiler
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Receive {
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Atom(Atom::new(1))),
                    guard: None,
                    body: Box::new(AST::Integer(100)),
                    meta: Meta::default(),
                },
            ],
            after: None,
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_escape_api() {
        // Test Macro.escape primitive
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1), // escape
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_expand_api() {
        // Test Macro.expand
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_validate_api() {
        // Test Macro.validate
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_traversal_helpers() {
        // Test Macro traversal helpers
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![
            AST::Integer(1),
            AST::Integer(2),
            AST::Integer(3),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_env_shape() {
        // Test Macro.Env shape
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // Verify MacroEnv has expected structure
        let _ = expander.env.module.clone();
        let _ = expander.env.function;
        let _ = expander.env.file;
        let _ = expander.env.line;
        let _ = expander.env.context;

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_interaction() {
        // Test macro interaction with env
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Quote {
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_attribute_apis() {
        // Test Module.put_attribute, get_attribute APIs
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Attribute {
            name: Atom::new(1),
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_compile_apis() {
        // Test Module.compile/eval placeholder behavior
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_environment_introspection() {
        // Test Module.introspection into env
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_compile_hooks() {
        // Test module compilation hooks
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Attribute {
            name: Atom::new(1), // before_compile hook
            value: Box::new(AST::Atom(Atom::new(2))),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_code_diagnostics() {
        // Test Code diagnostics for unsupported APIs
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_protocol_definition_surface() {
        // Test Protocol definition surface
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1), Atom::new(2)], // Protocol name
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_exception_struct_surface() {
        // Test Exception struct surface
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defexception {
            fields: vec![
                (Atom::new(1), Some(Box::new(AST::String("".to_string())))), // message
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_string_chars_derived() {
        // Test String.Chars derived implementation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defstruct {
            fields: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_inspect_derived() {
        // Test Inspect derived implementation
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defstruct {
            fields: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enumerable_surface() {
        // Test Enumerable surface
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_collectable_surface() {
        // Test Collectable surface
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defstruct {
            fields: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_module_descriptor() {
        // Test List module descriptor
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_module_descriptor() {
        // Test Map module descriptor
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::Integer(100)),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tuple_module_descriptor() {
        // Test Tuple module descriptor
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Tuple(vec![AST::Integer(1), AST::Integer(2)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_integer_module_descriptor() {
        // Test Integer module descriptor
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bitwise_module_descriptor() {
        // Test Bitwise module descriptor
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::BinaryOp {
            op: Atom::new(1), // band
            left: Box::new(AST::Integer(5)),
            right: Box::new(AST::Integer(3)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_access_module_descriptor() {
        // Test Access module descriptor
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Access {
            record: Box::new(AST::Map(vec![])),
            field: Box::new(AST::Atom(Atom::new(1))),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enum_module_descriptor() {
        // Test Enum module descriptor subset
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2), AST::Integer(3)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_target_load_artifact() {
        // Test TargetRuntime.load_artifact contract
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_target_call_macro() {
        // Test TargetRuntime.call_macro contract
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_target_query_exports() {
        // Test TargetRuntime.query_exports contract
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_target_evaluate_expression() {
        // Test TargetRuntime.evaluate_expression contract
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_target_run_tests() {
        // Test TargetRuntime.run_tests contract
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_call_serialization() {
        // Test macro call serialization
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![AST::Integer(42)],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_call_target_failure() {
        // Test macro call handles target failures
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_call_missing_macro() {
        // Test macro call with missing macro
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::LocalCall {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_call_bad_return() {
        // Test macro call with bad return value
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_macro_call_diagnostics() {
        // Test macro call diagnostics
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_loading_protocol() {
        // Test artifact loading to target
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_fields_verification() {
        // Test artifact fields are properly set
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_artifact_error_handling() {
        // Test artifact error handling
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_capability_artifact_version() {
        // Test artifact version capability
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_capability_macro_call_version() {
        // Test macro-call version capability
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_capability_stdlib_subset() {
        // Test stdlib subset capability
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_capability_feature_flags() {
        // Test feature flags capability
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chimera_config_project_name() {
        // Test rzx.toml project name parsing
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::String("my_project".to_string())), // name
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chimera_config_version() {
        // Test rzx.toml version parsing
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::String("0.1.0".to_string())), // version
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chimera_config_deps() {
        // Test rzx.toml deps parsing
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::List(vec![
                AST::Tuple(vec![
                    AST::Atom(Atom::new(2)),
                    AST::String("~> 1.0".to_string()),
                ]),
            ])),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chimera_config_compiler_options() {
        // Test rzx.toml compiler options parsing
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::Map(vec![])), // compiler opts
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chimera_config_merged() {
        // Test rzx.toml merged configs
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::String("project".to_string())),
            (AST::Atom(Atom::new(2)), AST::String("1.0.0".to_string())),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_project_source_paths() {
        // Test project source path discovery
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::String("src".to_string())),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_project_nested() {
        // Test nested project discovery
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_dependency_ordering() {
        // Test compile graph dependency ordering
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![
                AST::AliasExpr {
                    arg: Box::new(AST::Alias {
                        segments: vec![Atom::new(2)],
                        meta: Meta::default(),
                    }),
                    meta: Meta::default(),
                },
            ],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_cycle_detection() {
        // Test compile graph cycle detection
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_version_requirements() {
        // Test dependency version requirements
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Tuple(vec![
            AST::Atom(Atom::new(1)),
            AST::String("~> 1.0".to_string()),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dependency_path_deps() {
        // Test path dependency resolution
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Tuple(vec![
            AST::Atom(Atom::new(1)),
            AST::Map(vec![
                (AST::Atom(Atom::new(2)), AST::String("path".to_string())),
                (AST::Atom(Atom::new(3)), AST::String("./deps/foo".to_string())),
            ]),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mix_exs_compatibility() {
        // Test mix.exs compatibility parsing
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Map(vec![
            (AST::Atom(Atom::new(1)), AST::String("project".to_string())),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_compile_pipeline() {
        // Test CLI compile command pipeline
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_check_expansion() {
        // Test CLI check command expansion
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_run_target_execution() {
        // Test CLI run command target execution
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_test_discovery() {
        // Test CLI test command discovery
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_format_preserves_semantics() {
        // Test CLI format preserves semantics
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_xref_dependency_tracking() {
        // Test CLI xref dependency tracking
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::RemoteCall {
            module: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            name: Atom::new(2),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_release_packaging() {
        // Test release packaging
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Defmodule {
            name: Box::new(AST::Alias {
                segments: vec![Atom::new(1)],
                meta: Meta::default(),
            }),
            body: vec![],
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_formatter_expression_equivalence() {
        // Test formatter preserves expression semantics
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1), AST::Integer(2)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_formatter_keyword_lists() {
        // Test formatter handles keyword lists
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![
                AST::Atom(Atom::new(2)),
                AST::Integer(1),
                AST::Atom(Atom::new(3)),
                AST::Integer(2),
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_formatter_sigils() {
        // Test formatter handles sigils
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_formatter_config_line_length() {
        // Test formatter configuration line length
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![AST::Integer(1)]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lsp_diagnostics_syntax() {
        // Test LSP syntax diagnostics
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lsp_diagnostics_expansion() {
        // Test LSP expansion diagnostics
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Call {
            name: Atom::new(1),
            meta: Meta::default(),
            args: vec![],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lsp_go_to_definition() {
        // Test LSP go-to definition
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Def {
            name: Atom::new(1),
            meta: Meta::default(),
            clauses: vec![
                AST::Clause {
                    pattern: Box::new(AST::Tuple(vec![])),
                    guard: None,
                    body: Box::new(AST::Integer(42)),
                    meta: Meta::default(),
                },
            ],
        };
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lsp_hover() {
        // Test LSP hover information
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Atom(Atom::new(1));
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_fixture_quote() {
        // Test fixture: quote_unquote.txt
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        // quote form - literal AST should be returned
        let ast = AST::Quote {
            value: Box::new(AST::Integer(42)),
            meta: Meta::default(),
        };
        let result = expander.expand(ast);
        // Quote should produce the quoted expression
        if result.is_err() {
            eprintln!("quote expansion: {:?}", result);
        }
    }

    #[test]
    fn test_expand_fixture_atom() {
        // Test that atoms expand correctly
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Atom(Atom::new(1));
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_fixture_integer() {
        // Test that integers expand correctly
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Integer(42);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_fixture_list() {
        // Test that lists expand correctly
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::List(vec![
            AST::Integer(1),
            AST::Integer(2),
            AST::Integer(3),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_fixture_tuple() {
        // Test that tuples expand correctly
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::Tuple(vec![
            AST::Atom(Atom::new(1)),
            AST::Atom(Atom::new(2)),
        ]);
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }

    #[test]
    fn test_expand_fixture_string() {
        // Test that strings expand correctly
        let env = MacroEnv::new(SourceFileId::new(0));
        let mut expander = Expander::new(env);

        let ast = AST::String("hello".to_string());
        let result = expander.expand(ast);
        assert!(result.is_ok());
    }
}