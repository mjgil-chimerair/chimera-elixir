(* Macro Formalization for Chimera Elixir *)
(* Formalizes macro expansion invariants and hygiene boundaries *)

module Env = struct
  type t = {
    module_name: string option;
    function_name: (string * int) option;
    line: int;
    context: Context.t;
    aliases: (string, string list) Hashtbl.t;
    imports: (string, string * string) Hashtbl.t;
  }
  and Context.t = Default | Match | Guard | TypeSpec | MacroDefinition | Quote

  let empty = {
    module_name = None;
    function_name = None;
    line = 1;
    context = Default;
    aliases = Hashtbl.create 16;
    imports = Hashtbl.create 16;
  }
end

module Hygiene = struct
  type origin = Unspecified | File of string | Generated of string

  type t = {
    origin: origin;
    clashing: bool;
    generated: bool;
  }

  let make ?(origin=Unspecified) ?(clashing=false) ?(generated=false) () =
    { origin; clashing; generated }

  let mark_generated t = { t with generated = true }
  let mark_clashing t = { t with clashing = true }
  let with_origin origin t = { t with origin }

  let default = make ()
end

module Syntax = struct
  type 'a with_hygiene = {
    expr: 'a;
    hygiene: Hygiene.t;
    meta: Meta.t;
  }
  and Meta.t = {
    line: int;
    column: int;
    file: string option;
  }

  type var = string with_hygiene
  type atom = string with_hygiene
  type integer = int
  type float = float
  type string = string

  type pattern =
    | PWildcard
    | PVar of var
    | PAtom of atom
    | PInteger of integer
    | PString of string
    | PTuple of pattern list
    | PList of pattern * pattern option
    | PCons of pattern * pattern
    | PMap of (pattern * pattern) list

  type rec_expr =
    | ELiteral of literal
    | EVar of var
    | EAtom of atom
    | ECall of call
    | ETuple of rec_expr list
    | EList of rec_expr * rec_expr option
    | EMap of (rec_expr * rec_expr) list
    | EMatch of pattern * rec_expr * rec_expr
    | ESeq of rec_expr * rec_expr
    | ETry of rec_expr * (pattern * rec_expr) list * (pattern * rec_expr) list
    | EReceive of (pattern * rec_expr) list * (rec_expr * rec_expr) option
  and literal = Integer of int | Float of float | String of string | Atom of string
  and call = {
    function: rec_expr;
    arguments: rec_expr list;
    meta: Meta.t;
  }

  type quoted = Quoted of rec_expr with_hygiene

  and unquote_kind = Unquote | UnquoteSplicing

  and quoted_content =
    | Literal of literal
    | Var of var
    | Splicing of unquote_kind * rec_expr
    | Many of quoted_content list

  let meta_of_wrapped { meta; _ } = meta
  let hygiene_of_wrapped { hygiene; _ } = hygiene
end

module Expansion = struct
  let rec expand_quoted (env: Env.t) (quoted: Syntax.quoted) : Syntax.rec_expr =
    match quoted with
    | Quoted { expr; hygiene; meta } ->
      expand_content env expr hygiene

  and expand_content env content hygiene =
    match content with
    | Syntax.Literal lit -> Syntax.ELiteral lit
    | Syntax.Var { expr = v; _ } -> Syntax.EVar { expr = v; hygiene; meta = Hygiene.default }
    | Syntax.Splicing (kind, expr) ->
      let expanded = expand_expr env expr in
      (match kind with
       | Syntax.UnquoteSplicing ->
         (match expanded with
          | Syntax.EList (head, tail) -> head
          | _ -> raise (Invalid_argument "unquote_splicing requires a list"))
       | Syntax.Unquote -> expanded)
    | Syntax.Many items -> expand_many env items hygiene

  and expand_many env items hygiene =
    match items with
    | [] -> Syntax.ELiteral (Syntax.Integer 0) (* nil result *)
    | [item] -> expand_content env item hygiene
    | item :: rest ->
      Syntax.ESeq (expand_content env item hygiene, expand_many env rest hygiene)

  and expand_expr env expr =
    match expr with
    | Syntax.ELiteral lit -> Syntax.ELiteral lit
    | Syntax.EVar v -> Syntax.EVar v
    | Syntax.EAtom a -> Syntax.EAtom a
    | Syntax.ETuple items -> Syntax.ETuple (List.map (expand_expr env) items)
    | Syntax.EList (head, tail) ->
      Syntax.EList (expand_expr env head, Option.map (expand_expr env) tail)
    | Syntax.EMap pairs ->
      Syntax.EMap (List.map (fun (k, v) -> (expand_expr env k, expand_expr env v)) pairs)
    | Syntax.EMatch (pat, value, body) ->
      Syntax.EMatch (pat, expand_expr env value, expand_expr env body)
    | Syntax.ESeq (a, b) -> Syntax.ESeq (expand_expr env a, expand_expr env b)
    | Syntax.ETry (expr, rescue, catch) ->
      Syntax.ETry (expand_expr env expr,
                    List.map (fun (p, e) -> (p, expand_expr env e)) rescue,
                    List.map (fun (p, e) -> (p, expand_expr env e)) catch)
    | Syntax.EReceive (clauses, timeout) ->
      Syntax.EReceive (clauses, Option.map (fun (t, e) -> (expand_expr env t, expand_expr env e)) timeout)
    | Syntax.ECall call -> expand_call env call

  and expand_call env { Syntax.function; arguments; meta } =
    let func = expand_expr env function in
    let args = List.map (expand_expr env) arguments in
    (* For now, just reconstruct the call *)
    Syntax.ECall { function = func; arguments = args; meta }
end

module Tests = struct
  let test_hygiene_creation () =
    let h = Hygiene.make ~origin:(File "test.ex") () in
    assert (h.generated = false);
    assert (h.clashing = false);
    print_endline "test_hygiene_creation passed"

  let test_hygiene_mark_generated () =
    let h = Hygiene.make () in
    let h' = Hygiene.mark_generated h in
    assert (h'.generated = true);
    print_endline "test_hygiene_mark_generated passed"

  let test_expand_simple_literal () =
    let quoted = Syntax.Quoted {
      expr = Syntax.Literal (Syntax.Integer 42);
      hygiene = Hygiene.default;
      meta = { line = 1; column = 1; file = None };
    } in
    let expanded = Expansion.expand_quoted Env.empty quoted in
    (match expanded with
     | Syntax.ELiteral (Syntax.Integer n) when n = 42 -> ()
     | _ -> raise (Failure "expand_simple_literal failed"));
    print_endline "test_expand_simple_literal passed"

  let run_all () =
    test_hygiene_creation ();
    test_hygiene_mark_generated ();
    test_expand_simple_literal ();
    print_endline "All macro formalization tests passed"
end

let () = Tests.run_all ()