(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 * These functions build up the JSON necessary and then add facts
 * to the running result.
 *)

open Aast
open Ast_defs
open Hh_json
open Hh_prelude
open Symbol_build_json
open Symbol_builder_types
open Symbol_json_util

let add_container_decl_fact decl_pred name progress =
  let json_fact = JSON_Object [("name", build_name_json_nested name)] in
  add_fact decl_pred json_fact progress

let add_container_defn_fact ctx clss decl_id member_decls prog =
  let common_fields =
    [
      ("declaration", build_id_json decl_id);
      ("members", JSON_Array member_decls);
    ]
    @ build_ns_json_nested clss.c_namespace
  in
  let add_decls decls pred prog =
    List.fold decls ~init:([], prog) ~f:(fun (decl_refs, prog) decl ->
        let name = strip_tparams (get_type_from_hint ctx decl) in
        let (decl_id, prog) = add_container_decl_fact pred name prog in
        let ref = build_id_json decl_id in
        (ref :: decl_refs, prog))
  in
  let (defn_pred, json_fields, prog) =
    match get_container_kind clss with
    | InterfaceContainer ->
      let (extends, prog) =
        add_decls clss.c_extends InterfaceDeclaration prog
      in
      let req_fields = ("extends_", JSON_Array extends) :: common_fields in
      (InterfaceDefinition, req_fields, prog)
    | TraitContainer ->
      let (impls, prog) =
        add_decls clss.c_implements InterfaceDeclaration prog
      in
      let (uses, prog) = add_decls clss.c_uses TraitDeclaration prog in
      let req_fields =
        [("implements_", JSON_Array impls); ("uses", JSON_Array uses)]
        @ common_fields
      in
      (TraitDefinition, req_fields, prog)
    | ClassContainer ->
      let is_abstract =
        match clss.c_kind with
        | Cabstract -> true
        | _ -> false
      in
      let (class_fields, prog) =
        let (impls, prog) =
          add_decls clss.c_implements InterfaceDeclaration prog
        in
        let (uses, prog) = add_decls clss.c_uses TraitDeclaration prog in
        let req_class_fields =
          common_fields
          @ [
              ("isAbstract", JSON_Bool is_abstract);
              ("isFinal", JSON_Bool clss.c_final);
              ("implements_", JSON_Array impls);
              ("uses", JSON_Array uses);
            ]
        in
        match clss.c_extends with
        | [] -> (req_class_fields, prog)
        | [parent] ->
          let (decl_id, prog) =
            let parent_clss = strip_tparams (get_type_from_hint ctx parent) in
            add_container_decl_fact ClassDeclaration parent_clss prog
          in
          (("extends_", build_id_json decl_id) :: req_class_fields, prog)
        | _ ->
          Hh_logger.log
            "WARNING: skipping extends field for class with multiple parents %s"
            (snd clss.c_name);
          (req_class_fields, prog)
      in
      (ClassDefinition, class_fields, prog)
  in
  add_fact defn_pred (JSON_Object json_fields) prog

let add_property_decl_fact con_type decl_id name progress =
  let json_fact =
    JSON_Object
      [
        ("name", build_name_json_nested name);
        ("container", build_container_json_ref con_type decl_id);
      ]
  in
  add_fact PropertyDeclaration json_fact progress

let add_class_const_decl_fact con_type decl_id name progress =
  let json_fact =
    JSON_Object
      [
        ("name", build_name_json_nested name);
        ("container", build_container_json_ref con_type decl_id);
      ]
  in
  add_fact ClassConstDeclaration json_fact progress

let add_type_const_decl_fact con_type decl_id name progress =
  let json_fact =
    JSON_Object
      [
        ("name", build_name_json_nested name);
        ("container", build_container_json_ref con_type decl_id);
      ]
  in
  add_fact TypeConstDeclaration json_fact progress

let add_method_decl_fact con_type decl_id name progress =
  let json_fact =
    JSON_Object
      [
        ("name", build_name_json_nested name);
        ("container", build_container_json_ref con_type decl_id);
      ]
  in
  add_fact MethodDeclaration json_fact progress

let add_method_defn_fact ctx meth decl_id progress =
  let json_fact =
    JSON_Object
      [
        ("declaration", build_id_json decl_id);
        ("signature", build_signature_json ctx meth.m_params meth.m_ret);
        ("visibility", build_visibility_json meth.m_visibility);
        ("isAbstract", JSON_Bool meth.m_abstract);
        ("isAsync", build_is_async_json meth.m_fun_kind);
        ("isFinal", JSON_Bool meth.m_final);
        ("isStatic", JSON_Bool meth.m_static);
      ]
  in
  add_fact MethodDefinition json_fact progress

let add_property_defn_fact ctx prop decl_id progress =
  let base_fields =
    [
      ("declaration", build_id_json decl_id);
      ("visibility", build_visibility_json prop.cv_visibility);
      ("isFinal", JSON_Bool prop.cv_final);
      ("isAbstract", JSON_Bool prop.cv_abstract);
      ("isStatic", JSON_Bool prop.cv_is_static);
    ]
  in
  let json_fields =
    match hint_of_type_hint prop.cv_type with
    | None -> base_fields
    | Some h ->
      let ty = get_type_from_hint ctx h in
      ("type", build_type_json_nested ty) :: base_fields
  in
  add_fact PropertyDefinition (JSON_Object json_fields) progress

