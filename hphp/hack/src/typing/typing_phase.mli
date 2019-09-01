[@@@warning "-33"]
open Core_kernel
open Common
[@@@warning "+33"]
open Typing_defs
open Typing_env_types

type method_instantiation =
{
  use_pos: Pos.t;
  use_name: string;
  explicit_targs: decl ty list;
}

val env_with_self:
  env ->
  expand_env
val localize_with_self:
  env ->
  decl ty ->
  env * locl ty
val localize_with_self_possibly_enforceable:
  env ->
  decl ty ->
  env * locl possibly_enforced_ty
val localize:
  ety_env:expand_env ->
  env ->
  decl ty ->
  env * locl ty
val localize_ft:
  ?instantiation:method_instantiation ->
  ety_env:expand_env ->
  env ->
  decl fun_type ->
  env * locl fun_type
val localize_hint_with_self:
  env ->
  Aast.hint ->
  env * locl ty
val localize_hint:
  ety_env:expand_env ->
  env ->
  Aast.hint ->
  env * locl ty
val localize_generic_parameters_with_bounds:
  ety_env:expand_env ->
  env ->
  decl tparam list ->
  env * (locl ty * Ast_defs.constraint_kind * locl ty) list
val localize_where_constraints:
  ety_env:expand_env ->
  env ->
  Aast.where_constraint list ->
  env
val localize_with_dty_validator:
  env ->
  decl ty ->
  (expand_env -> decl ty -> unit) ->
  env * locl ty
val sub_type_decl:
  env ->
  decl ty ->
  decl ty ->
  Errors.typing_error_callback ->
  unit
val unify_decl:
  env ->
  decl ty ->
  decl ty ->
  Errors.typing_error_callback ->
  unit
val check_tparams_constraints:
  use_pos:Pos.t ->
  ety_env:expand_env ->
  env ->
  decl tparam list ->
  env
val check_where_constraints:
  in_class:bool ->
  use_pos:Pos.t ->
  ety_env:expand_env ->
  definition_pos:Pos.t ->
  env ->
  decl where_constraint list ->
  env
val decl:
  decl ty ->
  phase_ty
val locl:
  locl ty ->
  phase_ty
