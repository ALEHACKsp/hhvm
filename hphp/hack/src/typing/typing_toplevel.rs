// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use crate::{fun_env, stmt_env};
use decl_provider_rust::DeclProvider;
use oxidized::ast;
use typing_defs_rust::tast;
use typing_defs_rust::typing_make_type::TypeBuilder;

mod typing;
mod typing_continuations;
mod typing_enum;
mod typing_env;
mod typing_env_from_def;
mod typing_env_return_info;
mod typing_env_types;
mod typing_local_types;
mod typing_naming;
mod typing_object_get;
mod typing_per_cont_env;
mod typing_phase;
pub mod typing_print;
mod typing_solver;
mod typing_subtype;
mod typing_union;

pub use typing_env::*;
pub use typing_env_from_def::*;
pub use typing_env_types::*;

pub fn program<'a>(
    builder: &'a TypeBuilder<'a>,
    provider: &'a dyn DeclProvider,
    ast: &'a ast::Program,
) -> tast::Program<'a> {
    ast.iter()
        .filter_map(|x| def(builder, provider, x))
        .collect()
}

fn def<'a>(
    builder: &'a TypeBuilder<'a>,
    provider: &'a dyn DeclProvider,
    def: &'a ast::Def,
) -> Option<tast::Def<'a>> {
    match def {
        ast::Def::Fun(f) => fun_def(builder, provider, f),
        ast::Def::Stmt(x) => {
            match (*x).1 {
                // Currently this is done in naming.ml
                ast::Stmt_::Noop => None,
                ast::Stmt_::Markup(_) => None,
                _ => {
                    let mut env = stmt_env(builder, provider, x);
                    Some(tast::Def::mk_stmt(typing::stmt(&mut env, x)))
                }
            }
        }
        ast::Def::Class(_) => None,
        _ => unimplemented!(),
    }
}

fn fun_def<'a>(
    builder: &'a TypeBuilder<'a>,
    provider: &'a dyn DeclProvider,
    f: &'a ast::Fun_,
) -> Option<tast::Def<'a>> {
    let mut env = fun_env(builder, provider, f);
    let f = typing::fun(&mut env, f);
    typing_solver::solve_all_unsolved_tyvars(&mut env);
    Some(tast::Def::mk_fun(f))
}
