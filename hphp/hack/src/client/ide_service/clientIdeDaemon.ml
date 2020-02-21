(*
 * Copyright (c) 2019, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

open Core_kernel

type message = Message : 'a ClientIdeMessage.tracked_t -> message

type message_queue = message Lwt_message_queue.t

type initialized_state = {
  saved_state_info: Saved_state_loader.Naming_table_saved_state_info.t;
  hhi_root: Path.t;
  server_env: ServerEnv.env;
  ctx: Provider_context.t;
  changed_files_to_process: Path.Set.t;
  peak_changed_files_queue_size: int;
}

type state =
  | Initializing
  | Failed_to_initialize of ClientIdeMessage.error_data
  | Initialized of initialized_state

type t = {
  message_queue: message_queue;
  state: state;
}

let log s = Hh_logger.log ("[ide-daemon] " ^^ s)

let set_up_hh_logger_for_client_ide_service ~(root : Path.t) : unit =
  (* Log to a file on disk. Note that calls to `Hh_logger` will always write to
  `stderr`; this is in addition to that. *)
  let client_ide_log_fn = ServerFiles.client_ide_log root in
  begin
    try Sys.rename client_ide_log_fn (client_ide_log_fn ^ ".old")
    with _e -> ()
  end;
  Hh_logger.set_log
    client_ide_log_fn
    (Out_channel.create client_ide_log_fn ~append:true);
  log "Starting client IDE service at %s" client_ide_log_fn

let load_naming_table_from_saved_state_info
    (server_env : ServerEnv.env)
    (saved_state_info : Saved_state_loader.Naming_table_saved_state_info.t) :
    ServerEnv.env Lwt.t =
  let path =
    Saved_state_loader.Naming_table_saved_state_info.(
      Path.to_string saved_state_info.naming_table_path)
  in
  let naming_table = Naming_table.load_from_sqlite path in
  log "Loaded naming table from SQLite database at %s" path;
  let server_env = { server_env with ServerEnv.naming_table } in
  Lwt.return server_env

let load_saved_state
    (env : ServerEnv.env)
    ~(root : Path.t)
    ~(hhi_root : Path.t)
    ~(naming_table_saved_state_path : Path.t option) :
    (state, ClientIdeMessage.error_data) Lwt_result.t =
  log "[saved-state] Starting load in root %s" (Path.to_string root);
  let%lwt result =
    try%lwt
      let%lwt result =
        match naming_table_saved_state_path with
        | Some naming_table_saved_state_path ->
          (* Assume that there are no changed files on disk if we're getting
          passed the path to the saved-state directly, and that the saved-state
          corresponds to the current state of the world. *)
          let changed_files = [] in
          Lwt.return_ok
            ( {
                Saved_state_loader.Naming_table_saved_state_info
                .naming_table_path = naming_table_saved_state_path;
              },
              changed_files )
        | None ->
          let%lwt result =
            State_loader_lwt.load
              ~repo:root
              ~ignore_hh_version:false
              ~saved_state_type:Saved_state_loader.Naming_table
          in
          Lwt.return result
      in
      match result with
      | Ok (saved_state_info, changed_files) ->
        log
          "[saved-state] Naming table path: %s"
          Saved_state_loader.Naming_table_saved_state_info.(
            Path.to_string saved_state_info.naming_table_path);

        let%lwt server_env =
          load_naming_table_from_saved_state_info env saved_state_info
        in
        (* Track how many files we have to change locally *)
        HackEventLogger.serverless_ide_local_files
          ~local_file_count:(List.length changed_files);

        Lwt.return_ok
          (Initialized
             {
               saved_state_info;
               hhi_root;
               server_env;
               changed_files_to_process = Path.Set.of_list changed_files;
               ctx = Provider_utils.ctx_from_server_env server_env;
               peak_changed_files_queue_size = List.length changed_files;
             })
      | Error load_error ->
        let user_message =
          Saved_state_loader.user_message_of_error load_error
        in
        let log_string = Saved_state_loader.log_string_of_error load_error in
        let is_actionable = Saved_state_loader.is_error_actionable load_error in
        Lwt.return_error
          { ClientIdeMessage.user_message; log_string; is_actionable }
    with e ->
      let e = Exception.wrap e in
      Hh_logger.exc
        (Exception.to_exn e)
        ~prefix:"Uncaught exception in client IDE services"
        ~stack:(Exception.get_backtrace_string e);
      let user_message = "Uncaught exception in client IDE services" in
      Lwt.return_error
        {
          ClientIdeMessage.user_message;
          log_string = Exception.to_string e;
          is_actionable = false;
        }
  in
  Lwt.return result

let log_startup_time (component : string) (start_time : float) : float =
  let now = Unix.gettimeofday () in
  HackEventLogger.serverless_ide_startup ~component ~start_time;
  now

let initialize
    ({
       ClientIdeMessage.Initialize_from_saved_state.root;
       naming_table_saved_state_path;
       use_ranked_autocomplete;
     } :
      ClientIdeMessage.Initialize_from_saved_state.t) :
    (state, ClientIdeMessage.error_data) Lwt_result.t =
  let start_time = Unix.gettimeofday () in
  HackEventLogger.serverless_ide_set_root root;
  set_up_hh_logger_for_client_ide_service ~root;

  Relative_path.set_path_prefix Relative_path.Root root;
  let hhi_root = Hhi.get_hhi_root () in
  log "Extracted hhi files to directory %s" (Path.to_string hhi_root);
  Relative_path.set_path_prefix Relative_path.Hhi hhi_root;
  Relative_path.set_path_prefix Relative_path.Tmp (Path.make "/tmp");

  let server_args = ServerArgs.default_options ~root:(Path.to_string root) in
  let (server_config, server_local_config) =
    ServerConfig.load ServerConfig.filename server_args
  in
  let hhconfig_version =
    server_config |> ServerConfig.version |> Config_file.version_to_string_opt
  in
  HackEventLogger.set_hhconfig_version hhconfig_version;

  (* NOTE: We don't want to depend on shared memory in the long-term, since
  we're only running one process and don't need to share memory with anyone. To
  remove the shared memory usage here requires refactoring our heaps to never
  write to shared memory. *)
  let (_ : SharedMem.handle) =
    SharedMem.init ~num_workers:0 (ServerConfig.sharedmem_config server_config)
  in

  Provider_backend.set_local_memory_backend_with_defaults ();

  let genv =
    ServerEnvBuild.make_genv server_args server_config server_local_config []
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
  Parser_options_provider.set server_env.ServerEnv.popt;

  (* Use server_config to modify server_env with the correct symbol index *)
  let start_time = log_startup_time "basic_startup" start_time in
  let namespace_map =
    GlobalOptions.po_auto_namespace_map server_env.ServerEnv.tcopt
  in
  let sienv =
    SymbolIndex.initialize
      ~globalrev:None
      ~gleanopt:server_env.ServerEnv.gleanopt
      ~namespace_map
      ~provider_name:
        server_local_config.ServerLocalConfig.symbolindex_search_provider
      ~quiet:server_local_config.ServerLocalConfig.symbolindex_quiet
      ~ignore_hh_version:false
      ~savedstate_file_opt:
        server_local_config.ServerLocalConfig.symbolindex_file
      ~workers:None
  in
  let sienv =
    {
      sienv with
      SearchUtils.sie_log_timings = true;
      SearchUtils.use_ranked_autocomplete;
    }
  in
  let server_env = { server_env with ServerEnv.local_symbol_table = sienv } in
  let start_time = log_startup_time "symbol_index" start_time in
  if use_ranked_autocomplete then AutocompleteRankService.initialize ();
  let%lwt load_state_result =
    load_saved_state server_env ~root ~hhi_root ~naming_table_saved_state_path
  in
  let _ = log_startup_time "saved_state" start_time in
  match load_state_result with
  | Ok state ->
    log "Serverless IDE has completed initialization";
    Lwt.return_ok state
  | Error error_data ->
    log "Serverless IDE failed to initialize";
    Lwt.return_error error_data

let shutdown (state : state) : unit Lwt.t =
  match state with
  | Initializing
  | Failed_to_initialize _ ->
    log "No cleanup to be done";
    Lwt.return_unit
  | Initialized { hhi_root; _ } ->
    let hhi_root = Path.to_string hhi_root in
    log "Removing hhi directory %s..." hhi_root;
    Sys_utils.rm_dir_tree hhi_root;
    Lwt.return_unit

let restore_hhi_root_if_necessary (state : initialized_state) :
    initialized_state =
  if Sys.file_exists (Path.to_string state.hhi_root) then
    state
  else
    (* Some processes may clean up the temporary HHI directory we're using.
    Assume that such a process has deleted the directory, and re-write the HHI
    files to disk. *)
    let hhi_root = Hhi.get_hhi_root ~force_write:true () in
    log
      "Old hhi root %s no longer exists. Creating a new hhi root at %s"
      (Path.to_string state.hhi_root)
      (Path.to_string hhi_root);
    Relative_path.set_path_prefix Relative_path.Hhi hhi_root;
    { state with hhi_root }

let make_context_from_file_input
    (initialized_state : initialized_state)
    (path : Relative_path.t)
    (file_input : ServerCommandTypes.file_input) :
    state * Provider_context.t * Provider_context.entry =
  let initialized_state = restore_hhi_root_if_necessary initialized_state in
  let ctx = initialized_state.ctx in
  match Provider_utils.find_entry ~ctx ~path with
  | None ->
    let (ctx, entry) =
      Provider_utils.add_entry_from_file_input ~ctx ~path ~file_input
    in
    (Initialized { initialized_state with ctx }, ctx, entry)
  | Some entry ->
    (* Only reparse the file if the contents have actually changed.
     * If the user simply sends us a file_input variable with "FileName"
     * we shouldn't count that as a change. *)
    let any_changes =
      match file_input with
      | ServerCommandTypes.FileName _ -> false
      | ServerCommandTypes.FileContent content ->
        content
        <> entry.Provider_context.source_text.Full_fidelity_source_text.text
    in
    if any_changes then
      let (ctx, entry) =
        Provider_utils.add_entry_from_file_input ~ctx ~path ~file_input
      in
      (Initialized { initialized_state with ctx }, ctx, entry)
    else
      (Initialized initialized_state, ctx, entry)

let make_context_from_document_location
    (initialized_state : initialized_state)
    (document_location : ClientIdeMessage.document_location) :
    state * Provider_context.t * Provider_context.entry =
  let (file_path, file_input) =
    match document_location with
    | { ClientIdeMessage.file_contents = None; file_path; _ } ->
      let file_input = ServerCommandTypes.FileName (Path.to_string file_path) in
      (file_path, file_input)
    | { ClientIdeMessage.file_contents = Some file_contents; file_path; _ } ->
      let file_input = ServerCommandTypes.FileContent file_contents in
      (file_path, file_input)
  in
  let path =
    file_path |> Path.to_string |> Relative_path.create_detect_prefix
  in
  make_context_from_file_input initialized_state path file_input

module Handle_message_result = struct
  type 'a t =
    | Notification
    | Response of 'a
    | Error of ClientIdeMessage.error_data
end

let handle_message :
    type a.
    state ->
    string ->
    a ClientIdeMessage.t ->
    (state * a Handle_message_result.t) Lwt.t =
 fun state _tracking_id message ->
  let open ClientIdeMessage in
  match (state, message) with
  | (state, Shutdown ()) ->
    let%lwt () = shutdown state in
    Lwt.return (state, Handle_message_result.Response ())
  | ((Failed_to_initialize _ | Initializing), File_changed _) ->
    (* Should not happen. *)
    let error_data =
      {
        ClientIdeMessage.user_message =
          "IDE services could not process file change because "
          ^ "it failed to initialize or was still initializing. The caller "
          ^ "should have waited for the IDE services to become ready before "
          ^ "sending file-change notifications.";
        log_string = Exception.get_current_callstack_string 99;
        is_actionable = false;
      }
    in
    Lwt.return (state, Handle_message_result.Error error_data)
  | (Initialized initialized_state, File_changed path) ->
    (* Only invalidate when a hack file changes *)
    if FindUtils.file_filter (Path.to_string path) then
      let changed_files_to_process =
        Path.Set.add initialized_state.changed_files_to_process path
      in
      let peak_changed_files_queue_size =
        initialized_state.peak_changed_files_queue_size + 1
      in
      let ctx =
        Provider_utils.ctx_from_server_env initialized_state.server_env
      in
      let state =
        Initialized
          {
            initialized_state with
            changed_files_to_process;
            ctx;
            peak_changed_files_queue_size;
          }
      in
      Lwt.return (state, Handle_message_result.Notification)
    else
      Lwt.return (state, Handle_message_result.Notification)
  | (Initializing, Initialize_from_saved_state param) ->
    let%lwt result = initialize param in
    begin
      match result with
      | Ok state -> Lwt.return (state, Handle_message_result.Response ())
      | Error error_data ->
        Lwt.return
          ( Failed_to_initialize error_data,
            Handle_message_result.Error error_data )
    end
  | (Initialized _, Initialize_from_saved_state _) ->
    let error_data =
      {
        ClientIdeMessage.user_message =
          "Tried to initialize when already initialized";
        log_string = Exception.get_current_callstack_string 100;
        is_actionable = false;
      }
    in
    Lwt.return (state, Handle_message_result.Error error_data)
  | (Initializing, _) ->
    let error_data =
      {
        ClientIdeMessage.user_message =
          "IDE services have not yet been initialized";
        log_string = Exception.get_current_callstack_string 100;
        is_actionable = false;
      }
    in
    Lwt.return (state, Handle_message_result.Error error_data)
  | (Failed_to_initialize error_data, _) ->
    let error_data =
      {
        error_data with
        ClientIdeMessage.user_message =
          "IDE services failed to initialize: " ^ error_data.user_message;
      }
    in
    Lwt.return (state, Handle_message_result.Error error_data)
  | (Initialized initialized_state, File_opened { file_path; file_contents }) ->
    let path =
      file_path |> Path.to_string |> Relative_path.create_detect_prefix
    in
    let (state, _, _) =
      make_context_from_file_input
        initialized_state
        path
        (ServerCommandTypes.FileContent file_contents)
    in
    Lwt.return (state, Handle_message_result.Response ())
  | (Initialized initialized_state, Hover document_location) ->
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let result =
      Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
          ServerHover.go_quarantined
            ~ctx
            ~entry
            ~line:document_location.ClientIdeMessage.line
            ~column:document_location.ClientIdeMessage.column)
    in
    Lwt.return (state, Handle_message_result.Response result)
  (* Autocomplete *)
  | ( Initialized initialized_state,
      Completion
        { ClientIdeMessage.Completion.document_location; is_manually_invoked }
    ) ->
    (* Update the state of the world with the document as it exists in the IDE *)
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let sienv = initialized_state.server_env.ServerEnv.local_symbol_table in
    let result =
      ServerAutoComplete.go_ctx
        ~ctx
        ~entry
        ~sienv
        ~is_manually_invoked
        ~line:document_location.line
        ~column:document_location.column
    in
    Lwt.return (state, Handle_message_result.Response result)
  (* Autocomplete docblock resolve *)
  | (Initialized initialized_state, Completion_resolve param) ->
    let ctx = initialized_state.ctx in
    ClientIdeMessage.Completion_resolve.(
      let result =
        ServerDocblockAt.go_docblock_for_symbol
          ~ctx
          ~symbol:param.symbol
          ~kind:param.kind
      in
      Lwt.return (state, Handle_message_result.Response result))
  (* Autocomplete docblock resolve *)
  | (Initialized initialized_state, Completion_resolve_location param) ->
    ClientIdeMessage.Completion_resolve_location.(
      let (state, ctx, entry) =
        make_context_from_document_location
          initialized_state
          param.document_location
      in
      let result =
        Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
            ServerDocblockAt.go_docblock_ctx
              ~ctx
              ~entry
              ~line:param.document_location.line
              ~column:param.document_location.column
              ~kind:param.kind)
      in
      Lwt.return (state, Handle_message_result.Response result))
  (* Document highlighting *)
  | (Initialized initialized_state, Document_highlight document_location) ->
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let results =
      Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
          ServerHighlightRefs.go_quarantined
            ~ctx
            ~entry
            ~line:document_location.line
            ~column:document_location.column)
    in
    Lwt.return (state, Handle_message_result.Response results)
  (* Signature help *)
  | (Initialized initialized_state, Signature_help document_location) ->
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let results =
      Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
          ServerSignatureHelp.go_quarantined
            ~env:initialized_state.server_env
            ~ctx
            ~entry
            ~line:document_location.line
            ~column:document_location.column)
    in
    Lwt.return (state, Handle_message_result.Response results)
  (* Go to definition *)
  | (Initialized initialized_state, Definition document_location) ->
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let result =
      Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
          ServerGoToDefinition.go_quarantined
            ~ctx
            ~entry
            ~line:document_location.ClientIdeMessage.line
            ~column:document_location.ClientIdeMessage.column)
    in
    Lwt.return (state, Handle_message_result.Response result)
  (* Type Definition *)
  | (Initialized initialized_state, Type_definition document_location) ->
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let result =
      Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
          ServerTypeDefinition.go_quarantined
            ~ctx
            ~entry
            ~line:document_location.ClientIdeMessage.line
            ~column:document_location.ClientIdeMessage.column)
    in
    Lwt.return (state, Handle_message_result.Response result)
  (* Document Symbol *)
  | (Initialized initialized_state, Document_symbol document_location) ->
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let result = FileOutline.outline_ctx ~ctx ~entry in
    Lwt.return (state, Handle_message_result.Response result)
  (* Type Coverage *)
  | (Initialized initialized_state, Type_coverage document_identifier) ->
    let document_location =
      {
        file_path = document_identifier.file_path;
        file_contents = Some document_identifier.file_contents;
        line = 0;
        column = 0;
      }
    in
    let (state, ctx, entry) =
      make_context_from_document_location initialized_state document_location
    in
    let result =
      Provider_utils.respect_but_quarantine_unsaved_changes ~ctx ~f:(fun () ->
          ServerColorFile.go_quarantined ~ctx ~entry)
    in
    Lwt.return (state, Handle_message_result.Response result)

