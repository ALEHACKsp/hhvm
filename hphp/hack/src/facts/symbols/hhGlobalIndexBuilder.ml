(*
 * Copyright (c) 2016, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the BSD-style license found in the
 * LICENSE file in the "hack" directory of this source tree. An additional grant
 * of patent rights can be found in the PATENTS file in the same directory.
 *
 *)
open IndexBuilder
open IndexBuilderTypes

(* Basic help text - too many options to list all *)
let usage = Printf.sprintf "Usage: %s [opts] [repository]\n" Sys.argv.(0)

(* Create one worker per cpu *)
let init_workers () =
  let nbr_procs = Sys_utils.nproc () in
  let gc_control = GlobalConfig.gc_control in
  let config = GlobalConfig.default_sharedmem_config in
  let heap_handle = SharedMem.init config ~num_workers:nbr_procs in
  MultiWorker.make
    ?call_wrapper:None
    ~saved_state:()
    ~entry
    ~nbr_procs
    ~gc_control
    ~heap_handle

(* Parse command line options *)
let parse_options () : index_builder_context option =
  let sqlite_filename = ref None in
  let text_filename = ref None in
  let json_filename = ref None in
  let json_chunk_size = ref 500_000 in
  let json_repo_name = ref None in
  let custom_service = ref None in
  let custom_repo_name = ref None in
  let repository = ref None in
  let include_builtins = ref true in
  let silent = ref false in
  let options =
    ref
      [ ( "--sqlite",
          Arg.String (fun x -> sqlite_filename := Some x),
          "[filename]  Save the global index in a Sqlite database" );
        ( "--text",
          Arg.String (fun x -> text_filename := Some x),
          "[filename]  Save the global index in a finite-state transducer (FST) file"
        );
        ( "--json",
          Arg.String (fun x -> json_filename := Some x),
          "[filename]  Save the global index in a JSON file" );
        ( "--chunk-size",
          Arg.Int (fun x -> json_chunk_size := x),
          "[number]    Split the JSON file into chunks of a specified size" );
        ( "--upload-json-files",
          Arg.String (fun x -> json_repo_name := Some x),
          "[reponame]   Uploads all JSON files in specified repo name" );
        ( "--custom-service",
          Arg.String (fun x -> custom_service := Some x),
          "[service]  Use the custom symbol index writer" );
        ( "--custom-repo-name",
          Arg.String (fun x -> custom_repo_name := Some x),
          "[repo-name]  Send this repo name to the custom symbol index writer"
        );
        ( "--no-builtins",
          Arg.Unit (fun () -> include_builtins := false),
          "Disable processing of built-in HHI files" );
        ( "--silent",
          Arg.Unit (fun () -> silent := true),
          "Build without logging timing data" ) ]
  in
  Arg.parse_dynamic
    options
    (fun anonymous_arg -> repository := Some anonymous_arg)
    usage;

  (* Parameters for this execution *)
  match !repository with
  | None ->
    Printf.printf "%s" usage;
    None
  | Some repo ->
    if not !silent then
      Printf.printf "Building global symbol index for [%s]\n%!" repo;
    Some
      {
        repo_folder = repo;
        sqlite_filename = !sqlite_filename;
        text_filename = !text_filename;
        json_filename = !json_filename;
        json_chunk_size = !json_chunk_size;
        json_repo_name = !json_repo_name;
        custom_service = !custom_service;
        custom_repo_name = !custom_repo_name;
        include_builtins = !include_builtins;
        silent = !silent;
      }

(* Run the application *)
let main () : unit =
  Daemon.check_entry_point ();
  PidLog.init "/tmp/hh_server/global_index_builder.pids";
  PidLog.log ~reason:"main" (Unix.getpid ());
  let ctxt_opt = parse_options () in
  match ctxt_opt with
  | None -> ()
  | Some ctxt ->
    let workers = Some (init_workers ()) in
    IndexBuilder.go ctxt workers

(* Main entry point *)
let () =
  main ();
  Printf.printf "%s" "\n\n"
