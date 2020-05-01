// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use crate::typing_object_get;
use crate::typing_phase::{self, MethodInstantiation};
use crate::{typing_naming, typing_subtype};
use crate::{Env, LocalId, ParamMode};
use arena_trait::Arena;
use decl_rust::decl_subst as subst;
use oxidized::aast_defs::Sid;
use oxidized::shallow_decl_defs::ShallowClass;
use oxidized::typing_defs_core::Ty_ as DTy_;
use oxidized::{aast, aast_defs, ast, ast_defs, local_id, pos::Pos};
use typing_ast_rust::typing_inference_env::IntoUnionIntersectionIterator;
use typing_defs_rust::typing_reason::PReason_;
use typing_defs_rust::{tast, ExpandEnv, FunParam, FuncBodyAnn, SavedEnv, Ty, Ty_};

pub fn fun<'a>(env: &mut Env<'a>, f: &'a ast::Fun_) -> tast::Fun_<'a> {
    let ast = f.body.ast.iter().map(|x| stmt(env, x)).collect();

    // We put empty vec below for all of those, since real conversion is unimplemented
    assert!(f.user_attributes.is_empty());
    assert!(f.file_attributes.is_empty());

    let named_ret = match &f.ret.1 {
        None => None,
        Some(ty) => {
            let ty = typing_naming::name_hint(env, ty);
            Some(ty.clone())
        }
    };
    let ret = tast::TypeHint(env.genv.return_info.type_.type_, named_ret);

    tast::Fun_ {
        span: f.span.clone(),
        annotation: SavedEnv,
        mode: f.mode,
        ret,
        name: typing_naming::canonicalize(&f.name),
        tparams: vec![],
        where_constraints: f.where_constraints.clone(),
        body: tast::FuncBody {
            ast,
            annotation: FuncBodyAnn,
        },
        fun_kind: f.fun_kind,
        variadic: tast::FunVariadicity::FVnonVariadic,
        params: vec![],
        user_attributes: vec![],
        file_attributes: vec![],
        external: f.external,
        namespace: f.namespace.clone(),
        doc_comment: f.doc_comment.clone(),
        static_: f.static_,
    }
}

pub fn stmt<'a>(env: &mut Env<'a>, ast::Stmt(pos, s): &'a ast::Stmt) -> tast::Stmt<'a> {
    tast::Stmt(pos.clone(), stmt_(env, s))
}

fn stmt_<'a>(env: &mut Env<'a>, s: &'a ast::Stmt_) -> tast::Stmt_<'a> {
    match s {
        ast::Stmt_::Noop => tast::Stmt_::Noop,
        ast::Stmt_::Markup(x) => markup(&x),
        ast::Stmt_::Expr(x) => tast::Stmt_::mk_expr(expr(env, x)),
        ast::Stmt_::Return(e) => match e.as_ref() {
            None => unimplemented!("return; not yet implemented"),
            Some(e) => {
                let te = expr(env, e);
                typing_subtype::sub_type(env, (te.0).1, env.genv.return_info.type_.type_);
                tast::Stmt_::mk_return(Some(te))
            }
        },
        x => unimplemented!("{:#?}", x),
    }
}

fn markup<'a>(s: &ast::Pstring) -> tast::Stmt_<'a> {
    tast::Stmt_::mk_markup(s.clone())
}

