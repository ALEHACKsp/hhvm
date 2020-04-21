(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

open Hh_prelude
module PositionedSyntaxTree =
  Full_fidelity_syntax_tree.WithSyntax (Full_fidelity_positioned_syntax)

type entry = {
  path: Relative_path.t;
  contents: string;
  mutable source_text: Full_fidelity_source_text.t option;
  mutable parser_return: Parser_return.t option;
  mutable ast_errors: Errors.t option;
  mutable cst: PositionedSyntaxTree.t option;
  mutable tast: Tast.program option;
  mutable naming_and_typing_errors: Errors.t option;
  mutable symbols: Relative_path.t SymbolOccurrence.t list option;
}

type entries = entry Relative_path.Map.t

type t = {
  popt: ParserOptions.t;
  tcopt: TypecheckerOptions.t;
  backend: Provider_backend.t;
  entries: entries;
}

let empty_for_tool ~popt ~tcopt ~backend =
  { popt; tcopt; backend; entries = Relative_path.Map.empty }

let empty_for_worker ~popt ~tcopt =
  {
    popt;
    tcopt;
    backend = Provider_backend.Shared_memory;
    entries = Relative_path.Map.empty;
  }

let empty_for_test ~popt ~tcopt =
  {
    popt;
    tcopt;
    backend = Provider_backend.Shared_memory;
    entries = Relative_path.Map.empty;
  }

let empty_for_debugging ~popt ~tcopt =
  {
    popt;
    tcopt;
    backend = Provider_backend.Shared_memory;
    entries = Relative_path.Map.empty;
  }

let make_entry ~(path : Relative_path.t) ~(contents : string) : entry =
  {
    path;
    contents;
    source_text = None;
    parser_return = None;
    ast_errors = None;
    cst = None;
    tast = None;
    naming_and_typing_errors = None;
    symbols = None;
  }

let add_or_overwrite_entry ~(ctx : t) (entry : entry) : t =
  { ctx with entries = Relative_path.Map.add ctx.entries entry.path entry }

let add_or_overwrite_entry_contents
    ~(ctx : t) ~(path : Relative_path.t) ~(contents : string) : t * entry =
  let entry = make_entry ~path ~contents in
  (add_or_overwrite_entry ctx entry, entry)

let add_entry_if_missing ~(ctx : t) ~(path : Relative_path.t) : t * entry =
  match Relative_path.Map.find_opt ctx.entries path with
  | Some entry -> (ctx, entry)
  | None ->
    let contents = Sys_utils.cat (Relative_path.to_absolute path) in
    let entry = make_entry ~path ~contents in
    (add_or_overwrite_entry ctx entry, entry)

let get_popt (t : t) : ParserOptions.t = t.popt

let get_tcopt (t : t) : TypecheckerOptions.t = t.tcopt

let map_tcopt (t : t) ~(f : TypecheckerOptions.t -> TypecheckerOptions.t) : t =
  { t with tcopt = f t.tcopt }

let get_backend (t : t) : Provider_backend.t = t.backend

let get_entries (t : t) : entries = t.entries

(** ref_is_quarantined stores the stack at which it was last changed,
so we can give better failwith error messages where appropriate. *)
let ref_is_quarantined : (bool * Utils.callstack) ref =
  ref (false, Utils.Callstack "init")

let is_quarantined () : bool = !ref_is_quarantined |> fst

let set_is_quarantined_internal () : unit =
  match !ref_is_quarantined with
  | (true, Utils.Callstack stack) ->
    failwith ("set_is_quarantined: was already quarantined at\n" ^ stack)
  | (false, _) ->
    ref_is_quarantined :=
      (true, Utils.Callstack (Exception.get_current_callstack_string 99))

let unset_is_quarantined_internal () : unit =
  match !ref_is_quarantined with
  | (true, _) ->
    ref_is_quarantined :=
      (false, Utils.Callstack (Exception.get_current_callstack_string 99))
  | (false, Utils.Callstack stack) ->
    failwith
      ( "unset_is_quarantined: but quarantine had already been released at\n"
      ^ stack )

let get_telemetry (t : t) : Telemetry.t =
  let telemetry =
    Telemetry.create ()
    |> Telemetry.object_
         ~key:"entries"
         ~value:
           ( Telemetry.create ()
           |> Telemetry.int_
                ~key:"count"
                ~value:(Relative_path.Map.cardinal t.entries)
           |> Telemetry.int_
                ~key:"size"
                ~value:
                  (Relative_path.Map.fold
                     t.entries
                     ~init:0
                     ~f:(fun _path entry acc ->
                       acc + String.length entry.contents)) )
    |> Telemetry.string_
         ~key:"backend"
         ~value:(t.backend |> Provider_backend.t_to_string)
    |> Telemetry.object_ ~key:"SharedMem" ~value:(SharedMem.get_telemetry ())
    (* We get SharedMem telemetry for all providers, not just the SharedMem
  provider, just in case there are code paths which use SharedMem despite
  it not being the intended provider. *)
  in
  match t.backend with
  | Provider_backend.Local_memory lmem ->
    let open Provider_backend in
    telemetry
    |> Decl_cache.get_telemetry lmem.decl_cache ~key:"decl_cache"
    |> Shallow_decl_cache.get_telemetry
         lmem.shallow_decl_cache
         ~key:"shallow_decl_cache"
    |> Linearization_cache.get_telemetry
         lmem.linearization_cache
         ~key:"linearization_cache"
    |> Reverse_naming_table_delta.get_telemetry
         lmem.reverse_naming_table_delta
         ~key:"reverse_naming_table_delta"
    |> Fixmes.get_telemetry lmem.fixmes ~key:"fixmes"
  | _ -> telemetry

let reset_telemetry (t : t) : unit =
  match t.backend with
  | Provider_backend.Local_memory
      { Provider_backend.decl_cache; shallow_decl_cache; _ } ->
    Provider_backend.Decl_cache.reset_telemetry decl_cache;
    Provider_backend.Shallow_decl_cache.reset_telemetry shallow_decl_cache;
    ()
  | _ -> ()
