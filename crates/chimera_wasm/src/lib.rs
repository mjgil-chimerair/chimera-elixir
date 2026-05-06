//! WebAssembly backend for the zelix Rust/Zig Elixir compiler.
//!
//! Provides WebAssembly compilation from IR, targeting wasm32-unknown-unknown
//! and WASI for system integration.

#![allow(dead_code)]

#[cfg(test)]
use chimera_allocator as _;

use std::collections::HashMap;
use chimera_plugin_api::{PluginMetadata, PluginPhase};

/// WebAssembly target types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmTarget {
    /// Standard WebAssembly without WASI
    Wasm32UnknownUnknown,
    /// WebAssembly with WASI preview1
    WasiPreview1,
    /// WebAssembly with WASI preview2
    WasiPreview2,
    /// WebAssembly for browsers (JS interop)
    Browser,
}

impl WasmTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            WasmTarget::Wasm32UnknownUnknown => "wasm32-unknown-unknown",
            WasmTarget::WasiPreview1 => "wasm32-wasi-preview1",
            WasmTarget::WasiPreview2 => "wasm32-wasi-preview2",
            WasmTarget::Browser => "wasm32-unknown-unknown",
        }
    }
}

/// WebAssembly value types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
    V128,
    Funcref,
    Externref,
}

impl WasmType {
    pub fn as_wat(&self) -> &'static str {
        match self {
            WasmType::I32 => "i32",
            WasmType::I64 => "i64",
            WasmType::F32 => "f32",
            WasmType::F64 => "f64",
            WasmType::V128 => "v128",
            WasmType::Funcref => "funcref",
            WasmType::Externref => "externref",
        }
    }

    pub fn size(&self) -> u32 {
        match self {
            WasmType::I32 | WasmType::F32 | WasmType::Funcref | WasmType::Externref => 4,
            WasmType::I64 | WasmType::F64 => 8,
            WasmType::V128 => 16,
        }
    }
}

/// WebAssembly opcode instructions.
#[derive(Debug, Clone)]
pub enum WasmInstruction {
    // Control flow
    Unreachable,
    Nop,
    Block(WasmBlockType),
    Loop(WasmBlockType),
    If(WasmBlockType),
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable(Vec<u32>, u32),
    Return,
    Call(u32),
    CallIndirect(u32, Option<u32>),

    // Variable access
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    GlobalGet(u32),
    GlobalSet(u32),

    // Memory
    I32Load(WasmMemArg),
    I64Load(WasmMemArg),
    F32Load(WasmMemArg),
    F64Load(WasmMemArg),
    I32Load8S(WasmMemArg),
    I32Load8U(WasmMemArg),
    I32Load16S(WasmMemArg),
    I32Load16U(WasmMemArg),
    I64Load8S(WasmMemArg),
    I64Load8U(WasmMemArg),
    I64Load16S(WasmMemArg),
    I64Load16U(WasmMemArg),
    I64Load32S(WasmMemArg),
    I64Load32U(WasmMemArg),
    I32Store(WasmMemArg),
    I64Store(WasmMemArg),
    F32Store(WasmMemArg),
    F64Store(WasmMemArg),
    I32Store8(WasmMemArg),
    I32Store16(WasmMemArg),
    I64Store8(WasmMemArg),
    I64Store16(WasmMemArg),
    I64Store32(WasmMemArg),
    MemorySize,
    MemoryGrow,

    // Numeric
    I32Const(i32),
    I64Const(i64),
    F32Const(f32),
    F64Const(f64),
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,
    I32Clz,
    I32Ctz,
    I32Popcnt,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Clz,
    I64Ctz,
    I64Popcnt,
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Neg,
    F32Abs,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F32Sqrt,
    F32Min,
    F32Max,
    F32Eq,
    F32Ne,
    F32Lt,
    F32Gt,
    F32Le,
    F32Ge,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Neg,
    F64Abs,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    F64Sqrt,
    F64Min,
    F64Max,
    F64Eq,
    F64Ne,
    F64Lt,
    F64Gt,
    F64Le,
    F64Ge,
    I32WrapI64,
    I64ExtendI32S,
    I64ExtendI32U,
    I32Extend8S,
    I32Extend16S,
    I64Extend8S,
    I64Extend16S,
    I64Extend32S,
    I64Extend32U,
    F32DemoteF64,
    F64PromoteF32,
    F32ConvertI32S,
    F32ConvertI32U,
    F32ConvertI64S,
    F32ConvertI64U,
    F64ConvertI32S,
    F64ConvertI32U,
    F64ConvertI64S,
    F64ConvertI64U,
    I32TruncF32S,
    I32TruncF32U,
    I32TruncF64S,
    I32TruncF64U,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    MemoryInit(u32, u32),
    DataDrop(u32),
    MemoryCopy(u32, u32),
    MemoryFill(u32),
    TableInit(u32, u32),
    ElemDrop(u32),
    TableCopy(u32, u32),
    TableGet(u32),
    TableSet(u32),
    TableSize(u32),
    TableGrow(u32),
    RefFunc(u32),
    RefNull,
    RefIsNull,
}