fn expr<'a>(env: &mut Env<'a>, ast::Expr(pos, e): &'a ast::Expr) -> tast::Expr<'a> {
    let (ty, e) = match e {
        ast::Expr_::True => {
            let ty = env.bld().bool(env.bld().mk_rwitness(pos));
            let e = tast::Expr_::True;
            (ty, e)
        }
        ast::Expr_::False => {
            let ty = env.bld().bool(env.bld().mk_rwitness(pos));
            let e = tast::Expr_::False;
            (ty, e)
        }
        ast::Expr_::Int(s) => {
            let ty = env.bld().int(env.bld().mk_rwitness(pos));
            let e = tast::Expr_::Int(s.clone());
            (ty, e)
        }
        ast::Expr_::Float(s) => {
            let ty = env.bld().float(env.bld().mk_rwitness(pos));
            let e = tast::Expr_::Float(s.clone());
            (ty, e)
        }
        ast::Expr_::Null => {
            let ty = env.bld().null(env.bld().mk_rwitness(pos));
            let e = tast::Expr_::Null;
            (ty, e)
        }
        ast::Expr_::String(s) => {
            let ty = env.bld().string(env.bld().mk_rwitness(pos));
            let e = tast::Expr_::String(s.clone());
            (ty, e)
        }
        ast::Expr_::Call(x) => {
            // TODO(hrust) pseudo functions, might_throw
            let (call_type, e, explicit_targs, el, unpacked_element) = &**x;
            let in_suspend = false;
            check_call(
                env,
                pos,
                call_type,
                e,
                explicit_targs,
                el,
                unpacked_element,
                in_suspend,
            )
        }
        ast::Expr_::Binop(op) => match op.as_ref() {
            (ast_defs::Bop::Eq(None), e1, e2) => {
                let te2 = expr(env, e2);
                let tast::Expr((_pos2, ty2), _) = te2;

                let te1 = assign(env, e1, ty2);
                let tast::Expr((_pos1, ty), _) = te1;

                // TODO(hrust): reactivity

                // If we are assigning a local variable to another local
                // variable then the expression ID associated with e2 is
                // transferred to e1
                match (e1, e2) {
                    (ast::Expr(_, ast::Expr_::Lvar(id1)), ast::Expr(_, ast::Expr_::Lvar(id2))) => {
                        let ast::Lid(_, x1) = id1.as_ref();
                        let ast::Lid(_, x2) = id2.as_ref();
                        if let Some(eid2) = env.get_local_expr_id(x2.into()) {
                            env.set_local_expr_id(x1.into(), eid2);
                        }
                    }
                    _ => (),
                };
                (
                    ty,
                    tast::Expr_::Binop(Box::new((ast_defs::Bop::Eq(None), te1, te2))),
                )
            }
            op => unimplemented!("{:#?}", op),
        },
        ast::Expr_::Lvar(id) => {
            // TODO(hrust): using var
            // TODO(hrust): implement check_defined=false
            let ast::Lid(pos, x) = id.as_ref();
            let ty = env.get_local_check_defined(pos, x.into());
            (ty, tast::Expr_::Lvar(id.clone()))
        }
        ast::Expr_::New(x) => {
            let (class_id, explicit_targs, args, unpacked_arg, ctor_pos) = &**x;
            // TODO(hrust) the following does not exist in the OCaml version and would be done
            // (properly) in the naming phase
            let ast::ClassId(p0, class_id_) = class_id;
            let class_id = match class_id_ {
                ast::ClassId_::CIexpr(e) => {
                    let ast::Expr(_p1, e) = e;
                    match e {
                        ast::Expr_::Id(id) => env
                            .bld()
                            .alloc(aast::ClassId(p0.clone(), aast::ClassId_::CI(*id.clone()))),
                        _ => class_id,
                    }
                }
                _ => class_id,
            };
            // TODO(hrust) might_throw
            let (class_id, targs, args, unpacked_arg, ty, ctor_fty) = new_object(
                env,
                class_id.annot(),
                class_id,
                explicit_targs,
                args,
                unpacked_arg,
            );
            let ctor_annot = (ctor_pos, ctor_fty);
            (
                ty,
                tast::Expr_::New(Box::new((class_id, targs, args, unpacked_arg, ctor_annot))),
            )
        }
        x => unimplemented!("{:#?}", x),
    };
    // TODO(hrust) set_tyvar_variance
    ast::Expr((&pos, ty), e)
}

fn check_call<'a>(
    env: &mut Env<'a>,
    pos: &Pos,
    call_type: &ast::CallType,
    e: &'a ast::Expr,
    explicit_targs: &Vec<ast::Targ>,
    el: &'a Vec<ast::Expr>,
    unpacked_element: &Option<ast::Expr>,
    in_suspend: bool,
) -> (Ty<'a>, tast::Expr_<'a>) {
    dispatch_call(
        env,
        pos,
        call_type,
        e,
        explicit_targs,
        el,
        unpacked_element,
        in_suspend,
    )
    // TODO(hrust) forget_fake_members
}

