// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use hhas_attribute_rust::HhasAttribute;
use hhas_body_rust::HhasBody;
use hhas_class_rust::HhasClass;
use hhas_function_rust::HhasFunction;
use hhas_record_def_rust::HhasRecord;
use hhas_symbol_refs_rust::HhasSymbolRefs;
use hhas_typedef_rust::Typedef;

#[derive(Default, Debug)]
pub struct HhasProgram<'a> {
    pub is_hh: bool,
    /* TODO(hrust): add `pub adata: Hhas_adata.t list` */
    pub functions: Vec<HhasFunction<'a>>,
    pub classes: Vec<HhasClass<'a>>,
    pub record_defs: Vec<HhasRecord<'a>>,
    pub typedefs: Vec<Typedef<'a>>,
    pub file_attributes: Vec<HhasAttribute>,
    pub main: HhasBody<'a>,
    pub symbol_refs: Vec<HhasSymbolRefs>,
}
