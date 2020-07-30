(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

open Hh_prelude
open Hh_core
open Ifc_types
module A = Aast
module T = Typing_defs

(* Everything done in this file should eventually be merged in Hack's
   regular decl phase. Right now it is more convenient to keep things
   simple and separate here. *)

exception FlowDecl of string

let fail s = raise (FlowDecl s)

let policied_id = "\\Policied"

let infer_flows_id = "\\InferFlows"

let exception_id = "\\Exception"

let make_callable_name cls_name_opt name =
  match cls_name_opt with
  | None -> name
  | Some cls_name -> cls_name ^ "#" ^ name

let get_attr attr attrs =
  let is_attr a = String.equal (snd a.A.ua_name) attr in
  match List.filter ~f:is_attr attrs with
  | [] -> None
  | [a] -> Some a
  | _ -> fail ("multiple '" ^ attr ^ "' attributes found")

let callable_decl attrs =
  let fd_kind =
    if Option.is_some (get_attr infer_flows_id attrs) then
      FDInferFlows
    else
      FDPublic
  in
  { fd_kind }

let callable class_name_opt name attrs =
  let callable_name =
    match class_name_opt with
    | None -> name
    | Some class_name -> class_name ^ "#" ^ name
  in
  let fun_decl = callable_decl attrs in
  (callable_name, fun_decl)

let fun_ { A.f_name = (_, name); f_user_attributes = attrs; _ } =
  callable None name attrs

let meth class_name { A.m_name = (_, name); m_user_attributes = attrs; _ } =
  callable (Some class_name) name attrs

let immediate_supers { A.c_uses; A.c_extends; _ } =
  let id_of_hint = function
    | (_, A.Happly (id, _)) -> snd id
    | _ -> fail "unexpected hint in inheritance hierarchy"
  in
  List.map ~f:id_of_hint (c_extends @ c_uses)

(* A property declared in a trait T can be redeclared in a class or a trait
 * inheriting from T. When this property is policied it will be inherited with a
 * (possibly) different policied annotation. We need to pick one.
 *
 * Our criteria for resolution is:
 * 1. If both declarations are unpolicied, it is not a policied property;
 * 2. If only one is policied, it is a policied property (possibly with a purpose);
 * 3. If both of them are policied and
 *   a. neither has a purpose, property is policied without a purpose
 *   b. only one has a purpose, property is policied with that purpose
 *   c. both have the same purpose, property is policied with that purpose
 *   d. have differing purposes, it is an error
 *
 * (1) and (2) are enforced automatically by the virtue of only keeping track
 * of policied properties. The following function enforces (3).
 *)
let resolve_duplicate_policied_properties policied_properties =
  let err_msg name purp1 purp2 =
    name ^ " has purpose " ^ purp1 ^ " and " ^ purp2 ^ " due to inheritance"
  in
  let prop_table = Caml.Hashtbl.create 10 in
  let go pprop =
    match Caml.Hashtbl.find_opt prop_table pprop.pp_name with
    | Some pprop' ->
      begin
        match (pprop.pp_purpose, pprop'.pp_purpose) with
        | (Some purpose, Some purpose')
          when not @@ String.equal purpose purpose' ->
          fail @@ err_msg pprop.pp_name purpose purpose'
        | (Some _, None) -> Caml.Hashtbl.replace prop_table pprop.pp_name pprop
        | _ -> ()
      end
    | None -> Caml.Hashtbl.add prop_table pprop.pp_name pprop
  in
  List.iter ~f:go policied_properties;
  Caml.Hashtbl.fold (fun _ pprop acc -> pprop :: acc) prop_table []

let is_visible pp = not @@ A.equal_visibility A.Private pp.pp_visibility

let add_super class_decl_env class_decl_acc super =
  match SMap.find_opt super class_decl_env with
  | Some { cd_policied_properties } ->
    let super_props = List.filter ~f:is_visible cd_policied_properties in
    let props = super_props @ class_decl_acc.cd_policied_properties in
    { cd_policied_properties = props }
  | None -> fail @@ super ^ " wasn't found in the inheritance hierarchy"

let mk_policied_prop
    {
      A.cv_id = (_, pp_name);
      cv_type = (pp_type, _);
      cv_visibility = pp_visibility;
      cv_user_attributes = attrs;
      _;
    } =
  let find_policy attributes =
    match get_attr policied_id attributes with
    | None -> `No_policy
    | Some attr ->
      (match attr.A.ua_params with
      | [] -> `Policy None
      | [(_, A.String purpose)] -> `Policy (Some purpose)
      | _ -> fail "expected a string literal as a purpose argument.")
  in
  match find_policy attrs with
  | `No_policy -> None
  | `Policy pp_purpose -> Some { pp_name; pp_type; pp_visibility; pp_purpose }

let class_ class_decl_env class_ =
  let { A.c_name = (_, name); c_vars = properties; _ } = class_ in

  (* Class decl using the immediately available information of the base class *)
  let cd_policied_properties = List.filter_map ~f:mk_policied_prop properties in
  let base_class_decl = { cd_policied_properties } in

  (* Class decl extended with inherited policied properties *)
  let supers = immediate_supers class_ in
  let class_decl =
    let f = add_super class_decl_env in
    List.fold ~f ~init:base_class_decl supers
  in
  let cd_policied_properties =
    resolve_duplicate_policied_properties class_decl.cd_policied_properties
    |> List.sort ~compare:(fun p1 p2 -> String.compare p1.pp_name p2.pp_name)
  in
  let class_decl = { cd_policied_properties } in

  (* Function declarations out of methods *)
  let fun_decls = List.map ~f:(meth name) class_.A.c_methods in

  (name, class_decl, fun_decls)

let magic_class_decls =
  SMap.of_list
    [
      ("\\HH\\vec", { cd_policied_properties = [] });
      (exception_id, { cd_policied_properties = [] });
    ]

let topsort_classes classes =
  (* Record the class hierarchy *)
  let dependency_table = Caml.Hashtbl.create 10 in
  let id_of_hint = function
    | (_, A.Happly (id, _)) -> snd id
    | _ -> fail "unexpected hint in inheritance hierarchy"
  in
  let go ({ A.c_name; A.c_extends; A.c_uses; _ } as class_) =
    let supers = List.map ~f:id_of_hint (c_extends @ c_uses) in
    Caml.Hashtbl.add dependency_table (snd c_name) (class_, supers, false)
  in
  List.iter ~f:go classes;

  (* Put classes, traits, and interfaces in topological order *)
  let schedule = ref [] in
  let rec process id =
    match Caml.Hashtbl.find_opt dependency_table id with
    | Some (class_, dependencies, is_visited) ->
      if not is_visited then begin
        Caml.Hashtbl.replace dependency_table id (class_, dependencies, true);
        List.iter ~f:process dependencies;
        schedule := class_ :: !schedule
      end
    | None ->
      (* If it's a magic builtin, then it has no dependencies, so do nothing *)
      if not @@ SMap.mem id magic_class_decls then
        fail @@ id ^ " is missing entity in the inheritance hierarchy"
  in
  List.iter ~f:process (List.map ~f:(fun c -> snd c.A.c_name) classes);

  List.rev !schedule

(* Removes all the auxiliary info needed only during declaration analysis. *)
let collect_sigs defs =
  (* Prepare class and function definitions *)
  let pick = function
    | A.Class class_ -> `Fst class_
    | A.Fun fun_ -> `Snd fun_
    | _ -> `Trd ()
  in
  let (classes, funs, _) = List.partition3_map ~f:pick defs in
  let classes = topsort_classes classes in

  (* Process and accumulate function decls *)
  let fun_decls = SMap.of_list (List.map ~f:fun_ funs) in

  (* Process and accumulate class decls *)
  let init = { de_class = magic_class_decls; de_fun = fun_decls } in
  let add_class_decl { de_class; de_fun } cls =
    let (class_name, class_decl, meth_decls) = class_ de_class cls in
    let de_class = SMap.add class_name class_decl de_class in
    let de_fun = SMap.union (SMap.of_list meth_decls) de_fun in
    { de_class; de_fun }
  in
  List.fold ~f:add_class_decl ~init classes
