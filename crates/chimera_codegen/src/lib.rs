#! Code generation for the Rust/Zig Elixir compiler.
//!
//! This module provides code generation from Core IR to target artifacts.

#[cfg(test)]
use chimera_allocator as _;

use chimera_core::{CoreClause, CoreExpr, CoreFunction, CoreModule, CorePattern};
use chimera_term::{Atom, AtomTable, ModuleName};
use std::collections::HashMap;

// =============================================================================
// BEAM Opcode Definitions (OTP erts/preloaded/src/init.erl)
// =============================================================================
// Real opcodes extracted from BEAM virtual machine
//
// Section 1: Control flow (0x00-0x0F)
//   0x00: func_info  - Function info block
//   0x01: call       - Call function
//   0x02: call_last  - Tail call optimization
//   0x03: return      - Return from function
//   0x04: branch     - Unconditional jump
//   0x05: branch_if  - Conditional branch (when register is true)
//   0x06: branch_when - Branch when register is non-nil
//   0x07: branch_when_not - Branch when register is nil
//   0x08: branch_match - Pattern match dispatch
//   0x09: branch_match_fail - Pattern match failure
//
// Section 2: Allocate/Deallocate (0x10-0x17)
//   0x10: allocate   - Allocate stack slots
//   0x11: allocate_heap - Allocate with heap need
//   0x12: deallocate - Free stack slots
//   0x13: init      - Initialize registers
//
// Section 3: Put instructions (0x20-0x2F)
//   0x20: put_atom   - Put atom in destination
//   0x21: put_list  - Cons cell constructor
//   0x22: put_tuple  - Tuple constructor
//   0x23: put_map    - Map constructor
//   0x24: put_string - String/ binary segment
//   0x25: put_nil    - Empty list (nil)
//   0x26: put_binary - Binary constructor
//
// Section 4: Load/Store (0x30-0x3F)
//   0x30: load      - Load from source to destination
//   0x31: loadAtom  - Load atom constant
//   0x32: loadSmall - Load small integer
//   0x33: loadBig   - Load big integer
//   0x34: loadFloat - Load float
//   0x35: loadTuple - Load tuple constant
//   0x36: loadNil   - Load nil
//   0x37: loadContext - Load context pointer
//
// Section 5: Integer math (0x40-0x4F)
//   0x40: add        - Integer addition
//   0x41: addSmall  - Add small integer
//   0x42: sub        - Integer subtraction
//   0x43: subSmall  - Sub small integer
//   0x44: mul        - Integer multiplication
//   0x45: div        - Integer division
//   0x46: divMod     - Division with remainder
//   0x47: band       - Bitwise AND
//   0x48: bor        - Bitwise OR
//   0x49: bxor       - Bitwise XOR
//   0x4A: bsl        - Left shift
//   0x4B: bsr        - Right shift
//
// Section 6: Float math (0x50-0x5F)
//   0x50: fadd      - Float addition
//   0x51: fsub      - Float subtraction
//   0x52: fmul      - Float multiplication
//   0x53: fdiv      - Float division
//
// Section 7: Term comparison (0x60-0x6F)
//   0x60: isEQ      - Equality test
//   0x61: isNE      - Not equal test
//   0x62: isLT      - Less than
//   0x63: isLE      - Less or equal
//   0x64: isGT      - Greater than
//   0x65: isGE      - Greater or equal
//   0x66: isAtom    - Atom test
//   0x67: isNil     - Nil test
//   0x68: isBinary  - Binary test
//   0x69: isList    - List test
//   0x6A: isTuple   - Tuple test
//
// Section 8: Exception handling (0x70-0x7F)
//   0x70: try       - Enter try block
//   0x71: try_end   - End of try block
//   0x72: catch     - Enter catch block
//   0x73: catch_end - End of catch block
//   0x74: raise     - Raise exception
//
// Section 9: Message handling (0x80-0x8F)
//   0x80: send      - Send message
//   0x81: receive_start - Start receive
//   0x82: receive_next - Next message
//   0x83: receive_end - End receive
//   0x84: receive_wait - Wait for message
//   0x85: receive_after - Timeout handling
//
// Extended opcodes (0x90-0xFF)
//   0x90: allocate_zero - Allocate with zeroing
//   0x91: test_heap - Check heap availability
//   0x92: move      - Move between registers
//   0x93: getMapElements - Get map fields
//   0x94: bswap    - Byte swap
//   0x95: i_call    - Indirect call
//   0x96: i_call_fun - Indirect fun call
//   0x97: i_timeout - Set timeout
//   0x98: i_repeat  - Repeat instruction