fn dispatch_call<'a>(
    env: &mut Env<'a>,
    pos: &Pos,
    call_type: &ast::CallType,
    ast::Expr(fpos, fun_expr): &'a ast::Expr,
    explicit_targs: &Vec<ast::Targ>,
    args: &'a Vec<ast::Expr>,
    unpacked_arg: &Option<ast::Expr>,
    _in_suspend: bool,
) -> (Ty<'a>, tast::Expr_<'a>) {
    match fun_expr {
        ast::Expr_::Id(x) => {
            let (function_ty, type_args) = fun_type_of_id(env, x, explicit_targs, args);
            let (args, unpacked_arg, return_ty) = call(env, pos, function_ty, args, unpacked_arg);
            // TODO(hrust) reactivity stuff
            let fun_expr = tast::Expr(
                (fpos, function_ty),
                tast::Expr_::Id(Box::new(typing_naming::canonicalize(x))),
            );
            (
                return_ty,
                tast::Expr_::mk_call(call_type.clone(), fun_expr, type_args, args, unpacked_arg),
            )
        }
        ast::Expr_::ObjGet(x) => {
            let (receiver, method_id, nullflavor) = x.as_ref();
            let ast::Expr(pos_method_id, method_id_) = method_id;
            match method_id_ {
                ast::Expr_::Id(method_id_) => {
                    // TODO(hrust) set is_method below depending on call type
                    let is_method = true;
                    let receiver = expr(env, receiver);
                    // TODO(hrust) use nullflavor here
                    let nullsafe = None;
                    let (function_ty, type_args) = typing_object_get::obj_get(
                        env,
                        is_method,
                        nullsafe,
                        &receiver,
                        method_id_,
                        explicit_targs,
                    );
                    // TODO(hrust) coroutine check
                    let (args, unpacked_arg, return_ty) =
                        call(env, pos, function_ty, args, unpacked_arg);
                    let method_id = tast::Expr(
                        (pos_method_id, function_ty),
                        tast::Expr_::Id(method_id_.clone()),
                    );
                    let fun_expr = tast::Expr(
                        (fpos, function_ty),
                        tast::Expr_::ObjGet(Box::new((receiver, method_id, nullflavor.clone()))),
                    );
                    (
                        return_ty,
                        tast::Expr_::mk_call(
                            call_type.clone(),
                            fun_expr,
                            type_args,
                            args,
                            unpacked_arg,
                        ),
                    )
                }
                _ => unimplemented!(),
            }
        }
        x => unimplemented!("{:#?}", x),
    }
}

fn fun_type_of_id<'a>(
    env: &mut Env<'a>,
    ast::Id(pos, id): &'a ast::Sid,
    targs: &Vec<ast::Targ>,
    _el: &'a Vec<ast::Expr>,
) -> (Ty<'a>, Vec<tast::Targ<'a>>) {
    let bld = env.builder();
    match env.provider().get_fun(id) {
        None => unimplemented!(),
        Some(f) => {
            match f.type_.get_node() {
                DTy_::Tfun(ft) => {
                    // TODO(hrust) transform_special_fun_ty
                    let ety_env = bld.env_with_self();
                    // TODO(hrust) below: strip_ns id
                    let targs = typing_phase::localize_targs(env, pos, id, &ft.tparams, targs);
                    // TODO(hrust) pessimize
                    let instantiation = MethodInstantiation {
                        use_pos: pos,
                        use_name: id,
                        explicit_targs: &targs,
                    };
                    let ft = typing_phase::localize_ft(ety_env, env, ft, Some(instantiation));
                    let fty = bld.fun(bld.alloc(PReason_::from(f.type_.get_reason())), ft);
                    // TODO(hrust) check deprecated
                    (fty, targs)
                }
                _ => panic!("Expected function type"),
            }
        }
    }
}

fn call<'a>(
    env: &mut Env<'a>,
    _pos: &Pos,
    fty: Ty<'a>,
    el: &'a Vec<ast::Expr>,
    unpacked_arg: &Option<ast::Expr>,
) -> (Vec<tast::Expr<'a>>, Option<tast::Expr<'a>>, Ty<'a>) {
    // TODO(hrust) missing bits
    match fty.get_node() {
        Ty_::Tfun(ft) => {
            // TODO(hrust) retype magic fun, variadic param, expected ty, set tyvar variance
            let tel = el
                .iter()
                .zip(&ft.params)
                .map(|(e, param)| check_arg(env, e, param))
                .collect();
            // TODO(hrust) rx check_call
            let (unpacked_arg, _arity, _did_unpack) = match unpacked_arg {
                None => (None, el.len(), false),
                Some(_) => unimplemented!(),
            };
            // TODO(hrust) arity, inout, rx adjusted return
            (tel, unpacked_arg, ft.return_)
        }
        x => unimplemented!("{:#?}", x),
    }
}

fn check_arg<'a>(env: &mut Env<'a>, e: &'a ast::Expr, param: &FunParam<'a>) -> tast::Expr<'a> {
    // TODO(hrust) derive expected arg
    let te = expr(env, e);
    call_param(env, &te, param);
    te
}

fn call_param<'a>(env: &mut Env<'a>, te: &tast::Expr<'a>, param: &FunParam<'a>) -> () {
    // TODO(hrust) param_modes, dep_ty, coercion
    typing_subtype::sub_type(env, (te.0).1, param.type_);
}

