(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

module Compute_tast : sig
  type t = {
    tast: Tast.program;
    decl_cache_misses: int;
  }
end

module Compute_tast_and_errors : sig
  type t = {
    tast: Tast.program;
    errors: Errors.t;
    decl_cache_misses: int;
  }
end

(** Compute the given AST for the given path, and return an updated [t]
containing that entry. *)
val update_context :
  ctx:Provider_context.t ->
  path:Relative_path.t ->
  file_input:ServerCommandTypes.file_input ->
  Provider_context.t * Provider_context.entry

(** Load the declarations of [t] into any global-memory storage, then call
[f], then unload those declarations. Quarantine is REQUIRED in sIDE and
hh_server scenarios because it embodies local-file-changes and the naming-
table updates therein, and if you try to typecheck a local files without
those updates them it will often fail. Quarantine is INAPPROPRIATE in
mapreduce or other bulk-checking scenarios which operate solely off
files-on-disk and have no concept of unsaved-file-changes.
TODO: It's a bit confusing that quarantining is predicated upon ctx, and
hopefully we'll remove that dependency in future. *)
val respect_but_quarantine_unsaved_changes :
  ctx:Provider_context.t -> f:(unit -> 'a) -> 'a

(** Computes TAST and error-list by taking the AST in a context entry,
and typechecking it, and memoizing the result (caching the results in the
context entry). CAUTION: this function doesn't use a quarantine, and so
is inappropriate for IDE scenarios. *)
val compute_tast_and_errors_unquarantined :
  ctx:Provider_context.t ->
  entry:Provider_context.entry ->
  Compute_tast_and_errors.t

(** Same as [compute_tast_and_errors_unquarantined], but skips computing the
full error list. If the errors are needed at a later time, you'll have to incur
the full cost of recomputing the entire TAST and errors. *)
val compute_tast_unquarantined :
  ctx:Provider_context.t -> entry:Provider_context.entry -> Compute_tast.t

(** This function computes TAST and error-list. At the moment,
the suffix "quarantined" means that this function enforces a quarantine
in case one isn't yet in force. In future, it might mean that we assert
that a quarantine is already in force. CAUTION: this function is only
appropriate for IDE scenarios. *)
val compute_tast_and_errors_quarantined :
  ctx:Provider_context.t ->
  entry:Provider_context.entry ->
  Compute_tast_and_errors.t

(** Same as [compute_tast_and_errors_quarantined], but skips computing the full
error list. If the errors are needed at a later time, you'll have to incur the
full cost of recomputing the entire TAST and errors. *)
val compute_tast_quarantined :
  ctx:Provider_context.t -> entry:Provider_context.entry -> Compute_tast.t

(** Find an existing entry within the context.  Returns "None" if
that entry has not yet been observed. *)
val find_entry :
  ctx:Provider_context.t ->
  path:Relative_path.t ->
  Provider_context.entry option

(** Retrieve an existing memoized entry from the context, or fetch the
information from disk.  Does not modify the context.

This function is marked VOLATILE because it has the potential to read
data from disk.  Calling this function multiple times in a row may produce
different results if the data on disk changes.

Please only use this function to replace code that already reads data from
disk.
*)
val get_entry_VOLATILE :
  ctx:Provider_context.t -> path:Relative_path.t -> Provider_context.entry

(** Compute the CST *)
val compute_cst :
  ctx:Provider_context.t ->
  entry:Provider_context.entry ->
  Provider_context.PositionedSyntaxTree.t
