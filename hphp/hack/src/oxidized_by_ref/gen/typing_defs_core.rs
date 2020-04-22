// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
//
// @generated SignedSource<<802b26991597bf10e2d275ddcfda5150>>
//
// To regenerate this file, run:
//   hphp/hack/src/oxidized_by_ref/regen.sh

use serde::Serialize;

#[allow(unused_imports)]
use crate::*;

pub use crate::typing_reason as reason;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Visibility<'a> {
    Vpublic,
    Vprivate(&'a str),
    Vprotected(&'a str),
}

pub use oxidized::typing_defs_core::Exact;

pub use oxidized::typing_defs_core::ValKind;

pub use oxidized::typing_defs_core::ParamMutability;

pub use oxidized::typing_defs_core::FunTparamsKind;

pub use oxidized::typing_defs_core::ShapeKind;

pub use oxidized::typing_defs_core::ParamMode;

pub use oxidized::typing_defs_core::XhpAttrTag;

pub use oxidized::typing_defs_core::XhpAttr;

pub use oxidized::typing_defs_core::ConsistentKind;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DependentType<'a> {
    DTthis,
    DTcls(&'a str),
    DTexpr(ident::Ident<'a>),
}

pub use oxidized::typing_defs_core::DestructureKind;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Tparam<'a> {
    pub variance: oxidized::ast_defs::Variance,
    pub name: ast_defs::Id<'a>,
    pub constraints: &'a [(oxidized::ast_defs::ConstraintKind, Ty<'a>)],
    pub reified: oxidized::aast::ReifyKind,
    pub user_attributes: &'a [nast::UserAttribute<'a>],
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WhereConstraint<'a>(
    pub Ty<'a>,
    pub oxidized::ast_defs::ConstraintKind,
    pub Ty<'a>,
);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Ty<'a>(pub reason::Reason<'a>, pub &'a Ty_<'a>);

