(* Type System for Chimera Elixir *)
(* Prototype type checking and inference *)

module type EQUATABLE = sig
  type t
  val equal : t -> t -> bool
end

module MakeTypeSystem(E : EQUATABLE) = struct
  type type_expr =
    | TUnknown
    | TAny
    | TAtom of E.t
    | TInteger
    | TFloat
    | TString
    | TBinary
    | TList of type_expr
    | TTuple of type_expr list
    | TMap of type_expr * type_expr
    | TFunction of type_expr list * type_expr
    | TReference of string
    | TUnion of type_expr list
    | TVar of int

  type type_scheme = {
    vars: int list;
    body: type_expr;
  }

  type typing_env = {
    variables: (string, type_scheme) Hashtbl.t;
    module_types: (string, type_expr) Hashtbl.t;
    current_module: string option;
  }

  let empty_env () = {
    variables = Hashtbl.create 16;
    module_types = Hashtbl.create 16;
    current_module = None;
  }

  let fresh_var () =
    let counter = ref 0 in
    fun () ->
      let v = !counter in
      incr counter;
      TVar v

  let rec occurs_check (v : int) (t : type_expr) : bool =
    match t with
    | TUnknown | TAny | TInteger | TFloat | TString | TBinary -> false
    | TAtom _ -> false
    | TList t -> occurs_check v t
    | TTuple ts -> List.exists (occurs_check v) ts
    | TMap (k, v) -> occurs_check v k || occurs_check v v
    | TFunction (args, ret) ->
      List.exists (occurs_check v) args || occurs_check v ret
    | TReference _ -> false
    | TUnion ts -> List.exists (occurs_check v) ts
    | TVar v' -> v = v'

  let rec unify (t1 : type_expr) (t2 : type_expr) : (int * type_expr) list option =
    match t1, t2 with
    | TUnknown, _ -> Some []
    | _, TUnknown -> Some []
    | TAny, _ -> Some []
    | _, TAny -> Some []
    | TInteger, TInteger -> Some []
    | TFloat, TFloat -> Some []
    | TString, TString -> Some []
    | TBinary, TBinary -> Some []
    | TAtom a1, TAtom a2 -> if E.equal a1 a2 then Some [] else None
    | TList t1, TList t2 -> unify t1 t2
    | TTuple ts1, TTuple ts2 ->
      if List.length ts1 = List.length ts2 then
        unify_list ts1 ts2
      else None
    | TMap (k1, v1), TMap (k2, v2) ->
      (match unify k1 k2 with
       | Some subs -> (match unify v1 v2 with
         | Some subs2 -> Some (subs @ subs2)
         | None -> None)
       | None -> None)
    | TFunction (args1, ret1), TFunction (args2, ret2) ->
      (match unify ret1 ret2 with
       | Some subs -> unify_list args1 args2 >>= (fun args_subs ->
           Some (subs @ args_subs))
       | None -> None)
    | TVar v, t | t, TVar v ->
      if occurs_check v t then None
      else Some [(v, t)]
    | TReference n1, TReference n2 ->
      if n1 = n2 then Some [] else None
    | TUnion ts1, TUnion ts2 ->
      unify_list ts1 ts2
    | _ -> None

  and unify_list (ts1 : type_expr list) (ts2 : type_expr list) =
    match ts1, ts2 with
    | [], [] -> Some []
    | t1 :: ts1, t2 :: ts2 ->
      (match unify t1 t2 with
       | Some s1 -> (match unify_list ts1 ts2 with
         | Some s2 -> Some (s1 @ s2)
         | None -> None)
       | None -> None)
    | _ -> None

  let rec apply_subst (subs : (int * type_expr) list) (t : type_expr) : type_expr =
    match t with
    | TUnknown | TAny | TInteger | TFloat | TString | TBinary -> t
    | TAtom a -> TAtom a
    | TList t -> TList (apply_subst subs t)
    | TTuple ts -> TTuple (List.map (apply_subst subs) ts)
    | TMap (k, v) -> TMap (apply_subst subs k, apply_subst subs v)
    | TFunction (args, ret) ->
      TFunction (List.map (apply_subst subs) args, apply_subst subs ret)
    | TReference n -> TReference n
    | TUnion ts -> TUnion (List.map (apply_subst subs) ts)
    | TVar v ->
      (match List.assoc_opt v subs with
       | Some t -> apply_subst subs t
       | None -> t)

  let generalize (env : typing_env) (t : type_expr) : type_scheme =
    let free_vars = ref [] in
    let rec find_free t =
      match t with
      | TUnknown | TAny | TInteger | TFloat | TString | TBinary -> ()
      | TAtom _ -> ()
      | TList t -> find_free t
      | TTuple ts -> List.iter find_free ts
      | TMap (k, v) -> find_free k; find_free v
      | TFunction (args, ret) ->
        List.iter find_free args; find_free ret
      | TReference _ -> ()
      | TUnion ts -> List.iter find_free ts
      | TVar v -> if not (List.mem v !free_vars) then free_vars := v :: !free_vars
    in
    find_free t;
    { vars = !free_vars; body = t }

  let instantiate (scheme : type_scheme) : type_expr =
    let subst = List.map (fun v -> (v, fresh_var ())) scheme.vars in
    apply_subst subst scheme.body

  let rec infer_expr (env : typing_env) (expr : Syntax.expr) : (type_expr * (int * type_expr) list) =
    match expr with
    | Syntax.Literal (Syntax.Integer _) -> (TInteger, [])
    | Syntax.Literal (Syntax.Float _) -> (TFloat, [])
    | Syntax.Literal (Syntax.String _) -> (TString, [])
    | Syntax.Literal (Syntax.Atom _) -> (TAtom (Obj.magic ()), [])
    | Syntax.Variable name ->
      (match Hashtbl.find_opt env.variables name with
       | Some scheme -> (instantiate scheme, [])
       | None -> (TUnknown, []))
    | Syntax.Tuple es ->
      let (types, subs) = List.split (List.map (infer_expr env) es) in
      (TTuple types, subs)
    | Syntax.List (head, tail) ->
      let (h_type, h_subs) = infer_expr env head in
      let (t_type, t_subs) = match tail with
        | Some t -> infer_expr env t
        | None -> (TList TUnknown, [])
      in
      (TList h_type, h_subs @ t_subs)
    | _ -> (TUnknown, [])

  module Syntax = struct
    type literal = Integer of int | Float of float | String of string | Atom of string
    type expr =
      | Literal of literal
      | Variable of string
      | Tuple of expr list
      | List of expr * expr option
      | Map of (expr * expr) list
      | Call of expr * expr list
  end
