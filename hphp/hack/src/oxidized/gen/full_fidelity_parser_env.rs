// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
//
// @generated SignedSource<<a3752bdb80993ba69ec9a18d356cac82>>
//
// To regenerate this file, run:
//   hphp/hack/src/oxidized/regen.sh

use ocamlrep_derive::OcamlRep;
use serde::Deserialize;
use serde::Serialize;

use crate::file_info;

#[derive(Clone, Debug, Deserialize, OcamlRep, Serialize)]
pub struct FullFidelityParserEnv {
    pub hhvm_compat_mode: bool,
    pub php5_compat_mode: bool,
    pub codegen: bool,
    pub disable_lval_as_an_expression: bool,
    pub disable_nontoplevel_declarations: bool,
    pub mode: Option<file_info::Mode>,
    pub rust: bool,
    pub disable_legacy_soft_typehints: bool,
    pub allow_new_attribute_syntax: bool,
    pub disable_legacy_attribute_syntax: bool,
    pub leak_rust_tree: bool,
    pub enable_xhp_class_modifier: bool,
}