#[allow(dead_code)]
fn bind_param<'a>(env: &mut Env<'a>, ty1: Ty<'a>, param: &'a ast::FunParam) -> tast::FunParam<'a> {
    // TODO(hrust): if parameter has a default initializer, check its type
    let param_te: Option<tast::Expr<'a>> = match &param.expr {
        None => None,
        Some(e) => unimplemented!("{:?}", e),
    };
    // TODO(hrust): process user attributes
    let user_attributes: Vec<tast::UserAttribute<'a>> = if param.user_attributes.len() > 0 {
        unimplemented!("{:?}", param.user_attributes)
    } else {
        vec![]
    };
    let tparam = tast::FunParam {
        annotation: (&param.pos, ty1),
        type_hint: ast::TypeHint(ty1, param.type_hint.get_hint().clone()),
        is_variadic: param.is_variadic,
        pos: param.pos.clone(),
        name: param.name.clone(),
        expr: param_te,
        callconv: param.callconv,
        user_attributes,
        visibility: param.visibility,
    };
    // TODO(hrust): add type to locals env
    let mode = ParamMode::from(param.callconv);
    let id = LocalId::make_unscoped(&param.name);
    env.set_param(id, (ty1, mode));
    // TODO(hrust): has_accept_disposable_attribute
    // TODO(hrust): mutability
    tparam
}

/// Deal with assignment of a value of type ty2 to lvalue e1.
fn assign<'a>(
    env: &mut Env<'a>,
    ast::Expr(pos1, e1): &'a ast::Expr,
    ty2: Ty<'a>,
) -> tast::Expr<'a> {
    match e1 {
        ast::Expr_::Lvar(id) => {
            let aast_defs::Lid(_, x) = id.as_ref();
            set_valid_rvalue(env, pos1, x, ty2);
            // TODO(hrust): set_tyvar_variance
            tast::Expr((pos1, ty2), tast::Expr_::Lvar(id.clone()))
        }
        e1 => unimplemented!("{:?}", e1),
    }
}

fn set_valid_rvalue<'a>(env: &mut Env<'a>, p: &'a Pos, x: &'a local_id::LocalId, ty: Ty<'a>) {
    set_local(env, p, x, ty);
    // We are assigning a new value to the local variable, so we need to
    // generate a new expression id
    let id = env.ident();
    env.set_local_expr_id(x.into(), id);
}

fn set_local<'a>(env: &mut Env<'a>, _p: &'a Pos, x: &'a local_id::LocalId, ty: Ty<'a>) {
    // TODO(hrust): is_using_var
    env.set_local(x.into(), ty)
}

fn new_object<'a>(
    env: &mut Env<'a>,
    p: &'a Pos,
    class_id: &'a ast::ClassId,
    targs: &'a Vec<ast::Targ>,
    args: &'a Vec<ast::Expr>,
    unpacked_arg: &'a Option<ast::Expr>,
) -> (
    tast::ClassId<'a>,
    Vec<tast::Targ<'a>>,
    Vec<tast::Expr<'a>>,
    Option<tast::Expr<'a>>,
    Ty<'a>,
    Ty<'a>,
) {
    // TODO(hrust) missing parameters: expected, check_parent, check_not_abstract, is_using_clause
    let (tcid, targs, classes) = instantiable_cid(env, p, class_id, targs);
    // TODO(hrust) allow_abstract_bound_generic
    let (class_types_and_ctor_types, args): (Vec<_>, Vec<_>) = classes
        .iter()
        .map(|(_cid, class_info, cty)| {
            // TODO(hrust) check uninstantiable_error
            let targs = {
                // explicit type argument situation. But have I not done that yet in instantiable_cid -> ... -> static_class_id???
                targs.iter().map(|targ| targ.annot()).copied()
            };
            // TODO(hrust) invalid_new_disposable_error
            // TODO(hrust) deal with CIstatic
            // TODO(hrust) create expr dependent type
            // TODO(hrust) set_tyvar_variance
            // TODO(hrust) check_expected_ty
            let (args, unpacked_arg, ctor_fty) =
                call_construct(env, p, *class_info, targs, args, unpacked_arg, class_id);
            // TODO(hrust) new_inconsistent_kind error
            use ast::ClassId_ as CID;
            match class_id.get() {
                CID::CIparent | CID::CIexpr(_) => unimplemented!(),
                CID::CIstatic | CID::CIself | CID::CI(_) => ((cty, ctor_fty), (args, unpacked_arg)),
            }
        })
        .unzip();
    if class_types_and_ctor_types.len() != 1 {
        unimplemented!();
    }
    let (cty, ctor_fty) = class_types_and_ctor_types[0];
    let (args, unpacked_arg) = match args.into_iter().last() {
        None => unimplemented!(),
        Some(args) => args,
    };
    // TODO(hrust) create expression dependent type if necessary
    (tcid, targs, args, unpacked_arg, *cty, ctor_fty)
}

