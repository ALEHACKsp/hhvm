(**
 * Copyright (c) 2015, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the BSD-style license found in the
 * LICENSE file in the "hack" directory of this source tree. An additional grant
 * of patent rights can be found in the PATENTS file in the same directory.
 *
 *)

type t

val of_id : id:int -> t

val get_id : t -> int

val clear : t -> t

val update : t -> Errors.t -> t

val get_errors : t -> Errors.t
