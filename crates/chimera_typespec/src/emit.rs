//! Typespec emission for BEAM abstract format.
//!
//! Encodes typespecs into the BEAM abstract format used by
//! debug info and type documentation.

use super::*;
use chimera_term::{Atom, AtomTable, Term};

/// BEAM abstract format typespec encoding.
///
/// The BEAM abstract format represents typespecs as tuples:
/// - `type` -> {'type', type_name, type_def, type_arity}
/// - `spec` -> {'spec', function_name, [type_pair], clause_meta}
/// - `callback` -> {'callback', function_name, [type_pair], clause_meta}
#[derive(Debug, Clone)]
pub enum BeamTypeSpec {
    /// Abstract type definition
    Type {
        name: Atom,
        def: BeamType,
        arity: u8,
        meta: BeamTypeMeta,
    },
    /// Abstract spec (function type)
    Spec {
        name: Atom,
        arity: u8,
        type_pairs: Vec<(BeamType, BeamType)>,
        meta: BeamTypeMeta,
    },
    /// Abstract callback (behaviour callback)
    Callback {
        name: Atom,
        arity: u8,
        type_pairs: Vec<(BeamType, BeamType)>,
        meta: BeamTypeMeta,
    },
    /// Abstract opaque type
    Opaque {
        name: Atom,
        def: BeamType,
        arity: u8,
        meta: BeamTypeMeta,
    },
    /// Abstract typep (private type)
    Typep {
        name: Atom,
        def: BeamType,
        arity: u8,
        meta: BeamTypeMeta,
    },
}

#[derive(Debug, Clone)]
pub struct BeamTypeMeta {
    pub file_id: SourceFileId,
    pub line: u32,
    pub deprecated: Option<String>,
    pub doc: Option<String>,
}

impl BeamTypeMeta {
    pub fn from_spec_meta(meta: &SpecMeta) -> Self {
        BeamTypeMeta {
            file_id: meta.file_id,
            line: meta.line,
            deprecated: meta.deprecated.clone(),
            doc: meta.doc.clone(),
        }
    }

    pub fn from_type_meta(meta: &TypeMeta) -> Self {
        BeamTypeMeta {
            file_id: meta.file_id,
            line: meta.line,
            deprecated: None,
            doc: meta.doc.clone(),
        }
    }
}

/// BEAM type representation for abstract format.
#[derive(Debug, Clone)]
pub enum BeamType {
    /// Type variable (a, b, ...)
    Var(Atom),
    /// Atom literal
    LitAtom(Atom),
    /// Atom type (dynamic)
    AtomType,
    /// Remote type (Module:type())
    Remote {
        module: Atom,
        name: Atom,
        args: Vec<BeamType>,
    },
    /// Union type
    Union(Vec<BeamType>),
    /// List type
    List(Box<BeamType>),
    /// Improper list
    ImproperList(Box<BeamType>, Box<BeamType>),
    /// Tuple type
    Tuple(Vec<BeamType>),
    /// Map type
    Map(Vec<(BeamType, BeamType)>),
    /// Range type (integer range)
    Range(Box<BeamType>, Box<BeamType>),
    /// Function type
    Fun {
        args: Vec<BeamType>,
        return_type: Box<BeamType>,
    },
    /// Integer type (possibly with range)
    Integer,
    /// Float type
    Float,
    /// Number type
    Number,
    /// Binary type
    Binary,
    /// Bitstring type
    Bitstring(Option<Box<BeamType>>),
    /// String type
    String,
    /// Charlist type
    Charlist,
    /// Boolean type
    Boolean,
    /// Pid type
    Pid,
    /// Reference type
    Reference,
    /// Port type
    Port,
    /// Any type
    Any,
    /// None type
    None,
    /// Maybe type (Elixir's maybe渐)
    Maybe,
}

