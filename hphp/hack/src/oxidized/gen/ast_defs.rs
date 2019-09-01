// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
//
// @generated SignedSource<<f29ea026b4db32341092c7292e173894>>
//
// To regenerate this file, run:
//   hphp/hack/src/oxidized/regen.sh

use ocamlrep_derive::IntoOcamlRep;
use ocamlvalue_macro::Ocamlvalue;

use crate::pos;

pub use crate::shape_map;

pub use pos::Pos;

#[derive(Clone, Debug, IntoOcamlRep, Ocamlvalue)]
pub struct Id(pub Pos, pub String);

pub type Pstring = (Pos, String);

#[derive(Clone, Debug, IntoOcamlRep, Ocamlvalue)]
pub enum ShapeFieldName {
    SFlitInt(Pstring),
    SFlitStr(Pstring),
    SFclassConst(Id, Pstring),
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum Variance {
    Covariant,
    Contravariant,
    Invariant,
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum ConstraintKind {
    ConstraintAs,
    ConstraintEq,
    ConstraintSuper,
}

pub type Reified = bool;

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum ClassKind {
    Cabstract,
    Cnormal,
    Cinterface,
    Ctrait,
    Cenum,
    Crecord,
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum ParamKind {
    Pinout,
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum OgNullFlavor {
    OGNullthrows,
    OGNullsafe,
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum FunKind {
    FSync,
    FAsync,
    FGenerator,
    FAsyncGenerator,
    FCoroutine,
}

#[derive(Clone, Debug, IntoOcamlRep, Ocamlvalue)]
pub enum Bop {
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
    Eq(Option<Box<Bop>>),
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum Uop {
    Utild,
    Unot,
    Uplus,
    Uminus,
    Uincr,
    Udecr,
    Upincr,
    Updecr,
    Uref,
    Usilence,
}

#[derive(Clone, Copy, Debug, Eq, IntoOcamlRep, Ocamlvalue, PartialEq)]
pub enum FunDeclKind {
    FDeclAsync,
    FDeclSync,
    FDeclCoroutine,
}