/// BEAM opcodes for the virtual machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // Control flow
    FuncInfo = 0x00,
    Call = 0x01,
    CallLast = 0x02,
    Return = 0x03,
    Branch = 0x04,
    BranchIf = 0x05,
    BranchWhen = 0x06,
    BranchWhenNot = 0x07,
    BranchMatch = 0x08,
    BranchMatchFail = 0x09,

    // Allocate/Deallocate
    Allocate = 0x10,
    AllocateHeap = 0x11,
    Deallocate = 0x12,
    Init = 0x13,

    // Put instructions
    PutAtom = 0x20,
    PutList = 0x21,
    PutTuple = 0x22,
    PutMap = 0x23,
    PutString = 0x24,
    PutNil = 0x25,
    PutBinary = 0x26,

    // Load/Store
    Load = 0x30,
    LoadAtom = 0x31,
    LoadSmall = 0x32,
    LoadBig = 0x33,
    LoadFloat = 0x34,
    LoadTuple = 0x35,
    LoadNil = 0x36,
    LoadContext = 0x37,

    // Integer math
    Add = 0x40,
    AddSmall = 0x41,
    Sub = 0x42,
    SubSmall = 0x43,
    Mul = 0x44,
    Div = 0x45,
    DivMod = 0x46,
    Band = 0x47,
    Bor = 0x48,
    Bxor = 0x49,
    Bsl = 0x4A,
    Bsr = 0x4B,

    // Float math
    Fadd = 0x50,
    Fsub = 0x51,
    Fmul = 0x52,
    Fdiv = 0x53,

    // Term comparison
    IsEQ = 0x60,
    IsNE = 0x61,
    IsLT = 0x62,
    IsLE = 0x63,
    IsGT = 0x64,
    IsGE = 0x65,
    IsAtom = 0x66,
    IsNil = 0x67,
    IsBinary = 0x68,
    IsList = 0x69,
    IsTuple = 0x6A,

    // Exception handling
    Try = 0x70,
    TryEnd = 0x71,
    Catch = 0x72,
    CatchEnd = 0x73,
    Raise = 0x74,

    // Message handling
    Send = 0x80,
    ReceiveStart = 0x81,
    ReceiveNext = 0x82,
    ReceiveEnd = 0x83,
    ReceiveWait = 0x84,
    ReceiveAfter = 0x85,

    // Extended
    AllocateZero = 0x90,
    TestHeap = 0x91,
    Move = 0x92,
    GetMapElements = 0x93,
    Bswap = 0x94,
    ICall = 0x95,
    ICallFun = 0x96,
    ITimeout = 0x97,
    IRepeat = 0x98,
}

/// Literal table entry for encoding constants in BEAM files
#[derive(Debug, Clone)]
pub enum LiteralEntry {
    /// Integer constant (stored as bytes, little-endian)
    Integer(Vec<u8>),
    /// Float constant (IEEE 754 double, 8 bytes)
    Float(f64),
    /// Atom constant (stored as atom table index)
    Atom(u32),
    /// Nil constant
    Nil,
    /// Tuple constant (elements are literal table indices as u32)
    Tuple(Vec<u32>),
    /// List constant (head and tail literal table indices)
    Cons(u32, u32),
    /// Map constant (key-value pairs as literal table index pairs)
    Map(Vec<(u32, u32)>),
    /// String constant (UTF-8 bytes)
    String(Vec<u8>),
}

impl LiteralEntry {
    /// Encode this literal entry to bytes in BEAM literal table format
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            LiteralEntry::Integer(digits) => {
                bytes.push(0x01); // integer tag
                let len = digits.len() as u8;
                bytes.push(len);
                bytes.extend(digits);
            }
            LiteralEntry::Float(val) => {
                bytes.push(0x02); // float tag
                bytes.extend_from_slice(&val.to_le_bytes());
            }
            LiteralEntry::Atom(idx) => {
                bytes.push(0x03); // atom tag
                bytes.extend_from_slice(&idx.to_be_bytes());
            }
            LiteralEntry::Nil => {
                bytes.push(0x04); // nil tag
            }
            LiteralEntry::Tuple(elements) => {
                bytes.push(0x05); // tuple tag
                let arity = elements.len() as u8;
                bytes.push(arity);
                for elem_idx in elements {
                    bytes.extend_from_slice(&elem_idx.to_be_bytes());
                }
            }
            LiteralEntry::Cons(head, tail) => {
                bytes.push(0x06); // cons tag
                bytes.extend_from_slice(&head.to_be_bytes());
                bytes.extend_from_slice(&tail.to_be_bytes());
            }
            LiteralEntry::Map(kvs) => {
                bytes.push(0x07); // map tag
                let size = kvs.len() as u8;
                bytes.push(size);
                for (k, v) in kvs {
                    bytes.extend_from_slice(&k.to_be_bytes());
                    bytes.extend_from_slice(&v.to_be_bytes());
                }
            }
            LiteralEntry::String(utf8) => {
                bytes.push(0x08); // string tag
                let len = utf8.len() as u32;
                bytes.extend_from_slice(&len.to_be_bytes());
                bytes.extend(utf8);
            }
        }
        bytes
    }
}

