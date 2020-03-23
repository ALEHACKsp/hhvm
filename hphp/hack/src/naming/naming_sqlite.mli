(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

type db_path = Db_path of string [@@deriving show]

type save_result = {
  files_added: int;
  symbols_added: int;
}

val empty_save_result : save_result

type 'a forward_naming_table_delta =
  | Modified of 'a
  | Deleted
[@@deriving show]

type file_deltas = FileInfo.t forward_naming_table_delta Relative_path.Map.t

type local_changes = {
  file_deltas: file_deltas;
  base_content_version: string;
}
[@@deriving show]

val save_file_infos :
  string ->
  FileInfo.t Relative_path.Map.t ->
  base_content_version:string ->
  save_result

val update_file_infos : db_path -> local_changes -> unit

val get_local_changes : db_path -> local_changes

val fold :
  db_path:db_path ->
  init:'a ->
  f:(Relative_path.t -> FileInfo.t -> 'a -> 'a) ->
  file_deltas:file_deltas ->
  'a

val get_file_info : db_path -> Relative_path.t -> FileInfo.t option

val get_type_pos :
  db_path ->
  string ->
  case_insensitive:bool ->
  (Relative_path.t * Naming_types.kind_of_type) option

val get_fun_pos :
  db_path -> string -> case_insensitive:bool -> Relative_path.t option

val get_const_pos : db_path -> string -> Relative_path.t option

(** The canonical name (and assorted *Canon heaps) store the canonical name for a
    symbol, keyed off of the lowercase version of its name. We use the canon
    heaps to check for symbols which are redefined using different
    capitalizations so we can throw proper Hack errors for them. *)
val to_canon_name_key : string -> string