impl BeamType {
    /// Convert a TypespecType to a BeamType.
    pub fn from_typespec_type(ty: &TypespecType, atoms: &mut AtomTable) -> BeamType {
        match ty {
            TypespecType::Any => BeamType::Any,
            TypespecType::Atom(a) => BeamType::LitAtom(a.clone()),
            TypespecType::DynamicAtom => BeamType::AtomType,
            TypespecType::Integer => BeamType::Integer,
            TypespecType::Float => BeamType::Float,
            TypespecType::Number => BeamType::Number,
            TypespecType::Binary => BeamType::Binary,
            TypespecType::Bitstring(inner) => BeamType::Bitstring(
                inner
                    .as_ref()
                    .map(|t| Box::new(Self::from_typespec_type(t, atoms))),
            ),
            TypespecType::String => BeamType::String,
            TypespecType::Charlist => BeamType::Charlist,
            TypespecType::Boolean => BeamType::Boolean,
            TypespecType::List(inner) => {
                BeamType::List(Box::new(Self::from_typespec_type(inner, atoms)))
            }
            TypespecType::ImproperList(head, tail) => BeamType::ImproperList(
                Box::new(Self::from_typespec_type(head, atoms)),
                Box::new(Self::from_typespec_type(tail, atoms)),
            ),
            TypespecType::Tuple(items) => BeamType::Tuple(
                items
                    .iter()
                    .map(|t| Self::from_typespec_type(t, atoms))
                    .collect(),
            ),
            TypespecType::Map(pairs) => BeamType::Map(
                pairs
                    .iter()
                    .map(|(k, v)| {
                        (
                            Self::from_typespec_type(k, atoms),
                            Self::from_typespec_type(v, atoms),
                        )
                    })
                    .collect(),
            ),
            TypespecType::Union(types) => BeamType::Union(
                types
                    .iter()
                    .map(|t| Self::from_typespec_type(t, atoms))
                    .collect(),
            ),
            TypespecType::Range(start, end) => BeamType::Range(
                Box::new(Self::from_typespec_type(start, atoms)),
                Box::new(Self::from_typespec_type(end, atoms)),
            ),
            TypespecType::Pid => BeamType::Pid,
            TypespecType::Reference => BeamType::Reference,
            TypespecType::Port => BeamType::Port,
            TypespecType::Function { args, return_type } => BeamType::Fun {
                args: args
                    .iter()
                    .map(|t| Self::from_typespec_type(t, atoms))
                    .collect(),
                return_type: Box::new(Self::from_typespec_type(return_type, atoms)),
            },
            TypespecType::Remote { module, name, args } => BeamType::Remote {
                module: module.clone(),
                name: name.clone(),
                args: args
                    .iter()
                    .map(|t| Self::from_typespec_type(t, atoms))
                    .collect(),
            },
            TypespecType::Variable(v) => BeamType::Var(v.clone()),
            TypespecType::Parens(inner) => Self::from_typespec_type(inner, atoms),
            TypespecType::LitInteger(_n) => BeamType::Integer,
            TypespecType::LitAtom(a) => BeamType::LitAtom(a.clone()),
            TypespecType::RemoteType {
                module: _,
                name,
                args,
            } => BeamType::Remote {
                module: atoms.intern("Elixir"),
                name: name.clone(),
                args: args
                    .iter()
                    .map(|t| Self::from_typespec_type(t, atoms))
                    .collect(),
            },
            TypespecType::Struct { name, fields } => {
                // Struct type: map with special key
                let struct_key = atoms.intern("__struct__");
                let name_ty = BeamType::LitAtom(name.clone());
                let mut pairs = vec![(BeamType::LitAtom(struct_key), name_ty)];
                for (fname, fty) in fields {
                    pairs.push((
                        BeamType::LitAtom(fname.clone()),
                        Self::from_typespec_type(fty, atoms),
                    ));
                }
                BeamType::Map(pairs)
            }
            TypespecType::Maybe => BeamType::Maybe,
        }
    }
}