/// A shape may specify whether or not fields are required. For example, consider
/// this typedef:
///
/// ```
/// type ShapeWithOptionalField = shape(?'a' => ?int);
/// ```
///
/// With this definition, the field 'a' may be unprovided in a shape. In this
/// case, the field 'a' would have sf_optional set to true.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ShapeFieldType<'a> {
    pub optional: bool,
    pub ty: Ty<'a>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Ty_<'a> {
    /// The late static bound type of a class
    Tthis,
    /// Either an object type or a type alias, ty list are the arguments
    Tapply(nast::Sid<'a>, &'a [Ty<'a>]),
    /// Name of class, name of type const, remaining names of type consts
    Taccess(TaccessType<'a>),
    /// The type of the various forms of "array":
    ///
    /// ```
    /// Tarray (None, None)         => "array"
    /// Tarray (Some ty, None)      => "array<ty>"
    /// Tarray (Some ty1, Some ty2) => "array<ty1, ty2>"
    /// Tarray (None, Some ty)      => [invalid]
    /// ```
    Tarray(Option<Ty<'a>>, Option<Ty<'a>>),
    /// "Any" is the type of a variable with a missing annotation, and "mixed" is
    /// the type of a variable annotated as "mixed". THESE TWO ARE VERY DIFFERENT!
    /// Any unifies with anything, i.e., it is both a supertype and subtype of any
    /// other type. You can do literally anything to it; it's the "trust me" type.
    /// Mixed, on the other hand, is only a supertype of everything. You need to do
    /// a case analysis to figure out what it is (i.e., its elimination form).
    ///
    /// Here's an example to demonstrate:
    ///
    /// ```
    /// function f($x): int {
    ///   return $x + 1;
    /// }
    /// ```
    ///
    /// In that example, $x has type Tany. This unifies with anything, so adding
    /// one to it is allowed, and returning that as int is allowed.
    ///
    /// In contrast, if $x were annotated as mixed, adding one to that would be
    /// a type error -- mixed is not a subtype of int, and you must be a subtype
    /// of int to take part in addition. (The converse is true though -- int is a
    /// subtype of mixed.) A case analysis would need to be done on $x, via
    /// is_int or similar.
    ///
    /// mixed exists only in the decl_phase phase because it is desugared into ?nonnull
    /// during the localization phase.
    Tmixed,
    Tlike(Ty<'a>),
    /// Access to a Pocket Universe or Pocket Universes dependent type,
    /// denoted by Foo:@Bar.
    /// It might be unresolved at first (e.g. if Foo is a generic variable).
    /// Will be refined to Tpu, or to the actual type associated with an
    /// atom, once typechecking is successful.
    TpuAccess(Ty<'a>, nast::Sid<'a>),
    Tany(oxidized::tany_sentinel::TanySentinel),
    Terr,
    Tnonnull,
    /// A dynamic type is a special type which sometimes behaves as if it were a
    /// top type; roughly speaking, where a specific value of a particular type is
    /// expected and that type is dynamic, anything can be given. We call this
    /// behaviour "coercion", in that the types "coerce" to dynamic. In other ways it
    /// behaves like a bottom type; it can be used in any sort of binary expression
    /// or even have object methods called from it. However, it is in fact neither.
    ///
    /// it captures dynamicism within function scope.
    /// See tests in typecheck/dynamic/ for more examples.
    Tdynamic,
    /// Nullable, called "option" in the ML parlance.
    Toption(Ty<'a>),
    /// All the primitive types: int, string, void, etc.
    Tprim(aast::Tprim<'a>),
    /// A wrapper around fun_type, which contains the full type information for a
    /// function, method, lambda, etc.
    Tfun(FunType<'a>),
    /// Tuple, with ordered list of the types of the elements of the tuple.
    Ttuple(&'a [Ty<'a>]),
    /// Whether all fields of this shape are known, types of each of the
    /// known arms.
    Tshape(
        oxidized::typing_defs_core::ShapeKind,
        nast::shape_map::ShapeMap<'a, ShapeFieldType<'a>>,
    ),
    Tvar(ident::Ident<'a>),
    /// The type of a generic parameter. The constraints on a generic parameter
    /// are accessed through the lenv.tpenv component of the environment, which
    /// is set up when checking the body of a function or method. See uses of
    /// Typing_phase.localize_generic_parameters_with_bounds.
    Tgeneric(&'a str),
    /// Union type.
    /// The values that are members of this type are the union of the values
    /// that are members of the components of the union.
    /// Some examples (writing | for binary union)
    ///   Tunion []  is the "nothing" type, with no values
    ///   Tunion [int;float] is the same as num
    ///   Tunion [null;t] is the same as Toption t
    Tunion(&'a [Ty<'a>]),
    Tintersection(&'a [Ty<'a>]),
    /// Tdarray (ty1, ty2) => "darray<ty1, ty2>"
    Tdarray(Ty<'a>, Ty<'a>),
    /// Tvarray (ty) => "varray<ty>"
    Tvarray(Ty<'a>),
    /// Tvarray_or_darray (ty1, ty2) => "varray_or_darray<ty1, ty2>"
    TvarrayOrDarray(Ty<'a>, Ty<'a>),
    /// The type of an opaque type (e.g. a "newtype" outside of the file where it
    /// was defined) or enum. They are "opaque", which means that they only unify with
    /// themselves. However, it is possible to have a constraint that allows us to
    /// relax this. For example:
    ///
    ///   newtype my_type as int = ...
    ///
    /// Outside of the file where the type was defined, this translates to:
    ///
    ///   Tnewtype ((pos, "my_type"), [], Tprim Tint)
    ///
    /// Which means that my_type is abstract, but is subtype of int as well.
    Tnewtype(&'a str, &'a [Ty<'a>], Ty<'a>),
    /// see dependent_type
    Tdependent(DependentType<'a>, Ty<'a>),
    /// Tobject is an object type compatible with all objects. This type is also
    /// compatible with some string operations (since a class might implement
    /// __toString), but not with string type hints.
    ///
    /// Tobject is currently used to type code like:
    ///   ../test/typecheck/return_unknown_class.php
    Tobject,
    /// An instance of a class or interface, ty list are the arguments
    /// If exact=Exact, then this represents instances of *exactly* this class
    /// If exact=Nonexact, this also includes subclasses
    Tclass(
        nast::Sid<'a>,
        oxidized::typing_defs_core::Exact,
        &'a [Ty<'a>],
    ),
    /// Typing of Pocket Universe Expressions
    /// - first parameter is the enclosing class
    /// - second parameter is the name of the Pocket Universe Enumeration
    Tpu(Ty<'a>, nast::Sid<'a>),
    /// Typing of Pocket Universes type projections
    /// - first parameter is the Tgeneric in place of the member name
    /// - second parameter is the name of the type to project
    TpuTypeAccess(nast::Sid<'a>, nast::Sid<'a>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ConstraintType_<'a> {
    ThasMember(HasMember<'a>),
    /// The type of container destructuring via list() or splat `...`
    Tdestructure(Destructure<'a>),
    TCunion(Ty<'a>, ConstraintType<'a>),
    TCintersection(Ty<'a>, ConstraintType<'a>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct HasMember<'a> {
    pub name: nast::Sid<'a>,
    pub type_: Ty<'a>,
    /// This is required to check ambiguous object access, where sometimes
    /// HHVM would access the private member of a parent class instead of the
    /// one from the current class.
    pub class_id: nast::ClassId_<'a>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Destructure<'a> {
    /// This represents the standard parameters of a function or the fields in a list
    /// destructuring assignment. Example:
    ///
    /// function take(bool $b, float $f = 3.14, arraykey ...$aks): void {}
    /// function f((bool, float, int, string) $tup): void {
    ///   take(...$tup);
    /// }
    ///
    /// corresponds to the subtyping assertion
    ///
    /// (bool, float, int, string) <: splat([#1], [opt#2], ...#3)
    pub required: &'a [Ty<'a>],
    /// Represents the optional parameters in a function, only used for splats
    pub optional: &'a [Ty<'a>],
    /// Represents a function's variadic parameter, also only used for splats
    pub variadic: Option<Ty<'a>>,
    /// list() destructuring allows for partial matches on lists, even when the operation
    /// might throw i.e. list($a) = vec[];
    pub kind: oxidized::typing_defs_core::DestructureKind,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ConstraintType<'a>(pub reason::Reason<'a>, pub &'a ConstraintType_<'a>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum InternalType<'a> {
    LoclType(Ty<'a>),
    ConstraintType(ConstraintType<'a>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TaccessType<'a>(pub Ty<'a>, pub &'a [nast::Sid<'a>]);

/// represents reactivity of function
/// - None corresponds to non-reactive function
/// - Some reactivity - to reactive function with specified reactivity flavor
///
/// Nonreactive <: Local -t <: Shallow -t <: Reactive -t
///
/// MaybeReactive represents conditional reactivity of function that depends on
/// reactivity of function arguments
///
/// ```
///   <<__Rx>>
///   function f(<<__MaybeRx>> $g) { ... }
/// ```
///
/// call to function f will be treated as reactive only if $g is reactive
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Reactivity<'a> {
    Nonreactive,
    Local(Option<Ty<'a>>),
    Shallow(Option<Ty<'a>>),
    Reactive(Option<Ty<'a>>),
    Pure(Option<Ty<'a>>),
    MaybeReactive(&'a Reactivity<'a>),
    RxVar(Option<&'a Reactivity<'a>>),
}

/// The type of a function AND a method.
/// A function has a min and max arity because of optional arguments
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FunType<'a> {
    pub arity: FunArity<'a>,
    pub tparams: &'a [Tparam<'a>],
    pub where_constraints: &'a [WhereConstraint<'a>],
    pub params: FunParams<'a>,
    /// Carries through the sync/async information from the aast
    pub ret: PossiblyEnforcedTy<'a>,
    pub reactive: Reactivity<'a>,
    pub flags: isize,
}

/// Arity information for a fun_type; indicating the minimum number of
/// args expected by the function and the maximum number of args for
/// standard, non-variadic functions or the type of variadic argument taken
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FunArity<'a> {
    /// min; max is List.length ft_params
    Fstandard(isize),
    /// PHP5.6-style ...$args finishes the func declaration.
    /// min ; variadic param type
    Fvariadic(isize, FunParam<'a>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ParamRxAnnotation<'a> {
    ParamRxVar,
    ParamRxIfImpl(Ty<'a>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PossiblyEnforcedTy<'a> {
    /// True if consumer of this type enforces it at runtime
    pub enforced: bool,
    pub type_: Ty<'a>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FunParam<'a> {
    pub pos: pos::Pos<'a>,
    pub name: Option<&'a str>,
    pub type_: PossiblyEnforcedTy<'a>,
    pub rx_annotation: Option<ParamRxAnnotation<'a>>,
    pub flags: isize,
}

pub type FunParams<'a> = &'a [FunParam<'a>];