fn call_construct<'a>(
    env: &mut Env<'a>,
    p: &'a Pos,
    class: &'a ShallowClass,
    targ_tys: impl Iterator<Item = Ty<'a>>,
    args: &'a Vec<ast::Expr>,
    unpacked_arg: &'a Option<ast::Expr>,
    _class_id: &'a ast::ClassId,
) -> (Vec<tast::Expr<'a>>, Option<tast::Expr<'a>>, Ty<'a>) {
    // TODO(hrust) turn CIparent into CIstatic. WHY? o_O
    let mut ety_env = ExpandEnv {
        substs: subst::make_locl(env.bld(), &class.tparams, targ_tys),
        type_expansions: bumpalo::vec![in env.bld().alloc],
    };
    // TODO(hrust) check tparam constraints and where constraints
    // TODO(hrust) return any if xhp
    match env.get_construct(class) {
        None => unimplemented!(),
        Some(ctor) => {
            // TODO(hrust) check obj access and deprecation
            // TODO(hrust) pessimize
            let ctor_type = typing_phase::localize(&mut ety_env, env, &ctor.type_);
            // TODO(hrust) actually call the constructor and return its type and typed arguments
            let (args, unpacked_arg, _return_ty) = call(env, p, ctor_type, args, unpacked_arg);
            (args, unpacked_arg, ctor_type)
        }
    }
}

fn instantiable_cid<'a>(
    env: &mut Env<'a>,
    p: &'a Pos,
    class_id: &'a ast::ClassId,
    targs: &'a Vec<ast::Targ>,
) -> (
    tast::ClassId<'a>,
    Vec<tast::Targ<'a>>,
    Vec<(&'a Sid, &'a ShallowClass, Ty<'a>)>,
) {
    class_id_for_new(env, p, class_id, targs)
    // TODO(hrust) check instantiable
}

fn class_id_for_new<'a>(
    env: &mut Env<'a>,
    p: &'a Pos,
    class_id: &'a ast::ClassId,
    targs: &'a Vec<ast::Targ>,
) -> (
    tast::ClassId<'a>,
    Vec<tast::Targ<'a>>,
    Vec<(&'a Sid, &'a ShallowClass, Ty<'a>)>,
) {
    let (targs, class_id) = {
        let check_constraints = false;
        static_class_id(env, check_constraints, p, targs, class_id)
    };
    let provider = &env.genv.provider; // borrowing this here so I can chain iterators below
    let tys = class_id
        .annot()
        .1
        .into_union_inter_iter(&mut env.inference_env);
    // TODO(hrust) map TUtils.get_base_type
    let tys = tys
        .filter_map(|ty| {
            match ty.get_node() {
                Ty_::Tclass(sid, _, _) => {
                    match provider.get_class(sid.name()) {
                        None => None,
                        Some(class_info) => {
                            // TODO check generic, check generic is newable
                            Some((*sid, class_info, ty))
                        }
                    }
                }
                _ => None,
            }
        })
        .collect();
    (class_id, targs, tys)
}

fn static_class_id<'a>(
    env: &mut Env<'a>,
    check_constraints: bool,
    p: &'a Pos,
    targs: &'a Vec<ast::Targ>,
    class_id: &'a ast::ClassId,
) -> (Vec<tast::Targ<'a>>, tast::ClassId<'a>) {
    use ast::ClassId_ as CID;
    match class_id.get() {
        CID::CIparent | CID::CIself | CID::CIstatic | CID::CIexpr(_) => unimplemented!(),
        CID::CI(cid) => {
            // TODO(hrust) check if generic parameter
            match env.genv.provider.get_class(cid.name()) {
                None => unimplemented!(),
                Some(class_info) => {
                    let (ty, targs) = typing_phase::resolve_type_arguments_and_check_constraints(
                        env,
                        check_constraints,
                        cid.pos(),
                        cid,
                        class_id,
                        &class_info.tparams,
                        targs,
                    );
                    (
                        targs,
                        tast::ClassId((p, ty), tast::ClassId_::CI(cid.clone())),
                    )
                }
            }
        }
    }
}
