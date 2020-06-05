// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
//
// @generated SignedSource<<2d6b3112ac87de32a785c234e135be15>>
//
// To regenerate this file, run:
//   hphp/hack/src/oxidized_by_ref/regen.sh

use arena_trait::TrivialDrop;
use ocamlrep_derive::ToOcamlRep;
use serde::Serialize;

#[allow(unused_imports)]
use crate::*;

pub use crate::shape_map;

pub use pos::Pos;

#[derive(
    Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, ToOcamlRep
)]
pub struct Id<'a>(pub &'a Pos<'a>, pub &'a str);

pub type Pstring<'a> = (&'a Pos<'a>, &'a str);

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, ToOcamlRep
)]
pub enum ShapeFieldName<'a> {
    SFlitInt(Pstring<'a>),
    SFlitStr(Pstring<'a>),
    SFclassConst(Id<'a>, Pstring<'a>),
}
impl<'a> TrivialDrop for ShapeFieldName<'a> {}

pub use oxidized::ast_defs::Variance;

pub use oxidized::ast_defs::ConstraintKind;

pub use oxidized::ast_defs::Reified;

pub use oxidized::ast_defs::ClassKind;

pub use oxidized::ast_defs::ParamKind;

pub use oxidized::ast_defs::OgNullFlavor;

pub use oxidized::ast_defs::FunKind;

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, ToOcamlRep
)]
pub enum Bop<'a> {
    Plus,
    Minus,
    Star,
    Slash,
    Eqeq,
    Eqeqeq,
    Starstar,
    Diff,
    Diff2,
    Ampamp,
    Barbar,
    LogXor,
    Lt,
    Lte,
    Gt,
    Gte,
    Dot,
    Amp,
    Bar,
    Ltlt,
    Gtgt,
    Percent,
    Xor,
    Cmp,
    QuestionQuestion,
    Eq(Option<&'a Bop<'a>>),
}
impl<'a> TrivialDrop for Bop<'a> {}

pub use oxidized::ast_defs::Uop;
