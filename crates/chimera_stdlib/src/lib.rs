//! Standard library surface for the Rust/Zig Elixir compiler.
//!
//! This crate provides Rust-defined module descriptors for the core Elixir
//! standard library modules that are required for compilation: Kernel,
//! Macro, Module, Code, Protocol, Exception, etc.

#[cfg(test)]
use chimera_allocator as _;

use chimera_term::{Atom, ModuleName};
use std::collections::HashMap;

/// Built-in function kind for code generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKind {
    /// No special handling needed
    None,
    /// Type check (is_atom, is_binary, etc.)
    TypeCheck,
    /// Arithmetic operation (+, -, *, /)
    Arithmetic,
    /// Comparison operation (==, !=, <, >, etc.)
    Comparison,
    /// List operation (++, --, hd, tl, length)
    List,
    /// Tuple operation (elem, tuple_size)
    Tuple,
    /// Binary operation (<>, +++>)
    Binary,
    /// Node operation (node, self)
    Node,
    /// Conversion (atom_to_binary, binary_to_atom, etc.)
    Conversion,
    /// Other built-in
    Builtin,
}

/// A native function descriptor.
#[derive(Debug, Clone)]
pub struct NativeFunction {
    /// Function name (interned atom)
    pub name: Atom,
    /// Function arity
    pub arity: u8,
    /// Whether it's a special form
    pub special_form: bool,
    /// Implementation note
    pub description: &'static str,
    /// Built-in kind for code generation
    pub builtin_kind: BuiltinKind,
}

impl NativeFunction {
    /// Create a new native function.
    pub fn new(name: Atom, arity: u8, special_form: bool, desc: &'static str) -> Self {
        NativeFunction {
            name,
            arity,
            special_form,
            description: desc,
            builtin_kind: BuiltinKind::None,
        }
    }

    /// Create a type check function.
    pub fn type_check(name: Atom, arity: u8, desc: &'static str) -> Self {
        NativeFunction {
            name,
            arity,
            special_form: false,
            description: desc,
            builtin_kind: BuiltinKind::TypeCheck,
        }
    }

    /// Create a built-in function with a specific kind.
    pub fn builtin(name: Atom, arity: u8, kind: BuiltinKind, desc: &'static str) -> Self {
        NativeFunction {
            name,
            arity,
            special_form: false,
            description: desc,
            builtin_kind: kind,
        }
    }
}

/// A native macro descriptor.
#[derive(Debug, Clone)]
pub struct NativeMacro {
    /// Macro name
    pub name: Atom,
    /// Macro arity
    pub arity: u8,
    /// Implementation note
    pub description: &'static str,
}

/// A native module descriptor.
#[derive(Debug, Clone)]
pub struct NativeModule {
    /// Module name
    pub name: ModuleName,
    /// Functions (indexed by name for fast lookup)
    functions: HashMap<Atom, NativeFunction>,
    /// Macros
    macros: Vec<NativeMacro>,
    /// Types
    pub types: Vec<&'static str>,
    /// Attributes
    pub attributes: Vec<(&'static str, &'static str)>,
}

