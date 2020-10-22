// Copyright (c) 2019, Facebook, Inc.
// All rights reserved.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use std::collections::BTreeMap;

use bumpalo::Bump;

use arena_collections::AssocListMut;
use ocamlrep::rc::RcOc;
use oxidized::relative_path::RelativePath;
use oxidized_by_ref::{
    direct_decl_parser::{DeclLists, Decls},
    file_info,
};
use parser_core_types::{parser_env::ParserEnv, source_text::SourceText};

pub fn parse_decls<'a>(
    filename: RelativePath,
    text: &'a [u8],
    auto_namespace_map: &'a BTreeMap<String, String>,
    arena: &'a Bump,
) -> Decls<'a> {
    let text = SourceText::make(RcOc::new(filename), text);
    let (_, _errors, state, _mode) = direct_decl_parser::parse_script(
        &text,
        ParserEnv::default(),
        auto_namespace_map,
        arena,
        None,
    );
    let decls = state.decls;

    let mut classes = AssocListMut::new_in(arena);
    for &(name, decl) in decls.classes {
        classes.insert(name, decl);
    }
    let mut funs = AssocListMut::new_in(arena);
    for &(name, decl) in decls.funs {
        funs.insert(name, decl);
    }
    let mut typedefs = AssocListMut::new_in(arena);
    for &(name, decl) in decls.typedefs {
        typedefs.insert(name, decl);
    }
    let mut consts = AssocListMut::new_in(arena);
    for &(name, decl) in decls.consts {
        consts.insert(name, decl);
    }
    let mut records = AssocListMut::new_in(arena);
    for &(name, decl) in decls.records {
        records.insert(name, decl);
    }

    Decls {
        classes: classes.into(),
        funs: funs.into(),
        typedefs: typedefs.into(),
        consts: consts.into(),
        records: records.into(),
    }
}

pub fn parse_decl_lists<'a>(
    filename: RelativePath,
    text: &'a [u8],
    auto_namespace_map: &'a BTreeMap<String, String>,
    arena: &'a Bump,
) -> (DeclLists<'a>, Option<file_info::Mode>) {
    let text = SourceText::make(RcOc::new(filename), text);
    let (_, _errors, state, mode) = direct_decl_parser::parse_script(
        &text,
        ParserEnv::default(),
        auto_namespace_map,
        arena,
        None,
    );
    let decls = state.decls;
    (
        DeclLists {
            classes: decls.classes,
            funs: decls.funs,
            typedefs: decls.typedefs,
            consts: decls.consts,
            records: decls.records,
        },
        mode,
    )
}