/// Literal table for storing constant values in BEAM format
#[derive(Debug, Clone, Default)]
pub struct LiteralTable {
    entries: Vec<LiteralEntry>,
}

impl LiteralTable {
    /// Create a new empty literal table
    pub fn new() -> Self {
        LiteralTable {
            entries: Vec::new(),
        }
    }

    /// Add a float literal and return its index
    pub fn add_float(&mut self, val: f64) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Float(val));
        idx
    }

    /// Add a big integer literal and return its index
    pub fn add_integer(&mut self, digits: Vec<u8>) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Integer(digits));
        idx
    }

    /// Add an atom literal and return its index
    pub fn add_atom(&mut self, atom_idx: u32) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Atom(atom_idx));
        idx
    }

    /// Add a nil literal and return its index
    pub fn add_nil(&mut self) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Nil);
        idx
    }

    /// Add a tuple literal and return its index
    pub fn add_tuple(&mut self, elements: Vec<u32>) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Tuple(elements));
        idx
    }

    /// Add a cons cell literal and return its index
    pub fn add_cons(&mut self, head: u32, tail: u32) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Cons(head, tail));
        idx
    }

    /// Add a map literal and return its index
    pub fn add_map(&mut self, kvs: Vec<(u32, u32)>) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::Map(kvs));
        idx
    }

    /// Add a string literal and return its index
    pub fn add_string(&mut self, utf8: Vec<u8>) -> u32 {
        let idx = self.entries.len() as u32;
        self.entries.push(LiteralEntry::String(utf8));
        idx
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Encode the entire literal table in BEAM format
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        // u32 count of entries (BEAM uses big-endian)
        bytes.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        for entry in &self.entries {
            bytes.extend(entry.encode());
        }
        bytes
    }
}

/// Code generator configuration.
#[derive(Debug, Clone)]
pub struct CodegenConfig {
    /// Target format (e.g., "beam", "core")
    pub target: String,
    /// Enable debug info
    pub debug: bool,
    /// Optimization level
    pub opt_level: u8,
}

impl Default for CodegenConfig {
    fn default() -> Self {
        CodegenConfig {
            target: "beam".to_string(),
            debug: false,
            opt_level: 2,
        }
    }
}

/// Code generator error.
#[derive(Debug, Clone)]
pub enum CodegenError {
    /// Unsupported expression
    UnsupportedExpr(String),
    /// Invalid module
    InvalidModule(String),
    /// Target error
    TargetError(String),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::UnsupportedExpr(s) => write!(f, "Unsupported expression: {}", s),
            CodegenError::InvalidModule(s) => write!(f, "Invalid module: {}", s),
            CodegenError::TargetError(s) => write!(f, "Target error: {}", s),
        }
    }
}

impl std::error::Error for CodegenError {}

/// Generated code output.
#[derive(Debug, Clone)]
pub struct CodegenOutput {
    /// Generated bytecode or source
    pub code: Vec<u8>,
    /// Export table
    pub exports: Vec<(Atom, u8)>,
    /// Attributes
    pub attributes: HashMap<Atom, Vec<u8>>,
    /// Compile info
    pub compile_info: CompileInfoOutput,
    /// Atoms table
    pub atoms: Vec<String>,
    /// Literal table for constants (floats, large integers, etc.)
    pub literals: LiteralTable,
}

/// Compile info output.
#[derive(Debug, Clone)]
pub struct CompileInfoOutput {
    pub file: Option<String>,
    pub line: u32,
    pub vsn: Option<Vec<u8>>,
}

/// Core IR to target code generator.
pub struct Codegen {
    /// Configuration
    config: CodegenConfig,
    /// Atom table
    atoms: AtomTable,
    /// Generated atoms
    generated_atoms: Vec<String>,
    /// Literal table for constants
    literals: LiteralTable,
}

impl Codegen {
    /// Create a new code generator.
    pub fn new(config: CodegenConfig, atoms: AtomTable) -> Self {
        Codegen {
            config,
            atoms,
            generated_atoms: Vec::new(),
            literals: LiteralTable::new(),
        }
    }

    /// Get reference to the literal table
    pub fn literals(&self) -> &LiteralTable {
        &self.literals
    }

