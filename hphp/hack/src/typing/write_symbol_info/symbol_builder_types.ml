(*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

open Hh_json

type symbol_occurrences = {
  decls: Tast.def list;
  occurrences: Relative_path.t SymbolOccurrence.t list;
}

(* Predicate types for the JSON facts emitted *)
type predicate =
  | ClassConstDeclaration
  | ClassConstDefinition
  | ClassDeclaration
  | ClassDefinition
  | DeclarationComment
  | DeclarationLocation
  | EnumDeclaration
  | EnumDefinition
  | Enumerator
  | FileLines
  | FileXRefs
  | FunctionDeclaration
  | FunctionDefinition
  | GlobalConstDeclaration
  | GlobalConstDefinition
  | InterfaceDeclaration
  | InterfaceDefinition
  | MethodDeclaration
  | MethodDefinition
  | PropertyDeclaration
  | PropertyDefinition
  | TraitDeclaration
  | TraitDefinition
  | TypeConstDeclaration
  | TypeConstDefinition
  | TypedefDeclaration

type glean_json = {
  classConstDeclaration: json list;
  classConstDefinition: json list;
  classDeclaration: json list;
  classDefinition: json list;
  declarationComment: json list;
  declarationLocation: json list;
  enumDeclaration: json list;
  enumDefinition: json list;
  enumerator: json list;
  fileLines: json list;
  fileXRefs: json list;
  functionDeclaration: json list;
  functionDefinition: json list;
  globalConstDeclaration: json list;
  globalConstDefinition: json list;
  interfaceDeclaration: json list;
  interfaceDefinition: json list;
  methodDeclaration: json list;
  methodDefinition: json list;
  propertyDeclaration: json list;
  propertyDefinition: json list;
  traitDeclaration: json list;
  traitDefinition: json list;
  typeConstDeclaration: json list;
  typeConstDefinition: json list;
  typedefDeclaration: json list;
}

type result_progress = {
  resultJson: glean_json;
  (* Maps fact JSON key to a list of predicate/fact id pairs *)
  factIds: (predicate * int) list JMap.t;
}

type file_lines = {
  filepath: Relative_path.t;
  lineLengths: int list;
  endsInNewline: bool;
  hasUnicodeOrTabs: bool;
}

(* Containers that can be in inheritance relationships *)
type container_type =
  | ClassContainer
  | InterfaceContainer
  | TraitContainer
