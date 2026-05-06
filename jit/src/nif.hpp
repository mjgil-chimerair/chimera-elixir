// Chimera Elixir - NIF Bridge
// Rust/C++ native function bridge for Erlang NIFs

#pragma once

#include <erl_nif.h>
#include <memory>
#include <string>
#include <vector>
#include <functional>
#include <optional>
#include <variant>

namespace chimera {
namespace jit {
namespace nif {

using Term = ERL_NIF_TERM;
using Env = ErlNifEnv;
using Binary = ErlNifBinary;
using MutexPtr = std::unique_ptr<ERL_NIF_TERM, std::function<void(ERL_NIF_TERM*)>>;

enum class NifError {
  BadArg,
  BadExternal,
  Enomem,
  FunctionFailed,
};

class TermRef {
public:
  TermRef(Env* env, Term term) : env_(env), term_(term) {}
  TermRef(const TermRef&) = delete;
  TermRef& operator=(const TermRef&) = delete;
  TermRef(TermRef&& other) noexcept : env_(other.env_), term_(other.term_) {
    other.env_ = nullptr;
  }
  TermRef& operator=(TermRef&& other) noexcept {
    if (this != &other) {
      env_ = other.env_;
      term_ = other.term_;
      other.env_ = nullptr;
    }
    return *this;
  }

  Term get() const { return term_; }
  Env* env() const { return env_; }

  template<typename T>
  T get_value() const;

private:
  Env* env_;
  Term term_;
};

class Atom {
public:
  explicit Atom(Env* env, const std::string& name);
  explicit Atom(Env* env, Atom other);
  Term to_term() const { return term_; }
  std::string to_string() const { return name_; }
  static Atom from_index(Env* env, unsigned index);

private:
  Env* env_;
  Term term_;
  std::string name_;
};

class ListBuilder {
public:
  explicit ListBuilder(Env* env);
  ~ListBuilder();
  ListBuilder(const ListBuilder&) = delete;
  ListBuilder& operator=(const ListBuilder&) = delete;

  bool append(Term term);
  Term finish();

private:
  Env* env_;
  std::vector<Term> terms_;
};

class MapBuilder {
public:
  explicit MapBuilder(Env* env);
  ~MapBuilder();
  MapBuilder(const MapBuilder&) = delete;
  MapBuilder& operator=(const MapBuilder&) = delete;

  bool insert(Term key, Term value);
  Term finish();

private:
  Env* env_;
  std::vector<std::pair<Term, Term>> entries_;
};

class BinaryRef {
public:
  explicit BinaryRef(Env* env, Term term);
  ~BinaryRef();

  unsigned char* data() { return data_; }
  size_t size() const { return size_; }

private:
  Env* env_;
  unsigned char* data_;
  size_t size_;
};

template<typename T>
struct NifReturn {
  std::variant<T, NifError> value;
};

template<typename... Args>
using NifFunc = std::function<Term(Env*, Args...)>;

class NifBridge {
public:
  static constexpr unsigned Arity = 8;

  struct Export {
    std::string name;
    unsigned arity;
    void* func_ptr;
  };

  static std::optional<std::string> load(Env* env, Term load_info);
  static void unload(Env* env);

  static std::optional<Export> find_export(const std::string& name, unsigned arity);

  template<typename Ret, typename... Args>
  static Ret call(Env* env, const std::string& module, const std::string& func, Args... args);

  template<typename T>
  static T get_value(Env* env, Term term);

  static Term make_atom(Env* env, const std::string& name);
  static Term make_binary(Env* env, const void* data, size_t size);
  static Term make_list(Env* env, const std::vector<Term>& terms);
  static Term make_map(Env* env, const std::vector<std::pair<Term, Term>>& entries);

  static bool is_atom(Env* env, Term term);
  static bool is_binary(Env* env, Term term);
  static bool is_list(Env* env, Term term);
  static bool is_map(Env* env, Term term);
  static bool is_tuple(Env* env, Term term);
  static bool is_integer(Env* env, Term term);
  static bool is_number(Env* env, Term term);

  static std::string atom_to_string(Env* env, Term atom);
  static long get_integer(Env* env, Term term);
  static double get_double(Env* env, Term term);
  static std::vector<Term> list_to_vector(Env* env, Term list);
  static std::map<Term, Term> map_to_map(Env* env, Term map);

  static Term alloc_resource(Env* env, void* data, void (*dtor)(Env*, void*));
  static void* get_resource(Env* env, Term term, size_t size);

private:
  static bool loaded_;
  static std::vector<Export> exports_;
};

template<typename T>
T TermRef::get_value() const {
  return NifBridge::get_value<T>(env_, term_);
}

template<typename T>
T NifBridge::get_value(Env* env, Term term) {
  if constexpr (std::is_same_v<T, long>) {
    return get_integer(env, term);
  } else if constexpr (std::is_same_v<T, double>) {
    return get_double(env, term);
  } else if constexpr (std::is_same_v<T, std::string>) {
    return atom_to_string(env, term);
  } else if constexpr (std::is_same_v<T, Term>) {
    return term;
  }
  return T{};
}

template<typename Ret, typename... Args>
Ret NifBridge::call(Env* env, const std::string& module,
                    const std::string& func, Args... args) {
  ErlNifFuncPtr func_ptr = enif_find_function(env, module.c_str(), func.c_str(),
                                             sizeof...(Args));
  if (!func_ptr) {
    if constexpr (std::is_same_v<Ret, Term>) {
      return enif_make_atom(env, "error");
    }
    return Ret{};
  }

  if constexpr (std::is_same_v<Ret, Term>) {
    return func_ptr(env, std::initializer_list<Term>{args...}.begin());
  } else {
    Term result = func_ptr(env, std::initializer_list<Term>{args...}.begin());
    return get_value<Ret>(env, result);
  }
}

} // namespace nif
} // namespace jit
} // namespace chimera
