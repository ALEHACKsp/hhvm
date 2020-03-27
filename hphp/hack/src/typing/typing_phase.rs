// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use bumpalo::collections::vec::Vec as BVec;

use oxidized::ast;
use oxidized::ast_defs::Id;
use oxidized::pos::Pos;
use oxidized::typing_defs_core::{Tparam as DTparam, Ty as DTy, Ty_ as DTy_};
use typing_defs_rust::tast;
use typing_defs_rust::typing_defs::ExpandEnv_;
use typing_defs_rust::typing_defs_core::Ty;
use typing_defs_rust::typing_reason::PReason_;
use typing_env_rust::typing_env_types::Env;

/// Transforms a declaration phase type into a localized type. This performs
/// common operations that are necessary for this operation, specifically:
///   > Expand newtype/types
///   > Resolves the "this" type
///   > Instantiate generics
///   > ...
///
/// When keep track of additional information while localizing a type such as
/// what type defs were expanded to detect potentially recursive definitions..
pub fn localize<'a>(ety_env: &'a ExpandEnv_<'a>, env: &mut Env<'a>, dty: &'a DTy) -> Ty<'a> {
    let bld = env.builder();
    let DTy(r0, dty) = dty;
    let r = env
        .builder()
        .alloc_reason(PReason_::from_decl_provided_reason(&r0));
    match &**dty {
        // TODO(hrust) missing matches
        DTy_::Tgeneric(name) => match ety_env.substs.find(&&name[..]) {
            None => bld.generic(r, &name),
            Some(ty) => {
                let ty = env.inference_env.expand_type(ty);
                let r = bld.mk_rinstantiate(ty.get_reason(), &name, r);
                Ty::mk(r, ty.get_node())
            }
        },
        // TODO(hrust) missing matches
        DTy_::Tapply(cls, tys) => {
            let Id(_, cls_name) = cls;
            let tys = match env.genv.provider.get_class(&cls_name) {
                None => bld.vec_from_iter(tys.iter().map(|ty: &'a DTy| localize(ety_env, env, ty))),
                Some(class_info) => {
                    // TODO(hrust) global inference logic
                    // TODO(hrust) will the `unwrap` below panic?
                    // Assumption: r is always Rhint or at least always has a pos
                    localize_tparams(ety_env, env, r.pos.unwrap(), tys, &class_info.tparams)
                }
            };
            bld.class(r, &cls, tys)
        }
        _ => {
            // TODO(hrust) missing matches
            unimplemented!("{:#?}", dty)
        }
    }
}

/// Declare and localize the type arguments to a constructor or function, given
/// information about the declared type parameters in `decl_tparam list`. If no
/// explicit type arguments are given, generate fresh type variables in their
/// place; do the same for any wildcard explicit type arguments.
/// Report arity errors using `def_pos` (for the declared parameters), `use_pos`
/// (for the use-site) and `use_name` (the name of the constructor or function).
pub fn localize_targs<'a>(
    _env: &mut Env<'a>,
    _tparams: &Vec<DTparam>,
    _targs: &Vec<ast::Targ>,
) -> Vec<tast::Targ<'a>> {
    vec![] // TODO(hrust) fill in here
}

fn localize_tparams<'a>(
    ety_env: &'a ExpandEnv_<'a>,
    env: &mut Env<'a>,
    pos: &Pos,
    tys: &'a Vec<DTy>,
    tparams: &Vec<DTparam>,
) -> BVec<'a, Ty<'a>> {
    let bld = env.builder();
    let length = std::cmp::min(tys.len(), tparams.len());
    bld.vec_from_iter(
        tys.iter()
            .take(length)
            .zip(tparams.iter().take(length))
            .map(|(ty, tparam)| localize_tparam(ety_env, env, pos, ty, tparam)),
    )
}

fn localize_tparam<'a>(
    ety_env: &'a ExpandEnv_<'a>,
    env: &mut Env<'a>,
    _pos: &Pos,
    ty: &'a DTy,
    _tparam: &DTparam,
) -> Ty<'a> {
    // TODO(hrust) wildcard case
    localize(ety_env, env, ty)
}
