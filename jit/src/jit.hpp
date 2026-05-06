// Chimera Elixir - JIT Interface
// LLVM-based JIT compilation interface

#pragma once

#include <memory>
#include <string>
#include <vector>
#include <optional>
#include <functional>
#include <cstdint>
#include <llvm/ExecutionEngine/ExecutionEngine.h>
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/Type.h>
#include <llvm/IR/Value.h>
#include <llvm/IR/IRBuilder.h>

namespace chimera {
namespace jit {

class JITError : public std::runtime_error {
public:
  explicit JITError(const std::string& msg) : std::runtime_error(msg) {}
};

using ModuleHandle = std::shared_ptr<llvm::Module>;
using FunctionHandle = std::shared_ptr<llvm::Function>;
using ValueHandle = std::shared_ptr<llvm::Value>;

struct Type {
  enum class Kind { Void, Integer, Float, Double, Pointer, Array, Function, Struct };
  Kind kind;
  unsigned bit_width;
  std::vector<Type> fields;
  unsigned element_count;

  static Type void_type() { return {Kind::Void, 0, {}, 0}; }
  static Type int_type(unsigned bits) { return {Kind::Integer, bits, {}, 0}; }
  static Type float_type() { return {Kind::Float, 32, {}, 0}; }
  static Type double_type() { return {Kind::Double, 64, {}, 0}; }
  static Type pointer() { return {Kind::Pointer, 64, {}, 0}; }
  static Type pointer_to(const Type& element) {
    Type t = element;
    t.kind = Kind::Pointer;
    return t;
  }
  static Type array_type(const Type& element, unsigned count) {
    return {Kind::Array, 0, {}, count};
  }
  static Type function_type(const Type& return_type, const std::vector<Type>& params) {
    return {Kind::Function, 0, params, 0};
  }
};

class JITEngine {
public:
  JITEngine();
  ~JITEngine();

  JITEngine(const JITEngine&) = delete;
  JITEngine& operator=(const JITEngine&) = delete;
  JITEngine(JITEngine&&) noexcept;
  JITEngine& operator=(JITEngine&&) noexcept;

  void set_optimization_level(unsigned level);
  void set_relocation_mode(llvm::Reloc::Model model);

  ModuleHandle create_module(const std::string& name);
  FunctionHandle add_function(const std::string& name, const Type& return_type,
                              const std::vector<Type>& param_types);

  void* get_function_address(const std::string& module_name, const std::string& func_name);

  void remove_module(ModuleHandle module);

  std::optional<std::string> compile_module(ModuleHandle module);

  void enable_verification() { verify_ = true; }
  void disable_verification() { verify_ = false; }

  static std::unique_ptr<JITEngine> create();

private:
  std::unique_ptr<llvm::LLVMContext> context_;
  std::unique_ptr<llvm::ExecutionEngine> execution_engine_;
  std::vector<ModuleHandle> modules_;
  bool verify_;
};

class IRBuilder {
public:
  explicit IRBuilder(JITEngine& engine, llvm::IRBuilder<>& builder);

  ValueHandle get_current_block() const;
  void set_insert_point(ValueHandle block);

  ValueHandle create_alloca(const std::string& name, const Type& type);

  ValueHandle create_ret(ValueHandle value);
  ValueHandle create_ret_void();

  ValueHandle create_add(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_sub(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_mul(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_sdiv(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_srem(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_udiv(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_urem(ValueHandle lhs, ValueHandle rhs);

  ValueHandle create_and(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_or(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_xor(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_not(ValueHandle value);
  ValueHandle create_shl(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_lshr(ValueHandle lhs, ValueHandle rhs);

  ValueHandle create_icmp_eq(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_ne(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_ugt(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_uge(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_ult(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_ule(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_sgt(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_sge(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_slt(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_icmp_sle(ValueHandle lhs, ValueHandle rhs);

  ValueHandle create_fadd(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fsub(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fmul(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fdiv(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_frem(ValueHandle lhs, ValueHandle rhs);

  ValueHandle create_fcmp_eq(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_ne(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_ugt(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_uge(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_ult(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_ule(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_olt(ValueHandle lhs, ValueHandle rhs);
  ValueHandle create_fcmp_ole(ValueHandle lhs, ValueHandle rhs);

  ValueHandle create_load(ValueHandle ptr, const Type& type);
  ValueHandle create_store(ValueHandle value, ValueHandle ptr);

  ValueHandle create_gep(ValueHandle ptr, const std::vector<ValueHandle>& indices);
  ValueHandle create_inbounds_gep(ValueHandle ptr, const std::vector<ValueHandle>& indices);

  ValueHandle create_trunc(ValueHandle value, const Type& dest_type);
  ValueHandle create_zext(ValueHandle value, const Type& dest_type);
  ValueHandle create_sext(ValueHandle value, const Type& dest_type);
  ValueHandle create_fptrunc(ValueHandle value, const Type& dest_type);
  ValueHandle create_fpext(ValueHandle value, const Type& dest_type);
  ValueHandle create_bitcast(ValueHandle value, const Type& dest_type);
  ValueHandle create_inttoptr(ValueHandle value, const Type& dest_type);
  ValueHandle create_ptrtoint(ValueHandle value, const Type& dest_type);

  ValueHandle create_br(ValueHandle dest_block);
  ValueHandle create_cond_br(ValueHandle cond, ValueHandle true_block, ValueHandle false_block);

  ValueHandle create_switch(ValueHandle value, ValueHandle default_block,
                            const std::vector<std::pair<ValueHandle, ValueHandle>>& cases);

  ValueHandle create_call(ValueHandle func, const std::vector<ValueHandle>& args);
  ValueHandle create_phi(const std::vector<std::pair<ValueHandle, ValueHandle>>& incoming);

  ValueHandle create_select(ValueHandle cond, ValueHandle true_val, ValueHandle false_val);

  ValueHandle create_malloc(const Type& type);
  ValueHandle create_free(ValueHandle ptr);

private:
  JITEngine& engine_;
  llvm::IRBuilder<>& builder_;
};

class FunctionCompiler {
public:
  explicit FunctionCompiler(JITEngine& engine, FunctionHandle function);
  ~FunctionCompiler();

  IRBuilder& builder() { return builder_; }

  ValueHandle create_entry_block();
  ValueHandle create_block(const std::string& name);

  std::optional<std::string> finalize();

private:
  JITEngine& engine_;
  FunctionHandle function_;
  IRBuilder builder_;
  std::vector<ValueHandle> blocks_;
};

} // namespace jit
} // namespace chimera
