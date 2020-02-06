(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

(* `t` represents a decl service, i.e. something where you can make requests
for decls and get back answers. Often these requests will block upon IO. *)
type t

val rpc_get_gconst : t -> string -> Typing_defs.decl_ty option
