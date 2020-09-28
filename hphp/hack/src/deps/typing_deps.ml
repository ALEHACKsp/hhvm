(*
 * Copyright (c) 2015, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)
open Hh_core
open Reordered_argument_collections
open Utils

(***********************************************)
(* Which dependency graph format are we using? *)
(***********************************************)
type mode =
  | SQLiteMode  (** Legacy mode, with SQLite saved-state dependency graph *)
  | CustomMode  (** Custom mode, with the new custom dependency graph format *)

(** Which mode is active? *)
let mode = ref SQLiteMode

(******************************************)
(* Handling dependencies and their hashes *)
(******************************************)
module Dep = struct
  type dependent

  type dependency

  (** NOTE: keep in sync with `typing_deps.rs`. *)
  type _ variant =
    | GConst : string -> 'a variant
    | Fun : string -> 'a variant
    | Class : string -> 'a variant
    | Extends : string -> dependency variant
    | RecordDef : string -> 'a variant
    | Const : string * string -> dependency variant
    | Cstr : string -> dependency variant
    | Prop : string * string -> dependency variant
    | SProp : string * string -> dependency variant
    | Method : string * string -> dependency variant
    | SMethod : string * string -> dependency variant
    | AllMembers : string -> dependency variant
    | FunName : string -> 'a variant
    | GConstName : string -> 'a variant

  external hash1 : int -> string -> int = "hash1_ocaml" [@@noalloc]

  external hash2 : int -> string -> string -> int = "hash2_ocaml" [@@noalloc]

  (** Explicit [equals] function since [=] doesn't work on the [variant] GADT. *)
  let variant_equals : type a b. a variant -> b variant -> bool =
   fun lhs rhs ->
    match (lhs, rhs) with
    | (GConst lhs, GConst rhs) -> lhs = rhs
    | (Class lhs, Class rhs) -> lhs = rhs
    | (RecordDef lhs, RecordDef rhs) -> lhs = rhs
    | (Fun lhs, Fun rhs) -> lhs = rhs
    | (Const (lhs1, lhs2), Const (rhs1, rhs2)) -> lhs1 = rhs1 && lhs2 = rhs2
    | (Prop (lhs1, lhs2), Prop (rhs1, rhs2)) -> lhs1 = rhs1 && lhs2 = rhs2
    | (SProp (lhs1, lhs2), SProp (rhs1, rhs2)) -> lhs1 = rhs1 && lhs2 = rhs2
    | (Method (lhs1, lhs2), Method (rhs1, rhs2)) -> lhs1 = rhs1 && lhs2 = rhs2
    | (SMethod (lhs1, lhs2), SMethod (rhs1, rhs2)) -> lhs1 = rhs1 && lhs2 = rhs2
    | (GConstName lhs, GConstName rhs) -> lhs = rhs
    | (FunName lhs, FunName rhs) -> lhs = rhs
    | (AllMembers lhs, AllMembers rhs) -> lhs = rhs
    | (Extends lhs, Extends rhs) -> lhs = rhs
    | (GConst _, _) -> false
    | (Class _, _) -> false
    | (RecordDef _, _) -> false
    | (Fun _, _) -> false
    | (Cstr _, _) -> false
    | (Const _, _) -> false
    | (Prop _, _) -> false
    | (SProp _, _) -> false
    | (Method _, _) -> false
    | (SMethod _, _) -> false
    | (GConstName _, _) -> false
    | (FunName _, _) -> false
    | (AllMembers _, _) -> false
    | (Extends _, _) -> false

  type t = int

  (* Keep in sync with the tags for `DepType` in `typing_deps.rs`. *)
  let make : type a. a variant -> t = function
    | GConst name1 -> hash1 0 name1
    | Fun name1 -> hash1 1 name1
    | Class name1 -> hash1 2 name1
    | Extends name1 -> hash1 3 name1
    | RecordDef name1 -> hash1 4 name1
    | Const (name1, name2) -> hash2 5 name1 name2
    | Cstr name1 -> hash1 6 name1
    | Prop (name1, name2) -> hash2 7 name1 name2
    | SProp (name1, name2) -> hash2 8 name1 name2
    | Method (name1, name2) -> hash2 9 name1 name2
    | SMethod (name1, name2) -> hash2 10 name1 name2
    | AllMembers name1 -> hash1 11 name1
    | FunName name1 -> hash1 12 name1
    | GConstName name1 -> hash1 13 name1

  let is_class x = x land 1 = 1

  let extends_of_class x = x lxor 1

  let compare = ( - )

  let extract_name : type a. a variant -> string = function
    | GConst s -> Utils.strip_ns s
    | GConstName s -> Utils.strip_ns s
    | Const (cls, s) -> spf "%s::%s" (Utils.strip_ns cls) s
    | Class s -> Utils.strip_ns s
    | RecordDef s -> Utils.strip_ns s
    | Fun s -> Utils.strip_ns s
    | FunName s -> Utils.strip_ns s
    | Prop (cls, s) -> spf "%s::%s" (Utils.strip_ns cls) s
    | SProp (cls, s) -> spf "%s::%s" (Utils.strip_ns cls) s
    | Method (cls, s) -> spf "%s::%s" (Utils.strip_ns cls) s
    | SMethod (cls, s) -> spf "%s::%s" (Utils.strip_ns cls) s
    | Cstr s -> Utils.strip_ns s
    | AllMembers s -> Utils.strip_ns s
    | Extends s -> Utils.strip_ns s

  let to_debug_string = string_of_int

  let of_debug_string = int_of_string

  let variant_to_string : type a. a variant -> string =
   fun dep ->
    let prefix =
      match dep with
      | GConst _ -> "GConst"
      | GConstName _ -> "GConstName"
      | Const _ -> "Const"
      | Class _ -> "Class"
      | RecordDef _ -> "RecordDef"
      | Fun _ -> "Fun"
      | FunName _ -> "FunName"
      | Prop _ -> "Prop"
      | SProp _ -> "SProp"
      | Method _ -> "Method"
      | SMethod _ -> "SMethod"
      | Cstr _ -> "Cstr"
      | AllMembers _ -> "AllMembers"
      | Extends _ -> "Extends"
    in
    prefix ^ " " ^ extract_name dep
end

(***********************************************)
(* Dependency tracing                          *)
(***********************************************)

(** Whether or not to trace new dependency edges *)
let trace = ref true

(** List of callbacks, called when discovering dependency edges *)
let dependency_callbacks = Caml.Hashtbl.create 0

let add_dependency_callback cb_name cb =
  Caml.Hashtbl.replace dependency_callbacks cb_name cb

(** Set of dependencies as used in the legacy SQLite system. *)
module SQLiteDepSet = struct
  include Reordered_argument_set (Set.Make (Dep))

  let make () = empty

  let pp = make_pp (fun fmt -> Format.fprintf fmt "%d")
end

(** Set of dependencies used for the custom system

    The type `t` is an abstract type managed by `typing_deps.rs`.
    It is a pointer to an `OrdSet<Dep>`, a persistent Rust map *)
module CustomDepSet = struct
  type t (* Abstract type *)

  type elt = Dep.t

  external make : unit -> t = "hh_dep_set_make"

  external singleton : elt -> t = "hh_dep_set_singleton"

  external add : t -> elt -> t = "hh_dep_set_add"

  external union : t -> t -> t = "hh_dep_set_union"

  external inter : t -> t -> t = "hh_dep_set_inter"

  external diff : t -> t -> t = "hh_dep_set_diff"

  external mem : t -> elt -> bool = "hh_dep_set_mem"

  external elements : t -> elt list = "hh_dep_set_elements"

  external cardinal : t -> int = "hh_dep_set_cardinal"

  external is_empty : t -> bool = "hh_dep_set_is_empty"

  let iter s ~f = List.iter (elements s) ~f

  let fold : 'a. t -> init:'a -> f:(elt -> 'a -> 'a) -> 'a =
   fun s ~init ~f ->
    let l = elements s in
    List.fold l ~init ~f:(fun x acc -> f acc x)
end

(** Unified interface for `SQLiteDepSet` and `CustomDepSet`

    The actual representation dependends on the value in `mode`. *)
module DepSet = struct
  type t =
    | SQLiteDepSet of SQLiteDepSet.t
    | CustomDepSet of CustomDepSet.t

  type elt = Dep.t

  let make () =
    match !mode with
    | SQLiteMode -> SQLiteDepSet (SQLiteDepSet.make ())
    | CustomMode -> CustomDepSet (CustomDepSet.make ())

  let singleton elt =
    match !mode with
    | SQLiteMode -> SQLiteDepSet (SQLiteDepSet.singleton elt)
    | CustomMode -> CustomDepSet (CustomDepSet.singleton elt)

  let add s x =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet (SQLiteDepSet.add s x)
    | CustomDepSet s -> CustomDepSet (CustomDepSet.add s x)

  let union s1 s2 =
    match (s1, s2) with
    | (SQLiteDepSet s1, SQLiteDepSet s2) ->
      SQLiteDepSet (SQLiteDepSet.union s1 s2)
    | (CustomDepSet s1, CustomDepSet s2) ->
      CustomDepSet (CustomDepSet.union s1 s2)
    | _ -> failwith "incompatible dependency sets"

  let inter s1 s2 =
    match (s1, s2) with
    | (SQLiteDepSet s1, SQLiteDepSet s2) ->
      SQLiteDepSet (SQLiteDepSet.inter s1 s2)
    | (CustomDepSet s1, CustomDepSet s2) ->
      CustomDepSet (CustomDepSet.inter s1 s2)
    | _ -> failwith "incompatible dependency sets"

  let diff s1 s2 =
    match (s1, s2) with
    | (SQLiteDepSet s1, SQLiteDepSet s2) ->
      SQLiteDepSet (SQLiteDepSet.diff s1 s2)
    | (CustomDepSet s1, CustomDepSet s2) ->
      CustomDepSet (CustomDepSet.diff s1 s2)
    | _ -> failwith "incompatible dependency sets"

  let iter s ~f =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.iter s ~f
    | CustomDepSet s -> CustomDepSet.iter s ~f

  let fold : 'a. t -> init:'a -> f:(elt -> 'a -> 'a) -> 'a =
   fun s ~init ~f ->
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.fold s ~init ~f
    | CustomDepSet s -> CustomDepSet.fold s ~init ~f

  let mem s elt =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.mem s elt
    | CustomDepSet s -> CustomDepSet.mem s elt

  let elements s =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.elements s
    | CustomDepSet s -> CustomDepSet.elements s

  let cardinal s =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.cardinal s
    | CustomDepSet s -> CustomDepSet.cardinal s

  let is_empty s =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.is_empty s
    | CustomDepSet s -> CustomDepSet.is_empty s

  let pp fmt s =
    match s with
    | SQLiteDepSet s -> SQLiteDepSet.pp fmt s
    | CustomDepSet _s -> failwith ""
end

module VisitedSet = struct
  type custom_t (* abstract type managed by Rust, RefCell<BTreeSet<Dep>> *)

  external hh_visited_set_make : unit -> custom_t = "hh_visited_set_make"

  type t =
    | SQLiteVisitedSet of SQLiteDepSet.t ref
    | CustomVisitedSet of custom_t

  let make () : t =
    match !mode with
    | SQLiteMode -> SQLiteVisitedSet (ref (SQLiteDepSet.make ()))
    | CustomMode -> CustomVisitedSet (hh_visited_set_make ())
end

module NamingHash = struct
  type t = int64

  (** NOTE: MUST KEEP IN SYNC with the implementation in `typing_deps.rs`.

  In [Dep.make], we produce a 31-bit hash. In the naming table saved-state,
  we want to use a hash to identify entries, but to avoid collisions, we want
  to preserve as many bits as possible of the hash. This can be done by
  combining it with a larger hash to set the other bits.

  When it comes time to query the naming table, we can get an conservative
  overestimate of the symbols corresponding to dependency hashes by arranging
  the bits so that we can do a range query (which can make use of the SQLite
  index).

  Given a dependency hash with bits in the form DDDD and the additional
  naming table hash bits in the form NNNN, we want to produce a large hash of
  the form

    DDDDNNNN

  Then later we can query the naming table SQLite database for all rows in
  the range DDDD0000 to DDDD1111. *)
  let combine_hashes ~(dep_hash : int64) ~(naming_hash : int64) : int64 =
    (* We use a 64-bit integer with OCaml/SQLite, but for clarity with
    debugging, we limit the hash size to 63 bits, so that we can convert it
    to an OCaml integer. Then we set the top bit to 0 to ensure that it's
    positive, leaving 62 bits. The dependency hash is 31 bits, so we can add
    an additional 31 bits from the naming hash.

    We make sure we only have have 31 bits to begin with (this is not the case
    when in the new 64-bit hash world) *)
    let dep_hash =
      Int64.logand dep_hash 0b01111111_11111111_11111111_11111111L
    in
    let upper_31_bits = Int64.shift_left dep_hash 31 in
    let lower_31_bits =
      Int64.logand naming_hash 0b01111111_11111111_11111111_11111111L
    in
    Int64.logor upper_31_bits lower_31_bits

  let unsupported (variant : 'a Dep.variant) =
    failwith
      ( "Unsupported dependency variant type for naming table hash: "
      ^ Dep.variant_to_string variant )

  let get_dep_variant_name : type a. a Dep.variant -> string =
    let open Dep in
    function
    | Class name -> name
    | RecordDef name -> name
    | Fun name -> name
    | GConst name -> name
    | GConstName _ as variant -> unsupported variant
    | Const _ as variant -> unsupported variant
    | AllMembers _ as variant -> unsupported variant
    | FunName _ as variant -> unsupported variant
    | Prop _ as variant -> unsupported variant
    | SProp _ as variant -> unsupported variant
    | Method _ as variant -> unsupported variant
    | SMethod _ as variant -> unsupported variant
    | Cstr _ as variant -> unsupported variant
    | Extends _ as variant -> unsupported variant

  let make (variant : 'a Dep.variant) : t =
    let dep_hash = variant |> Dep.make |> Int64.of_int in
    let naming_hash =
      variant |> get_dep_variant_name |> SharedMemHash.hash_string
    in
    combine_hashes ~dep_hash ~naming_hash

  let naming_table_hash_lower_bound_mask =
    let lower_31_bits_set_to_1 = (1 lsl 31) - 1 in
    let upper_31_bits_set_to_1 = ~-lower_31_bits_set_to_1 in
    Int64.of_int upper_31_bits_set_to_1

  let make_lower_bound (hash : Dep.t) : t =
    let hash = hash lsl 31 |> Int64.of_int in
    Int64.logand hash naming_table_hash_lower_bound_mask

  let naming_table_hash_upper_bound_mask =
    let lower_31_bits_set_to_1 = (1 lsl 31) - 1 in
    Int64.of_int lower_31_bits_set_to_1

  let make_upper_bound (hash : Dep.t) : t =
    let upper_31_bits = hash lsl 31 |> Int64.of_int in
    Int64.logor upper_31_bits naming_table_hash_upper_bound_mask

  let to_int64 (t : t) : int64 = t
end

(** Graph management in the legacy SQLite system.

    Consult hh_shared.c for the underlying representation. *)
module SQLiteGraph = struct
  module DepSet = SQLiteDepSet

  external hh_add_dep : int -> unit = "hh_add_dep"

  external hh_get_dep : int -> int list = "hh_get_dep"

  external hh_get_dep_sqlite : int -> int list = "hh_get_dep_sqlite"

  external allow_dependency_table_reads : bool -> bool
    = "hh_allow_dependency_table_reads"

  external assert_allow_dependency_table_reads : unit -> unit
    = "hh_assert_allow_dependency_table_reads"

  let hh_add_dep x = WorkerCancel.with_worker_exit (fun () -> hh_add_dep x)

  let hh_get_dep x = WorkerCancel.with_worker_exit (fun () -> hh_get_dep x)

  let add x y = hh_add_dep ((x lsl 31) lor y)

  let get x =
    assert_allow_dependency_table_reads ();
    let deps = DepSet.make () in
    let deps = List.fold_left ~init:deps ~f:DepSet.add (hh_get_dep x) in
    let deps = List.fold_left ~init:deps ~f:DepSet.add (hh_get_dep_sqlite x) in
    deps

  let get_ideps_from_hash x =
    (* FIXME: It shouldn't be necessary to add x to the result set here. We do so
     because historically, we added self-referential edges to the dependency
     graph (e.g., Class "\Foo" -> Class "\Foo"). We no longer do this in order
     to save memory, but we aren't yet confident that these edges were not
     relied upon anywhere. *)
    SQLiteDepSet.add (get x) x

  let add_idep_directly_to_graph ~(dependent : Dep.t) ~(dependency : Dep.t) :
      unit =
    add dependency dependent

  let add_idep
      (dependent : Dep.dependent Dep.variant)
      (dependency : Dep.dependency Dep.variant) =
    if Dep.variant_equals dependent dependency then
      ()
    else (
      Caml.Hashtbl.iter (fun _ f -> f dependent dependency) dependency_callbacks;
      if !trace then
        add_idep_directly_to_graph
          ~dependent:(Dep.make dependent)
          ~dependency:(Dep.make dependency)
    )

  let rec get_extend_deps ~visited ~source_class ~acc =
    if DepSet.mem !visited source_class then
      acc
    else (
      visited := DepSet.add !visited source_class;
      let cid_hash = Dep.extends_of_class source_class in
      let ideps = get_ideps_from_hash cid_hash in
      DepSet.fold
        ~f:
          begin
            fun obj acc ->
            if Dep.is_class obj then
              let acc = DepSet.add acc obj in
              get_extend_deps visited obj acc
            else
              acc
          end
        ideps
        ~init:acc
    )

  let add_extend_deps deps =
    let trace = ref (DepSet.make ()) in
    DepSet.fold deps ~init:deps ~f:(fun dep acc ->
        if not @@ Dep.is_class dep then
          acc
        else
          get_extend_deps trace dep acc)

  let add_typing_deps deps =
    DepSet.fold deps ~init:deps ~f:(fun dep acc ->
        DepSet.union (get_ideps_from_hash dep) acc)

  let add_all_deps x = x |> add_extend_deps |> add_typing_deps
end

(** Graph management in the new system with custom file format. *)
module CustomGraph = struct
  module DepSet = CustomDepSet

  external assert_master : unit -> unit = "assert_master"

  let allow_reads_ref = ref false

  let allow_dependency_table_reads flag =
    assert_master ();
    let prev = !allow_reads_ref in
    allow_reads_ref := flag;
    prev

  external get_ideps_from_hash : Dep.t -> DepSet.t
    = "hh_custom_dep_graph_get_ideps_from_hash"

  external add_typing_deps : DepSet.t -> DepSet.t
    = "hh_custom_dep_graph_add_typing_deps"

  external add_extend_deps : DepSet.t -> DepSet.t
    = "hh_custom_dep_graph_add_extend_deps"

  external get_extend_deps :
    VisitedSet.custom_t -> Dep.t -> DepSet.t -> DepSet.t
    = "hh_custom_dep_graph_get_extend_deps"

  let add_all_deps x = x |> add_extend_deps |> add_typing_deps
end

external hh_load_custom_dep_graph : string -> (unit, string) result
  = "hh_load_custom_dep_graph"

let load_custom_dep_graph : string -> (unit, string) result =
 fun fn ->
  let result = hh_load_custom_dep_graph fn in
  mode := CustomMode;
  result

(*****************************************************************************)
(* Module keeping track which files contain the toplevel definitions. *)
(*****************************************************************************)

module Files = struct
  let ifiles : (Dep.t, Relative_path.Set.t) Hashtbl.t ref =
    ref (Hashtbl.create 23)

  let get_files deps =
    DepSet.fold
      ~f:
        begin
          fun dep acc ->
          try
            let files = Hashtbl.find !ifiles dep in
            Relative_path.Set.union files acc
          with Not_found -> acc
        end
      deps
      ~init:Relative_path.Set.empty

  let deps_of_file_info (file_info : FileInfo.t) : DepSet.t =
    let {
      FileInfo.funs;
      classes;
      record_defs;
      typedefs;
      consts;
      comments = _;
      file_mode = _;
      hash = _;
    } =
      file_info
    in
    let empty = DepSet.make () in
    let consts =
      List.fold_left
        consts
        ~f:
          begin
            fun acc (_, const_id) ->
            DepSet.add acc (Dep.make (Dep.GConst const_id))
          end
        ~init:empty
    in
    let funs =
      List.fold_left
        funs
        ~f:
          begin
            fun acc (_, fun_id) ->
            DepSet.add acc (Dep.make (Dep.Fun fun_id))
          end
        ~init:empty
    in
    let classes =
      List.fold_left
        classes
        ~f:
          begin
            fun acc (_, class_id) ->
            DepSet.add acc (Dep.make (Dep.Class class_id))
          end
        ~init:empty
    in
    let classes =
      List.fold_left
        record_defs
        ~f:
          begin
            fun acc (_, type_id) ->
            DepSet.add acc (Dep.make (Dep.Class type_id))
          end
        ~init:classes
    in
    let classes =
      List.fold_left
        typedefs
        ~f:
          begin
            fun acc (_, type_id) ->
            DepSet.add acc (Dep.make (Dep.Class type_id))
          end
        ~init:classes
    in
    let defs = DepSet.union funs classes in
    let defs = DepSet.union defs consts in
    defs

  let update_file filename info =
    DepSet.iter (deps_of_file_info info) ~f:(fun def ->
        let previous =
          try Hashtbl.find !ifiles def
          with Not_found -> Relative_path.Set.empty
        in
        Hashtbl.replace !ifiles def (Relative_path.Set.add previous filename))
end

module ForTest = struct
  let compute_dep_hash = Dep.make

  let combine_hashes = NamingHash.combine_hashes
end

(** As part of few optimizations (prechecked files, interruptible typechecking), we
    allow the dependency table to get out of date (in order to be able to prioritize
    other work, like reporting errors in currently open file). This flag is there
    to avoid accidental reads of this stale data - anyone attempting to do so would
    either acknowledge that they don't care about accuracy (by setting this flag
    themselves), or plug in to a hh_server mechanic that will delay executing such
    command until dependency table is back up to date.

    TODO: For SQLiteMode, this flag lives in shared memory. Yet, I think dependencies
    are only read from the master thread (proof: g_db in hh_shared.c is NOT in
    shared memory!). In any case this mechanism should be cleaned up.
    *)
let allow_dependency_table_reads flag =
  match !mode with
  | SQLiteMode -> SQLiteGraph.allow_dependency_table_reads flag
  | CustomMode -> CustomGraph.allow_dependency_table_reads flag

let add_idep dependent dependency =
  match !mode with
  | SQLiteMode -> SQLiteGraph.add_idep dependent dependency
  | CustomMode -> (* TODO(hverr): implement *) ()

let add_idep_directly_to_graph ~dependent ~dependency =
  match !mode with
  | SQLiteMode -> SQLiteGraph.add_idep_directly_to_graph ~dependent ~dependency
  | CustomMode -> (* TODO(hverr): implement, only used in hh_fanout *) ()

let get_ideps_from_hash hash =
  let open DepSet in
  match !mode with
  | SQLiteMode -> SQLiteDepSet (SQLiteGraph.get_ideps_from_hash hash)
  | CustomMode -> CustomDepSet (CustomGraph.get_ideps_from_hash hash)

let get_ideps dependency = get_ideps_from_hash (Dep.make dependency)

let get_extend_deps ~visited ~source_class ~acc =
  let open DepSet in
  let open VisitedSet in
  match (visited, acc) with
  | (SQLiteVisitedSet visited, SQLiteDepSet acc) ->
    let acc = SQLiteGraph.get_extend_deps ~visited ~source_class ~acc in
    SQLiteDepSet acc
  | (CustomVisitedSet visited, CustomDepSet acc) ->
    let acc = CustomGraph.get_extend_deps visited source_class acc in
    CustomDepSet acc
  | _ -> failwith "incompatible dep set types"

let add_extend_deps acc =
  let open DepSet in
  match acc with
  | SQLiteDepSet acc -> SQLiteDepSet (SQLiteGraph.add_extend_deps acc)
  | CustomDepSet acc -> CustomDepSet (CustomGraph.add_extend_deps acc)

let add_typing_deps acc =
  let open DepSet in
  match acc with
  | SQLiteDepSet acc -> SQLiteDepSet (SQLiteGraph.add_typing_deps acc)
  | CustomDepSet acc -> CustomDepSet (CustomGraph.add_typing_deps acc)

let add_all_deps acc =
  let open DepSet in
  match acc with
  | SQLiteDepSet acc -> SQLiteDepSet (SQLiteGraph.add_all_deps acc)
  | CustomDepSet acc -> CustomDepSet (CustomGraph.add_all_deps acc)
