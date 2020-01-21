(*
 * Copyright (c) 2018, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)
open Hh_core
open ServerCommandTypes

let go file_input tcopt =
  let ctx = Provider_context.empty ~tcopt in
  let path =
    match file_input with
    | FileName file_name -> Relative_path.create_detect_prefix file_name
    | FileContent _ -> Relative_path.default
  in
  let (_ctx, entry) = Provider_utils.update_context ~ctx ~path ~file_input in
  let { Provider_utils.Compute_tast_and_errors.errors; _ } =
    Provider_utils.compute_tast_and_errors_unquarantined ~ctx ~entry
  in
  errors |> Errors.get_sorted_error_list |> List.map ~f:Errors.to_absolute