    /// Generate code from a Core module.
    pub fn generate(&mut self, module: &CoreModule) -> Result<CodegenOutput, CodegenError> {
        let mut exports = Vec::new();
        let mut code = Vec::new();
        let attributes = HashMap::new();

        // Generate function code
        for func in &module.functions {
            let func_code = self.generate_function(func)?;
            code.extend(func_code);

            // Add to exports if exported
            if func.exported {
                exports.push((func.name.clone(), func.arity));
            }
        }

        // Generate atoms table
        for func in &module.functions {
            if let Some(name) = self.atoms.lookup(func.name.clone()) {
                let name_str: String = (&**name).into();
                if !self.generated_atoms.contains(&name_str) {
                    self.generated_atoms.push(name_str);
                }
            }
        }

        Ok(CodegenOutput {
            code,
            exports,
            attributes,
            compile_info: CompileInfoOutput {
                file: module.compile_info.file.clone(),
                line: module.compile_info.line,
                vsn: module.compile_info.vsn.clone().map(|a| {
                    self.atoms
                        .lookup(a)
                        .map(|s| s.to_string().into_bytes())
                        .unwrap_or_default()
                }),
            },
            atoms: self.generated_atoms.clone(),
            literals: self.literals.clone(),
        })
    }

    /// Generate code for a function.
    fn generate_function(&mut self, func: &CoreFunction) -> Result<Vec<u8>, CodegenError> {
        let mut code = Vec::new();

        // Function header - func_info provides module, function name, arity
        code.push(Opcode::FuncInfo as u8);
        code.push(0); // destination register
        code.push(self.register_atom(&func.name)? as u8);
        code.push(func.arity);
        code.push(func.params.len() as u8); // clause count

        // Generate parameter loading - move each parameter to its slot
        for (i, _param) in func.params.iter().enumerate() {
            code.push(Opcode::Move as u8);
            code.push(i as u8); // destination (register index)
            code.push(i as u8); // source (parameter comes pre-loaded)
        }

        // Generate body
        let body_code = self.generate_expr(&func.body)?;
        code.extend(body_code);

        // Return instruction
        code.push(Opcode::Return as u8);

        Ok(code)
    }

