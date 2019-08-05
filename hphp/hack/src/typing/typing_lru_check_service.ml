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
open Typing_check_service

(* Helper function to process a single [file_computation] *)
let process_file_computation
    ~(dynamic_view_files: Relative_path.Set.t)
    ~(opts: GlobalOptions.t)
    ~(fc: file_computation)
    ~(errors: Errors.t)
    : Errors.t * file_computation list =
  let process_file_wrapper = process_file dynamic_view_files opts in
  match fc with
  | path, Check info ->
    process_file_wrapper errors (path, info)
  | path, Declare ->
    let errors = Decl_service.decl_file errors path in
    (errors, [])

let process_in_parallel
    (dynamic_view_files: Relative_path.Set.t)
    (lru_host_env: Shared_lru.host_env)
    (opts: TypecheckerOptions.t)
    (fnl: file_computation list)
    ~(interrupt: 'a MultiWorker.interrupt_config)
    : Errors.t * 'a * file_computation list =
  TypeCheckStore.store opts;
  let files_initial_count = List.length fnl in
  ServerProgress.send_percentage_progress_to_monitor
    "typechecking" 0 files_initial_count "files";

  let next ~(batch_size: int) (files_to_process, errors_acc, num_files) =
    let files_to_process, next_input =
      match files_to_process with
      | [] -> [], Hack_bucket.Done
      | _ ->
        let batch, remaining = List.split_n files_to_process batch_size in
        remaining, Hack_bucket.Job batch
    in
    (files_to_process, errors_acc, num_files), next_input
  in

  let job (fc_lst: file_computation list) =
    (* Setup prior to processing files *)
    let opts = TypeCheckStore.load () in
    SharedMem.allow_removes false;
    SharedMem.invalidate_caches();
    File_provider.local_changes_push_stack ();
    Ast_provider.local_changes_push_stack ();

    (* Job helper definition *)
    let rec job_helper ~(fc_lst: file_computation list) ~(acc: Errors.t) =
      match fc_lst with
      | [] -> acc
      | fc :: fc_tl ->
        (* Note: the second param will need to be handled if deferred_decls
         * are released. *)
        let new_errors, _ =
          process_file_computation
            ~dynamic_view_files
            ~opts
            ~errors:Errors.empty
            ~fc
        in
        (* Errors.merge is a List.rev_append, so put the [acc] second *)
        let acc = Errors.merge new_errors acc in
        (* Check if we should exit due to memory pressure *)
        job_helper ~fc_lst:fc_tl ~acc
    in
    (* Process the files! *)
    let errors = job_helper ~fc_lst ~acc:Errors.empty in

    (* Clean up after processing files *)
    Ast_provider.local_changes_pop_stack ();
    File_provider.local_changes_pop_stack ();
    SharedMem.allow_removes true;

    let num_files_checked = List.length fc_lst in
    errors, num_files_checked
  in

  let callback
      (fnl, errors_acc, files_count_acc)
      (errors, num_files) =
    ServerProgress.send_percentage_progress_to_monitor
      "typechecking" (files_count_acc + num_files) files_initial_count "files";
    (* Errors.merge is a List.rev_append, so put the [acc] second *)
    let errors_acc = Errors.merge errors errors_acc in
    fnl, errors_acc, (files_count_acc + num_files)
  in

  let max_size = Bucket.max_size () in
  let batch_size = Bucket.calculate_bucket_size
    ~num_jobs:files_initial_count
    ~num_workers:(lru_host_env.Shared_lru.num_workers)
    ~max_size
  in

  (* Start shared_lru workers. *)
  let _files_left, errors ,_num_files_checked = Shared_lru.run
    ~host_env:lru_host_env
    ~initial_env:(fnl, Errors.empty, 0)
    ~job
    ~callback
    ~next:(next ~batch_size)
  in

  let env = interrupt.MultiThreadedCall.env in
  TypeCheckStore.clear();
  errors, env, []


(* Disclaimer: does not actually go with interrupt yet, although it will
 * in a future version. The function is named the same as the one in
 * [typing_check_service] to easily call the new one in [serverTypeCheck] *)
let go_with_interrupt
    (lru_host_env: Shared_lru.host_env)
    (opts: TypecheckerOptions.t)
    (dynamic_view_files: Relative_path.Set.t)
    (fnl: (Relative_path.t * FileInfo.names) list)
    ~(interrupt: 'a MultiWorker.interrupt_config)
    : (computation_kind, Errors.t, 'a) job_result =
  Hh_logger.log "Using shared_lru workers to typecheck!";
  let fnl = List.map fnl ~f:(fun (path, names) -> path, Check names) in
  process_in_parallel
    dynamic_view_files
    lru_host_env
    opts
    fnl
    ~interrupt

let go
    (lru_host_env: Shared_lru.host_env)
    (opts: TypecheckerOptions.t)
    (dynamic_view_files: Relative_path.Set.t)
    (fnl: (Relative_path.t * FileInfo.names) list)
    : Errors.t =
  let interrupt = MultiThreadedCall.no_interrupt () in
  let res, (), cancelled =
    go_with_interrupt
      lru_host_env
      opts
      dynamic_view_files
      fnl
      ~interrupt
  in
  assert (cancelled = []);
  res
