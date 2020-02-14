(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

open Core_kernel
module PositionedSyntaxTree =
  Full_fidelity_syntax_tree.WithSyntax (Full_fidelity_positioned_syntax)

type entry = {
  file_input: ServerCommandTypes.file_input;
  path: Relative_path.t;
  source_text: Full_fidelity_source_text.t;
  comments: Parser_return.comments;
  ast: Nast.program;
  ast_errors: Errors.t;
  mutable cst: PositionedSyntaxTree.t option;
  mutable tast: Tast.program option;
  mutable tast_errors: Errors.t option;
  mutable symbols: Relative_path.t SymbolOccurrence.t list option;
}

type t = {
  tcopt: TypecheckerOptions.t;
  backend: Provider_backend.t;
  entries: entry Relative_path.Map.t;
}

let empty_for_tool ~tcopt ~backend =
  { tcopt; backend; entries = Relative_path.Map.empty }

let empty_for_worker ~tcopt =
  {
    tcopt;
    backend = Provider_backend.Shared_memory;
    entries = Relative_path.Map.empty;
  }

let empty_for_test ~tcopt =
  {
    tcopt;
    backend = Provider_backend.Shared_memory;
    entries = Relative_path.Map.empty;
  }

let empty_for_debugging ~tcopt =
  {
    tcopt;
    backend = Provider_backend.Shared_memory;
    entries = Relative_path.Map.empty;
  }

let map_tcopt (t : t) ~(f : TypecheckerOptions.t -> TypecheckerOptions.t) : t =
  { t with tcopt = f t.tcopt }

let global_context : t option ref = ref None

let get_file_input ~(ctx : t) ~(path : Relative_path.t) :
    ServerCommandTypes.file_input =
  match Relative_path.Map.find_opt ctx.entries path with
  | Some { file_input; _ } -> file_input
  | None -> ServerCommandTypes.FileName (Relative_path.to_absolute path)

let get_fileinfo ~(entry : entry) : FileInfo.t =
  let (funs, classes, record_defs, typedefs, consts) =
    Nast.get_defs entry.ast
  in
  {
    FileInfo.empty_t with
    FileInfo.funs;
    classes;
    record_defs;
    typedefs;
    consts;
  }

let get_global_context () : t option = !global_context

let set_global_context_internal (t : t) : unit =
  match !global_context with
  | Some _ ->
    failwith "set_global_context_internal: a global context is already set"
  | None -> global_context := Some t

let unset_global_context_internal () : unit =
  match !global_context with
  | Some _ -> global_context := None
  | None -> failwith "unset_global_context_internal: no global context is set"

let get_telemetry (t : t) (telemetry : Telemetry.t) : Telemetry.t =
  let telemetry =
    telemetry
    |> Telemetry.string_
         ~key:"backend"
         ~value:(t.backend |> Provider_backend.t_to_string)
  in
  match t.backend with
  | Provider_backend.Local_memory { decl_cache; shallow_decl_cache; _ } ->
    telemetry
    |> Telemetry.object_
         ~key:"decl_cache"
         ~value:(Provider_backend.Decl_cache.get_telemetry decl_cache)
    |> Telemetry.object_
         ~key:"shallow_decl_cache"
         ~value:
           (Provider_backend.Shallow_decl_cache.get_telemetry
              shallow_decl_cache)
  | _ -> telemetry

let reset_telemetry (t : t) : unit =
  match t.backend with
  | Provider_backend.Local_memory { decl_cache; shallow_decl_cache; _ } ->
    Provider_backend.Decl_cache.reset_telemetry decl_cache;
    Provider_backend.Shallow_decl_cache.reset_telemetry shallow_decl_cache;
    ()
  | _ -> ()