impl BeamTypeSpec {
    /// Convert a Typespec to BeamTypeSpec representation.
    pub fn from_typespec(spec: &Typespec, atoms: &mut AtomTable) -> BeamTypeSpec {
        match spec {
            Typespec::Type {
                name,
                type_def,
                meta,
            } => {
                let arity = count_type_arity(type_def);
                BeamTypeSpec::Type {
                    name: name.clone(),
                    def: BeamType::from_typespec_type(type_def, atoms),
                    arity,
                    meta: BeamTypeMeta::from_type_meta(meta),
                }
            }
            Typespec::Typep {
                name,
                type_def,
                meta,
            } => {
                let arity = count_type_arity(type_def);
                BeamTypeSpec::Typep {
                    name: name.clone(),
                    def: BeamType::from_typespec_type(type_def, atoms),
                    arity,
                    meta: BeamTypeMeta::from_type_meta(meta),
                }
            }
            Typespec::Opaque {
                name,
                type_def,
                meta,
            } => {
                let arity = count_type_arity(type_def);
                BeamTypeSpec::Opaque {
                    name: name.clone(),
                    def: BeamType::from_typespec_type(type_def, atoms),
                    arity,
                    meta: BeamTypeMeta::from_type_meta(meta),
                }
            }
            Typespec::Spec {
                name,
                arity,
                params,
                return_type: _,
                meta,
            } => {
                let type_pairs = params
                    .iter()
                    .map(|arg| {
                        let arg_ty = arg.type_def();
                        (BeamType::Any, BeamType::from_typespec_type(arg_ty, atoms))
                    })
                    .collect();
                BeamTypeSpec::Spec {
                    name: name.clone(),
                    arity: *arity,
                    type_pairs,
                    meta: BeamTypeMeta::from_spec_meta(meta),
                }
            }
            Typespec::Callback {
                name,
                arity,
                params,
                return_type: _,
                meta,
            } => {
                let type_pairs = params
                    .iter()
                    .map(|arg| {
                        let arg_ty = arg.type_def();
                        (BeamType::Any, BeamType::from_typespec_type(arg_ty, atoms))
                    })
                    .collect();
                BeamTypeSpec::Callback {
                    name: name.clone(),
                    arity: *arity,
                    type_pairs,
                    meta: BeamTypeMeta::from_spec_meta(meta),
                }
            }
            Typespec::MacroCallback {
                name,
                arity,
                params,
                return_type: _,
                meta,
            } => {
                // MacroCallbacks are emitted as callbacks with __macro__ prefix
                let type_pairs = params
                    .iter()
                    .map(|arg| {
                        let arg_ty = arg.type_def();
                        (BeamType::Any, BeamType::from_typespec_type(arg_ty, atoms))
                    })
                    .collect();
                let name_lookup = atoms.lookup(name.clone()).map_or("unknown", |v| v);
                BeamTypeSpec::Callback {
                    name: atoms.intern(&format!("__macro___{}", name_lookup)),
                    arity: *arity,
                    type_pairs,
                    meta: BeamTypeMeta::from_spec_meta(meta),
                }
            }
        }
    }

    /// Convert to BEAM abstract format term (ETF tuple).
    pub fn to_term(&self, atoms: &mut AtomTable) -> Term {
        match self {
            BeamTypeSpec::Type {
                name,
                def,
                arity,
                meta: _,
            } => {
                let type_atom = atoms.intern("type");
                let name_term = Term::Atom(name.clone());
                let def_term = beam_type_to_term(def, atoms);
                let arity_term = Term::SmallInt(*arity as i64);
                Term::Tuple(vec![Term::Atom(type_atom), name_term, def_term, arity_term])
            }
            BeamTypeSpec::Opaque {
                name,
                def,
                arity,
                meta: _,
            } => {
                let opaque_atom = atoms.intern("opaque");
                let name_term = Term::Atom(name.clone());
                let def_term = beam_type_to_term(def, atoms);
                let arity_term = Term::SmallInt(*arity as i64);
                Term::Tuple(vec![
                    Term::Atom(opaque_atom),
                    name_term,
                    def_term,
                    arity_term,
                ])
            }
            BeamTypeSpec::Typep {
                name,
                def,
                arity,
                meta: _,
            } => {
                let typep_atom = atoms.intern("typep");
                let name_term = Term::Atom(name.clone());
                let def_term = beam_type_to_term(def, atoms);
                let arity_term = Term::SmallInt(*arity as i64);
                Term::Tuple(vec![
                    Term::Atom(typep_atom),
                    name_term,
                    def_term,
                    arity_term,
                ])
            }
            BeamTypeSpec::Spec {
                name,
                arity: _,
                type_pairs,
                meta: _,
            } => {
                let spec_atom = atoms.intern("spec");
                let name_term = Term::Atom(name.clone());
                let pairs_term = Term::List(
                    type_pairs
                        .iter()
                        .map(|(constraint, ty)| {
                            Term::Tuple(vec![
                                beam_type_to_term(constraint, atoms),
                                beam_type_to_term(ty, atoms),
                            ])
                        })
                        .collect(),
                );
                Term::Tuple(vec![Term::Atom(spec_atom), name_term, pairs_term])
            }
            BeamTypeSpec::Callback {
                name,
                arity: _,
                type_pairs,
                meta: _,
            } => {
                let callback_atom = atoms.intern("callback");
                let name_term = Term::Atom(name.clone());
                let pairs_term = Term::List(
                    type_pairs
                        .iter()
                        .map(|(constraint, ty)| {
                            Term::Tuple(vec![
                                beam_type_to_term(constraint, atoms),
                                beam_type_to_term(ty, atoms),
                            ])
                        })
                        .collect(),
                );
                Term::Tuple(vec![Term::Atom(callback_atom), name_term, pairs_term])
            }
        }
    }
}

