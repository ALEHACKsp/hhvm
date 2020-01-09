(*
 * Copyright (c) 2015, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

(* cf: typing_env_types_sig.mli - These files should be the same *)
open Hh_prelude
open Typing_defs
module TPEnv = Type_parameter_env
module ITySet = Internal_type_set

type locl_ty = Typing_defs.locl_ty

[@@@warning "-32"]

let show_local_id_set_t _ = "<local_id_set_t>"

let pp_local_id_set_t _ _ = Printf.printf "%s\n" "<local_id_set_t>"

type local_id_set_t = Local_id.Set.t

let show_local_env _ = "<local_env>"

let pp_local_env _ _ = Printf.printf "%s\n" "<local_env>"

(* Local environment includes types of locals and bounds on type parameters. *)
type local_env = {
  per_cont_env: Typing_per_cont_env.t;
  local_mutability: Typing_mutability_env.mutability_env;
  (* Whether current environment is reactive *)
  local_reactive: reactivity;
  (* Local variables that were assigned in a `using` clause *)
  local_using_vars: local_id_set_t;
}

let show_env _ = "<env>"

let pp_env _ _ = Printf.printf "%s\n" "<env>"

let show_genv _ = "<genv>"

let pp_genv _ _ = Printf.printf "%s\n" "<genv>"

let show_anon _ = "<anon>"

let pp_anon _ _ = Printf.printf "%s\n" "<anon>"

let show_tfun _ = "<tfun>"

let pp_tfun _ _ = Printf.printf "%s\n" "<tfun>"

[@@@warning "+32"]

type env = {
  (* position of the function/method being checked *)
  function_pos: Pos.t;
  tyvar_occurrences: Typing_tyvar_occurrences.t;
  fresh_typarams: SSet.t;
  lenv: local_env;
  genv: genv;
  decl_env: Decl_env.env;
  in_loop: bool;
  in_try: bool;
  in_case: bool;
  inside_constructor: bool;
  inside_ppl_class: bool;
  (* A set of constraints that are global to a given method *)
  global_tpenv: TPEnv.t;
  log_levels: int SMap.t;
  inference_env: Typing_inference_env.t;
  allow_wildcards: bool;
  big_envs: (Pos.t * env) list ref;
  pessimize: bool;
}

and genv = {
  tcopt: TypecheckerOptions.t;
  return: Typing_env_return_info.t;
  (* For each function parameter, its type and calling convention. *)
  params: (locl_ty * param_mode) Local_id.Map.t;
  (* condition types associated with parameters.
     For every mayberx parameter that has condition type we create
     fresh type parameter (see: make_local_param_ty) and store mapping
     fresh type name -> condition type in env so it can be retrieved later *)
  condition_types: decl_ty SMap.t;
  (* Identifier and type of the parent class if it exists *)
  parent: (string * decl_ty) option;
  (* Identifier and type (instatiated at its generic parameters) of
     the enclosing class if there is one *)
  self: (string * locl_ty) option;
  static: bool;
  fun_kind: Ast_defs.fun_kind;
  val_kind: Typing_defs.val_kind;
  fun_mutable: param_mutability option;
  anons: anon IMap.t;
  file: Relative_path.t;
}

(* A type-checker for an anonymous function
 * Parameters are
 * - the environment
 * - types of the parameters under which the body should be checked
 * - the arity of the function
 * - the expected return type of the body (optional)
 *)
and anon_log = locl_ty list * locl_ty list

and anon = {
  rx: reactivity;
  is_coroutine: Aast.is_coroutine;
  counter: anon_log ref;
  pos: Pos.t;
  typecheck:
    ?el:Nast.expr list ->
    ?ret_ty:locl_ty ->
    env ->
    locl_fun_params ->
    locl_fun_arity ->
    env * Tast.expr * locl_ty;
}

let env_reactivity env = env.lenv.local_reactive