let write_message
    ~(out_fd : Lwt_unix.file_descr)
    ~(message : ClientIdeMessage.message_from_daemon) : unit Lwt.t =
  let%lwt (_ : int) = Marshal_tools_lwt.to_fd_with_preamble out_fd message in
  Lwt.return_unit

let write_status ~(out_fd : Lwt_unix.file_descr) (state : state) : unit Lwt.t =
  match state with
  | Initializing
  | Failed_to_initialize _ ->
    Lwt.return_unit
  | Initialized { changed_files_to_process; peak_changed_files_queue_size; _ }
    ->
    if Path.Set.is_empty changed_files_to_process then
      let%lwt () =
        write_message
          ~out_fd
          ~message:
            (ClientIdeMessage.Notification ClientIdeMessage.Done_processing)
      in
      Lwt.return_unit
    else
      let total = peak_changed_files_queue_size in
      let processed = total - Path.Set.cardinal changed_files_to_process in
      let%lwt () =
        write_message
          ~out_fd
          ~message:
            (ClientIdeMessage.Notification
               (ClientIdeMessage.Processing_files
                  { ClientIdeMessage.Processing_files.processed; total }))
      in
      Lwt.return_unit

let serve ~(in_fd : Lwt_unix.file_descr) ~(out_fd : Lwt_unix.file_descr) :
    unit Lwt.t =
  let rec flush_event_logger () : unit Lwt.t =
    let%lwt () = Lwt_unix.sleep 0.5 in
    Lwt.async EventLoggerLwt.flush;
    flush_event_logger ()
  in
  let rec pump_message_queue (message_queue : message_queue) : unit Lwt.t =
    try%lwt
      let%lwt { ClientIdeMessage.tracking_id; message } =
        Marshal_tools_lwt.from_fd_with_preamble in_fd
      in
      let is_queue_open =
        Lwt_message_queue.push
          message_queue
          (Message { ClientIdeMessage.tracking_id; message })
      in
      match message with
      | ClientIdeMessage.Shutdown () -> Lwt.return_unit
      | _ when not is_queue_open -> Lwt.return_unit
      | _ -> pump_message_queue message_queue
    with e ->
      let e = Exception.wrap e in
      Lwt_message_queue.close message_queue;
      Exception.reraise e
  in
  let rec handle_messages (t : t) : unit Lwt.t =
    match t with
    | {
     message_queue;
     state =
       Initialized ({ server_env; changed_files_to_process; ctx; _ } as state);
    }
      when Lwt_message_queue.is_empty message_queue
           && (not (Lwt_unix.readable in_fd))
           && not (Path.Set.is_empty changed_files_to_process) ->
      (* Process the next file change, but only if we have no new events to
      handle. To ensure correctness, we would have to actually process all file
      change events *before* we processed any other IDE queries. However, we're
      trying to maximize availability, even if occasionally we give stale
      results. We can revisit this trade-off later if we decide that the stale
      results are baffling users. *)
      let next_file = Path.Set.choose changed_files_to_process in
      let changed_files_to_process =
        Path.Set.remove changed_files_to_process next_file
      in
      let%lwt server_env =
        try%lwt
          let%lwt server_env =
            Lwt_utils.with_context
              ~enter:(fun () ->
                Provider_context.set_global_context_internal ctx;
                Lwt.return_unit)
              ~exit:(fun () ->
                Provider_context.unset_global_context_internal ();
                Lwt.return_unit)
              ~do_:(fun () ->
                let%lwt server_env =
                  ClientIdeIncremental.process_changed_file
                    server_env
                    ctx
                    next_file
                in
                Lwt.return server_env)
          in
          Lwt.return server_env
        with exn ->
          let e = Exception.wrap exn in
          HackEventLogger.uncaught_exception exn;
          Hh_logger.exception_
            e
            ~prefix:
              (Printf.sprintf
                 "Uncaught exception when processing changed file: %s"
                 (Path.to_string next_file));
          Lwt.return server_env
      in
      let%lwt state =
        if Path.Set.is_empty changed_files_to_process then
          Lwt.return
            (Initialized
               {
                 state with
                 server_env;
                 changed_files_to_process;
                 peak_changed_files_queue_size = 0;
               })
        else
          Lwt.return
            (Initialized { state with server_env; changed_files_to_process })
      in
      let%lwt () = write_status ~out_fd state in
      handle_messages { t with state }
    | t ->
      let%lwt message = Lwt_message_queue.pop t.message_queue in
      (match message with
      | None -> Lwt.return_unit
      | Some (Message { ClientIdeMessage.tracking_id; message }) ->
        let unblocked_time = Unix.gettimeofday () in
        let%lwt state =
          try%lwt
            let%lwt (state, response) =
              handle_message t.state tracking_id message
            in
            match response with
            | Handle_message_result.Notification ->
              (* No response needed for notifications. *)
              Lwt.return state
            | Handle_message_result.Response response ->
              let message =
                ClientIdeMessage.Response
                  { ClientIdeMessage.response = Ok response; unblocked_time }
              in
              let%lwt () = write_message ~out_fd ~message in
              Lwt.return state
            | Handle_message_result.Error error_data ->
              let message =
                ClientIdeMessage.Response
                  {
                    ClientIdeMessage.response = Error error_data;
                    unblocked_time;
                  }
              in
              let%lwt () = write_message ~out_fd ~message in
              Lwt.return state
          with e ->
            let e = Exception.wrap e in
            let user_message = Exception.to_string e in
            log "Exception: %s" user_message;
            let error_data =
              {
                ClientIdeMessage.user_message;
                log_string = Exception.get_backtrace_string e;
                is_actionable = false;
              }
            in

            (* If we were responding to a message, but threw an exception, write
            that exception as a response. *)
            let message =
              ClientIdeMessage.Response
                { ClientIdeMessage.response = Error error_data; unblocked_time }
            in
            let%lwt () = write_message ~out_fd ~message in
            Lwt.return t.state
        in
        handle_messages { t with state })
  in
  try%lwt
    let message_queue = Lwt_message_queue.create () in
    let flusher_promise = flush_event_logger () in
    let%lwt () = handle_messages { message_queue; state = Initializing }
    and () = pump_message_queue message_queue in
    Lwt.cancel flusher_promise;
    Lwt.return_unit
  with e ->
    let e = Exception.wrap e in
    log "Exception occurred while handling RPC call: %s" (Exception.to_string e);
    Lwt.return_unit

let daemon_main
    (args : ClientIdeMessage.daemon_args)
    (channels : ('a, 'b) Daemon.channel_pair) : unit =
  Printexc.record_backtrace true;
  let (ic, oc) = channels in
  let in_fd = Lwt_unix.of_unix_file_descr (Daemon.descr_of_in_channel ic) in
  let out_fd = Lwt_unix.of_unix_file_descr (Daemon.descr_of_out_channel oc) in
  let daemon_init_id =
    Printf.sprintf
      "%s.%s"
      args.ClientIdeMessage.init_id
      (Random_id.short_string ())
  in
  HackEventLogger.serverless_ide_init ~init_id:daemon_init_id;
  Hh_logger.Level.set_min_level_file Hh_logger.Level.Info;
  Hh_logger.Level.set_min_level_stderr Hh_logger.Level.Error;
  if args.ClientIdeMessage.verbose then
    Hh_logger.Level.set_min_level Hh_logger.Level.Debug;
  Lwt_main.run (serve ~in_fd ~out_fd)

let daemon_entry_point : (ClientIdeMessage.daemon_args, unit, unit) Daemon.entry
    =
  Daemon.register_entry_point "ClientIdeService" daemon_main