/// Count the number of type variables in a type definition (for arity).
fn count_type_arity(type_def: &TypespecType) -> u8 {
    let mut vars = std::collections::HashSet::new();
    collect_variables(type_def, &mut vars);
    vars.len() as u8
}

/// Collect all type variables from a type.
fn collect_variables(ty: &TypespecType, vars: &mut std::collections::HashSet<Atom>) {
    match ty {
        TypespecType::Variable(v) => {
            vars.insert(v.clone());
        }
        TypespecType::List(inner) => collect_variables(inner, vars),
        TypespecType::ImproperList(h, t) => {
            collect_variables(h, vars);
            collect_variables(t, vars);
        }
        TypespecType::Tuple(items) => items.iter().for_each(|t| collect_variables(t, vars)),
        TypespecType::Map(pairs) => pairs.iter().for_each(|(k, v)| {
            collect_variables(k, vars);
            collect_variables(v, vars);
        }),
        TypespecType::Union(types) => types.iter().for_each(|t| collect_variables(t, vars)),
        TypespecType::Range(s, e) => {
            collect_variables(s, vars);
            collect_variables(e, vars);
        }
        TypespecType::Function { args, return_type } => {
            args.iter().for_each(|t| collect_variables(t, vars));
            collect_variables(return_type, vars);
        }
        TypespecType::Remote { args, .. } => args.iter().for_each(|t| collect_variables(t, vars)),
        TypespecType::RemoteType { args, .. } => {
            args.iter().for_each(|t| collect_variables(t, vars))
        }
        TypespecType::Struct { fields, .. } => {
            fields.iter().for_each(|(_, t)| collect_variables(t, vars))
        }
        _ => {}
    }
}