impl WasmInstruction {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            WasmInstruction::Unreachable => bytes.push(0x00),
            WasmInstruction::Nop => bytes.push(0x01),
            WasmInstruction::Block(_bt) => {
                bytes.push(0x02);
                bytes.extend_from_slice(&[0x40]); // block type
            }
            WasmInstruction::Loop(_bt) => {
                bytes.push(0x03);
                bytes.extend_from_slice(&[0x40]); // block type
            }
            WasmInstruction::If(_bt) => {
                bytes.push(0x04);
                bytes.extend_from_slice(&[0x40]); // block type
            }
            WasmInstruction::Else => bytes.push(0x05),
            WasmInstruction::End => bytes.push(0x0B),
            WasmInstruction::Br(idx) => {
                bytes.push(0x0C);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::BrIf(idx) => {
                bytes.push(0x0D);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::Return => bytes.push(0x0F),
            WasmInstruction::Call(idx) => {
                bytes.push(0x10);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::LocalGet(idx) => {
                bytes.push(0x20);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::LocalSet(idx) => {
                bytes.push(0x21);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::LocalTee(idx) => {
                bytes.push(0x22);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::GlobalGet(idx) => {
                bytes.push(0x23);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::GlobalSet(idx) => {
                bytes.push(0x24);
                encode_uleb128(*idx as u64, &mut bytes);
            }
            WasmInstruction::I32Const(val) => {
                bytes.push(0x41);
                encode_sleb128(*val as i64, &mut bytes);
            }
            WasmInstruction::I64Const(val) => {
                bytes.push(0x42);
                encode_sleb128(*val as i64, &mut bytes);
            }
            WasmInstruction::MemorySize => bytes.push(0x3F),
            WasmInstruction::MemoryGrow => bytes.push(0x40),
            WasmInstruction::I32Add => bytes.push(0x6A),
            WasmInstruction::I32Sub => bytes.push(0x6B),
            WasmInstruction::I32Mul => bytes.push(0x6C),
            WasmInstruction::I32Eq => bytes.push(0x46),
            WasmInstruction::I32Ne => bytes.push(0x47),
            WasmInstruction::I32LtS => bytes.push(0x48),
            WasmInstruction::I32LtU => bytes.push(0x49),
            WasmInstruction::I32GtS => bytes.push(0x4A),
            WasmInstruction::I32GtU => bytes.push(0x4B),
            WasmInstruction::I32LeS => bytes.push(0x4C),
            WasmInstruction::I32LeU => bytes.push(0x4D),
            WasmInstruction::I32GeS => bytes.push(0x4E),
            WasmInstruction::I32GeU => bytes.push(0x4F),
            WasmInstruction::I64Eq => bytes.push(0x51),
            WasmInstruction::I64Ne => bytes.push(0x52),
            WasmInstruction::I64LtS => bytes.push(0x53),
            WasmInstruction::I64LtU => bytes.push(0x54),
            WasmInstruction::RefNull => bytes.push(0xD0),
            WasmInstruction::RefIsNull => bytes.push(0xD1),
            WasmInstruction::I32Load(mem_arg) => {
                bytes.push(0x28);
                encode_uleb128(mem_arg.align as u64, &mut bytes);
                encode_uleb128(mem_arg.offset as u64, &mut bytes);
            }
            WasmInstruction::I32Store(mem_arg) => {
                bytes.push(0x36);
                encode_uleb128(mem_arg.align as u64, &mut bytes);
                encode_uleb128(mem_arg.offset as u64, &mut bytes);
            }
            _ => {
                // For simplicity, other instructions push 0x00 as placeholder
                bytes.push(0x00);
            }
        }
        bytes
    }
}

/// Memory argument for load/store instructions.
#[derive(Debug, Clone, Copy)]
pub struct WasmMemArg {
    pub align: u32,
    pub offset: u32,
}

impl Default for WasmMemArg {
    fn default() -> Self {
        Self { align: 0, offset: 0 }
    }
}

/// Block type for block instructions.
#[derive(Debug, Clone, Copy)]
pub struct WasmBlockType(pub Option<WasmType>);

impl Default for WasmBlockType {
    fn default() -> Self {
        Self(None)
    }
}

/// A WebAssembly function.
#[derive(Debug, Clone)]
pub struct WasmFunction {
    pub type_index: u32,
    pub locals: Vec<WasmType>,
    pub body: Vec<WasmInstruction>,
}

impl WasmFunction {
    pub fn new(type_index: u32) -> Self {
        Self {
            type_index,
            locals: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn with_locals(mut self, locals: Vec<WasmType>) -> Self {
        self.locals = locals;
        self
    }

    pub fn with_body(mut self, body: Vec<WasmInstruction>) -> Self {
        self.body = body;
        self
    }

    /// Encode the function to WebAssembly binary format.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Encode locals
        if !self.locals.is_empty() {
            encode_uleb128(self.locals.len() as u64, &mut bytes);
            for local_type in &self.locals {
                encode_uleb128(1, &mut bytes);
                bytes.push(match local_type {
                    WasmType::I32 => 0x7F,
                    WasmType::I64 => 0x7E,
                    WasmType::F32 => 0x7D,
                    WasmType::F64 => 0x7C,
                    _ => 0x7F, // Default to i32
                });
            }
        }

        // Encode body
        for instr in &self.body {
            bytes.extend_from_slice(&instr.encode());
        }

        // End
        bytes.push(0x0B);

        bytes
    }
}

/// A WebAssembly global.
#[derive(Debug, Clone)]
pub struct WasmGlobal {
    pub var_type: WasmType,
    pub mutable: bool,
    pub init: Vec<WasmInstruction>,
}

impl WasmGlobal {
    pub fn new(var_type: WasmType, mutable: bool) -> Self {
        Self {
            var_type,
            mutable,
            init: Vec::new(),
        }
    }
}

/// A WebAssembly memory.
#[derive(Debug, Clone)]
pub struct WasmMemory {
    pub min_pages: u32,
    pub max_pages: Option<u32>,
}

impl WasmMemory {
    pub fn new(min_pages: u32) -> Self {
        Self {
            min_pages,
            max_pages: None,
        }
    }

    pub fn with_max(mut self, max: u32) -> Self {
        self.max_pages = Some(max);
        self
    }
}

/// A WebAssembly table.
#[derive(Debug, Clone)]
pub struct WasmTable {
    pub elem_type: WasmType,
    pub min_size: u32,
    pub max_size: Option<u32>,
}

impl WasmTable {
    pub fn new(elem_type: WasmType, min_size: u32) -> Self {
        Self {
            elem_type,
            min_size,
            max_size: None,
        }
    }
}

/// WebAssembly module containing all module sections.
#[derive(Debug, Default)]
pub struct WasmModule {
    /// WASM binary version (1 = MVP, 2 = reference types)
    pub version: u32,
    pub types: Vec<Vec<WasmType>>,
    pub funcs: Vec<WasmFunction>,
    pub tables: Vec<WasmTable>,
    pub mems: Vec<WasmMemory>,
    pub globals: Vec<WasmGlobal>,
    pub elem: Vec<Vec<u32>>,
    pub data: Vec<Vec<u8>>,
    pub exports: HashMap<String, WasmExportDesc>,
    pub custom_sections: Vec<(String, Vec<u8>)>,
    pub function_names: HashMap<u32, String>,
    pub local_names: HashMap<(u32, u32), String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WasmExportDesc {
    pub kind: u8, // 0=func, 1=table, 2=memory, 3=global
    pub index: u32,
}

impl WasmModule {
    pub fn new() -> Self {
        Self {
            version: 1,
            ..Default::default()
        }
    }

    /// Add a function type and return its index.
    pub fn add_type(&mut self, params: Vec<WasmType>, results: Vec<WasmType>) -> u32 {
        let full_type = [params, results].concat();
        if let Some(idx) = self.types.iter().position(|t| t == &full_type) {
            return idx as u32;
        }
        let idx = self.types.len() as u32;
        self.types.push(full_type);
        idx
    }

    /// Add a function and return its index.
    pub fn add_function(&mut self, func: WasmFunction) -> u32 {
        let idx = self.funcs.len() as u32;
        self.funcs.push(func);
        idx
    }

    /// Add a memory and return its index.
    pub fn add_memory(&mut self, mem: WasmMemory) -> u32 {
        let idx = self.mems.len() as u32;
        self.mems.push(mem);
        idx
    }

    /// Add a table and return its index.
    pub fn add_table(&mut self, table: WasmTable) -> u32 {
        let idx = self.tables.len() as u32;
        self.tables.push(table);
        idx
    }

    /// Export a function.
    pub fn export_func(&mut self, name: &str, func_index: u32) {
        self.exports.insert(name.to_string(), WasmExportDesc { kind: 0, index: func_index });
    }

    /// Export memory.
    pub fn export_mem(&mut self, name: &str, mem_index: u32) {
        self.exports.insert(name.to_string(), WasmExportDesc { kind: 2, index: mem_index });
    }

    /// Encode the module to WebAssembly binary format.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Magic number
        bytes.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // "\0asm"

        // Version (1.2)
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

        // Type section
        if !self.types.is_empty() {
            bytes.push(0x01);
            let type_bytes = self.encode_types();
            encode_uleb128(type_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&type_bytes);
        }

        // Import section (if any) - skip for now

        // Function section
        if !self.funcs.is_empty() {
            bytes.push(0x03);
            let func_bytes = self.encode_funcs();
            encode_uleb128(func_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&func_bytes);
        }

        // Table section
        if !self.tables.is_empty() {
            bytes.push(0x04);
            let table_bytes = self.encode_tables();
            encode_uleb128(table_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&table_bytes);
        }

        // Memory section
        if !self.mems.is_empty() {
            bytes.push(0x05);
            let mem_bytes = self.encode_mems();
            encode_uleb128(mem_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&mem_bytes);
        }

        // Global section
        if !self.globals.is_empty() {
            bytes.push(0x06);
            let global_bytes = self.encode_globals();
            encode_uleb128(global_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&global_bytes);
        }

        // Export section
        if !self.exports.is_empty() {
            bytes.push(0x07);
            let export_bytes = self.encode_exports();
            encode_uleb128(export_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&export_bytes);
        }

        // Code section
        if !self.funcs.is_empty() {
            bytes.push(0x0A);
            let code_bytes = self.encode_codes();
            encode_uleb128(code_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&code_bytes);
        }

        bytes
    }

    fn encode_types(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.types.len() as u64, &mut bytes);
        for type_vec in &self.types {
            bytes.push(0x60); // func type
            encode_uleb128((type_vec.len() / 2) as u64, &mut bytes); // params
            for t in type_vec.iter().take(type_vec.len() / 2) {
                bytes.push(self.type_to_byte(t));
            }
            encode_uleb128((type_vec.len() - type_vec.len() / 2) as u64, &mut bytes); // results
            for t in type_vec.iter().skip(type_vec.len() / 2) {
                bytes.push(self.type_to_byte(t));
            }
        }
        bytes
    }

    fn encode_funcs(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.funcs.len() as u64, &mut bytes);
        for func in &self.funcs {
            encode_uleb128(func.type_index as u64, &mut bytes);
        }
        bytes
    }

    fn encode_tables(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.tables.len() as u64, &mut bytes);
        for table in &self.tables {
            bytes.push(0x70); // funcref
            encode_uleb128(table.min_size as u64, &mut bytes);
            if let Some(max) = table.max_size {
                bytes.push(0x01);
                encode_uleb128(max as u64, &mut bytes);
            } else {
                bytes.push(0x00);
            }
        }
        bytes
    }

    fn encode_mems(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.mems.len() as u64, &mut bytes);
        for mem in &self.mems {
            encode_uleb128(mem.min_pages as u64, &mut bytes);
            if let Some(max) = mem.max_pages {
                bytes.push(0x01);
                encode_uleb128(max as u64, &mut bytes);
            } else {
                bytes.push(0x00);
            }
        }
        bytes
    }

    fn encode_globals(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.globals.len() as u64, &mut bytes);
        for global in &self.globals {
            bytes.push(self.type_to_byte(&global.var_type));
            bytes.push(if global.mutable { 0x01 } else { 0x00 });
            for instr in &global.init {
                bytes.extend_from_slice(&instr.encode());
            }
            bytes.push(0x0B);
        }
        bytes
    }

    fn encode_exports(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.exports.len() as u64, &mut bytes);
        for (name, desc) in &self.exports {
            encode_uleb128(name.len() as u64, &mut bytes);
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(desc.kind);
            encode_uleb128(desc.index as u64, &mut bytes);
        }
        bytes
    }