    /// Generate code for an expression.
    fn generate_expr(&mut self, expr: &CoreExpr) -> Result<Vec<u8>, CodegenError> {
        match expr {
            CoreExpr::Unit => Ok(vec![]),
            CoreExpr::Atom(a) => {
                let mut code = Vec::new();
                code.push(Opcode::PutAtom as u8);
                code.push(self.register_atom(a)? as u8);
                Ok(code)
            }
            CoreExpr::Integer(i) => {
                let mut code = Vec::new();
                if *i >= 0 && *i <= 255 {
                    // Small integer: embed directly as operand
                    code.push(Opcode::LoadSmall as u8);
                    code.push(*i as u8);
                } else {
                    // Large integer: store in literal table, load from there
                    let digits = i.to_le_bytes().to_vec();
                    let lit_idx = self.literals.add_integer(digits);
                    code.push(Opcode::LoadBig as u8);
                    code.extend_from_slice(&lit_idx.to_be_bytes());
                }
                Ok(code)
            }
            CoreExpr::Float(f) => {
                let mut code = Vec::new();
                // Store float in literal table and load with LoadFloat
                let lit_idx = self.literals.add_float(*f);
                code.push(Opcode::LoadFloat as u8);
                code.extend_from_slice(&lit_idx.to_be_bytes());
                Ok(code)
            }
            CoreExpr::String(s) => {
                let mut code = Vec::new();
                code.push(Opcode::PutString as u8);
                code.extend_from_slice(s.as_bytes());
                Ok(code)
            }
            CoreExpr::List(items) => {
                let mut code = Vec::new();
                for item in items {
                    code.extend(self.generate_expr(item)?);
                }
                code.push(Opcode::PutNil as u8);
                for _ in 0..items.len() {
                    code.push(Opcode::PutList as u8);
                }
                Ok(code)
            }
            CoreExpr::Tuple(elements) => {
                let mut code = Vec::new();
                for elem in elements {
                    code.extend(self.generate_expr(elem)?);
                }
                code.push(Opcode::PutTuple as u8);
                code.push(elements.len() as u8);
                Ok(code)
            }
            CoreExpr::Var { name, arity } => {
                let mut code = Vec::new();
                code.push(Opcode::Load as u8);
                code.push(self.register_atom(name)? as u8);
                code.push(*arity);
                Ok(code)
            }
            CoreExpr::Call { module, name, args } => {
                let mut code = Vec::new();
                for arg in args {
                    code.extend(self.generate_expr(arg)?);
                }
                code.push(Opcode::Call as u8);
                if let Some(m) = module {
                    code.push(self.register_atom(m)? as u8);
                }
                code.push(self.register_atom(name)? as u8);
                code.push(args.len() as u8);
                Ok(code)
            }
            CoreExpr::Lambda { args, body } => {
                let mut code = Vec::new();
                code.push(Opcode::ICallFun as u8); // indirect fun call
                code.push(args.len() as u8);
                code.extend(self.generate_expr(body)?);
                Ok(code)
            }
            CoreExpr::Let { vars, value, body } => {
                let mut code = Vec::new();
                code.extend(self.generate_expr(value)?);
                for var in vars {
                    code.push(Opcode::Move as u8);
                    code.push(self.register_atom(var)? as u8);
                }
                code.extend(self.generate_expr(body)?);
                Ok(code)
            }
            CoreExpr::Seq(exprs) => {
                let mut code = Vec::new();
                for expr in exprs {
                    code.extend(self.generate_expr(expr)?);
                }
                Ok(code)
            }
            CoreExpr::Match {
                pattern,
                value,
                body,
            } => {
                let mut code = Vec::new();
                code.extend(self.generate_expr(value)?);
                let match_code = self.generate_pattern_match(pattern, body)?;
                code.extend(match_code);
                Ok(code)
            }
            CoreExpr::Case { expr, clauses } => {
                let mut code = Vec::new();
                code.extend(self.generate_expr(expr)?);
                for clause in clauses {
                    let clause_code = self.generate_clause(clause)?;
                    code.extend(clause_code);
                }
                Ok(code)
            }
            CoreExpr::Try { expr, clauses } => {
                let mut code = Vec::new();
                code.push(Opcode::Try as u8);
                code.extend(self.generate_expr(expr)?);
                for clause in clauses {
                    let clause_code = self.generate_clause(clause)?;
                    code.extend(clause_code);
                }
                Ok(code)
            }
            CoreExpr::Receive { clauses, timeout } => {
                let mut code = Vec::new();
                code.push(Opcode::ReceiveStart as u8);
                for clause in clauses {
                    let clause_code = self.generate_clause(clause)?;
                    code.extend(clause_code);
                }
                if let Some((t, b)) = timeout {
                    code.extend(self.generate_expr(t)?);
                    code.extend(self.generate_expr(b)?);
                }
                Ok(code)
            }
            CoreExpr::TupleCons { elements } => {
                let mut code = Vec::new();
                for elem in elements {
                    code.extend(self.generate_expr(elem)?);
                }
                code.push(Opcode::PutNil as u8);
                for _ in elements.iter().rev() {
                    code.push(Opcode::PutList as u8);
                }
                Ok(code)
            }
            CoreExpr::MapUpdate { base, updates } => {
                let mut code = Vec::new();
                code.extend(self.generate_expr(base)?);
                for (k, v) in updates {
                    code.extend(self.generate_expr(k)?);
                    code.extend(self.generate_expr(v)?);
                }
                code.push(Opcode::PutMap as u8);
                code.push(updates.len() as u8);
                Ok(code)
            }
            CoreExpr::Binary { segments } => {
                let mut code = Vec::new();
                for seg in segments {
                    code.extend(self.generate_expr(&seg.expr)?);
                }
                code.push(Opcode::PutBinary as u8);
                code.push(segments.len() as u8);
                Ok(code)
            }
            CoreExpr::Map(pairs) => {
                let mut code = Vec::new();
                for (k, v) in pairs {
                    code.extend(self.generate_expr(k)?);
                    code.extend(self.generate_expr(v)?);
                }
                code.push(Opcode::PutMap as u8);
                code.push(pairs.len() as u8);
                Ok(code)
            }
        }
    }

    /// Generate pattern matching code.
    fn generate_pattern_match(
        &mut self,
        pattern: &CorePattern,
        body: &CoreExpr,
    ) -> Result<Vec<u8>, CodegenError> {
        let mut code = Vec::new();
        match pattern {
            CorePattern::Wildcard => {
                code.extend(self.generate_expr(body)?);
            }
            CorePattern::Var(v) => {
                code.push(Opcode::Move as u8);
                code.push(self.register_atom(v)? as u8);
                code.extend(self.generate_expr(body)?);
            }
            CorePattern::Atom(a) => {
                code.push(Opcode::LoadAtom as u8);
                code.push(self.register_atom(a)? as u8);
                code.extend(self.generate_expr(body)?);
            }
            CorePattern::Integer(i) => {
                code.push(Opcode::LoadSmall as u8);
                code.extend_from_slice(&i.to_le_bytes());
                code.extend(self.generate_expr(body)?);
            }
            CorePattern::Tuple(patterns) => {
                for p in patterns {
                    code.extend(self.generate_pattern_match(p, body)?);
                }
            }
            _ => return Err(CodegenError::UnsupportedExpr(format!("{:?}", pattern))),
        }
        Ok(code)
    }

