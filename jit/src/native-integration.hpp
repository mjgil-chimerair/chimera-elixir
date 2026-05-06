// Chimera Elixir - Native Integration Layer
// C++ bridge between BEAM runtime and native code

#pragma once

#include "nif.hpp"
#include "jit.hpp"

#include <vector>
#include <string>
#include <map>
#include <functional>
#include <memory>
#include <optional>

namespace chimera {
namespace jit {
namespace native {

using NifBridge = nif::NifBridge;
using JITEngine = jit::JITEngine;
using Term = nif::Term;

enum class CallResult {
  Ok,
  Error,
  Raise,
};

struct ExceptionInfo {
  std::string reason;
  std::vector<Term> stacktrace;
};

class NativeCallback {
public:
  virtual ~NativeCallback() = default;
  virtual CallResult call(nif::Env* env, std::vector<Term>& args,
                         Term& result, ExceptionInfo& exc) = 0;
};

template<typename F>
class NativeCallbackImpl : public NativeCallback {
public:
  explicit NativeCallbackImpl(F func) : func_(std::move(func)) {}

  CallResult call(nif::Env* env, std::vector<Term>& args,
                 Term& result, ExceptionInfo& exc) override {
    try {
      result = std::apply(func_, std::tuple_cat(
        std::make_tuple(env),
        std::array<Term, 0>{},
        args));
      return CallResult::Ok;
    } catch (const std::exception& e) {
      exc.reason = e.what();
      return CallResult::Raise;
    }
  }

private:
  F func_;
};

class Module {
public:
  Module() = default;
  ~Module() = default;

  bool export_function(const std::string& name, unsigned arity,
                      std::shared_ptr<NativeCallback> callback);

  bool export_nif(const std::string& name, unsigned arity,
                 void* func_ptr);

  std::optional<Term> call_function(nif::Env* env, const std::string& name,
                                   const std::vector<Term>& args);

  std::optional<Term> call_nif(nif::Env* env, const std::string& name,
                               const std::vector<Term>& args);

  std::vector<std::string> get_exports() const;

  static std::shared_ptr<Module> load(nif::Env* env, const std::string& path);
  static bool unload(nif::Env* env, std::shared_ptr<Module> module);

private:
  std::map<std::string, std::pair<unsigned, std::shared_ptr<NativeCallback>>> functions_;
  std::map<std::string, std::pair<unsigned, void*>> nifs_;
};

class TypeRegistry {
public:
  static TypeRegistry& instance();

  bool register_type(const std::string& name, const std::string& module,
                    const std::string& type_name);

  std::optional<std::string> find_type(const std::string& module,
                                       const std::string& type_name);

  bool register_converter(const std::string& from_type, const std::string& to_type,
                        std::function<Term(nif::Env*, Term)> converter);

  std::optional<std::function<Term(nif::Env*, Term)>> get_converter(
      const std::string& from_type, const std::string& to_type);

  Term convert(nif::Env* env, Term term,
              const std::string& from_type, const std::string& to_type);

private:
  TypeRegistry() = default;

  struct TypeInfo {
    std::string module;
    std::string type_name;
  };

  std::map<std::string, TypeInfo> types_;
  std::map<std::pair<std::string, std::string>,
           std::function<Term(nif::Env*, Term)>> converters_;
};

class NativeRuntime {
public:
  static NativeRuntime& instance();

  bool initialize(nif::Env* env);
  void shutdown();

  std::shared_ptr<Module> load_module(nif::Env* env, const std::string& path);
  bool unload_module(nif::Env* env, std::shared_ptr<Module> module);

  void* get_code_handle() const { return code_handle_; }
  const std::string& get_lib_path() const { return lib_path_; }

private:
  NativeRuntime() = default;
  ~NativeRuntime() = default;

  NativeRuntime(const NativeRuntime&) = delete;
  NativeRuntime& operator=(const NativeRuntime&) = delete;

  void* code_handle_;
  std::string lib_path_;
  std::vector<std::shared_ptr<Module>> loaded_modules_;
};

inline bool Module::export_function(const std::string& name, unsigned arity,
                                   std::shared_ptr<NativeCallback> callback) {
  functions_[name] = {arity, std::move(callback)};
  return true;
}

inline bool Module::export_nif(const std::string& name, unsigned arity,
                              void* func_ptr) {
  nifs_[name] = {arity, func_ptr};
  return true;
}

inline std::vector<std::string> Module::get_exports() const {
  std::vector<std::string> exports;
  for (const auto& [name, _] : functions_) {
    exports.push_back(name);
  }
  for (const auto& [name, _] : nifs_) {
    exports.push_back(name);
  }
  return exports;
}

} // namespace native
} // namespace jit
} // namespace chimera