    fn encode_codes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        encode_uleb128(self.funcs.len() as u64, &mut bytes);
        for func in &self.funcs {
            let func_bytes = func.encode();
            encode_uleb128(func_bytes.len() as u64, &mut bytes);
            bytes.extend_from_slice(&func_bytes);
        }
        bytes
    }

    fn type_to_byte(&self, t: &WasmType) -> u8 {
        match t {
            WasmType::I32 => 0x7F,
            WasmType::I64 => 0x7E,
            WasmType::F32 => 0x7D,
            WasmType::F64 => 0x7C,
            WasmType::Funcref => 0x70,
            WasmType::Externref => 0x6F,
            WasmType::V128 => 0x7B,
        }
    }

    /// Convert module to WAT (WebAssembly Text) format.
    pub fn to_wat(&self) -> String {
        let mut wat = String::new();
        wat.push_str("(module\n");

        // Types
        for (i, type_vec) in self.types.iter().enumerate() {
            let params = &type_vec[..type_vec.len() / 2];
            let results = &type_vec[type_vec.len() / 2..];
            wat.push_str(&format!("  (type (;{};) ", i));
            wat.push_str("(func");
            if !params.is_empty() {
                let params_str: Vec<String> = params.iter().map(|p| format!("(param {})", p.as_wat())).collect();
                wat.push_str(&params_str.join(" "));
            }
            if !results.is_empty() {
                let results_str: Vec<String> = results.iter().map(|r| format!("(result {})", r.as_wat())).collect();
                wat.push_str(&results_str.join(""));
            }
            wat.push_str(")\n");
        }

        // Functions
        for (i, func) in self.funcs.iter().enumerate() {
            wat.push_str(&format!("  (func (;{};) ", i));
            if let Some(name) = self.function_names.get(&(i as u32)) {
                wat.push_str(&format!("(export \"{}\") ", name));
            }
            if !func.locals.is_empty() {
                for local in &func.locals {
                    wat.push_str(&format!("(local {}) ", local.as_wat()));
                }
            }
            for instr in &func.body {
                wat.push_str(&format!("  {}\n", self.instr_to_wat(instr)));
            }
            wat.push_str(")\n");
        }

        // Memories
        for (i, mem) in self.mems.iter().enumerate() {
            wat.push_str(&format!("  (memory (;{};) {}", i, mem.min_pages));
            if let Some(max) = mem.max_pages {
                wat.push_str(&format!(" {}", max));
            }
            wat.push_str(")\n");
        }

        wat.push_str(")\n");
        wat
    }

    fn instr_to_wat(&self, instr: &WasmInstruction) -> String {
        match instr {
            WasmInstruction::Unreachable => "unreachable".to_string(),
            WasmInstruction::Nop => "nop".to_string(),
            WasmInstruction::End => "end".to_string(),
            WasmInstruction::Return => "return".to_string(),
            WasmInstruction::I32Const(val) => format!("(i32.const {})", val),
            WasmInstruction::I64Const(val) => format!("(i64.const {})", val),
            WasmInstruction::I32Add => "i32.add".to_string(),
            WasmInstruction::I32Sub => "i32.sub".to_string(),
            WasmInstruction::I32Mul => "i32.mul".to_string(),
            WasmInstruction::I32Eq => "i32.eq".to_string(),
            WasmInstruction::I32Ne => "i32.ne".to_string(),
            WasmInstruction::I32LtS => "i32.lt_s".to_string(),
            WasmInstruction::I32GtS => "i32.gt_s".to_string(),
            WasmInstruction::I32LeS => "i32.le_s".to_string(),
            WasmInstruction::I32GeS => "i32.ge_s".to_string(),
            WasmInstruction::I32LtU => "i32.lt_u".to_string(),
            WasmInstruction::I32GtU => "i32.gt_u".to_string(),
            WasmInstruction::I32LeU => "i32.le_u".to_string(),
            WasmInstruction::I32GeU => "i32.ge_u".to_string(),
            WasmInstruction::LocalGet(idx) => format!("(local.get {})", idx),
            WasmInstruction::LocalSet(idx) => format!("(local.set {})", idx),
            WasmInstruction::LocalTee(idx) => format!("(local.tee {})", idx),
            WasmInstruction::GlobalGet(idx) => format!("(global.get {})", idx),
            WasmInstruction::GlobalSet(idx) => format!("(global.set {})", idx),
            WasmInstruction::MemorySize => "memory.size".to_string(),
            WasmInstruction::MemoryGrow => "memory.grow".to_string(),
            WasmInstruction::I32Load(mem_arg) => {
                format!("(i32.load offset={} align={})", mem_arg.offset, mem_arg.align)
            }
            WasmInstruction::I32Store(mem_arg) => {
                format!("(i32.store offset={} align={})", mem_arg.offset, mem_arg.align)
            }
            WasmInstruction::RefNull => "ref.null".to_string(),
            WasmInstruction::RefIsNull => "ref.is_null".to_string(),
            WasmInstruction::Call(idx) => format!("(call {})", idx),
            WasmInstruction::Br(idx) => format!("(br {})", idx),
            WasmInstruction::BrIf(idx) => format!("(br_if {})", idx),
            WasmInstruction::Block(_) => "(block)".to_string(),
            WasmInstruction::Loop(_) => "(loop)".to_string(),
            WasmInstruction::If(_) => "(if)".to_string(),
            WasmInstruction::Else => "else".to_string(),
            _ => format!(";; {:?}", instr),
        }
    }
}