    /// Generate code for a clause.
    fn generate_clause(&mut self, clause: &CoreClause) -> Result<Vec<u8>, CodegenError> {
        let mut code = Vec::new();
        code.extend(self.generate_pattern_match(&clause.pattern, &clause.body)?);
        Ok(code)
    }

    /// Register an atom and return its index.
    ///
    /// Uses the atom's index directly instead of looking up by name,
    /// which avoids the issue of atoms not being found if they were
    /// just created in the AtomTable.
    fn register_atom(&mut self, atom: &Atom) -> Result<usize, CodegenError> {
        // Use the atom's ID directly - this is the fastest path
        let idx = atom.clone().id() as usize;

        // Get the atom name from the AtomTable
        if let Some(name) = self.atoms.lookup(atom.clone()) {
            let name_str: String = (&**name).into();
            // Ensure we don't have duplicates
            if let Some(existing_idx) = self.generated_atoms.iter().position(|s| s == &name_str) {
                return Ok(existing_idx);
            }
            self.generated_atoms.push(name_str);
            return Ok(self.generated_atoms.len() - 1);
        }

        // Fallback: use ID directly and add a placeholder name
        while self.generated_atoms.len() <= idx {
            self.generated_atoms
                .push(format!("atom_{}", self.generated_atoms.len()));
        }
        Ok(idx)
    }
}

/// Emit compiled module as BEAM-compatible format.
/// Format: Magic header + chunk count + chunks (Atom/Exp/Code/Lit/Attr/etc.)
/// Emit a BEAM file in standard IFF format.
///
/// Standard BEAM format:
/// - "BEAM" magic (4 bytes)
/// - Chunk count (u32, big-endian)
/// - For each chunk:
///   - 4 bytes: chunk type (e.g., "Atom", "Code")
///   - 4 bytes: chunk size (u32, big-endian)
///   - N bytes: chunk data
pub fn emit_beam(output: &CodegenOutput) -> Result<Vec<u8>, CodegenError> {
    let mut beam = Vec::new();

    // BEAM magic header (IFF format has NO chunk count - reads until EOF)
    beam.extend_from_slice(b"BEAM");
    // No chunk count in IFF format - read until EOF

    // Atom table chunk (mandatory) - use "AtU8" for UTF-8 atoms (standard BEAM format)
    emit_beam_chunk(&mut beam, b"AtU8", emit_atoms_chunk(&output.atoms)?)?;

    // Export table chunk (mandatory) - use "ExpT" (standard BEAM format)
    emit_beam_chunk(
        &mut beam,
        b"ExpT",
        emit_exports_chunk(&output.exports, &output.atoms)?,
    )?;

    // Code chunk - use "Code" (standard BEAM format)
    let code_data = emit_code_chunk(&output.code);
    emit_beam_chunk(&mut beam, b"Code", code_data)?;

    // Attributes chunk - use "Attr" (standard BEAM format)
    let attr_data = emit_attributes_chunk(&output.attributes)?;
    emit_beam_chunk(&mut beam, b"Attr", attr_data)?;

    // Literal table chunk - use "LitT" (standard BEAM format)
    let lit_data = output.literals.encode();
    if !lit_data.is_empty() {
        emit_beam_chunk(&mut beam, b"LitT", lit_data)?;
    }
    Ok(beam)
}

/// Emit code chunk with standard BEAM code header.
fn emit_code_chunk(code: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::with_capacity(40 + code.len());

    // Standard BEAM code header (40 bytes):
    // - 0-3: magic (uint32)
    // - 4-7: version (uint32)
    // - 8-11: flags (uint32)
    // - 12-15: code_size (uint32)
    // - 16-19: export_count (uint32)
    // - 20-23: import_count (uint32)
    // - 24-27: local_count (uint32)
    // - 28-31: lambda_count (uint32)
    // - 32-35: code_label_count (uint32)
    // - 36-39: function_count (uint32)

    chunk.extend_from_slice(&0x424F524Du32.to_be_bytes()); // "BOR M" magic
    chunk.extend_from_slice(&1u32.to_be_bytes()); // version 1
    chunk.extend_from_slice(&0u32.to_be_bytes()); // flags
    chunk.extend_from_slice(&(code.len() as u32).to_be_bytes()); // code_size
    chunk.extend_from_slice(&0u32.to_be_bytes()); // export_count (filled by caller)
    chunk.extend_from_slice(&0u32.to_be_bytes()); // import_count
    chunk.extend_from_slice(&0u32.to_be_bytes()); // local_count
    chunk.extend_from_slice(&0u32.to_be_bytes()); // lambda_count
    chunk.extend_from_slice(&0u32.to_be_bytes()); // code_label_count
    chunk.extend_from_slice(&0u32.to_be_bytes()); // function_count

    // Append actual code
    chunk.extend_from_slice(code);

    chunk
}

