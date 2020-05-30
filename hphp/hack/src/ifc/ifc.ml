(* Copyright (c) 2020, Facebook, Inc.
   All rights reserved. *)
open Hh_prelude
open Hh_core
open Ifc_types
module Env = Ifc_env
module Logic = Ifc_logic
module Decl = Ifc_decl
module Pp = Ifc_pretty
module A = Aast
module T = Typing_defs
module L = Logic.Infix

exception FlowInference of string

let fail s = raise (FlowInference s)

(* A constraint accumulator that registers a subtyping
   requirement t1 <: t2 *)
let rec subtype t1 t2 acc =
  match (t1, t2) with
  | (Tprim p1, Tprim p2) -> L.(p1 < p2) acc
  | (Ttuple tl1, Ttuple tl2) ->
    (match List.zip tl1 tl2 with
    | Some zip ->
      List.fold zip ~init:acc ~f:(fun acc (t1, t2) -> subtype t1 t2 acc)
    | None -> fail "incompatible tuple types")
  | (Tclass (name, pol, prop_pol_map), Tclass (name', pol', prop_pol_map'))
    when String.equal name name' ->
    let pol_zip =
      match List.zip (SMap.values prop_pol_map) (SMap.values prop_pol_map') with
      | Some zip -> zip
      | None -> fail "Same class with differing policied properties"
    in
    (* Invariant in property policies *)
    List.fold pol_zip ~init:acc ~f:(fun acc (t1, t2) -> equivalent t1 t2 acc)
    (* Covariant in class policy *)
    |> L.(pol < pol')
  | _ -> fail "unhandled subtyping query"

(* A constraint accumulator that registers that t1 = t2 *)
and equivalent t1 t2 acc = subtype t1 t2 (subtype t2 t1 acc)

(* Generates a fresh supertype of the argument type *)
let weaken stk env ty =
  let rec freshen acc ty =
    let on_list mk tl =
      let (acc, tl') = List.map_env acc tl ~f:freshen in
      (acc, mk tl')
    in
    match ty with
    | Tprim p ->
      let p' = Env.new_policy_var stk in
      (L.(p < p') acc, Tprim p')
    | Ttuple tl -> on_list (fun l -> Ttuple l) tl
    | Tunion tl -> on_list (fun l -> Tunion l) tl
    | Tinter tl -> on_list (fun l -> Tinter l) tl
    | Tclass (name, pol, prop_pol_map) ->
      let super_pol = Env.new_policy_var stk in
      let acc = L.(pol < super_pol) acc in
      (acc, Tclass (name, super_pol, prop_pol_map))
  in
  let (acc, ty') = freshen env.e_acc ty in
  (Env.acc env (fun _ -> acc), ty')

(* A constraint accumulator registering that the type t depends on
   policies in the list pl (seen differently, the policies in pl
   flow into the type t) *)
let rec add_dependencies pl t acc =
  match t with
  | Tinter [] ->
    (* The policy p flows into a value of type mixed.
       Here we choose that mixed will be public; we
       could choose a different default policy or
       have mixed carry a policy (preferable). *)
    L.(pl <* [Pbot]) acc
  | Tprim pol
  (* For classes, we only add a dependency to the class policy and not to its
     properties *)
  | Tclass (_, pol, _) ->
    L.(pl <* [pol]) acc
  | Tunion tl
  | Ttuple tl
  | Tinter tl ->
    let f acc t = add_dependencies pl t acc in
    List.fold tl ~init:acc ~f

let policy_join stk env p1 p2 =
  match Logic.policy_join p1 p2 with
  | Some p -> (env, p)
  | None ->
    (match (p1, p2) with
    | (Pfree_var (v1, s1), Pfree_var (v2, s2))
      when equal_policy_var v1 v2 && Scope.equal s1 s2 ->
      (env, p1)
    | _ ->
      let pv = Env.new_policy_var stk in
      let env = Env.acc env L.(p1 < pv && p2 < pv) in
      (env, pv))

exception NoUnion

let rec union_types_exn stk env t1 t2 =
  let union_types_exn = union_types_exn stk in
  match (t1, t2) with
  | (Tprim p1, Tprim p2) ->
    let (env, p) = policy_join stk env p1 p2 in
    (env, Tprim p)
  | (Ttuple tl1, Ttuple tl2) ->
    let tpairs =
      match List.zip tl1 tl2 with
      | Some l -> l
      | None -> raise NoUnion
    in
    let (env, tl) =
      let f env (t1, t2) = union_types_exn env t1 t2 in
      List.map_env env tpairs ~f
    in
    (env, Ttuple tl)
  | (Tunion u, t)
  | (t, Tunion u) ->
    let rec try_union env t = function
      | tu :: u ->
        (try
           let (env, t') = union_types_exn env tu t in
           (env, t' :: u)
         with NoUnion ->
           let (env, u') = try_union env t u in
           (env, tu :: u'))
      | [] -> (env, [t])
    in
    let (env, u) = try_union env t u in
    (env, Tunion u)
  | (Tinter i, t)
  | (t, Tinter i) ->
    let (env, i') =
      let f env = union_types_exn env t in
      List.map_env env i ~f
    in
    (env, Tinter i')
  | ( Tclass (name1, class_pol1, prop_ptype_map1),
      Tclass (name2, class_pol2, prop_ptype_map2) )
    when String.equal name1 name2 ->
    let (env, class_pol) = policy_join stk env class_pol1 class_pol2 in
    let (env, prop_ptype_map) =
      let combine env prop_name ptype1_opt ptype2_opt =
        match (ptype1_opt, ptype2_opt) with
        | (Some ptype1, Some ptype2) ->
          let e_acc = equivalent ptype1 ptype2 env.e_acc in
          ({ env with e_acc }, Some ptype1)
        | (_, _) ->
          fail
            ( "Impossible happened."
            ^ " One of the two property types of the same class do not have the property '"
            ^ prop_name
            ^ "'." )
      in
      SMap.merge_env env prop_ptype_map1 prop_ptype_map2 ~combine
    in
    (env, Tclass (name1, class_pol, prop_ptype_map))
  | _ -> raise NoUnion

let mk_union t1 t2 =
  match (t1, t2) with
  | (Tunion u1, Tunion u2) -> Tunion (u1 @ u2)
  | (Tunion u, t)
  | (t, Tunion u) ->
    Tunion (t :: u)
  | _ -> Tunion [t1; t2]

let union_types stk env t1 t2 =
  (try union_types_exn stk env t1 t2 with NoUnion -> (env, mk_union t1 t2))

let rec class_ptype stk name =
  let { psig_policied_properties } =
    match SMap.find_opt name stk.e_psig_env with
    | Some class_policy_sig -> class_policy_sig
    | None -> fail ("Could not found a class policy signature for " ^ name)
  in
  let class_policy = Env.new_policy_var stk in
  let policy_map =
    List.map psig_policied_properties (fun (prop, ty) -> (prop, ptype stk ty))
    |> SMap.of_list
  in
  Tclass (name, class_policy, policy_map)

(* Turns a locl_ty into a type with policy annotations;
   the policy annotations are fresh policy variables *)
and ptype stk (t : T.locl_ty) =
  match T.get_node t with
  | T.Tprim _ -> Tprim (Env.new_policy_var stk)
  | T.Ttuple tyl -> Ttuple (List.map ~f:(ptype stk) tyl)
  | T.Tunion tyl -> Tunion (List.map ~f:(ptype stk) tyl)
  | T.Tintersection tyl -> Tinter (List.map ~f:(ptype stk) tyl)
  | T.Tclass ((_, name), _, _) -> class_ptype stk name
  (* ---  types below are not yet unpported *)
  | T.Tdependent (_, _ty) -> fail "Tdependent"
  | T.Tdarray (_keyty, _valty) -> fail "Tdarray"
  | T.Tvarray _ty -> fail "Tvarray"
  | T.Tvarray_or_darray (_keyty, _valty) -> fail "Tvarray_or_darray"
  | T.Tany _sentinel -> fail "Tany"
  | T.Terr -> fail "Terr"
  | T.Tnonnull -> fail "Tnonnull"
  | T.Tdynamic -> fail "Tdynamic"
  | T.Toption _ty -> fail "Toption"
  | T.Tfun _fun_ty -> fail "Tfun"
  | T.Tshape (_sh_kind, _sh_type_map) -> fail "Tshape"
  | T.Tvar _id -> fail "Tvar"
  | T.Tgeneric _name -> fail "Tgeneric"
  | T.Tnewtype (_name, _ty_list, _as_bound) -> fail "Tnewtype"
  | T.Tobject -> fail "Tobject"
  | T.Tpu (_locl_ty, _sid) -> fail "Tpu"
  | T.Tpu_type_access (_sid1, _sid2) -> fail "Tpu_type_access"

let add_params stk env params =
  let add_param env p =
    let pty = ptype stk (fst p.A.param_type_hint) in
    let lid = Local_id.make_unscoped p.A.param_name in
    Env.set_local_type env lid pty
  in
  List.fold params ~init:env ~f:add_param

type lvalue =
  | Local of A.local_id
  | Property of ptype

let binop stk env ty1 ty2 =
  match (ty1, ty2) with
  | (Tprim p1, Tprim p2) ->
    let (env, pj) = policy_join stk env p1 p2 in
    (env, Tprim pj)
  | _ -> fail "unexpected Binop types"

let property_ptype obj_ptype property =
  let get_property prop_ptype_map =
    match SMap.find_opt property prop_ptype_map with
    | Some ptype -> ptype
    | None -> Tprim Pbot
  in
  match obj_ptype with
  | Tclass (_, _, prop_ptype_map) -> get_property prop_ptype_map
  | _ ->
    fail
      ( "Impossible happened."
      ^ " Couldn't find a class for the property '"
      ^ property
      ^ "'." )

let rec lvalue stk env (((_epos, _ety), e) : Tast.expr) =
  match e with
  | A.Lvar (_pos, lid) -> (env, Local lid)
  | A.Obj_get (obj, (_, A.Id (_, property)), _) ->
    let (env, obj_ptype) = expr stk env obj in
    let obj_pol =
      match obj_ptype with
      | Tclass (_, obj_pol, _) -> obj_pol
      | _ -> fail ("Couldn't find a class for the property '" ^ property ^ "'.")
    in
    let prop_ptype = property_ptype obj_ptype property in
    let env = Env.acc env (add_dependencies [obj_pol] prop_ptype) in
    (env, Property prop_ptype)
  | _ -> fail "unsupported lvalue"

(* Generate flow constraints for an expression *)
and expr stk env (((_epos, _ety), e) : Tast.expr) =
  let expr = expr stk in
  match e with
  | A.True
  | A.False
  | A.Int _
  | A.Float _
  | A.String _ ->
    (* literals are public *)
    (env, Tprim Pbot)
  | A.Binop (Ast_defs.Eq op, e1, e2) ->
    let (env, tyo) = lvalue stk env e1 in
    let (env, ty) = expr env e2 in
    let env =
      match tyo with
      | Local lid ->
        let (env, ty) =
          if Option.is_none op then
            (env, ty)
          else
            let lty = Env.get_local_type env lid in
            binop stk env lty ty
        in
        let (env, ty) = weaken stk env ty in
        let env = Env.acc env (add_dependencies stk.s_lpc ty) in
        Env.set_local_type env lid ty
      | Property prop_ptype ->
        let (env, ty) =
          if Option.is_none op then
            (env, ty)
          else
            binop stk env prop_ptype ty
        in
        let env = Env.acc env (subtype ty prop_ptype) in
        Env.acc env (add_dependencies stk.s_gpc prop_ptype)
    in
    (env, ty)
  | A.Binop (_, e1, e2) ->
    let (env, ty1) = expr env e1 in
    let (env, ty2) = expr env e2 in
    binop stk env ty1 ty2
  | A.Lvar (_pos, lid) -> (env, Env.get_local_type env lid)
  | A.Obj_get (obj, (_, A.Id (_, property)), _) ->
    let (env, obj_ptype) = expr env obj in
    let prop_ptype = property_ptype obj_ptype property in
    let (env, super_ptype) = weaken stk env prop_ptype in
    let env =
      match obj_ptype with
      | Tclass (_, obj_pol, _) ->
        Env.acc env (add_dependencies [obj_pol] super_ptype)
      | _ -> fail ("Couldn't find a class for the property '" ^ property ^ "'.")
    in
    (env, super_ptype)
  | A.This ->
    (match env.e_this with
    | Some ptype -> (env, ptype)
    | None -> fail "Encountered $this outside of a class context")
  | A.BracedExpr e -> expr env e
  (* --- expressions below are not yet supported *)
  | A.Array _
  | A.Darray (_, _)
  | A.Varray (_, _)
  | A.Shape _
  | A.ValCollection (_, _, _)
  | A.KeyValCollection (_, _, _)
  | A.Null
  | A.Omitted
  | A.Id _
  | A.Dollardollar _
  | A.Clone _
  | A.Obj_get (_, _, _)
  | A.Array_get (_, _)
  | A.Class_get (_, _)
  | A.Class_const (_, _)
  | A.Call (_, _, _, _, _)
  | A.FunctionPointer (_, _)
  | A.String2 _
  | A.PrefixedString (_, _)
  | A.Yield _
  | A.Yield_break
  | A.Yield_from _
  | A.Await _
  | A.Suspend _
  | A.List _
  | A.Expr_list _
  | A.Cast (_, _)
  | A.Unop (_, _)
  | A.Pipe (_, _, _)
  | A.Eif (_, _, _)
  | A.Is (_, _)
  | A.As (_, _, _)
  | A.New (_, _, _, _, _)
  | A.Record (_, _, _)
  | A.Efun (_, _)
  | A.Lfun (_, _)
  | A.Xml (_, _, _)
  | A.Callconv (_, _)
  | A.Import (_, _)
  | A.Collection (_, _, _)
  | A.ParenthesizedExpr _
  | A.Lplaceholder _
  | A.Fun_id _
  | A.Method_id (_, _)
  | A.Method_caller (_, _)
  | A.Smethod_id (_, _)
  | A.Pair _
  | A.Assert _
  | A.PU_atom _
  | A.PU_identifier (_, _, _)
  | A.Any ->
    fail "expr"

let rec stmt stk env ((_pos, s) : Tast.stmt) =
  match s with
  | A.Expr e ->
    let (env, _ty) = expr stk env e in
    env
  | A.If (e, b1, b2) ->
    let (env, ety) = expr stk env e in
    let stk' =
      let epol =
        match ety with
        | Tprim p -> p
        | _ -> fail "condition expression must be of type bool"
      in
      { stk with s_lpc = epol :: stk.s_lpc; s_gpc = epol :: stk.s_gpc }
    in
    let cenv = Env.get_cenv env in
    let env = block stk' (Env.set_cenv env cenv) b1 in
    let cenv1 = Env.get_cenv env in
    let env = block stk' (Env.set_cenv env cenv) b2 in
    let cenv2 = Env.get_cenv env in
    let union = union_types stk in
    Env.merge_and_set_cenv ~union env cenv1 cenv2
  | A.Return (Some e) ->
    let (env, te) = expr stk env e in
    Env.acc env (subtype te env.e_ret)
  | A.Return None -> env
  | _ -> env

and block stk env (blk : Tast.block) =
  List.fold_left ~f:(stmt stk) ~init:env blk

let func_or_method psig_env class_name_opt name saved_env params body lrty =
  begin
    try
      let scope = Scope.alloc () in
      let stk = Env.new_stk psig_env scope in
      let this_ty = Option.map class_name_opt (class_ptype stk) in
      let ret = ptype stk lrty in
      let env = Env.new_env saved_env this_ty ret in
      let env = add_params stk env params in
      let beg_env = env in
      let env = block stk env body.A.fb_ast in
      let end_env = env in
      Format.printf "Analyzing %s:@." name;
      Format.printf "* @[<hov2>Params:@ %a@]@." Pp.locals beg_env;
      Format.printf "* Final env:@.  %a@." Pp.env end_env
    with FlowInference s -> Format.printf "  Failure: %s@." s
  end;
  Format.printf "@."

let walk_tast psig_env =
  let def = function
    | A.Fun
        {
          A.f_name = (_, name);
          f_annotation = saved_env;
          f_params = params;
          f_body = body;
          f_ret = (lrty, _);
          _;
        } ->
      func_or_method psig_env None name saved_env params body lrty
    | A.Class { A.c_name = (_, class_name); c_methods = methods; _ } ->
      let handle_method
          {
            A.m_name = (_, name);
            m_annotation = saved_env;
            m_params = params;
            m_body = body;
            m_ret = (lrty, _);
            _;
          } =
        func_or_method
          psig_env
          (Some class_name)
          name
          saved_env
          params
          body
          lrty
      in
      List.iter methods ~f:handle_method
    | _ -> ()
  in
  List.iter ~f:def

let do_ files_info opts ctx =
  Relative_path.Map.iter files_info ~f:(fun path i ->
      (* skip decls and partial *)
      match i.FileInfo.file_mode with
      | Some FileInfo.Mstrict ->
        let (ctx, entry) = Provider_context.add_entry_if_missing ~ctx ~path in
        let { Tast_provider.Compute_tast.tast; _ } =
          Tast_provider.compute_tast_unquarantined ~ctx ~entry
        in
        let psig_env = Decl.collect_class_policy_sigs tast in
        Format.printf "%a" Pp.policy_sig_env psig_env;

        if String.equal opts "prtast" then
          Format.printf "TAST: %a@." Tast.pp_program tast;

        walk_tast psig_env tast
      | _ -> ());
  ()

let magic_builtins =
  [|
    ( "ifc_magic.hhi",
      {|<?hh // strict
class Policied implements HH\InstancePropertyAttribute, HH\ClassAttribute { }
|}
    );
  |]