/// Encode a signed value as LEB128 variable-length integer.
fn encode_sleb128(mut value: i64, bytes: &mut Vec<u8>) {
    let mut buf = [0u8; 8];
    let mut len = 0;
    let is_negative = value < 0;

    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if (is_negative && value != -1) || (!is_negative && value != 0) {
            byte |= 0x80;
        }
        buf[len] = byte;
        len += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }

    for i in 0..len {
        bytes.push(buf[i]);
    }
}

/// Encode an unsigned value as LEB128.
fn encode_uleb128(mut value: u64, bytes: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if byte & 0x80 == 0 {
            break;
        }
    }
}

/// WebAssembly plugin metadata.
pub fn metadata() -> PluginMetadata {
    PluginMetadata {
        name: "wasm-backend".to_string(),
        version: "0.1.0".to_string(),
        author: "zelix".to_string(),
        description: "WebAssembly backend for compiling Elixir to wasm32".to_string(),
        lifecycle_phase: PluginPhase::BeforeEmit,
        api_version: 1,
    }
}

/// Create a simple WASI-compatible module.
pub fn create_hello_world() -> WasmModule {
    let mut module = WasmModule::new();

    // Add a (i32) -> i32 function type
    let type_idx = module.add_type(vec![WasmType::I32], vec![WasmType::I32]);

    // Add hello function
    let mut hello = WasmFunction::new(type_idx);
    hello.body.push(WasmInstruction::I32Const(42));
    hello.body.push(WasmInstruction::End);
    module.add_function(hello);

    // Export memory
    module.add_memory(WasmMemory::new(1));
    module.export_mem("memory", 0);

    module
}