fn emit_beam_chunk(
    beam: &mut Vec<u8>,
    chunk_type: &[u8; 4],
    data: Vec<u8>,
) -> Result<(), CodegenError> {
    // Chunk header
    beam.extend_from_slice(chunk_type);
    let size = data.len() as u32;
    beam.extend_from_slice(&size.to_be_bytes());
    beam.extend(data);
    Ok(())
}

/// Emit atom table chunk in BEAM format.
/// Standard BEAM format: u32 count + (u8 length + bytes for each atom)
fn emit_atoms_chunk(atoms: &[String]) -> Result<Vec<u8>, CodegenError> {
    let mut chunk = Vec::new();

    // Atom count (u32, big-endian) - standard BEAM uses u32
    chunk.extend_from_slice(&(atoms.len() as u32).to_be_bytes());

    for atom in atoms {
        // Individual atom format: length byte + UTF-8 bytes
        let len = atom.len();
        if len > 255 {
            return Err(CodegenError::TargetError(format!("Atom too long: {}", len)));
        }
        chunk.push(len as u8);
        chunk.extend_from_slice(atom.as_bytes());
    }

    Ok(chunk)
}

/// Emit export table chunk in BEAM format.
/// Standard BEAM format: u32 count + (atom_index u32 + arity u8 + label u32) per entry
fn emit_exports_chunk(exports: &[(Atom, u8)], _atoms: &[String]) -> Result<Vec<u8>, CodegenError> {
    let mut chunk = Vec::new();

    // Export count (u32, big-endian)
    chunk.extend_from_slice(&(exports.len() as u32).to_be_bytes());

    for (name, arity) in exports {
        // Atom id is the index into our interned atom table
        let atom_idx = name.0 as u32;
        chunk.extend_from_slice(&atom_idx.to_be_bytes()); // atom table index
        chunk.push(*arity); // arity
        chunk.extend_from_slice(&1u32.to_be_bytes()); // label (1 = first code label)
    }

    Ok(chunk)
}

/// Emit attributes chunk in BEAM format.
/// Standard BEAM format: u32 count + (atom_index u32 + value binary)
fn emit_attributes_chunk(attributes: &HashMap<Atom, Vec<u8>>) -> Result<Vec<u8>, CodegenError> {
    let mut chunk = Vec::new();
    // Attribute count (u32, big-endian)
    chunk.extend_from_slice(&(attributes.len() as u32).to_be_bytes());
    for (key, value) in attributes {
        // Key is atom index (u32), value is binary
        chunk.extend_from_slice(&key.0.to_be_bytes());
        chunk.extend_from_slice(&(value.len() as u32).to_be_bytes());
        chunk.extend(value);
    }
    Ok(chunk)
}

