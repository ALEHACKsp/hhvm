(**
 * Copyright (c) 2015, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

 open Typing_defs
 module TySet = Typing_set
 module type S = sig

 (* Local environment includes types of locals and bounds on type parameters. *)
 type local_env = {
   per_cont_env       : Typing_per_cont_env.t;
   local_mutability   : Typing_mutability_env.mutability_env;
   local_reactive : reactivity;
   (* Local variables that were assigned in a `using` clause *)
   local_using_vars   : Local_id.Set.t;
 }

 type tyvar_info = {
   tyvar_pos: Pos.t;
   eager_solve_fail: bool;
   appears_covariantly: bool;
   appears_contravariantly: bool;
   lower_bounds : TySet.t;
   upper_bounds : TySet.t;
   type_constants : (Aast.sid * locl ty) SMap.t;
 }
 type tvenv = tyvar_info IMap.t

 type env = {
   (* position of the function/method being checked *)
   function_pos: Pos.t  ;
   tenv    : locl ty IMap.t ;
   subst   : int IMap.t ;
   fresh_typarams : SSet.t;
   lenv    : local_env  ;
   genv    : genv       ;
   decl_env: Decl_env.env;
   in_loop : bool       ;
   in_try  : bool       ;
   in_case : bool       ;
   inside_constructor: bool;
   inside_ppl_class: bool;
   (* A set of constraints that are global to a given method *)
   global_tpenv : Type_parameter_env.t;
   subtype_prop : Typing_logic.subtype_prop;
   log_levels : int SMap.t;
   tvenv : tvenv;
   tyvars_stack : (Pos.t * Ident.t list) list;
   allow_wildcards : bool;
   big_envs : (Pos.t * env) list ref ;
 }
and genv = {
  tcopt   : TypecheckerOptions.t;
  return  : Typing_env_return_info.t;
  (* For each function parameter, its type and calling convention. *)
  params  : (locl ty * param_mode) Local_id.Map.t;
  (* condition types associated with parameters.
     For every mayberx parameter that has condition type we create
     fresh type parameter (see: make_local_param_ty) and store mapping
     fresh type name -> condition type in env so it can be retrieved later *)
  condition_types: decl ty SMap.t;
  parent_id : string;
  parent  : decl ty;
  (* Identifier of the enclosing class *)
  self_id : string;
  (* Type of the enclosing class, instantiated at its generic parameters *)
  self    : locl ty;
  static  : bool;
  fun_kind : Ast_defs.fun_kind;
  val_kind : Typing_defs.val_kind;
  fun_mutable : param_mutability option;
  anons   : anon IMap.t;
  file    : Relative_path.t;
}

(* A type-checker for an anonymous function
 * Parameters are
 * - the environment
 * - types of the parameters under which the body should be checked
 * - the arity of the function
 * - the expected return type of the body (optional)
 *)
and anon_log = locl ty list * locl ty list
and anon = {
  rx : reactivity;
  is_coroutine : Aast.is_coroutine;
  counter : anon_log ref;
  pos : Pos.t;
  typecheck :
    ?el:Nast.expr list ->
    ?ret_ty: locl ty ->
    env ->
    locl fun_params ->
    locl fun_arity ->
    env * Tast.expr * locl ty;
}

end