impl NativeModule {
    /// Create a new native module.
    pub fn new(name: ModuleName) -> Self {
        NativeModule {
            name,
            functions: HashMap::new(),
            macros: Vec::new(),
            types: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Add a function.
    pub fn add_function(&mut self, name: Atom, arity: u8, special_form: bool, desc: &'static str) {
        let func = NativeFunction::new(name.clone(), arity, special_form, desc);
        self.functions.insert(name, func);
    }

    /// Add a built-in function.
    pub fn add_builtin(&mut self, name: Atom, arity: u8, kind: BuiltinKind, desc: &'static str) {
        let func = NativeFunction::builtin(name.clone(), arity, kind, desc);
        self.functions.insert(name, func);
    }

    /// Add a type check function.
    pub fn add_type_check(&mut self, name: Atom, arity: u8, desc: &'static str) {
        let func = NativeFunction::type_check(name.clone(), arity, desc);
        self.functions.insert(name, func);
    }

    /// Get a function by name.
    pub fn get_function(&self, name: &Atom) -> Option<&NativeFunction> {
        self.functions.get(name)
    }

    /// Get all functions.
    pub fn functions(&self) -> Vec<&NativeFunction> {
        self.functions.values().collect()
    }

    /// Add a macro.
    pub fn add_macro(&mut self, name: Atom, arity: u8, desc: &'static str) {
        self.macros.push(NativeMacro {
            name,
            arity,
            description: desc,
        });
    }

    /// Add a type.
    pub fn add_type(&mut self, t: &'static str) {
        self.types.push(t);
    }

    /// Add an attribute.
    pub fn add_attribute(&mut self, name: &'static str, value: &'static str) {
        self.attributes.push((name, value));
    }
}

/// Standard library registry.
pub struct Stdlib {
    /// Registered modules
    modules: HashMap<ModuleName, NativeModule>,
}

impl Stdlib {
    /// Create a new standard library registry.
    pub fn new() -> Self {
        let mut modules = HashMap::new();

        // Register Kernel
        modules.insert(
            ModuleName::new(vec![Atom::new(0)]),
            Self::kernel_module(),
        );

        // Register Macro
        modules.insert(
            ModuleName::new(vec![Atom::new(1)]),
            Self::macro_module(),
        );

        // Register Module
        modules.insert(
            ModuleName::new(vec![Atom::new(2)]),
            Self::module_module(),
        );

        // Register Code
        modules.insert(
            ModuleName::new(vec![Atom::new(3)]),
            Self::code_module(),
        );

        // Register Kernel.SpecialForms
        modules.insert(
            ModuleName::new(vec![Atom::new(4), Atom::new(5)]),
            Self::special_forms_module(),
        );

        Stdlib { modules }
    }

    /// Get a module by name.
    pub fn get(&self, name: &ModuleName) -> Option<&NativeModule> {
        self.modules.get(name)
    }

    /// Get all module names.
    pub fn module_names(&self) -> Vec<ModuleName> {
        self.modules.keys().cloned().collect()
    }

    /// Kernel module - core Elixir functions and operators.
    fn kernel_module() -> NativeModule {
        let mut module = NativeModule::new(ModuleName::new(vec![Atom::new(0)]));

        // Arithmetic - these map to BEAM opcodes
        module.add_builtin(Atom::new(10), 2, BuiltinKind::Arithmetic, "+/2 addition");
        module.add_builtin(Atom::new(11), 2, BuiltinKind::Arithmetic, "-/2 subtraction");
        module.add_builtin(Atom::new(12), 2, BuiltinKind::Arithmetic, "*/2 multiplication");
        module.add_builtin(Atom::new(13), 2, BuiltinKind::Arithmetic, "//2 division");
        module.add_builtin(Atom::new(14), 1, BuiltinKind::Arithmetic, "+/1 unary plus");
        module.add_builtin(Atom::new(15), 1, BuiltinKind::Arithmetic, "-/1 unary minus");

        // Comparison - these map to BEAM comparison opcodes
        module.add_builtin(Atom::new(16), 2, BuiltinKind::Comparison, "==/2 equal");
        module.add_builtin(Atom::new(17), 2, BuiltinKind::Comparison, "=/=/2 strict equal");
        module.add_builtin(Atom::new(18), 2, BuiltinKind::Comparison, "!=/2 not equal");
        module.add_builtin(Atom::new(19), 2, BuiltinKind::Comparison, "=/=/2 strict not equal");
        module.add_builtin(Atom::new(20), 2, BuiltinKind::Comparison, "</2 less than");
        module.add_builtin(Atom::new(21), 2, BuiltinKind::Comparison, "<=/2 less than or equal");
        module.add_builtin(Atom::new(22), 2, BuiltinKind::Comparison, ">/2 greater than");
        module.add_builtin(Atom::new(23), 2, BuiltinKind::Comparison, ">=/2 greater than or equal");

        // Boolean
        module.add_builtin(Atom::new(24), 1, BuiltinKind::Builtin, "not/1 logical not");
        module.add_builtin(Atom::new(25), 2, BuiltinKind::Builtin, "and/2 logical and");
        module.add_builtin(Atom::new(26), 2, BuiltinKind::Builtin, "or/2 logical or");

        // List operations
        module.add_builtin(Atom::new(27), 2, BuiltinKind::List, "++/2 list concatenation");
        module.add_builtin(Atom::new(28), 2, BuiltinKind::List, "--/2 list difference");
        module.add_builtin(Atom::new(63), 2, BuiltinKind::List, "length/1");
        module.add_builtin(Atom::new(59), 1, BuiltinKind::List, "hd/1");
        module.add_builtin(Atom::new(73), 1, BuiltinKind::List, "tl/1");

        // Binary operations
        module.add_builtin(Atom::new(29), 2, BuiltinKind::Binary, "<>/2 binary concatenation");
        module.add_builtin(Atom::new(30), 2, BuiltinKind::Binary, "++>/2 string concatenation");

        // Type checks - these emit type test opcodes
        module.add_type_check(Atom::new(31), 1, "is_atom/1");
        module.add_type_check(Atom::new(32), 1, "is_binary/1");
        module.add_type_check(Atom::new(33), 1, "is_bitstring/1");
        module.add_type_check(Atom::new(34), 1, "is_boolean/1");
        module.add_type_check(Atom::new(35), 1, "is_float/1");
        module.add_type_check(Atom::new(36), 1, "is_function/1");
        module.add_type_check(Atom::new(37), 1, "is_integer/1");
        module.add_type_check(Atom::new(38), 1, "is_list/1");
        module.add_type_check(Atom::new(39), 1, "is_map/1");
        module.add_type_check(Atom::new(40), 1, "is_number/1");
        module.add_type_check(Atom::new(41), 1, "is_pid/1");
        module.add_type_check(Atom::new(42), 1, "is_port/1");
        module.add_type_check(Atom::new(43), 1, "is_reference/1");
        module.add_type_check(Atom::new(44), 1, "is_tuple/1");

        // Tuple operations
        module.add_builtin(Atom::new(56), 2, BuiltinKind::Tuple, "elem/2");
        module.add_builtin(Atom::new(75), 2, BuiltinKind::Tuple, "tuple_size/1");

        // Node operations
        module.add_builtin(Atom::new(68), 0, BuiltinKind::Node, "node/0");
        module.add_builtin(Atom::new(69), 1, BuiltinKind::Node, "node/1");
        module.add_builtin(Atom::new(71), 0, BuiltinKind::Node, "self/0");

        // Conversion functions
        module.add_builtin(Atom::new(48), 1, BuiltinKind::Conversion, "atom_to_binary/1");
        module.add_builtin(Atom::new(49), 1, BuiltinKind::Conversion, "atom_to_list/1");
        module.add_builtin(Atom::new(50), 1, BuiltinKind::Conversion, "binary_to_atom/1");
        module.add_builtin(Atom::new(51), 1, BuiltinKind::Conversion, "binary_to_list/1");
        module.add_builtin(Atom::new(52), 1, BuiltinKind::Conversion, "bit_size/1");
        module.add_builtin(Atom::new(53), 1, BuiltinKind::Conversion, "byte_size/1");
        module.add_builtin(Atom::new(61), 2, BuiltinKind::Conversion, "integer_to_binary/2");

        // Other Kernel functions
        module.add_builtin(Atom::new(45), 1, BuiltinKind::Builtin, "abs/1");
        module.add_builtin(Atom::new(46), 1, BuiltinKind::Builtin, "apply/2");
        module.add_builtin(Atom::new(47), 3, BuiltinKind::Builtin, "apply/3");
        module.add_builtin(Atom::new(54), 1, BuiltinKind::Builtin, "ceil/1");
        module.add_builtin(Atom::new(55), 2, BuiltinKind::Builtin, "div/2");
        module.add_builtin(Atom::new(57), 2, BuiltinKind::Builtin, "float/1");
        module.add_builtin(Atom::new(58), 1, BuiltinKind::Builtin, "floor/1");
        module.add_builtin(Atom::new(60), 1, BuiltinKind::Builtin, "integer/1");
        module.add_builtin(Atom::new(64), 1, BuiltinKind::Builtin, "map_size/1");
        module.add_builtin(Atom::new(65), 1, BuiltinKind::Builtin, "max/1");
        module.add_builtin(Atom::new(66), 1, BuiltinKind::Builtin, "min/1");
        module.add_builtin(Atom::new(67), 2, BuiltinKind::Builtin, "mod/2");
        module.add_builtin(Atom::new(70), 1, BuiltinKind::Builtin, "round/1");
        module.add_builtin(Atom::new(72), 1, BuiltinKind::Builtin, "size/1");
        module.add_builtin(Atom::new(74), 1, BuiltinKind::Builtin, "trunc/1");
        module.add_builtin(Atom::new(76), 0, BuiltinKind::Builtin, "unique_identifier/0");

        // Kernel macros (special forms handled separately)
        module.add_macro(Atom::new(77), 2, "if/2");
        module.add_macro(Atom::new(78), 1, "unless/2");
        module.add_macro(Atom::new(79), 1, "cond/1");
        module.add_macro(Atom::new(80), 1, "for/1");

        module
    }

    /// Macro module - compile-time macro support.
    fn macro_module() -> NativeModule {
        let mut module = NativeModule::new(ModuleName::new(vec![Atom::new(1)]));

        module.add_function(Atom::new(100), 1, false, "escape/1");
        module.add_function(Atom::new(101), 1, false, "expand/1");
        module.add_function(Atom::new(102), 1, false, "expand/2");
        module.add_function(Atom::new(103), 1, false, "expand_literals/1");
        module.add_function(Atom::new(104), 2, false, "pipe/2");
        module.add_function(Atom::new(105), 2, false, "signature/1");
        module.add_function(Atom::new(106), 1, false, "to_binary/1");
        module.add_function(Atom::new(107), 1, false, "validate/1");
        module.add_function(Atom::new(108), 1, false, "jaro_distance/2");

        // Macro.Env functions
        module.add_function(Atom::new(110), 0, false, "env/0");
        module.add_function(Atom::new(111), 0, false, "lookup/1");
        module.add_function(Atom::new(112), 0, false, "stacktrace/0");

        module
    }

    /// Module module - compile-time module introspection and attributes.
    fn module_module() -> NativeModule {
        let mut module = NativeModule::new(ModuleName::new(vec![Atom::new(2)]));

        module.add_function(Atom::new(120), 1, false, "add_attribute/2");
        module.add_function(Atom::new(121), 1, false, "compile_attribute/1");
        module.add_function(Atom::new(122), 2, false, "defdelegate/2");
        module.add_function(Atom::new(123), 1, false, "defmacro/1");
        module.add_function(Atom::new(124), 2, false, "defmacro/2");
        module.add_function(Atom::new(125), 1, false, "defmacrop/1");
        module.add_function(Atom::new(126), 2, false, "defmacrop/2");
        module.add_function(Atom::new(127), 1, false, "defmodule/2");
        module.add_function(Atom::new(128), 1, false, "def/1");
        module.add_function(Atom::new(129), 2, false, "def/2");
        module.add_function(Atom::new(130), 1, false, "defp/1");
        module.add_function(Atom::new(131), 2, false, "defp/2");
        module.add_function(Atom::new(132), 1, false, "defstruct/1");
        module.add_function(Atom::new(133), 1, false, "get_attribute/2");
        module.add_function(Atom::new(134), 0, false, "has_attribute?/1");
        module.add_function(Atom::new(135), 0, false, "info/1");
        module.add_function(Atom::new(136), 2, false, "make_attribute/2");
        module.add_function(Atom::new(137), 1, false, "module/0");
        module.add_function(Atom::new(138), 1, false, "open?(0");
        module.add_function(Atom::new(139), 1, false, "put_attribute/3");
        module.add_function(Atom::new(140), 2, false, "register_attribute/2");
        module.add_function(Atom::new(141), 1, false, "remove_attribute/2");

        module
    }

    /// Code module - code loading and compilation utilities.
    fn code_module() -> NativeModule {
        let mut module = NativeModule::new(ModuleName::new(vec![Atom::new(3)]));

        module.add_function(Atom::new(150), 1, false, "compile_file/1");
        module.add_function(Atom::new(151), 2, false, "compile_file/2");
        module.add_function(Atom::new(152), 1, false, "compile_path/1");
        module.add_function(Atom::new(153), 2, false, "compile_path/2");
        module.add_function(Atom::new(154), 1, false, "eval_file/1");
        module.add_function(Atom::new(155), 2, false, "eval_file/2");
        module.add_function(Atom::new(156), 1, false, "eval_path/1");
        module.add_function(Atom::new(157), 2, false, "eval_path/2");
        module.add_function(Atom::new(158), 1, false, "ensure_compiled/1");
        module.add_function(Atom::new(159), 1, false, "ensure_loaded/1");
        module.add_function(Atom::new(160), 1, false, "format!/1");
        module.add_function(Atom::new(161), 1, false, "get_docs/1");
        module.add_function(Atom::new(162), 1, false, "is_path/1");
        module.add_function(Atom::new(163), 1, false, "loaded/0");
        module.add_function(Atom::new(164), 1, false, "preload/1");
        module.add_function(Atom::new(165), 0, false, "root_dir/0");
        module.add_function(Atom::new(166), 0, false, "sticky_dir/0");
        module.add_function(Atom::new(167), 1, false, "require_file/1");

        module
    }

    /// Kernel.SpecialForms module.
    fn special_forms_module() -> NativeModule {
        let mut module = NativeModule::new(ModuleName::new(vec![Atom::new(4), Atom::new(5)]));

        // Special forms - these are compiler-recognized constructs
        module.add_function(Atom::new(200), 1, true, "alias/1");
        module.add_function(Atom::new(201), 1, true, "require/1");
        module.add_function(Atom::new(202), 1, true, "import/1");
        module.add_function(Atom::new(203), 2, true, "use/2");
        module.add_function(Atom::new(204), 1, true, "quote/1");
        module.add_function(Atom::new(205), 1, true, "unquote/1");
        module.add_function(Atom::new(206), 1, true, "unquote_splicing/1");
        module.add_function(Atom::new(207), 2, true, "case/2");
        module.add_function(Atom::new(208), 1, true, "cond/1");
        module.add_function(Atom::new(209), 2, true, "fn/2");
        module.add_function(Atom::new(210), 2, true, "with/2");
        module.add_function(Atom::new(211), 2, true, "for/2");
        module.add_function(Atom::new(212), 2, true, "receive/2");
        module.add_function(Atom::new(213), 1, true, "try/1");
        module.add_function(Atom::new(214), 1, true, "catch/1");
        module.add_function(Atom::new(215), 1, true, "rescue/1");
        module.add_function(Atom::new(216), 1, true, "after/1");
        module.add_function(Atom::new(217), 2, true, "raise/2");
        module.add_function(Atom::new(218), 1, true, "reraise/2");
        module.add_function(Atom::new(219), 2, true, "match/2");
        module.add_function(Atom::new(220), 2, true, "bind_quoted/2");
        module.add_function(Atom::new(221), 2, true, "<<>>/2");
        module.add_function(Atom::new(222), 2, true, "%{}/2");
        module.add_function(Atom::new(223), 3, true, "&/3");
        module.add_function(Atom::new(224), 1, true, "__ENV__/0");
        module.add_function(Atom::new(225), 0, true, "__MODULE__/0");
        module.add_function(Atom::new(226), 0, true, "__DIR__/0");
        module.add_function(Atom::new(227), 0, true, "__CALLER__/0");
        module.add_function(Atom::new(228), 0, true, "__STACKTRACE__/0");
        module.add_function(Atom::new(229), 1, true, "__aliases__/1");
        module.add_function(Atom::new(230), 1, true, "__block__/1");
        module.add_function(Atom::new(231), 2, true, "::/2");
        module.add_function(Atom::new(232), 3, true, "=/3");

        module
    }
}

impl Default for Stdlib {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in protocol definitions.
#[derive(Debug, Clone)]
pub struct ProtocolDef {
    /// Protocol name
    pub name: ModuleName,
    /// Required functions
    pub functions: Vec<(&'static str, u8)>,
    /// Implementations
    pub implementations: Vec<ModuleName>,
}

impl ProtocolDef {
    /// Create a new protocol.
    pub fn new(name: ModuleName) -> Self {
        ProtocolDef {
            name,
            functions: Vec::new(),
            implementations: Vec::new(),
        }
    }

    /// Add a required function.
    pub fn add_function(&mut self, name: &'static str, arity: u8) {
        self.functions.push((name, arity));
    }

    /// Add an implementation.
    pub fn add_implementation(&mut self, module: ModuleName) {
        self.implementations.push(module);
    }
}

/// Protocol registry.
pub struct Protocols {
    /// Registered protocols
    protocols: HashMap<ModuleName, ProtocolDef>,
}

impl Protocols {
    /// Create a new protocol registry.
    pub fn new() -> Self {
        let mut protocols = HashMap::new();

        // String.Chars protocol
        let mut string_chars = ProtocolDef::new(ModuleName::new(vec![Atom::new(300)]));
        string_chars.add_function("to_string", 1);
        protocols.insert(ModuleName::new(vec![Atom::new(300)]), string_chars);

        // Inspect protocol
        let mut inspect = ProtocolDef::new(ModuleName::new(vec![Atom::new(301)]));
        inspect.add_function("inspect", 1);
        protocols.insert(ModuleName::new(vec![Atom::new(301)]), inspect);

        // Enumerable protocol
        let mut enumerable = ProtocolDef::new(ModuleName::new(vec![Atom::new(302)]));
        enumerable.add_function("count", 1);
        enumerable.add_function("member?", 2);
        enumerable.add_function("slice", 1);
        enumerable.add_function("reduce", 3);
        enumerable.add_function("init", 1);
        enumerable.add_function("next", 1);
        protocols.insert(ModuleName::new(vec![Atom::new(302)]), enumerable);

        // Collectable protocol
        let mut collectable = ProtocolDef::new(ModuleName::new(vec![Atom::new(303)]));
        collectable.add_function("collect", 1);
        protocols.insert(ModuleName::new(vec![Atom::new(303)]), collectable);

        // Inspect.Algebra protocol
        let mut inspect_algebra = ProtocolDef::new(ModuleName::new(vec![Atom::new(304)]));
        inspect_algebra.add_function("to_algebra", 2);
        protocols.insert(ModuleName::new(vec![Atom::new(304)]), inspect_algebra);

        // List.Chars protocol
        let mut list_chars = ProtocolDef::new(ModuleName::new(vec![Atom::new(305)]));
        list_chars.add_function("to_charlist", 1);
        protocols.insert(ModuleName::new(vec![Atom::new(305)]), list_chars);

        // Encoding protocol
        let mut encoding = ProtocolDef::new(ModuleName::new(vec![Atom::new(306)]));
        encoding.add_function("decode", 2);
        protocols.insert(ModuleName::new(vec![Atom::new(306)]), encoding);

        Protocols { protocols }
    }

    /// Get a protocol by name.
    pub fn get(&self, name: &ModuleName) -> Option<&ProtocolDef> {
        self.protocols.get(name)
    }

    /// Get all protocol names.
    pub fn protocol_names(&self) -> Vec<ModuleName> {
        self.protocols.keys().cloned().collect()
    }

    /// Check if a type implements a specific protocol.
    pub fn implements(&self, protocol_name: &ModuleName, type_module: &ModuleName) -> bool {
        if let Some(proto) = self.protocols.get(protocol_name) {
            proto.implementations.iter().any(|impl_module| impl_module == type_module)
        } else {
            false
        }
    }

    /// Get all implementations of a protocol.
    pub fn implementations_of(&self, protocol_name: &ModuleName) -> Option<&Vec<ModuleName>> {
        self.protocols.get(protocol_name).map(|p| &p.implementations)
    }

    /// Find which protocol implements a given function.
    /// Returns the protocol name if found.
    pub fn find_protocol_for_function(&self, function_name: &str) -> Option<ModuleName> {
        for (proto_name, proto) in &self.protocols {
            if proto.functions.iter().any(|(name, _)| *name == function_name) {
                return Some(proto_name.clone());
            }
        }
        None
    }

    /// Check if a function is a protocol function.
    pub fn is_protocol_function(&self, function_name: &str) -> bool {
        self.find_protocol_for_function(function_name).is_some()
    }
}

impl Default for Protocols {
    fn default() -> Self {
        Self::new()
    }
}

/// Exception module.
#[derive(Debug, Clone)]
pub struct ExceptionDef {
    /// Exception name
    pub name: Atom,
    /// Error code (for tagged exceptions)
    pub error_code: u32,
    /// Description
    pub description: &'static str,
    /// Original module (for wrapped exceptions)
    pub original_module: Option<Atom>,
    /// Arguments for formatting
    pub arity: u8,
    /// Fields for exception payload
    pub fields: Vec<&'static str>,
    /// Additional attributes
    pub attributes: Vec<(&'static str, &'static str)>,
}

impl ExceptionDef {
    /// Create a new exception.
    pub fn new(name: Atom) -> Self {
        ExceptionDef {
            name,
            error_code: 0,
            description: "runtime error",
            original_module: None,
            arity: 0,
            fields: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Create a new exception with error code.
    pub fn with_code(name: Atom, code: u32) -> Self {
        ExceptionDef {
            name,
            error_code: code,
            description: "runtime error",
            original_module: None,
            arity: 0,
            fields: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set the original module (for wrapped exceptions).
    pub fn with_original_module(mut self, module: Atom) -> Self {
        self.original_module = Some(module);
        self
    }

    /// Set the arity.
    pub fn with_arity(mut self, arity: u8) -> Self {
        self.arity = arity;
        self
    }

    /// Add a field.
    pub fn with_field(mut self, field: &'static str) -> Self {
        self.fields.push(field);
        self
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, name: &'static str, value: &'static str) -> Self {
        self.attributes.push((name, value));
        self
    }
}

/// Exception registry.
pub struct Exceptions {
    /// Registered exceptions
    exceptions: Vec<ExceptionDef>,
}

impl Exceptions {
    /// Create a new exception registry.
    pub fn new() -> Self {
        let mut exceptions = Vec::new();

        // RuntimeError (error code 1)
        exceptions.push(ExceptionDef::with_code(Atom::new(400), 1)
            .with_description("runtime error"));

        // ArgumentError (error code 2)
        exceptions.push(ExceptionDef::with_code(Atom::new(401), 2)
            .with_description("argument error")
            .with_field("argument"));

        // ArithmeticError (error code 3)
        exceptions.push(ExceptionDef::with_code(Atom::new(402), 3)
            .with_description("arithmetic error"));

        // BadArityError (error code 4)
        exceptions.push(ExceptionDef::with_code(Atom::new(403), 4)
            .with_description("bad arity error")
            .with_field("function")
            .with_field("arity"));

        // BadFunctionError (error code 5)
        exceptions.push(ExceptionDef::with_code(Atom::new(404), 5)
            .with_description("bad function error")
            .with_field("function"));

        // BadMatchError (error code 6)
        exceptions.push(ExceptionDef::with_code(Atom::new(405), 6)
            .with_description("bad match error")
            .with_field("value"));

        // CaseClauseError (error code 7)
        exceptions.push(ExceptionDef::with_code(Atom::new(406), 7)
            .with_description("case clause error")
            .with_field("value"));

        // CondClauseError (error code 8)
        exceptions.push(ExceptionDef::with_code(Atom::new(407), 8)
            .with_description("cond clause error"));

        // Protocol.UndefinedError (error code 9)
        exceptions.push(ExceptionDef::with_code(Atom::new(408), 9)
            .with_description("protocol undefined error")
            .with_field("protocol")
            .with_field("type"));

        // SyntaxError (error code 10)
        exceptions.push(ExceptionDef::with_code(Atom::new(409), 10)
            .with_description("syntax error")
            .with_field("token"));

        // TokenMissingError (error code 11)
        exceptions.push(ExceptionDef::with_code(Atom::new(410), 11)
            .with_description("token missing error")
            .with_field("expected"));

        Exceptions { exceptions }
    }

    /// Get all exceptions.
    pub fn all(&self) -> &[ExceptionDef] {
        &self.exceptions
    }
}

impl Default for Exceptions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdlib_new() {
        let stdlib = Stdlib::new();
        assert!(!stdlib.module_names().is_empty());
    }

    #[test]
    fn test_stdlib_kernel() {
        let stdlib = Stdlib::new();
        let kernel = stdlib.get(&ModuleName::new(vec![Atom::new(0)]));
        assert!(kernel.is_some());
        let kernel = kernel.unwrap();
        assert!(!kernel.functions.is_empty());
        assert!(!kernel.macros.is_empty());
    }

    #[test]
    fn test_stdlib_macro() {
        let stdlib = Stdlib::new();
        let macro_mod = stdlib.get(&ModuleName::new(vec![Atom::new(1)]));
        assert!(macro_mod.is_some());
    }

    #[test]
    fn test_stdlib_module() {
        let stdlib = Stdlib::new();
        let module = stdlib.get(&ModuleName::new(vec![Atom::new(2)]));
        assert!(module.is_some());
    }

    #[test]
    fn test_stdlib_code() {
        let stdlib = Stdlib::new();
        let code = stdlib.get(&ModuleName::new(vec![Atom::new(3)]));
        assert!(code.is_some());
    }

    #[test]
    fn test_stdlib_special_forms() {
        let stdlib = Stdlib::new();
        let forms = stdlib.get(&ModuleName::new(vec![Atom::new(4), Atom::new(5)]));
        assert!(forms.is_some());
        let forms = forms.unwrap();
        // Special forms should all be marked as special_form = true
        for func in forms.functions() {
            assert!(func.special_form, "Function {:?} should be special form", func.name);
        }
    }

    #[test]
    fn test_protocols_new() {
        let protocols = Protocols::new();
        assert!(!protocols.protocol_names().is_empty());
    }

    #[test]
    fn test_protocols_string_chars() {
        let protocols = Protocols::new();
        let string_chars = protocols.get(&ModuleName::new(vec![Atom::new(300)]));
        assert!(string_chars.is_some());
    }

    #[test]
    fn test_protocols_enumerable() {
        let protocols = Protocols::new();
        let enumerable = protocols.get(&ModuleName::new(vec![Atom::new(302)]));
        assert!(enumerable.is_some());
        let enumerable = enumerable.unwrap();
        assert!(enumerable.functions.len() >= 6); // count, member?, slice, reduce, init, next
    }

    #[test]
    fn test_exceptions_new() {
        let exceptions = Exceptions::new();
        assert!(!exceptions.all().is_empty());
    }

    #[test]
    fn test_exception_def() {
        let exc = ExceptionDef::new(Atom::new(500));
        assert_eq!(exc.name.id(), 500);
    }

    #[test]
    fn test_native_module_builder() {
        let mut module = NativeModule::new(ModuleName::new(vec![Atom::new(600)]));
        module.add_function(Atom::new(601), 2, false, "test/2");
        module.add_macro(Atom::new(602), 1, "test_macro/1");
        module.add_type("test_type()");
        module.add_attribute("moduledoc", "Test module");

        assert_eq!(module.functions().len(), 1);
        assert_eq!(module.macros.len(), 1);
        assert_eq!(module.types.len(), 1);
        assert_eq!(module.attributes.len(), 1);
    }

    #[test]
    fn test_protocol_def_builder() {
        let mut proto = ProtocolDef::new(ModuleName::new(vec![Atom::new(700)]));
        proto.add_function("required_func", 2);
        proto.add_implementation(ModuleName::new(vec![Atom::new(701)]));

        assert_eq!(proto.functions.len(), 1);
        assert_eq!(proto.implementations.len(), 1);
    }

    #[test]
    fn test_kernel_builtin_kind_type_check() {
        let stdlib = Stdlib::new();
        let kernel = stdlib.get(&ModuleName::new(vec![Atom::new(0)])).unwrap();
        // is_atom should be a type check
        let is_atom = kernel.get_function(&Atom::new(31));
        assert!(is_atom.is_some());
        assert_eq!(is_atom.unwrap().builtin_kind, BuiltinKind::TypeCheck);
    }

    #[test]
    fn test_kernel_builtin_kind_arithmetic() {
        let stdlib = Stdlib::new();
        let kernel = stdlib.get(&ModuleName::new(vec![Atom::new(0)])).unwrap();
        // + should be arithmetic
        let add = kernel.get_function(&Atom::new(10));
        assert!(add.is_some());
        assert_eq!(add.unwrap().builtin_kind, BuiltinKind::Arithmetic);
    }

    #[test]
    fn test_kernel_builtin_kind_comparison() {
        let stdlib = Stdlib::new();
        let kernel = stdlib.get(&ModuleName::new(vec![Atom::new(0)])).unwrap();
        // == should be comparison
        let eq = kernel.get_function(&Atom::new(16));
        assert!(eq.is_some());
        assert_eq!(eq.unwrap().builtin_kind, BuiltinKind::Comparison);
    }

    #[test]
    fn test_kernel_builtin_kind_list() {
        let stdlib = Stdlib::new();
        let kernel = stdlib.get(&ModuleName::new(vec![Atom::new(0)])).unwrap();
        // length should be list operation
        let length = kernel.get_function(&Atom::new(63));
        assert!(length.is_some());
        assert_eq!(length.unwrap().builtin_kind, BuiltinKind::List);
    }

    #[test]
    fn test_kernel_functions_count() {
        let stdlib = Stdlib::new();
        let kernel = stdlib.get(&ModuleName::new(vec![Atom::new(0)])).unwrap();
        // Kernel should have many functions defined
        assert!(kernel.functions().len() > 50);
    }

    #[test]
    fn test_protocol_find_for_function() {
        let protocols = Protocols::new();
        // inspect is a protocol function
        let proto = protocols.find_protocol_for_function("inspect");
        assert!(proto.is_some());
    }

    #[test]
    fn test_protocol_is_protocol_function() {
        let protocols = Protocols::new();
        assert!(protocols.is_protocol_function("inspect"));
        assert!(protocols.is_protocol_function("to_string"));
        assert!(!protocols.is_protocol_function("not_a_real_function"));
    }

    #[test]
    fn test_protocols_enumerable_functions() {
        let protocols = Protocols::new();
        let enumerable = protocols.get(&ModuleName::new(vec![Atom::new(302)])).unwrap();
        // Enumerable should have count, member?, slice, reduce, init, next
        let func_names: Vec<_> = enumerable.functions.iter().map(|(n, _)| *n).collect();
        assert!(func_names.contains(&"count"));
        assert!(func_names.contains(&"member?"));
        assert!(func_names.contains(&"reduce"));
    }

    #[test]
    fn test_protocol_add_implementation() {
        let mut proto = ProtocolDef::new(ModuleName::new(vec![Atom::new(900)]));
        proto.add_function("test_func", 1);
        proto.add_implementation(ModuleName::new(vec![Atom::new(901)]));
        assert!(proto.implementations.len() == 1);
    }

    #[test]
    fn test_exception_error_codes() {
        let exceptions = Exceptions::new();
        let all = exceptions.all();
        // Each exception should have a unique error code
        let mut codes: Vec<u32> = all.iter().map(|e| e.error_code).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
    }

    #[test]
    fn test_exception_with_fields() {
        let exc = ExceptionDef::new(Atom::new(500))
            .with_description("test error")
            .with_field("arg1")
            .with_field("arg2");
        assert_eq!(exc.fields.len(), 2);
    }

    #[test]
    fn test_exception_with_original_module() {
        let exc = ExceptionDef::new(Atom::new(500))
            .with_original_module(Atom::new(600));
        assert!(exc.original_module.is_some());
        assert_eq!(exc.original_module.unwrap().id(), 600);
    }

    #[test]
    fn test_exceptions_count() {
        let exceptions = Exceptions::new();
        // Should have 11 exception types defined
        assert!(exceptions.all().len() >= 11);
    }
}
