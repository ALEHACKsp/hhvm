(**
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
*)

module Hack_bucket = Bucket
open Core_kernel
module Bucket = Hack_bucket

let should_do_remote
    (opts: TypecheckerOptions.t)
    (fnl: (Relative_path.t * FileInfo.names) list)
    ~(t: float)
    : bool * float =
  let remote_type_check = TypecheckerOptions.remote_type_check opts in
  let remote_type_check_threshold = TypecheckerOptions.remote_type_check_threshold opts in
  let file_count = List.length fnl in
  let do_remote = match remote_type_check, remote_type_check_threshold with
    | true, Some remote_type_check_threshold when file_count >= remote_type_check_threshold ->
      Hh_logger.log
        "Going to schedule work because file count %d >= threshold %d"
        file_count
        remote_type_check_threshold;
      true
    | _ -> false
  in
  do_remote, (Hh_logger.log_duration
    (Printf.sprintf
      "Calculated whether should do remote type checking (%B)"
      do_remote)
    t)

let go
    (workers: MultiWorker.worker list option)
    (opts: TypecheckerOptions.t)
    ~(naming_table: Naming_table.t option)
    ~(naming_sqlite_path: string option)
    ~(eden_threshold: int)
    (fnl: (Relative_path.t * FileInfo.names) list)
    : Errors.t =
  let t = Unix.gettimeofday() in
  let num_remote_workers = TypecheckerOptions.num_remote_workers opts in
  let open RemoteScheduler in
  let errors = RemoteScheduler.go {
    bin_root = Path.make (Filename.dirname Sys.argv.(0));
    eden_threshold;
    files = Some fnl;
    naming_sqlite_path;
    naming_table;
    num_remote_workers;
    root = Path.make (Relative_path.path_of_prefix Relative_path.Root);
    timeout = 9999;
    workers;
  }
  in
  let _t = Hh_logger.log_duration "Finished remote type checking" t in
  errors
