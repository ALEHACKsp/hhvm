(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

module Hh_daemon = Daemon
open Core

type state_backend = OCaml_state_backend

type env = {
  from: string;
  root: Path.t;
  ignore_hh_version: bool;
  verbosity: Calculate_fanout.Verbosity.t;
  naming_table_path: Path.t option;
  dep_table_path: Path.t option;
  watchman_sockname: Path.t option;
  changed_files: Relative_path.Set.t;
  state_path: Path.t option;
  state_backend: state_backend;
}

type setup_result = {
  workers: MultiWorker.worker list;
  ctx: Provider_context.t;
}

type saved_state_result = {
  naming_table: Naming_table.t;
  naming_table_path: Path.t;
  dep_table_path: Path.t;
  saved_state_changed_files: Relative_path.Set.t;
}

let set_up_global_environment (env : env) : setup_result =
  let server_args =
    ServerArgs.default_options_with_check_mode ~root:(Path.to_string env.root)
  in
  let (server_config, server_local_config) =
    ServerConfig.load ~silent:false ServerConfig.filename server_args
  in
  let genv =
    ServerEnvBuild.make_genv server_args server_config server_local_config []
    (* no workers *)
  in
  let server_env = ServerEnvBuild.make_env genv.ServerEnv.config in
  (* We need shallow class declarations so that we can invalidate individual
  members in a class hierarchy. *)
  let server_env =
    {
      server_env with
      ServerEnv.tcopt =
        {
          server_env.ServerEnv.tcopt with
          GlobalOptions.tco_shallow_class_decl = true;
        };
    }
  in

  let (ctx, workers, _time_taken) =
    Batch_init.init
      ~root:env.root
      ~shmem_config:(ServerConfig.sharedmem_config server_config)
      ~popt:server_env.ServerEnv.popt
      ~tcopt:server_env.ServerEnv.tcopt
      (Unix.gettimeofday ())
  in
  { workers; ctx }

let load_saved_state ~(env : env) ~(setup_result : setup_result) :
    saved_state_result Lwt.t =
  let%lwt (naming_table_path, naming_table_changed_files) =
    match env.naming_table_path with
    | Some naming_table_path -> Lwt.return (naming_table_path, [])
    | None ->
      let%lwt naming_table_saved_state =
        State_loader_lwt.load
          ~watchman_opts:
            Saved_state_loader.Watchman_options.
              { root = env.root; sockname = env.watchman_sockname }
          ~ignore_hh_version:env.ignore_hh_version
          ~saved_state_type:Saved_state_loader.Naming_table
      in
      (match naming_table_saved_state with
      | Error load_error ->
        failwith
          (Printf.sprintf
             "Failed to load naming-table saved-state, and saved-state files were not manually provided on command-line: %s"
             (Saved_state_loader.debug_details_of_error load_error))
      | Ok (saved_state_info, changed_files) ->
        Lwt.return
          ( saved_state_info
              .Saved_state_loader.Naming_table_info.naming_table_path,
            changed_files ))
  and (dep_table_path, dep_table_changed_files) =
    match env.dep_table_path with
    | Some dep_table_path -> Lwt.return (dep_table_path, [])
    | None ->
      let%lwt dep_table_saved_state =
        State_loader_lwt.load
          ~watchman_opts:
            Saved_state_loader.Watchman_options.
              { root = env.root; sockname = env.watchman_sockname }
          ~ignore_hh_version:env.ignore_hh_version
          ~saved_state_type:Saved_state_loader.Naming_and_dep_table
      in
      (match dep_table_saved_state with
      | Error load_error ->
        failwith
          (Printf.sprintf
             "Failed to load dep-table saved-state, and saved-state files were not manually provided on command-line: %s"
             (Saved_state_loader.debug_details_of_error load_error))
      | Ok (saved_state_info, changed_files) ->
        Lwt.return
          ( saved_state_info
              .Saved_state_loader.Naming_and_dep_table_info.dep_table_path,
            changed_files ))
  in
  let changed_files =
    Relative_path.Set.union
      (Relative_path.Set.of_list naming_table_changed_files)
      (Relative_path.Set.of_list dep_table_changed_files)
  in
  let changed_files =
    Relative_path.Set.filter changed_files ~f:(fun path ->
        FindUtils.file_filter (Relative_path.to_absolute path))
  in

  SharedMem.load_dep_table_sqlite
    (Path.to_string dep_table_path)
    env.ignore_hh_version;
  let naming_table =
    Naming_table.load_from_sqlite
      setup_result.ctx
      (Path.to_string naming_table_path)
  in
  let naming_table =
    Relative_path.Set.fold
      changed_files
      ~init:naming_table
      ~f:(fun path naming_table ->
        let { ClientIdeIncremental.naming_table; _ } =
          ClientIdeIncremental.update_naming_tables_for_changed_file
            ~naming_table
            ~sienv:SearchUtils.quiet_si_env
            ~backend:(Provider_context.get_backend setup_result.ctx)
            ~popt:(Provider_context.get_popt setup_result.ctx)
            ~path
        in
        naming_table)
  in
  Lwt.return
    {
      naming_table;
      naming_table_path;
      dep_table_path;
      saved_state_changed_files = changed_files;
    }

let make_incremental_state ~(env : env) : Incremental.state =
  let state_path =
    match env.state_path with
    | Some state_path -> state_path
    | None ->
      let state_path = Path.make "/tmp/hh_fanout" in
      let state_path =
        Path.concat state_path (Path.slash_escaped_string_of_path env.root)
      in
      let state_path =
        Path.concat
          state_path
          (match Build_banner.banner with
          | Some banner -> banner
          | None -> "development")
      in
      let state_path = Path.concat state_path "hh_fanout_state" in
      state_path
  in
  Hh_logger.log "State path: %s" (Path.to_string state_path);
  match env.state_backend with
  | OCaml_state_backend -> Incremental_ocaml.make state_path

let advance_cursor
    ~(env : env)
    ~(setup_result : setup_result)
    ~(saved_state_result : saved_state_result)
    ~(incremental_state : Incremental.state)
    ~(input_files : Relative_path.Set.t)
    ~(cursor_id : string option) : Incremental.cursor * Incremental.cursor_id =
  Utils.try_finally
    ~finally:(fun () -> incremental_state#save)
    ~f:(fun () ->
      let client_id =
        incremental_state#look_up_client_id
          {
            Incremental.from = env.from;
            dep_table_saved_state_path = saved_state_result.dep_table_path;
            naming_table_saved_state_path =
              Naming_sqlite.Db_path
                (Path.to_string saved_state_result.naming_table_path);
          }
      in
      let (cursor, cursor_changed_files) =
        match cursor_id with
        | None ->
          let cursor = incremental_state#make_default_cursor client_id in
          let cursor = Result.ok_or_failwith cursor in
          (cursor, saved_state_result.saved_state_changed_files)
        | Some cursor_id ->
          let cursor =
            incremental_state#look_up_cursor
              ~client_id:(Some client_id)
              ~cursor_id
          in
          let cursor = Result.ok_or_failwith cursor in
          (cursor, Relative_path.Set.empty)
      in
      let cursor_changed_files =
        Relative_path.Set.union cursor_changed_files env.changed_files
      in
      let cursor_changed_files =
        Relative_path.Set.union cursor_changed_files input_files
      in
      let cursor =
        cursor#advance
          setup_result.ctx
          setup_result.workers
          cursor_changed_files
      in
      let new_cursor_id = incremental_state#add_cursor cursor in
      let dep_graph_delta = cursor#get_dep_graph_delta in
      HashSet.iter dep_graph_delta ~f:(fun (dependent, dependency) ->
          Typing_deps.add_idep_directly_to_graph dependent dependency);

      (cursor, new_cursor_id))

let mode_calculate
    ~(env : env) ~(input_files : Path.Set.t) ~(cursor_id : string option) :
    unit Lwt.t =
  let telemetry = Telemetry.create () in
  let setup_result = set_up_global_environment env in
  let%lwt ({ naming_table = old_naming_table; _ } as saved_state_result) =
    load_saved_state ~env ~setup_result
  in

  let input_files =
    Path.Set.fold input_files ~init:Relative_path.Set.empty ~f:(fun path acc ->
        let path = Relative_path.create_detect_prefix (Path.to_string path) in
        Relative_path.Set.add acc path)
  in
  let incremental_state = make_incremental_state ~env in
  let (cursor, cursor_id) =
    advance_cursor
      ~env
      ~setup_result
      ~saved_state_result
      ~incremental_state
      ~input_files
      ~cursor_id
  in
  let file_deltas = cursor#get_file_deltas in
  let new_naming_table =
    Naming_table.update_from_deltas old_naming_table file_deltas
  in

  let {
    Calculate_fanout.fanout_dependents = _;
    fanout_files;
    explanations;
    telemetry = calculate_fanout_telemetry;
  } =
    Calculate_fanout.go
      ~verbosity:env.verbosity
      ~old_naming_table
      ~new_naming_table
      ~file_deltas
      ~input_files
  in
  let telemetry =
    Telemetry.object_
      telemetry
      ~key:"calculate_fanout"
      ~value:calculate_fanout_telemetry
  in

  let telemetry =
    Telemetry.int_
      telemetry
      ~key:"num_input_files"
      ~value:(Relative_path.Set.cardinal input_files)
  in
  let telemetry =
    Telemetry.int_
      telemetry
      ~key:"num_fanout_files"
      ~value:(Relative_path.Set.cardinal fanout_files)
  in

  let json =
    Hh_json.JSON_Object
      [
        ( "files",
          Hh_json.JSON_Array
            ( fanout_files
            |> Relative_path.Set.elements
            |> List.map ~f:Relative_path.to_absolute
            |> List.map ~f:Hh_json.string_ ) );
        ( "explanations",
          Hh_json.JSON_Object
            (Relative_path.Map.fold explanations ~init:[] ~f:(fun k v acc ->
                 let path = Relative_path.suffix k in
                 let explanation = Calculate_fanout.explanation_to_json v in
                 (path, explanation) :: acc)) );
        ( "cursor",
          let (Incremental.Cursor_id cursor_id) = cursor_id in
          Hh_json.JSON_String cursor_id );
        ("telemetry", Telemetry.to_json telemetry);
      ]
  in
  Hh_json.json_to_multiline_output Out_channel.stdout json;
  Lwt.return_unit

let verbosity_arg =
  Command.Arg_type.create (fun x ->
      match x with
      | "low" -> Calculate_fanout.Verbosity.Low
      | "high" -> Calculate_fanout.Verbosity.High
      | other ->
        Printf.eprintf
          "Invalid verbosity: %s (valid values are 'low', 'high')"
          other;
        exit 1)

let dep_hash_arg = Command.Arg_type.create Typing_deps.Dep.of_debug_string

let path_arg = Command.Arg_type.create Path.make

let parse_env () =
  let open Command.Param in
  let open Command.Let_syntax in
  let%map from =
    flag
      "--from"
      (required string)
      ~doc:"FROM A descriptive string indicating the caller of this program."
  and root =
    flag
      "--root"
      (optional string)
      ~doc:
        "DIR The root directory to run in. If not set, will attempt to locate one by searching upwards for an `.hhconfig` file."
  and verbosity =
    flag
      "--verbosity"
      (optional_with_default Calculate_fanout.Verbosity.Low verbosity_arg)
      ~doc:
        "VERBOSITY How much debugging output to include in the result. May slow down the query."
  and ignore_hh_version =
    flag
      "--ignore-hh-version"
      no_arg
      ~doc:
        "Skip the consistency check for the version that this program was built with versus the version of the server that built the saved-state."
  and naming_table_path =
    flag
      "--naming-table-path"
      (optional path_arg)
      ~doc:"PATH The path to the naming table SQLite saved-state."
  and dep_table_path =
    flag
      "--dep-table-path"
      (optional path_arg)
      ~doc:"PATH The path to the dependency table saved-state."
  and watchman_sockname =
    flag
      "--watchman-sockname"
      (optional path_arg)
      ~doc:"PATH The path to the Watchman socket to use."
  and changed_files =
    flag
      "--changed-file"
      (listed string)
      ~doc:
        ( "PATH A file which has changed since last time `hh_fanout` was invoked. "
        ^ "May be specified multiple times. "
        ^ "Not necessary for the caller to pass unless Watchman is unavailable."
        )
  and state_path =
    flag
      "--state-path"
      (optional path_arg)
      ~doc:
        ( "PATH The path to the persistent state on disk. "
        ^ "If not provided, will use the default path for the repository." )
  and state_backend =
    flag
      "--state-backend"
      (optional_with_default
         OCaml_state_backend
         (Command.Arg_type.create (fun state_backend ->
              match state_backend with
              | "ocaml" -> OCaml_state_backend
              | _ ->
                failwith
                  (Printf.sprintf
                     "Unrecognized state backend: %s"
                     state_backend))))
      ~doc:"The implementation of persistent state to use."
  in
  let root =
    match root with
    | Some root -> Path.make root
    | None -> Wwwroot.get None
  in
  (* Interpret relative paths with respect to the root from here on. That way,
      we can write `hh_fanout --root ~/www foo/bar.php` and it will work regardless
      of the directory that we invoked this executable from. *)
  Sys.chdir (Path.to_string root);
  Relative_path.set_path_prefix Relative_path.Root root;
  let changed_files =
    changed_files
    |> Sys_utils.parse_path_list
    |> List.map ~f:(fun path -> Relative_path.create_detect_prefix path)
    |> Relative_path.Set.of_list
  in

  {
    from;
    root;
    ignore_hh_version;
    verbosity;
    naming_table_path;
    dep_table_path;
    watchman_sockname;
    changed_files;
    state_path;
    state_backend;
  }

let calculate_subcommand =
  let open Command.Param in
  let open Command.Let_syntax in
  Command.basic
    ~summary:"Determines which files must be rechecked after a change"
    (let%map env = parse_env ()
     and input_files = anon (sequence ("filename" %: string))
     and cursor_id =
       flag
         "--cursor"
         (optional string)
         ~doc:"CURSOR The cursor that the previous request returned."
     in

     let input_files =
       input_files
       |> Sys_utils.parse_path_list
       |> List.map ~f:Path.make
       |> Path.Set.of_list
     in
     if Path.Set.is_empty input_files then
       Hh_logger.warn "Warning: list of input files is empty.";

     (fun () -> Lwt_main.run (mode_calculate ~env ~input_files ~cursor_id)))

let mode_debug ~(env : env) ~(path : Path.t) ~(cursor_id : string option) :
    unit Lwt.t =
  let ({ ctx; workers; _ } as setup_result) = set_up_global_environment env in
  let%lwt ({ naming_table = old_naming_table; _ } as saved_state_result) =
    load_saved_state ~env ~setup_result
  in

  let path = Relative_path.create_detect_prefix (Path.to_string path) in
  let input_files = Relative_path.Set.singleton path in
  let incremental_state = make_incremental_state ~env in
  let (cursor, cursor_id) =
    advance_cursor
      ~env
      ~setup_result
      ~saved_state_result
      ~incremental_state
      ~input_files
      ~cursor_id
  in
  let file_deltas = cursor#get_file_deltas in
  let new_naming_table =
    Naming_table.update_from_deltas old_naming_table file_deltas
  in

  let json =
    Debug_fanout.go
      ~ctx
      ~workers
      ~old_naming_table
      ~new_naming_table
      ~file_deltas
      ~path
    |> Debug_fanout.result_to_json
  in
  let json =
    Hh_json.JSON_Object
      [
        ( "cursor",
          let (Incremental.Cursor_id cursor_id) = cursor_id in
          Hh_json.JSON_String cursor_id );
        ("debug", json);
      ]
  in
  Hh_json.json_to_multiline_output Out_channel.stdout json;
  Lwt.return_unit

let debug_subcommand =
  let open Command.Param in
  let open Command.Let_syntax in
  Command.basic
    ~summary:"Produces debugging information about the fanout of a certain file"
    (let%map env = parse_env ()
     and path = anon ("FILE" %: string)
     and cursor_id =
       flag
         "--cursor"
         (optional string)
         ~doc:"CURSOR The cursor that the previous request returned."
     in
     let path = Path.make path in
     (fun () -> Lwt_main.run (mode_debug ~env ~path ~cursor_id)))

let mode_query
    ~(env : env) ~(dep_hash : Typing_deps.Dep.t) ~(include_extends : bool) :
    unit Lwt.t =
  let setup_result = set_up_global_environment env in
  let%lwt (_saved_state_result : saved_state_result) =
    load_saved_state ~env ~setup_result
  in
  let json =
    Query_fanout.go ~dep_hash ~include_extends |> Query_fanout.result_to_json
  in
  let json = Hh_json.JSON_Object [("result", json)] in
  Hh_json.json_to_multiline_output Out_channel.stdout json;
  Lwt.return_unit

let query_subcommand =
  let open Command.Param in
  let open Command.Let_syntax in
  Command.basic
    ~summary:"Get the edges for which the given input node is a dependency"
    (let%map env = parse_env ()
     and include_extends =
       flag
         "--include-extends"
         no_arg
         ~doc:
           "Traverse the extends dependencies for this node and include them in the output as well"
     and dep_hash = anon ("HASH" %: dep_hash_arg) in
     (fun () -> Lwt_main.run (mode_query ~env ~dep_hash ~include_extends)))

let mode_query_path
    ~(env : env) ~(source : Typing_deps.Dep.t) ~(dest : Typing_deps.Dep.t) :
    unit Lwt.t =
  let setup_result = set_up_global_environment env in
  let%lwt (_saved_state_result : saved_state_result) =
    load_saved_state ~env ~setup_result
  in
  let json = Query_path.go ~source ~dest |> Query_path.result_to_json in
  let json = Hh_json.JSON_Object [("result", json)] in
  Hh_json.json_to_multiline_output Out_channel.stdout json;
  Lwt.return_unit

let query_path_subcommand =
  let open Command.Param in
  let open Command.Let_syntax in
  Command.basic
    ~summary:
      "Find a path of dependencies edges leading from one node to another"
    ~readme:(fun () ->
      String.strip
        {|
Produces a list of nodes in the dependency graph connected by typing- or
extends-dependency edges. This is a list of n nodes, where the leading pairs
are connected by extends-dependency edges, and the last pair is connected by
a typing-dependency edge.
|})
    (let%map env = parse_env ()
     and source = anon ("SOURCE-HASH" %: dep_hash_arg)
     and dest = anon ("DEST-HASH" %: dep_hash_arg) in
     (fun () -> Lwt_main.run (mode_query_path ~env ~source ~dest)))

let () =
  EventLogger.init EventLogger.Event_logger_fake 0.0;
  Hh_daemon.check_entry_point ();
  Command.run
  @@ Command.group
       ~summary:"Provides access to Hack's dependency graph"
       [
         ("calculate", calculate_subcommand);
         ("debug", debug_subcommand);
         ("query", query_subcommand);
         ("query-path", query_path_subcommand);
       ]