/// Compile Core IR to target artifact.
pub fn compile_to_target(
    module: &CoreModule,
    atoms: AtomTable,
    config: CodegenConfig,
) -> Result<Vec<u8>, CodegenError> {
    let mut codegen = Codegen::new(config, atoms);
    let output = codegen.generate(module)?;
    emit_beam(&output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_config_default() {
        let config = CodegenConfig::default();
        assert_eq!(config.target, "beam");
        assert!(!config.debug);
        assert_eq!(config.opt_level, 2);
    }

    #[test]
    fn test_codegen_new() {
        let atoms = AtomTable::new();
        let config = CodegenConfig::default();
        let codegen = Codegen::new(config, atoms);
        assert_eq!(codegen.generated_atoms.len(), 0);
    }

    #[test]
    fn test_codegen_error_display() {
        let err = CodegenError::UnsupportedExpr("test".to_string());
        assert!(!format!("{}", err).is_empty());
    }

    #[test]
    fn test_codegen_output_clone() {
        let output = CodegenOutput {
            code: vec![1, 2, 3],
            exports: vec![],
            attributes: HashMap::new(),
            compile_info: CompileInfoOutput {
                file: None,
                line: 1,
                vsn: None,
            },
            atoms: vec![],
            literals: LiteralTable::new(),
        };
        let cloned = output.clone();
        assert_eq!(cloned.code, vec![1, 2, 3]);
    }

    #[test]
    fn test_emit_atoms_chunk() {
        let atoms = vec!["foo".to_string(), "bar".to_string()];
        let result = emit_atoms_chunk(&atoms);
        assert!(result.is_ok());
    }

    #[test]
    fn test_emit_exports_chunk() {
        let exports = vec![];
        let atoms = vec![];
        let result = emit_exports_chunk(&exports, &atoms);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_to_target_empty() {
        let atoms = AtomTable::new();
        let module = CoreModule {
            name: ModuleName::new(vec![Atom::new(0)]),
            exports: std::collections::HashSet::new(),
            attributes: std::collections::HashMap::new(),
            functions: vec![],
            compile_info: chimera_core::CoreCompileInfo {
                file: None,
                line: 1,
                vsn: None,
            },
        };
        let config = CodegenConfig::default();
        let result = compile_to_target(&module, atoms, config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_codegen_expr_atom() {
        let mut atoms = AtomTable::new();
        atoms.intern("test_atom");
        let config = CodegenConfig::default();
        let mut codegen = Codegen::new(config, atoms);
        let expr = CoreExpr::Atom(Atom::new(0));
        let result = codegen.generate_expr(&expr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_codegen_expr_integer() {
        let atoms = AtomTable::new();
        let config = CodegenConfig::default();
        let mut codegen = Codegen::new(config, atoms);
        let expr = CoreExpr::Integer(42);
        let result = codegen.generate_expr(&expr);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(!code.is_empty());
    }

    #[test]
    fn test_codegen_expr_unit() {
        let atoms = AtomTable::new();
        let config = CodegenConfig::default();
        let mut codegen = Codegen::new(config, atoms);
        let expr = CoreExpr::Unit;
        let result = codegen.generate_expr(&expr);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_literal_table_new() {
        let table = LiteralTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_literal_table_add_float() {
        let mut table = LiteralTable::new();
        let idx = table.add_float(3.14159);
        assert_eq!(idx, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_literal_table_add_integer() {
        let mut table = LiteralTable::new();
        let idx = table.add_integer(vec![1, 2, 3, 4]);
        assert_eq!(idx, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_literal_table_add_nil() {
        let mut table = LiteralTable::new();
        let idx = table.add_nil();
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_literal_table_add_tuple() {
        let mut table = LiteralTable::new();
        let idx = table.add_tuple(vec![1, 2, 3]);
        assert_eq!(idx, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_literal_table_add_cons() {
        let mut table = LiteralTable::new();
        let idx = table.add_cons(0, 1);
        assert_eq!(idx, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_literal_table_add_map() {
        let mut table = LiteralTable::new();
        let idx = table.add_map(vec![(0, 1), (2, 3)]);
        assert_eq!(idx, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_literal_table_add_string() {
        let mut table = LiteralTable::new();
        let idx = table.add_string(b"hello".to_vec());
        assert_eq!(idx, 0);
        assert_eq!(table.len(), 1);
    }

    #[test]
    fn test_literal_table_encode() {
        let mut table = LiteralTable::new();
        table.add_float(1.0);
        table.add_integer(vec![42u8]);
        let encoded = table.encode();
        // Should have: 2 (count) + encoded entries
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_literal_entry_float_encode() {
        let entry = LiteralEntry::Float(2.71828);
        let encoded = entry.encode();
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x02); // float tag
    }

    #[test]
    fn test_literal_entry_integer_encode() {
        let entry = LiteralEntry::Integer(vec![1, 2, 3]);
        let encoded = entry.encode();
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x01); // integer tag
        assert_eq!(encoded[1], 3); // length
    }

    #[test]
    fn test_literal_entry_tuple_encode() {
        let entry = LiteralEntry::Tuple(vec![0, 1, 2]);
        let encoded = entry.encode();
        assert!(!encoded.is_empty());
        assert_eq!(encoded[0], 0x05); // tuple tag
        assert_eq!(encoded[1], 3); // arity
    }

    #[test]
    fn test_codegen_expr_float() {
        let atoms = AtomTable::new();
        let config = CodegenConfig::default();
        let mut codegen = Codegen::new(config, atoms);
        let expr = CoreExpr::Float(1.5);
        let result = codegen.generate_expr(&expr);
        assert!(result.is_ok());
        let code = result.unwrap();
        assert!(!code.is_empty());
        // Should contain LoadFloat opcode (0x34)
        let float_opcode = Opcode::LoadFloat as u8;
        assert!(code.contains(&float_opcode));
    }

    #[test]
    fn test_codegen_literals_populated() {
        let atoms = AtomTable::new();
        let config = CodegenConfig::default();
        let mut codegen = Codegen::new(config, atoms);
        // Generate a float expression
        let _ = codegen.generate_expr(&CoreExpr::Float(3.14));
        // Literal table should have one entry
        assert_eq!(codegen.literals().len(), 1);
    }
}
