// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
//
// @generated SignedSource<<ab6370be5ebda5b7d7099ea15faa693f>>
//
// To regenerate this file, run:
//   hphp/hack/src/oxidized_by_ref/regen.sh

use serde::Serialize;

#[allow(unused_imports)]
use crate::*;

pub use typing_defs_flags::*;

pub use typing_defs_core::*;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClassElt<'a> {
    pub visibility: Visibility<'a>,
    pub type_: oxidized::lazy::Lazy<Ty<'a>>,
    /// identifies the class from which this elt originates
    pub origin: &'a str,
    pub deprecated: Option<&'a str>,
    pub pos: oxidized::lazy::Lazy<pos::Pos<'a>>,
    pub flags: isize,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FunElt<'a> {
    pub deprecated: Option<&'a str>,
    pub type_: Ty<'a>,
    pub decl_errors: Option<errors::Errors<'a>>,
    pub pos: pos::Pos<'a>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClassConst<'a> {
    pub synthesized: bool,
    pub abstract_: bool,
    pub pos: pos::Pos<'a>,
    pub type_: Ty<'a>,
    pub expr: Option<nast::Expr<'a>>,
    /// identifies the class from which this const originates
    pub origin: &'a str,
}

/// The position is that of the hint in the `use` / `implements` AST node
/// that causes a class to have this requirement applied to it. E.g.
///
/// ```
/// class Foo {}
///
/// interface Bar {
///   require extends Foo; <- position of the decl_phase ty
/// }
///
/// class Baz extends Foo implements Bar { <- position of the `implements`
/// }
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Requirement<'a>(pub pos::Pos<'a>, pub Ty<'a>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClassType<'a> {
    pub need_init: bool,
    /// Whether the typechecker knows of all (non-interface) ancestors
    /// and thus known all accessible members of this class
    pub members_fully_known: bool,
    pub abstract_: bool,
    pub final_: bool,
    pub const_: bool,
    /// True when the class is annotated with the __PPL attribute.
    pub ppl: bool,
    /// When a class is abstract (or in a trait) the initialization of
    /// a protected member can be delayed
    pub deferred_init_members: s_set::SSet<'a>,
    pub kind: oxidized::ast_defs::ClassKind,
    pub is_xhp: bool,
    pub has_xhp_keyword: bool,
    pub is_disposable: bool,
    pub name: &'a str,
    pub pos: pos::Pos<'a>,
    pub tparams: &'a [Tparam<'a>],
    pub where_constraints: &'a [WhereConstraint<'a>],
    pub consts: s_map::SMap<'a, ClassConst<'a>>,
    pub typeconsts: s_map::SMap<'a, TypeconstType<'a>>,
    pub pu_enums: s_map::SMap<'a, PuEnumType<'a>>,
    pub props: s_map::SMap<'a, ClassElt<'a>>,
    pub sprops: s_map::SMap<'a, ClassElt<'a>>,
    pub methods: s_map::SMap<'a, ClassElt<'a>>,
    pub smethods: s_map::SMap<'a, ClassElt<'a>>,
    /// the consistent_kind represents final constructor or __ConsistentConstruct
    pub construct: (Option<ClassElt<'a>>, oxidized::typing_defs::ConsistentKind),
    /// This includes all the classes, interfaces and traits this class is
    /// using.
    pub ancestors: s_map::SMap<'a, Ty<'a>>,
    pub req_ancestors: &'a [Requirement<'a>],
    /// the extends of req_ancestors
    pub req_ancestors_extends: s_set::SSet<'a>,
    pub extends: s_set::SSet<'a>,
    pub enum_type: Option<EnumType<'a>>,
    pub sealed_whitelist: Option<s_set::SSet<'a>>,
    pub decl_errors: Option<errors::Errors<'a>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TypeconstAbstractKind<'a> {
    TCAbstract(Option<Ty<'a>>),
    TCPartiallyAbstract,
    TCConcrete,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TypeconstType<'a> {
    pub abstract_: TypeconstAbstractKind<'a>,
    pub name: nast::Sid<'a>,
    pub constraint: Option<Ty<'a>>,
    pub type_: Option<Ty<'a>>,
    pub origin: &'a str,
    pub enforceable: (pos::Pos<'a>, bool),
    pub reifiable: Option<pos::Pos<'a>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PuEnumType<'a> {
    pub name: nast::Sid<'a>,
    pub is_final: bool,
    pub case_types: s_map::SMap<'a, (nast::Sid<'a>, oxidized::aast::ReifyKind)>,
    pub case_values: s_map::SMap<'a, (nast::Sid<'a>, Ty<'a>)>,
    pub members: s_map::SMap<'a, PuMemberType<'a>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PuMemberType<'a> {
    pub atom: nast::Sid<'a>,
    pub types: s_map::SMap<'a, (nast::Sid<'a>, Ty<'a>)>,
    pub exprs: s_map::SMap<'a, nast::Sid<'a>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EnumType<'a> {
    pub base: Ty<'a>,
    pub constraint: Option<Ty<'a>>,
}

pub use oxidized::typing_defs::RecordFieldReq;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RecordDefType<'a> {
    pub name: nast::Sid<'a>,
    pub extends: Option<nast::Sid<'a>>,
    pub fields: &'a [(nast::Sid<'a>, oxidized::typing_defs::RecordFieldReq)],
    pub abstract_: bool,
    pub pos: pos::Pos<'a>,
    pub errors: Option<errors::Errors<'a>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TypedefType<'a> {
    pub pos: pos::Pos<'a>,
    pub vis: oxidized::aast::TypedefVisibility,
    pub tparams: &'a [Tparam<'a>],
    pub constraint: Option<Ty<'a>>,
    pub type_: Ty<'a>,
    pub decl_errors: Option<errors::Errors<'a>>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DeserializationError<'a> {
    /// The type was valid, but some component thereof was a decl_ty when we
    /// expected a locl_phase ty, or vice versa.
    WrongPhase(&'a str),
    /// The specific type or some component thereof is not one that we support
    /// deserializing, usually because not enough information was serialized to be
    /// able to deserialize it again.
    NotSupported(&'a str),
    /// The input JSON was invalid for some reason.
    DeserializationError(&'a str),
}