let add_class_const_defn_fact ctx const decl_id progress =
  let base_fields =
    [
      ("declaration", build_id_json decl_id);
      ("isAbstract", JSON_Bool (is_none const.cc_expr));
    ]
  in
  let json_fields =
    match const.cc_type with
    | None -> base_fields
    | Some h ->
      let ty = get_type_from_hint ctx h in
      ("type", build_type_json_nested ty) :: base_fields
  in
  add_fact ClassConstDefinition (JSON_Object json_fields) progress

let add_type_const_defn_fact ctx tc decl_id progress =
  let base_fields =
    [
      ("declaration", build_id_json decl_id);
      ("kind", build_type_const_kind_json tc.c_tconst_abstract);
    ]
  in
  let json_fields =
    match tc.c_tconst_type with
    | None -> base_fields
    | Some h ->
      let ty = get_type_from_hint ctx h in
      ("type", build_type_json_nested ty) :: base_fields
  in
  add_fact TypeConstDefinition (JSON_Object json_fields) progress

let add_enum_decl_fact name progress =
  let json_fact = JSON_Object [("name", build_name_json_nested name)] in
  add_fact EnumDeclaration json_fact progress

let add_enum_defn_fact ctx enm enum_id enumerators progress =
  let json_fields =
    let json_fields =
      [
        ("declaration", build_id_json enum_id);
        ("enumerators", JSON_Array enumerators);
      ]
      @ build_ns_json_nested enm.c_namespace
    in
    match enm.c_enum with
    | None -> json_fields
    | Some en ->
      let json_fields =
        ("enumBase", build_type_json_nested (get_type_from_hint ctx en.e_base))
        :: json_fields
      in
      (match en.e_constraint with
      | None -> json_fields
      | Some c ->
        ("enumConstraint", build_type_json_nested (get_type_from_hint ctx c))
        :: json_fields)
  in
  add_fact EnumDefinition (JSON_Object json_fields) progress

let add_enumerator_fact decl_id const_name progress =
  let json_fact =
    JSON_Object
      [
        ("name", build_name_json_nested const_name);
        ("enumeration", build_id_json decl_id);
      ]
  in
  add_fact Enumerator json_fact progress

let add_func_decl_fact name progress =
  let json_fact = JSON_Object [("name", build_name_json_nested name)] in
  add_fact FunctionDeclaration json_fact progress

let add_func_defn_fact ctx elem decl_id progress =
  let json_fields =
    [
      ("declaration", build_id_json decl_id);
      ("signature", build_signature_json ctx elem.f_params elem.f_ret);
      ("isAsync", build_is_async_json elem.f_fun_kind);
    ]
    @ build_ns_json_nested elem.f_namespace
  in
  add_fact FunctionDefinition (JSON_Object json_fields) progress

let add_typedef_decl_fact name elem progress =
  let is_transparent =
    match elem.t_vis with
    | Transparent -> true
    | Opaque -> false
  in
  let json_fields =
    [
      ("name", build_name_json_nested name);
      ("isTransparent", JSON_Bool is_transparent);
    ]
    @ build_ns_json_nested elem.t_namespace
  in
  add_fact TypedefDeclaration (JSON_Object json_fields) progress

let add_gconst_decl_fact name progress =
  let json_fact = JSON_Object [("name", build_name_json_nested name)] in
  add_fact GlobalConstDeclaration json_fact progress

let add_gconst_defn_fact ctx elem decl_id progress =
  let base_fields =
    [("declaration", build_id_json decl_id)]
    @ build_ns_json_nested elem.cst_namespace
  in
  let json_fields =
    match elem.cst_type with
    | None -> base_fields
    | Some h ->
      let ty = get_type_from_hint ctx h in
      ("type", build_type_json_nested ty) :: base_fields
  in
  let json_fact = JSON_Object json_fields in
  add_fact GlobalConstDefinition json_fact progress

let add_decl_loc_fact pos decl_json progress =
  let filepath = Relative_path.to_absolute (Pos.filename pos) in
  let json_fact =
    JSON_Object
      [
        ("declaration", decl_json);
        ("file", build_file_json filepath);
        ("span", build_bytespan_json pos);
      ]
  in
  add_fact DeclarationLocation json_fact progress

let add_decl_comment_fact doc pos decl_json progress =
  let filepath = Relative_path.to_absolute (Pos.filename pos) in
  let json_fact =
    JSON_Object
      [
        ("declaration", decl_json);
        ("file", build_file_json filepath);
        ("comment", build_comment_json_nested doc);
      ]
  in
  add_fact DeclarationComment json_fact progress

let add_file_lines_fact file_info progress =
  let json_fact = build_file_lines_json file_info in
  add_fact FileLines json_fact progress

let add_file_xrefs_fact filepath xref_map progress =
  let json_fact =
    JSON_Object
      [("file", build_file_json filepath); ("xrefs", build_xrefs_json xref_map)]
  in
  add_fact FileXRefs json_fact progress
