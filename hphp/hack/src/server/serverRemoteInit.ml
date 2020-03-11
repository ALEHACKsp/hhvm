(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

let init
    (ctx : Provider_context.t)
    (workers : MultiWorker.worker list option)
    ~(worker_key : string)
    ~(check_id : string)
    ~(bin_root : Path.t)
    ~(root : Path.t) : unit =
  let (server
        : (module RemoteWorker.RemoteServerApi
             with type naming_table = Naming_table.t option)) =
    ServerApi.make_remote_server_api ctx workers
  in
  let (worker_env : Naming_table.t option RemoteWorker.work_env) =
    RemoteWorker.make_env ctx ~bin_root ~check_id ~key:worker_key ~root server
  in

  RemoteWorker.go worker_env
