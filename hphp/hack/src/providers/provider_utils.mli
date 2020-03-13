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
    telemetry: Telemetry.t;
  }
end

module Compute_tast_and_errors : sig
  type t = {
    tast: Tast.program;
    errors: Errors.t;
    telemetry: Telemetry.t;
  }
end

(** Construct a [Provider_context.t] from the configuration information
contained within a [ServerEnv.env]. *)
val ctx_from_server_env : ServerEnv.env -> Provider_context.t

(** Read the contents at [path] from disk and create a new
[Provider_context.entry] representing that file. The returned
[Provider_context.t] contains that new entry.

If an [entry] is already present for the given [path], then this function
overwrites that entry in the returned [Provider_context.t]. If you want to see
if an entry already exists, you can use [Provider_utils.find_entry].

It's important to pass around the resulting [Provider_context.t]. That way, if a
subsequent operation tries to access data about the same file, it will be
returned the same [entry]. *)
val add_entry :
  ctx:Provider_context.t ->
  path:Relative_path.t ->
  Provider_context.t * Provider_context.entry

(** Same as [add_entry], but using the provided file contents. This is primarily
useful in the IDE (which may have unsaved changes), or for testing (to pretend
that a certain file exists on disk). *)
val add_entry_from_file_contents :
  ctx:Provider_context.t ->
  path:Relative_path.t ->
  contents:string ->
  Provider_context.t * Provider_context.entry

(** Same as [add_entry], but using the provided [ServerCommandTypes.file_input].
This is useful in some IDE code paths. *)
val add_entry_from_file_input :
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
