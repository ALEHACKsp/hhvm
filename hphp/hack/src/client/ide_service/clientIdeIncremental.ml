(*
 * Copyright (c) 2019, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

open Core_kernel
open Reordered_argument_collections

let log s = Hh_logger.log ("[ide-incremental] " ^^ s)

let strip_positions symbols =
  List.fold symbols ~init:SSet.empty ~f:(fun acc (_, x) -> SSet.add acc x)

(* Print old and new symbols in a file after a change *)
let log_file_info_change
    ~(old_file_info : FileInfo.t option)
    ~(new_file_info : FileInfo.t option)
    ~(start_time : float)
    ~(path : Relative_path.t) : unit =
  let end_time = Unix.gettimeofday () in
  FileInfo.(
    let list_symbols_in_file_info file_info =
      let symbol_list_to_string symbols =
        let num_symbols = List.length symbols in
        let max_num_symbols_to_show = 5 in
        match symbols with
        | [] -> "<none>"
        | symbols when num_symbols <= max_num_symbols_to_show ->
          symbols |> strip_positions |> SSet.elements |> String.concat ~sep:", "
        | symbols ->
          let num_remaining_symbols = num_symbols - max_num_symbols_to_show in
          let symbols = List.take symbols max_num_symbols_to_show in
          Printf.sprintf
            "%s (+%d more...)"
            ( symbols
            |> strip_positions
            |> SSet.elements
            |> String.concat ~sep:", " )
            num_remaining_symbols
      in
      match file_info with
      | Some file_info ->
        Printf.sprintf
          "funs: %s, classes: %s, typedefs: %s, consts: %s"
          (symbol_list_to_string file_info.funs)
          (symbol_list_to_string file_info.classes)
          (symbol_list_to_string file_info.typedefs)
          (symbol_list_to_string file_info.consts)
      | None -> "<file absent>"
    in
    let verb =
      match (old_file_info, new_file_info) with
      | (Some _, Some _) -> "updated"
      | (Some _, None) -> "deleted"
      | (None, Some _) -> "added"
      | (None, None) ->
        (* May or may not indicate a bug in either the language client or the
      language server.

      - Could happen if the language client sends spurious notifications.
      - Could happen if the editor writes files in a certain way, such as if
        they delete the file before moving a new one into place.
      - Could happen if the language server was not able to read the file,
        despite it existing on disk (e.g. due to permissions). In this case,
        we would fail to generate its [FileInfo.t] and assume that it was
        deleted. This is correct from a certain point of view.
      - Could happen due to a benign race condition where we process
        file-change notifications more slowly than they happen. If a file is
        quickly created, then deleted before we process the create event,
        we'll think it was deleted twice. This is the correct way to handle
        the race condition.
      *)
        "spuriously updated"
    in
    log
      "File changed (%.3fs) %s %s: old: %s vs. new: %s"
      (end_time -. start_time)
      (Relative_path.to_absolute path)
      verb
      (list_symbols_in_file_info old_file_info)
      (list_symbols_in_file_info new_file_info))

(*
 * This fetches the new names out of the modified file
 * Result: (old * new)
 *)
let compute_fileinfo_for_path (env : ServerEnv.env) (path : Relative_path.t) :
    (FileInfo.t option * FileInfo.t option * Facts.facts option) Lwt.t =
  let start_time = Unix.gettimeofday () in
  let naming_table = env.ServerEnv.naming_table in
  let old_file_info = Naming_table.get_file_info naming_table path in
  (* Fetch file contents *)
  let%lwt contents = Lwt_utils.read_all (Relative_path.to_absolute path) in
  let contents = Result.ok contents in
  let (new_file_info, facts) =
    match contents with
    | None -> (None, None)
    (* The file couldn't be read from disk. Assume it's been deleted or is
      otherwise inaccessible. We've already deleted the entries from the naming
      table and reverse naming table, so there's nothing left to do here. *)
    | Some contents ->
      (* We don't want our symbols to be mangled for export.  Mangling would
       * convert :xhp:myclass to __xhp_myclass, which would fail name lookup *)
      Facts_parser.mangle_xhp_mode := false;
      let popt = env.ServerEnv.popt in
      let facts =
        Facts_parser.from_text
          ~php5_compat_mode:false
          ~hhvm_compat_mode:true
          ~disable_nontoplevel_declarations:false
          ~disable_legacy_soft_typehints:false
          ~allow_new_attribute_syntax:false
          ~disable_legacy_attribute_syntax:false
          ~enable_xhp_class_modifier:
            (ParserOptions.enable_xhp_class_modifier popt)
          ~disable_xhp_element_mangling:
            (ParserOptions.disable_xhp_element_mangling popt)
          ~filename:path
          ~text:contents
      in
      let (funs, classes, record_defs, typedefs, consts) =
        match facts with
        | None ->
          (* File failed to parse or was not a Hack file. *)
          ([], [], [], [], [])
        | Some facts ->
          let to_ids name_type names =
            List.map names ~f:(fun name ->
                let fixed_name = Utils.add_ns name in
                let pos = FileInfo.File (name_type, path) in
                (pos, fixed_name))
          in
          let funs = facts.Facts.functions |> to_ids FileInfo.Fun in
          (* Classes and typedefs are both stored under `types`. There's also a
          `typeAliases` field which only stores typedefs that we could use if we
          wanted, but we write out the pattern-matches here for
          exhaustivity-checking. *)
          let classes =
            facts.Facts.types
            |> Facts.InvSMap.filter (fun _k v ->
                   Facts.(
                     match v.kind with
                     | TKClass
                     | TKInterface
                     | TKEnum
                     | TKTrait
                     | TKUnknown
                     | TKMixed ->
                       true
                     | TKTypeAlias
                     | TKRecord ->
                       false))
            |> Facts.InvSMap.keys
            |> to_ids FileInfo.Class
          in
          let record_defs =
            facts.Facts.types
            |> Facts.InvSMap.filter (fun _k v -> Facts.(v.kind = TKRecord))
            |> Facts.InvSMap.keys
            |> to_ids FileInfo.RecordDef
          in
          let typedefs =
            facts.Facts.types
            |> Facts.InvSMap.filter (fun _k v ->
                   Facts.(
                     match v.kind with
                     | TKTypeAlias -> true
                     | TKClass
                     | TKInterface
                     | TKEnum
                     | TKTrait
                     | TKUnknown
                     | TKMixed
                     | TKRecord ->
                       false))
            |> Facts.InvSMap.keys
            |> to_ids FileInfo.Typedef
          in
          let consts = facts.Facts.constants |> to_ids FileInfo.Const in
          (funs, classes, record_defs, typedefs, consts)
      in
      let fi_mode =
        Full_fidelity_parser.parse_mode
          (Full_fidelity_source_text.make path contents)
        |> Option.value (* TODO: is this a reasonable default? *)
             ~default:FileInfo.Mstrict
      in
      ( Some
          {
            FileInfo.file_mode = Some fi_mode;
            funs;
            classes;
            record_defs;
            typedefs;
            consts;
            hash = None;
            comments = None;
          },
        facts )
  in
  log_file_info_change ~old_file_info ~new_file_info ~start_time ~path;
  Lwt.return (old_file_info, new_file_info, facts)

let update_naming_table
    ~(env : ServerEnv.env)
    ~(path : Relative_path.t)
    ~(old_file_info : FileInfo.t option)
    ~(new_file_info : FileInfo.t option) : ServerEnv.env =
  let naming_table = env.ServerEnv.naming_table in
  (* Remove the old entries from the forward and reverse naming tables. *)
  let naming_table =
    match old_file_info with
    | None -> naming_table
    | Some old_file_info ->
      (* Update reverse naming table *)
      FileInfo.(
        Naming_global.remove_decls
          ~funs:(strip_positions old_file_info.funs)
          ~classes:(strip_positions old_file_info.classes)
          ~record_defs:(strip_positions old_file_info.record_defs)
          ~typedefs:(strip_positions old_file_info.typedefs)
          ~consts:(strip_positions old_file_info.consts);

        (* Update and return the forward naming table *)
        Naming_table.remove naming_table path)
  in
  (* Update forward naming table and reverse naming table with the new
  declarations. *)
  let naming_table =
    match new_file_info with
    | None -> naming_table
    | Some new_file_info ->
      (* Update reverse naming table.
      TODO: this doesn't handle name collisions in erroneous programs.
      NOTE: We don't use [Naming_global.ndecl_file_fast] here because it
      attempts to look up the symbol by doing a file parse, but the file may not
      exist on disk anymore. We also don't need to do the file parse in this
      case anyways, since we just did one and know for a fact where the symbol
      is. *)
      let open FileInfo in
      List.iter new_file_info.funs ~f:(fun (pos, fun_name) ->
          Naming_table.Funs.add fun_name pos);
      List.iter new_file_info.classes ~f:(fun (pos, class_name) ->
          Naming_table.Types.add class_name (pos, Naming_table.TClass));
      List.iter new_file_info.record_defs ~f:(fun (pos, record_def_name) ->
          Naming_table.Types.add record_def_name (pos, Naming_table.TRecordDef));
      List.iter new_file_info.typedefs ~f:(fun (pos, typedef_name) ->
          Naming_table.Types.add typedef_name (pos, Naming_table.TTypedef));
      List.iter new_file_info.consts ~f:(fun (pos, const_name) ->
          Naming_table.Consts.add const_name pos);

      (* Update and return the forward naming table *)
      Naming_table.update naming_table path new_file_info
  in
  { env with ServerEnv.naming_table }

let invalidate_decls
    ~(ctx : Provider_context.t) ~(old_file_info : FileInfo.t option) : unit =
  (* TODO(ljw): this isn't right... It's correct for us to invalidate shallow
  decls found in this file. But notionally, for correctness, we should also
  invalidate all decls and all linearizations. *)
  match old_file_info with
  | None -> ()
  | Some { FileInfo.funs; classes; record_defs; typedefs; consts; _ } ->
    funs |> strip_positions |> SSet.iter ~f:(Decl_provider.invalidate_fun ctx);
    classes
    |> strip_positions
    |> SSet.iter ~f:(fun class_name ->
           Decl_provider.invalidate_class ctx class_name;
           Shallow_classes_provider.invalidate_class ctx class_name);
    record_defs
    |> strip_positions
    |> SSet.iter ~f:(Decl_provider.invalidate_record_def ctx);
    typedefs
    |> strip_positions
    |> SSet.iter ~f:(Decl_provider.invalidate_typedef ctx);
    consts
    |> strip_positions
    |> SSet.iter ~f:(Decl_provider.invalidate_gconst ctx);
    ()

let update_symbol_index
    ~(env : ServerEnv.env)
    ~(path : Relative_path.t)
    ~(facts : Facts.facts option) : ServerEnv.env =
  match facts with
  | None ->
    let paths = Relative_path.Set.singleton path in
    let local_symbol_table =
      SymbolIndex.remove_files ~sienv:env.ServerEnv.local_symbol_table ~paths
    in
    { env with ServerEnv.local_symbol_table }
  | Some facts ->
    let local_symbol_table =
      SymbolIndex.update_from_facts
        ~sienv:env.ServerEnv.local_symbol_table
        ~path
        ~facts
    in
    { env with ServerEnv.local_symbol_table }

let process_changed_file
    ~(env : ServerEnv.env) ~(ctx : Provider_context.t) ~(path : Path.t) :
    ServerEnv.env Lwt.t =
  let str_path = Path.to_string path in
  match Relative_path.strip_root_if_possible str_path with
  | None ->
    log "Ignored change to file %s, as it is not within our repo root" str_path;
    Lwt.return env
  | Some path ->
    let path = Relative_path.from_root path in
    if not (FindUtils.path_filter path) then
      Lwt.return env
    else
      let%lwt (old_file_info, new_file_info, facts) =
        compute_fileinfo_for_path env path
      in
      invalidate_decls ~ctx ~old_file_info;
      let env = update_naming_table ~env ~path ~old_file_info ~new_file_info in
      let env = update_symbol_index ~env ~path ~facts in
      Lwt.return env