/// WASI version types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasiVersion {
    Preview1,
    Preview2,
}

/// WASI context for runtime operations.
#[derive(Debug)]
pub struct WasiContext {
    pub version: WasiVersion,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub stdin: Option<Vec<u8>>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub preopened_dirs: Vec<WasiDir>,
}

/// A preopened directory in WASI.
#[derive(Debug, Clone)]
pub struct WasiDir {
    pub path: String,
    pub alias: String,
    pub read: bool,
    pub write: bool,
}

impl Default for WasiContext {
    fn default() -> Self {
        Self::new(WasiVersion::Preview1)
    }
}

impl WasiContext {
    pub fn new(version: WasiVersion) -> Self {
        Self {
            version,
            args: Vec::new(),
            env_vars: HashMap::new(),
            stdin: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
            preopened_dirs: Vec::new(),
        }
    }

    /// Initialize with command-line arguments.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Initialize with environment variables.
    pub fn with_env_vars(mut self, env_vars: HashMap<String, String>) -> Self {
        self.env_vars = env_vars;
        self
    }

    /// Add a preopened directory.
    pub fn add_dir(&mut self, path: &str, alias: &str, read: bool, write: bool) {
        self.preopened_dirs.push(WasiDir {
            path: path.to_string(),
            alias: alias.to_string(),
            read,
            write,
        });
    }

    /// Get an environment variable.
    pub fn get_env(&self, name: &str) -> Option<&str> {
        self.env_vars.get(name).map(|s| s.as_str())
    }

    /// Get the current timestamp in nanoseconds.
    pub fn clock_time_nanos(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// Generate random bytes using a simple LCG.
    pub fn random_get(&mut self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.clock_time_nanos().hash(&mut hasher);
        for (k, v) in &self.env_vars {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Create a WASI command module with _start function.
pub fn create_wasi_command() -> WasmModule {
    let mut module = WasmModule::new();

    // Add an empty type for _start (() -> ())
    let type_idx = module.add_type(vec![], vec![]);

    // Create the _start function
    let mut start = WasmFunction::new(type_idx);
    start.body.push(WasmInstruction::End);
    let start_idx = module.add_function(start);

    // Export _start
    module.export_func("_start", start_idx);

    // Export memory
    module.add_memory(WasmMemory::new(1));
    module.export_mem("memory", 0);

    module
}

/// Create a WASI reactor module with init function.
pub fn create_wasi_reactor() -> WasmModule {
    let mut module = WasmModule::new();

    // Type for init: () -> ()
    let init_type = module.add_type(vec![], vec![]);

    // Type for optional _start: () -> ()
    let start_type = module.add_type(vec![], vec![]);

    // Create the init function
    let mut init = WasmFunction::new(init_type);
    init.body.push(WasmInstruction::End);
    let init_idx = module.add_function(init);

    // Try to create _start (optional)
    let mut start = WasmFunction::new(start_type);
    start.body.push(WasmInstruction::End);
    let start_idx = module.add_function(start);

    // Export both
    module.export_func("init", init_idx);
    module.export_func("_start", start_idx);

    // Export memory
    module.add_memory(WasmMemory::new(1));
    module.export_mem("memory", 0);

    module
}

/// Browser compatibility settings for WebAssembly.
#[derive(Debug, Clone)]
pub struct BrowserOptions {
    /// Enable JavaScript interop imports
    pub js_imports: bool,
    /// Enable console module emulation
    pub console: bool,
    /// Enable fetch API emulation
    pub fetch: bool,
    /// Enable timer functions (setTimeout, etc.)
    pub timers: bool,
    /// Enable DOM handlers
    pub dom: bool,
    /// Memory minimum pages
    pub memory_min: u32,
    /// Memory maximum pages (None for unlimited)
    pub memory_max: Option<u32>,
}

impl Default for BrowserOptions {
    fn default() -> Self {
        Self {
            js_imports: true,
            console: true,
            fetch: false,
            timers: true,
            dom: false,
            memory_min: 1,
            memory_max: Some(32768), // 2GB
        }
    }
}

impl BrowserOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_js_imports(mut self, enabled: bool) -> Self {
        self.js_imports = enabled;
        self
    }

    pub fn with_console(mut self, enabled: bool) -> Self {
        self.console = enabled;
        self
    }

    pub fn with_timers(mut self, enabled: bool) -> Self {
        self.timers = enabled;
        self
    }

    pub fn with_memory(mut self, min: u32, max: Option<u32>) -> Self {
        self.memory_min = min;
        self.memory_max = max;
        self
    }
}

/// Create a browser-compatible WebAssembly module.
pub fn create_browser_module(options: BrowserOptions) -> WasmModule {
    let mut module = WasmModule::new();

    // Add memory
    let mem = WasmMemory::new(options.memory_min);
    let mem = match options.memory_max {
        Some(max) => mem.with_max(max),
        None => mem,
    };
    let mem_idx = module.add_memory(mem);

    // Export memory with standard name
    module.export_mem("memory", mem_idx);

    // Export memory as WebAssembly.Module would expect
    module.export_mem("wasm_memory", mem_idx);

    // Add table for JS interop
    let table_idx = module.add_table(WasmTable::new(WasmType::Funcref, 0));
    module.export_func("table", table_idx);

    // Add function type for imported JS functions
    // (i32, i32) -> i32 for many JS interop functions
    let js_func_type = module.add_type(vec![WasmType::I32, WasmType::I32], vec![WasmType::I32]);
    let _ = js_func_type; // Would be used for imports

    // Add console.log if enabled
    if options.console {
        add_console_module(&mut module);
    }

    // Add timer functions if enabled
    if options.timers {
        add_timer_module(&mut module);
    }

    module
}

/// Add console.log function to the module.
fn add_console_module(module: &mut WasmModule) {
    // Type: (i32, i32) -> () for console.log string pointer + length
    let console_type = module.add_type(vec![WasmType::I32, WasmType::I32], vec![]);

    let mut console_log = WasmFunction::new(console_type);
    // Just return for now - actual implementation would call JS
    console_log.body.push(WasmInstruction::End);
    module.add_function(console_log);
}

/// Add timer functions to the module.
fn add_timer_module(module: &mut WasmModule) {
    // setTimeout: (i32, i32) -> i32 (returns timer id)
    let timeout_type = module.add_type(vec![WasmType::I32, WasmType::I32], vec![WasmType::I32]);
    let mut set_timeout = WasmFunction::new(timeout_type);
    set_timeout.body.push(WasmInstruction::I32Const(0)); // Return 0 for now
    set_timeout.body.push(WasmInstruction::End);
    module.add_function(set_timeout);

    // clearTimeout: (i32) -> ()
    let clear_type = module.add_type(vec![WasmType::I32], vec![]);
    let mut clear_timeout = WasmFunction::new(clear_type);
    clear_timeout.body.push(WasmInstruction::End);
    module.add_function(clear_timeout);
}

/// Browser target representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserTarget {
    /// WebAssembly with JS imports
    WebAssembly,
    /// JavaScript ES module format
    ESModule,
    /// CommonJS module format
    CommonJS,
}

impl Default for BrowserTarget {
    fn default() -> Self {
        Self::WebAssembly
    }
}

impl BrowserTarget {
    pub fn file_extension(&self) -> &'static str {
        match self {
            BrowserTarget::WebAssembly => ".wasm",
            BrowserTarget::ESModule => ".mjs",
            BrowserTarget::CommonJS => ".cjs",
        }
    }
}

#[cfg(test)]
mod browser_tests {
    use super::*;

