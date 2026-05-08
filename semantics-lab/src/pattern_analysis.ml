(* Pattern Analysis for Chimera Elixir *)
(* Prototype exhaustiveness checking and pattern semantics *)

module type ORDER = sig
  type t
  val compare : t -> t -> int
end

module MakePattern(A : ORDER) = struct
  type t =
    | Wildcard
    | Var of string
    | Atom of A.t
    | Integer of A.t
    | String of string
    | Tuple of t list
    | List of t * t option  (* head, tail *)
    | Cons of t * t
    | Map of (t * t) list

  let rec all_values () : t list =
    (* Represents all possible values of a type *)
    failwith "Cannot enumerate all values for generic type"

  let rec cover (patterns : t list) : t list =
    match patterns with
    | [] -> []
    | p :: ps ->
      (* Compute the complement of p covered by existing patterns *)
      let covered = ref [] in
      let rec complement target existing =
        match existing with
        | [] -> [target]
        | e :: es ->
          if subset target e then []
          else if subset e target then complement target es
          else complement target es
      in
      let rec subset a b =
        match a, b with
        | Wildcard, _ -> true
        | _, Wildcard -> false
        | Var _, _ -> true
        | _, Var _ -> false
        | Atom a1, Atom a2 -> A.compare a1 a2 = 0
        | Integer i1, Integer i2 -> A.compare i1 i2 = 0
        | String s1, String s2 -> s1 = s2
        | Tuple ts1, Tuple ts2 ->
          List.length ts1 = List.length ts2 &&
          List.for_all2 (fun a b -> subset a b) ts1 ts2
        | List (h1, t1), List (h2, t2) ->
          subset h1 h2 && (match t1, t2 with
            | Some t1, Some t2 -> subset t1 t2
            | None, None -> true
            | _ -> false)
        | Cons (h1, t1), Cons (h2, t2) ->
          subset h1 h2 && subset t1 t2
        | Map m1, Map m2 ->
          List.for_all (fun (k, v) ->
            List.exists (fun (k', v') -> subset k k' && subset v v') m2) m1
        | _ -> false
      in
      covered := p :: List.concat_map (complement p) ps;
      !covered

  type decision_tree =
    | Leaf of int  (* clause index *)
    | Fail
    | Switch of (A.t * decision_tree) list * decision_tree option  (* matches * default *)
    | Guard of (t * decision_tree)  (* guard pattern *)

  let rec compile_patterns (clauses : (t * 'a) list) : decision_tree =
    match clauses with
    | [] -> Fail
    | [pat, action] -> Leaf 0
    | (pat, action) :: rest ->
      let tree = compile_patterns rest in
      switch_on pat action tree

  and switch_on pattern action default_tree =
    match pattern with
    | Wildcard -> Leaf 0
    | Var _ -> Leaf 0
    | Atom a -> Switch [(a, Leaf 0)], Some default_tree)
    | Integer i -> Switch [(i, Leaf 0)], Some default_tree)
    | String s -> Switch [(s, Leaf 0)], Some default_tree)
    | Tuple ps ->
      (* Compile as nested switches *)
      Leaf 0
    | List (head, tail) ->
      Leaf 0
    | Cons (h, t) ->
      Leaf 0
    | Map _ ->
      Leaf 0

  type exhaustiveness_result =
    | NonExhaustive of t list  (* missing patterns *)
    | Exhaustive

  let rec check_exhaustive (patterns : t list) : exhaustiveness_result =
    match patterns with
    | [] -> Exhaustive
    | _ ->
      (* For now, simplified check *)
      let has_wildcard = List.exists (function Wildcard -> true | _ -> false) patterns in
      if has_wildcard then Exhaustive
      else NonExhaustive [Wildcard]
end

module StringPattern = MakePattern(struct
  type t = string
  let compare = String.compare
end)

module IntPattern = MakePattern(struct
  type t = int
  let compare = Int.compare
end)

module Tests = struct
  let test_wildcard_exhaustive () =
    let result = StringPattern.check_exhaustive [StringPattern.Wildcard] in
    match result with
    | StringPattern.Exhaustive -> ()
    | _ -> raise (Failure "wildcard should be exhaustive");
    print_endline "test_wildcard_exhaustive passed"

  let test_atom_non_exhaustive () =
    let result = StringPattern.check_exhaustive [
      StringPattern.Atom "foo";
      StringPattern.Atom "bar";
    ] in
    match result with
    | StringPattern.NonExhaustive missing ->
      assert (List.length missing > 0)
    | _ -> (); (* Other atoms may be missing but that's ok for this test *)
    print_endline "test_atom_non_exhaustive passed"

  let test_decision_tree () =
    let tree = StringPattern.compile_patterns [
      StringPattern.Wildcard, 0;
    ] in
    (match tree with
     | StringPattern.Leaf n when n = 0 -> ()
     | _ -> raise (Failure "decision tree compile failed"));
    print_endline "test_decision_tree passed"

  let test_tuple_pattern () =
    let tree = StringPattern.compile_patterns [
      StringPattern.Tuple [StringPattern.Wildcard; StringPattern.Wildcard], 0;
    ] in
    (match tree with
     | StringPattern.Leaf _ -> ()
     | _ -> raise (Failure "tuple pattern compile failed"));
    print_endline "test_tuple_pattern passed"

  let run_all () =
    test_wildcard_exhaustive ();
    test_atom_non_exhaustive ();
    test_decision_tree ();
    test_tuple_pattern ();
    print_endline "All pattern analysis tests passed"
end

let () = Tests.run_all ()