end

module StringTypeSystem = MakeTypeSystem(struct
  type t = string
  let equal = (=)
end)

module Tests = struct
  let test_unify_integers () =
    let result = StringTypeSystem.unify StringTypeSystem.TInteger StringTypeSystem.TInteger in
    match result with
    | Some [] -> ()
    | _ -> raise (Failure "unify integers failed");
    print_endline "test_unify_integers passed"

  let test_unify_var () =
    let v = StringTypeSystem.TVar 0 in
    let result = StringTypeSystem.unify v StringTypeSystem.TInteger in
    match result with
    | Some [(0, StringTypeSystem.TInteger)] -> ()
    | _ -> raise (Failure "unify var failed");
    print_endline "test_unify_var passed"

  let test_occurs_check () =
    let v = 0 in
    let result = StringTypeSystem.occurs_check v StringTypeSystem.TVar v in
    assert (result = true);
    print_endline "test_occurs_check passed"

  let test_generalize () =
    let env = StringTypeSystem.empty_env () in
    let scheme = StringTypeSystem.generalize env StringTypeSystem.TInteger in
    assert (scheme.body = StringTypeSystem.TInteger);
    print_endline "test_generalize passed"

  let run_all () =
    test_unify_integers ();
    test_unify_var ();
    test_occurs_check ();
    test_generalize ();
    print_endline "All type system tests passed"
end

let () = Tests.run_all ()