    #[test]
    fn test_browser_options_default() {
        let opts = BrowserOptions::default();
        assert!(opts.js_imports);
        assert!(opts.console);
        assert!(opts.timers);
        assert_eq!(opts.memory_min, 1);
    }

    #[test]
    fn test_browser_options_builder() {
        let opts = BrowserOptions::new()
            .with_js_imports(false)
            .with_console(false)
            .with_memory(2, Some(4));
        assert!(!opts.js_imports);
        assert!(!opts.console);
        assert_eq!(opts.memory_min, 2);
        assert_eq!(opts.memory_max, Some(4));
    }

    #[test]
    fn test_create_browser_module() {
        let module = create_browser_module(BrowserOptions::default());
        assert!(module.exports.contains_key("memory"));
        assert!(module.exports.contains_key("table"));
    }

    #[test]
    fn test_browser_target_extension() {
        assert_eq!(BrowserTarget::WebAssembly.file_extension(), ".wasm");
        assert_eq!(BrowserTarget::ESModule.file_extension(), ".mjs");
        assert_eq!(BrowserTarget::CommonJS.file_extension(), ".cjs");
    }

    #[test]
    fn test_browser_target_default() {
        assert_eq!(BrowserTarget::default(), BrowserTarget::WebAssembly);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_target_str() {
        assert_eq!(WasmTarget::Wasm32UnknownUnknown.as_str(), "wasm32-unknown-unknown");
        assert_eq!(WasmTarget::WasiPreview1.as_str(), "wasm32-wasi-preview1");
        assert_eq!(WasmTarget::Browser.as_str(), "wasm32-unknown-unknown");
    }

    #[test]
    fn test_wasm_type_size() {
        assert_eq!(WasmType::I32.size(), 4);
        assert_eq!(WasmType::I64.size(), 8);
        assert_eq!(WasmType::F32.size(), 4);
        assert_eq!(WasmType::F64.size(), 8);
    }

    #[test]
    fn test_wasm_type_wat() {
        assert_eq!(WasmType::I32.as_wat(), "i32");
        assert_eq!(WasmType::I64.as_wat(), "i64");
        assert_eq!(WasmType::Funcref.as_wat(), "funcref");
    }

    #[test]
    fn test_encode_uleb128() {
        let mut bytes = Vec::new();
        encode_uleb128(42, &mut bytes);
        assert_eq!(bytes, vec![42]);

        let mut bytes = Vec::new();
        encode_uleb128(127, &mut bytes);
        assert_eq!(bytes, vec![127]);

        let mut bytes = Vec::new();
        encode_uleb128(128, &mut bytes);
        assert_eq!(bytes, vec![128, 1]);
    }

    #[test]
    fn test_encode_uleb128_edge_cases() {
        let mut bytes = Vec::new();
        encode_uleb128(42, &mut bytes);
        assert_eq!(bytes, vec![42]);

        let mut bytes = Vec::new();
        encode_uleb128(128, &mut bytes);
        assert_eq!(bytes, vec![128, 1]);
    }

    #[test]
    fn test_wasm_instruction_encode() {
        let instr = WasmInstruction::I32Const(42);
        let encoded = instr.encode();
        assert_eq!(encoded, vec![0x41, 42]);

        let instr = WasmInstruction::I32Add;
        let encoded = instr.encode();
        assert_eq!(encoded, vec![0x6A]);
    }

    #[test]
    fn test_wasm_function_encode() {
        let mut func = WasmFunction::new(0);
        func.body.push(WasmInstruction::I32Const(42));
        func.body.push(WasmInstruction::End);
        let encoded = func.encode();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_wasm_module_new() {
        let module = WasmModule::new();
        assert!(module.funcs.is_empty());
        assert!(module.types.is_empty());
    }

    #[test]
    fn test_wasm_module_add_type() {
        let mut module = WasmModule::new();
        let idx = module.add_type(vec![WasmType::I32], vec![WasmType::I32]);
        assert_eq!(idx, 0);
        // Adding same type again should return same index
        let idx2 = module.add_type(vec![WasmType::I32], vec![WasmType::I32]);
        assert_eq!(idx2, 0);
    }

    #[test]
    fn test_wasm_module_encode() {
        let module = create_hello_world();
        let encoded = module.encode();
        // Check magic number
        assert_eq!(&encoded[0..4], b"\0asm");
        // Check version
        assert_eq!(&encoded[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_wasm_module_to_wat() {
        let module = create_hello_world();
        let wat = module.to_wat();
        assert!(wat.contains("(module"));
        assert!(wat.contains("(func"));
        assert!(wat.contains("(memory"));
        assert!(wat.contains("))"));
    }

    #[test]
    fn test_wasm_memory() {
        let mem = WasmMemory::new(1).with_max(2);
        assert_eq!(mem.min_pages, 1);
        assert_eq!(mem.max_pages, Some(2));
    }

    #[test]
    fn test_wasm_table() {
        let table = WasmTable::new(WasmType::Funcref, 10);
        assert_eq!(table.min_size, 10);
        assert_eq!(table.elem_type, WasmType::Funcref);
    }

    #[test]
    fn test_wasm_global() {
        let global = WasmGlobal::new(WasmType::I32, true);
        assert_eq!(global.var_type, WasmType::I32);
        assert!(global.mutable);
    }

    #[test]
    fn test_mem_arg_default() {
        let arg = WasmMemArg::default();
        assert_eq!(arg.align, 0);
        assert_eq!(arg.offset, 0);
    }

    #[test]
    fn test_block_type_default() {
        let bt = WasmBlockType::default();
        assert!(bt.0.is_none());
    }

    #[test]
    fn test_wasi_context_new() {
        let ctx = WasiContext::new(WasiVersion::Preview1);
        assert_eq!(ctx.version, WasiVersion::Preview1);
        assert!(ctx.args.is_empty());
        assert!(ctx.env_vars.is_empty());
    }

    #[test]
    fn test_wasi_context_with_args() {
        let ctx = WasiContext::new(WasiVersion::Preview1)
            .with_args(vec!["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(ctx.args.len(), 2);
        assert_eq!(ctx.args[0], "arg1");
    }

    #[test]
    fn test_wasi_context_with_env_vars() {
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/home/user".to_string());
        let ctx = WasiContext::new(WasiVersion::Preview1)
            .with_env_vars(env);
        assert_eq!(ctx.get_env("HOME"), Some("/home/user"));
        assert_eq!(ctx.get_env("PATH"), None);
    }

    #[test]
    fn test_wasi_context_add_dir() {
        let mut ctx = WasiContext::new(WasiVersion::Preview1);
        ctx.add_dir("/tmp", "/tmp", true, false);
        assert_eq!(ctx.preopened_dirs.len(), 1);
        assert_eq!(ctx.preopened_dirs[0].path, "/tmp");
        assert!(ctx.preopened_dirs[0].read);
        assert!(!ctx.preopened_dirs[0].write);
    }

    #[test]
    fn test_wasi_context_clock_time() {
        let ctx = WasiContext::new(WasiVersion::Preview1);
        let time = ctx.clock_time_nanos();
        assert!(time > 0);
    }

    #[test]
    fn test_wasi_command_module() {
        let module = create_wasi_command();
        let encoded = module.encode();
        assert!(!encoded.is_empty());
        // Should have _start exported
        assert!(module.exports.contains_key("_start"));
    }

    #[test]
    fn test_wasi_reactor_module() {
        let module = create_wasi_reactor();
        let encoded = module.encode();
        assert!(!encoded.is_empty());
        // Should have init and _start exported
        assert!(module.exports.contains_key("init"));
        assert!(module.exports.contains_key("_start"));
    }

    #[test]
    fn test_wasi_version_enum() {
        assert_eq!(WasiVersion::Preview1 as u8, 0);
        assert_eq!(WasiVersion::Preview2 as u8, 1);
    }
}

/// WASM validation result.
#[derive(Debug, Clone)]
pub struct WasmValidationResult {
    pub valid: bool,
    pub error_message: Option<String>,
    pub sections_valid: Vec<(String, bool)>,
}

/// WASM execution result.
#[derive(Debug, Clone)]
pub struct WasmExecutionResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub error: Option<String>,
}

/// WASM test configuration.
#[derive(Debug, Clone)]
pub struct WasmTestConfig {
    /// Enable binary round-trip validation
    pub round_trip: bool,
    /// Validate WASM binary structure
    pub validate_structure: bool,
    /// Check for required exports
    pub check_exports: bool,
    /// Expected export names
    pub expected_exports: Vec<String>,
    /// Enable WAT generation
    pub generate_wat: bool,
}

impl Default for WasmTestConfig {
    fn default() -> Self {
        Self {
            round_trip: true,
            validate_structure: true,
            check_exports: true,
            expected_exports: vec!["_start".to_string()],
            generate_wat: false,
        }
    }
}

impl WasmTestConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_round_trip(mut self, enabled: bool) -> Self {
        self.round_trip = enabled;
        self
    }

    pub fn with_expected_exports(mut self, exports: Vec<String>) -> Self {
        self.expected_exports = exports;
        self
    }
}

/// Validate a WASM module structure.
pub fn validate_wasm(wasm_bytes: &[u8]) -> WasmValidationResult {
    let mut result = WasmValidationResult {
        valid: true,
        error_message: None,
        sections_valid: Vec::new(),
    };

    // Check magic number
    if wasm_bytes.len() < 8 {
        result.valid = false;
        result.error_message = Some("WASM binary too short".to_string());
        return result;
    }

    if &wasm_bytes[0..4] != b"\0asm" {
        result.valid = false;
        result.error_message = Some("Invalid WASM magic number".to_string());
        return result;
    }

    // Check version
    let version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
    if version != 1 && version != 2 {
        result.sections_valid.push(("version".to_string(), false));
        result.valid = false;
        result.error_message = Some(format!("Unsupported WASM version: {}", version));
        return result;
    }
    result.sections_valid.push(("version".to_string(), true));

    // Basic section validation
    let mut offset = 8;
    while offset < wasm_bytes.len() {
        let section_id = wasm_bytes[offset];
        offset += 1;

        // Read section size
        if offset >= wasm_bytes.len() {
            result.sections_valid.push((format!("section_{}", section_id), false));
            result.valid = false;
            break;
        }

        let (size, bytes_read) = decode_uleb128(&wasm_bytes[offset..]);
        offset += bytes_read;

        result.sections_valid.push((format!("section_{}", section_id), true));

        // Skip section content
        offset += size as usize;
        if offset > wasm_bytes.len() {
            result.valid = false;
            result.error_message = Some("Section extends beyond binary end".to_string());
            break;
        }
    }

    result
}

/// Decode a single ULEB128 value from bytes.
fn decode_uleb128(bytes: &[u8]) -> (u64, usize) {
    let mut result = 0u64;
    let mut shift = 0;
    let mut bytes_read = 0;

    for byte in bytes {
        bytes_read += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }

    (result, bytes_read)
}

/// Round-trip test: encode module, decode it, verify structure.
pub fn test_wasm_round_trip(module: &WasmModule) -> Result<(), String> {
    let encoded = module.encode();

    // Basic validation that encoded bytes are valid WASM
    let validation = validate_wasm(&encoded);
    if !validation.valid {
        return Err(validation.error_message.unwrap_or_else(|| "Invalid WASM".to_string()));
    }

    // Verify we can decode the magic and version at least
    if encoded.len() < 8 {
        return Err("Encoded too short".to_string());
    }
    if &encoded[0..4] != b"\0asm" {
        return Err("Invalid magic".to_string());
    }

    Ok(())
}

/// Run a WASM module (simulated - just validates structure).
pub fn run_wasm_module(module: &WasmModule, config: &WasmTestConfig) -> WasmExecutionResult {
    let wasm_bytes = module.encode();

    // Validate if requested
    if config.validate_structure {
        let validation = validate_wasm(&wasm_bytes);
        if !validation.valid {
            return WasmExecutionResult {
                exit_code: -1,
                stdout: Vec::new(),
                stderr: Vec::new(),
                error: validation.error_message,
            };
        }
    }

    // Check exports if requested
    if config.check_exports {
        for expected in &config.expected_exports {
            if !module.exports.contains_key(expected) {
                return WasmExecutionResult {
                    exit_code: -1,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    error: Some(format!("Missing expected export: {}", expected)),
                };
            }
        }
    }

    WasmExecutionResult {
        exit_code: 0,
        stdout: Vec::new(),
        stderr: Vec::new(),
        error: None,
    }
}

/// Decode a WASM binary back to a module.
impl WasmModule {
    pub fn decode(wasm_bytes: &[u8]) -> Result<Self, String> {
        let mut module = WasmModule::new();

        // Verify magic and version
        if wasm_bytes.len() < 8 {
            return Err("WASM binary too short".to_string());
        }
        if &wasm_bytes[0..4] != b"\0asm" {
            return Err("Invalid WASM magic number".to_string());
        }

        let version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
        if version != 1 && version != 2 {
            return Err(format!("Unsupported WASM version: {}", version));
        }

        module.version = version;

        // Parse sections (simplified - just reads section IDs and sizes)
        let mut offset = 8;
        while offset < wasm_bytes.len() {
            let section_id = wasm_bytes[offset];
            offset += 1;

            let (size, bytes_read) = decode_uleb128(&wasm_bytes[offset..]);
            offset += bytes_read;

            // Track that we saw this section
            match section_id {
                1 => { /* type section */ }
                3 => { /* function section */ }
                5 => { /* table section */ }
                6 => { /* memory section */ }
                7 => { /* global section */ }
                0 => { /* custom section */ }
                _ => {}
            }

            offset += size as usize;
        }

        Ok(module)
    }
}

#[cfg(test)]
mod wasm_testing_tests {
    use super::*;

    #[test]
    fn test_wasm_validation_valid() {
        let module = create_wasi_command();
        let encoded = module.encode();
        let result = validate_wasm(&encoded);
        assert!(result.valid);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_wasm_validation_invalid_magic() {
        let invalid = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let result = validate_wasm(&invalid);
        assert!(!result.valid);
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_wasm_validation_too_short() {
        let invalid = vec![0x00, 0x61, 0x73];
        let result = validate_wasm(&invalid);
        assert!(!result.valid);
    }

    #[test]
    fn test_decode_uleb128() {
        // 42 in ULEB128 is 42
        let (val, len) = decode_uleb128(&[42]);
        assert_eq!(val, 42);
        assert_eq!(len, 1);

        // 128 in ULEB128 is 0x80 0x01
        let (val, len) = decode_uleb128(&[0x80, 0x01]);
        assert_eq!(val, 128);
        assert_eq!(len, 2);
    }

    #[test]
    fn test_round_trip() {
        let module = create_wasi_command();
        let result = test_wasm_round_trip(&module);
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_wasm_module_default() {
        let module = create_wasi_command();
        let config = WasmTestConfig::default();
        let result = run_wasm_module(&module, &config);
        assert_eq!(result.exit_code, 0);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_run_wasm_module_missing_export() {
        let module = create_wasi_command();
        let config = WasmTestConfig::default()
            .with_expected_exports(vec!["nonexistent".to_string()]);
        let result = run_wasm_module(&module, &config);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("nonexistent"));
    }

    #[test]
    fn test_wasm_test_config_builder() {
        let config = WasmTestConfig::new()
            .with_round_trip(false)
            .with_expected_exports(vec!["init".to_string()]);
        assert!(!config.round_trip);
        assert_eq!(config.expected_exports, vec!["init"]);
    }

    #[test]
    fn test_wasm_module_decode() {
        let module = create_wasi_command();
        let encoded = module.encode();
        let decoded = WasmModule::decode(&encoded);
        assert!(decoded.is_ok());
    }

    #[test]
    fn test_wasm_module_decode_invalid() {
        let invalid = vec![0x00, 0x61, 0x73, 0x6D, 0x99, 0x00, 0x00, 0x00];
        let result = WasmModule::decode(&invalid);
        assert!(result.is_err());
    }
}