/// Convert BeamType to Term for ETF encoding.
fn beam_type_to_term(ty: &BeamType, atoms: &mut AtomTable) -> Term {
    match ty {
        BeamType::Var(v) => {
            let var_atom = atoms.intern("var");
            Term::Tuple(vec![Term::Atom(var_atom), Term::Atom(v.clone())])
        }
        BeamType::LitAtom(a) => {
            let atom_atom = atoms.intern("atom");
            Term::Tuple(vec![Term::Atom(atom_atom), Term::Atom(a.clone())])
        }
        BeamType::Remote { module, name, args } => {
            let remote_atom = atoms.intern("remote");
            let module_term = Term::Atom(module.clone());
            let name_term = Term::Atom(name.clone());
            let args_term = Term::List(args.iter().map(|t| beam_type_to_term(t, atoms)).collect());
            Term::Tuple(vec![
                Term::Atom(remote_atom),
                module_term,
                name_term,
                args_term,
            ])
        }
        BeamType::Union(types) => {
            let union_atom = atoms.intern("union");
            let types_term =
                Term::List(types.iter().map(|t| beam_type_to_term(t, atoms)).collect());
            Term::Tuple(vec![Term::Atom(union_atom), types_term])
        }
        BeamType::List(inner) => {
            let list_atom = atoms.intern("list");
            let inner_term = beam_type_to_term(inner, atoms);
            Term::Tuple(vec![Term::Atom(list_atom), inner_term])
        }
        BeamType::ImproperList(head, tail) => {
            let improper_atom = atoms.intern("improper_list");
            let head_term = beam_type_to_term(head, atoms);
            let tail_term = beam_type_to_term(tail, atoms);
            Term::Tuple(vec![Term::Atom(improper_atom), head_term, tail_term])
        }
        BeamType::Tuple(items) => {
            let tuple_atom = atoms.intern("tuple");
            let items_term =
                Term::List(items.iter().map(|t| beam_type_to_term(t, atoms)).collect());
            Term::Tuple(vec![Term::Atom(tuple_atom), items_term])
        }
        BeamType::Map(pairs) => {
            let map_atom = atoms.intern("map");
            let pairs_term = Term::List(
                pairs
                    .iter()
                    .map(|(k, v)| {
                        Term::Tuple(vec![
                            beam_type_to_term(k, atoms),
                            beam_type_to_term(v, atoms),
                        ])
                    })
                    .collect(),
            );
            Term::Tuple(vec![Term::Atom(map_atom), pairs_term])
        }
        BeamType::Range(start, end) => {
            let range_atom = atoms.intern("range");
            let start_term = beam_type_to_term(start, atoms);
            let end_term = beam_type_to_term(end, atoms);
            Term::Tuple(vec![Term::Atom(range_atom), start_term, end_term])
        }
        BeamType::Fun { args, return_type } => {
            let fun_atom = atoms.intern("fun");
            let args_term = Term::List(args.iter().map(|t| beam_type_to_term(t, atoms)).collect());
            let return_term = beam_type_to_term(return_type, atoms);
            Term::Tuple(vec![Term::Atom(fun_atom), args_term, return_term])
        }
        BeamType::Integer => {
            let integer_atom = atoms.intern("integer");
            Term::Atom(integer_atom)
        }
        BeamType::Float => {
            let float_atom = atoms.intern("float");
            Term::Atom(float_atom)
        }
        BeamType::Number => {
            let number_atom = atoms.intern("number");
            Term::Atom(number_atom)
        }
        BeamType::Binary => {
            let binary_atom = atoms.intern("binary");
            Term::Atom(binary_atom)
        }
        BeamType::Bitstring(inner) => {
            let bitstring_atom = atoms.intern("bitstring");
            match inner {
                Some(t) => Term::Tuple(vec![
                    Term::Atom(bitstring_atom),
                    beam_type_to_term(t, atoms),
                ]),
                None => Term::Atom(bitstring_atom),
            }
        }
        BeamType::String => {
            let string_atom = atoms.intern("string");
            Term::Atom(string_atom)
        }
        BeamType::Charlist => {
            let charlist_atom = atoms.intern("charlist");
            Term::Atom(charlist_atom)
        }
        BeamType::Boolean => {
            let boolean_atom = atoms.intern("boolean");
            Term::Atom(boolean_atom)
        }
        BeamType::AtomType => {
            let atom_type_atom = atoms.intern("atom");
            Term::Atom(atom_type_atom)
        }
        BeamType::Pid => {
            let pid_atom = atoms.intern("pid");
            Term::Atom(pid_atom)
        }
        BeamType::Reference => {
            let reference_atom = atoms.intern("reference");
            Term::Atom(reference_atom)
        }
        BeamType::Port => {
            let port_atom = atoms.intern("port");
            Term::Atom(port_atom)
        }
        BeamType::Any => {
            let any_atom = atoms.intern("any");
            Term::Atom(any_atom)
        }
        BeamType::None => {
            let none_atom = atoms.intern("none");
            Term::Atom(none_atom)
        }
        BeamType::Maybe => {
            let maybe_atom = atoms.intern("maybe");
            Term::Atom(maybe_atom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beam_type_from_integer() {
        let ty = TypespecType::Integer;
        let mut atoms = AtomTable::new();
        let beam = BeamType::from_typespec_type(&ty, &mut atoms);
        assert!(matches!(beam, BeamType::Integer));
    }

    #[test]
    fn test_beam_type_from_list() {
        let inner = Box::new(TypespecType::Integer);
        let ty = TypespecType::List(inner);
        let mut atoms = AtomTable::new();
        let beam = BeamType::from_typespec_type(&ty, &mut atoms);
        match beam {
            BeamType::List(inner) => {
                assert!(matches!(*inner, BeamType::Integer));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_beam_type_from_remote() {
        let mut atoms = AtomTable::new();
        let module = atoms.intern("Enum");
        let name = atoms.intern("t");
        let ty = TypespecType::Remote {
            module,
            name,
            args: vec![TypespecType::Integer],
        };
        let beam = BeamType::from_typespec_type(&ty, &mut atoms);
        match beam {
            BeamType::Remote {
                module: ref m,
                name: ref n,
                args,
            } => {
                // module and name were moved into ty, but beam contains clones
                // We can't compare directly, just check structure
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected Remote"),
        }
    }

    #[test]
    fn test_beam_type_from_union() {
        let ty = TypespecType::Union(vec![TypespecType::Integer, TypespecType::Float]);
        let mut atoms = AtomTable::new();
        let beam = BeamType::from_typespec_type(&ty, &mut atoms);
        match beam {
            BeamType::Union(types) => {
                assert_eq!(types.len(), 2);
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn test_count_type_arity_simple() {
        let ty = TypespecType::Integer;
        assert_eq!(count_type_arity(&ty), 0);
    }

    #[test]
    fn test_count_type_arity_with_var() {
        let mut atoms = AtomTable::new();
        let var = atoms.intern("a");
        let ty = TypespecType::Variable(var);
        assert_eq!(count_type_arity(&ty), 1);
    }

    #[test]
    fn test_count_type_arity_function() {
        let mut atoms = AtomTable::new();
        let var_a = atoms.intern("a");
        let var_b = atoms.intern("b");
        let ty = TypespecType::Function {
            args: vec![TypespecType::Variable(var_a)],
            return_type: Box::new(TypespecType::Variable(var_b)),
        };
        // Two unique variables
        assert_eq!(count_type_arity(&ty), 2);
    }

    #[test]
    fn test_beam_typespec_from_spec() {
        let mut atoms = AtomTable::new();
        let spec = Typespec::Spec {
            name: atoms.intern("add"),
            arity: 2,
            params: vec![
                TypespecArg::Anonymous(Box::new(TypespecType::Integer)),
                TypespecArg::Anonymous(Box::new(TypespecType::Integer)),
            ],
            return_type: Box::new(TypespecType::Integer),
            meta: SpecMeta::new(SourceFileId::new(0)),
        };
        let beam = BeamTypeSpec::from_typespec(&spec, &mut atoms);
        match beam {
            BeamTypeSpec::Spec {
                name: _,
                arity,
                type_pairs,
                ..
            } => {
                assert_eq!(arity, 2);
                assert_eq!(type_pairs.len(), 2);
            }
            _ => panic!("expected Spec"),
        }
    }

    #[test]
    fn test_beam_typespec_from_type() {
        let mut atoms = AtomTable::new();
        let spec = Typespec::Type {
            name: atoms.intern("my_type"),
            type_def: Box::new(TypespecType::Integer),
            meta: TypeMeta::new(SourceFileId::new(0)),
        };
        let beam = BeamTypeSpec::from_typespec(&spec, &mut atoms);
        match beam {
            BeamTypeSpec::Type { name: _, arity, .. } => {
                assert_eq!(arity, 0);
            }
            _ => panic!("expected Type"),
        }
    }

    #[test]
    fn test_beam_typespec_to_term() {
        let mut atoms = AtomTable::new();
        let spec = Typespec::Spec {
            name: atoms.intern("add"),
            arity: 2,
            params: vec![
                TypespecArg::Anonymous(Box::new(TypespecType::Integer)),
                TypespecArg::Anonymous(Box::new(TypespecType::Integer)),
            ],
            return_type: Box::new(TypespecType::Integer),
            meta: SpecMeta::new(SourceFileId::new(0)),
        };
        let beam = BeamTypeSpec::from_typespec(&spec, &mut atoms);
        let term = beam.to_term(&mut atoms);
        // Should be a tuple
        assert!(matches!(term, Term::Tuple(_)));
    }
}
