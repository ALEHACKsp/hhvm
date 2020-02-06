// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use core_utils_rust as core_utils;
use namespaces_rust as namespaces;
use naming_attributes_rust as naming_attributes;
use naming_special_names_rust as sn;
use std::collections::HashSet;

use oxidized::{
    aast_visitor::{NodeMut, VisitorMut},
    ast::*,
    ast_defs::*,
    namespace_env,
};

fn is_special_identifier(name: &str) -> bool {
    use lazy_static::lazy_static;

    lazy_static! {
        static ref SPECIAL_IDENTIFIERS: HashSet<&'static str> = vec![
            sn::members::M_CLASS,
            sn::classes::PARENT,
            sn::classes::SELF,
            sn::classes::STATIC,
            sn::special_idents::THIS,
            sn::special_idents::DOLLAR_DOLLAR,
            sn::typehints::WILDCARD,
        ]
        .into_iter()
        .collect();
    }

    SPECIAL_IDENTIFIERS.contains(&name)
}

#[derive(Clone)]
struct Env {
    // TODO(hrust) I wanted to make namespace and type_params str's references but couldn't
    // find a way to specify that the lifetimes of these outlived the node I was taking them from
    namespace: ocamlrep::rc::RcOc<namespace_env::Env>,
    type_params: HashSet<String>,
    in_ppl: bool,
}

impl Env {
    fn make(namespace: ocamlrep::rc::RcOc<namespace_env::Env>) -> Self {
        Self {
            namespace,
            type_params: HashSet::new(),
            in_ppl: false,
        }
    }

    // TODO: While elaboration for codegen and typing is similar, there are currently a
    // couple differences between the two and are toggled by this flag (XHP).
    // It would be nice to eventually eliminate the discrepancies between the two.
    pub fn in_codegen(&self) -> bool {
        self.namespace.is_codegen
    }

    fn extend_tparams(&mut self, tparaml: &[Tparam]) {
        for tparam in tparaml {
            self.type_params.insert(tparam.name.1.clone());
        }
    }

    fn extend_tparams_puenum(&mut self, cases: &[(Sid, ReifyKind)]) {
        for case in cases {
            self.type_params.insert((case.0).1.clone());
        }
    }

    fn elaborate_type_name(&self, id: &mut Id) {
        let name = &id.1;
        if self.type_params.contains::<str>(name)
            || is_special_identifier(name)
            || name.starts_with("$")
        {
            return;
        } else {
            id.1 =
                namespaces::elaborate_id(&self.namespace, namespaces::ElaborateKind::Class, id).1;
        }
    }

    fn handle_special_calls(&self, call: &mut Expr_) {
        match call {
            Expr_::Call(c) => {
                let (func, args) = (&c.1, &mut c.3);
                match func {
                    Expr(_, Expr_::Id(id))
                        if id.1 == sn::autoimported_functions::FUN_ && args.len() == 1 =>
                    {
                        match &args[0] {
                            Expr(p, Expr_::String(fn_name)) => {
                                let fn_name = core_utils::add_ns(&fn_name);
                                args[0] = Expr(p.clone(), Expr_::String(fn_name.to_string()));
                            }
                            _ => (),
                        }
                    }
                    Expr(_, Expr_::Id(id))
                        if (id.1 == sn::autoimported_functions::METH_CALLER
                            || id.1 == sn::autoimported_functions::CLASS_METH)
                            && args.len() == 2
                            && !self.in_codegen() =>
                    {
                        match &args[0] {
                            Expr(p, Expr_::String(cl_name)) => {
                                let cl_name = core_utils::add_ns(&cl_name);
                                args[0] = Expr(p.clone(), Expr_::String(cl_name.to_string()));
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }
}

fn is_reserved_type_hint(name: &str) -> bool {
    let base_name = core_utils::strip_ns(name);
    return sn::typehints::is_reserved_type_hint(&base_name) || sn::rx::is_reactive_typehint(name);
}

struct ElaborateNamespacesVisitor {}

impl VisitorMut for ElaborateNamespacesVisitor {
    type Context = Env;
    type Ex = Pos;
    type Fb = ();
    type En = ();
    type Hi = ();

    fn object(
        &mut self,
    ) -> &mut dyn VisitorMut<
        Context = Self::Context,
        Ex = Self::Ex,
        Fb = Self::Fb,
        En = Self::En,
        Hi = Self::Hi,
    > {
        self
    }

    // Namespaces were already precomputed by ElaborateDefs
    // The following functions just set the namespace env correctly
    fn visit_class_(&mut self, env: &mut Env, cd: &mut Class_) {
        let mut env = env.clone();
        env.in_ppl = naming_attributes::mem(
            sn::user_attributes::PROBABILISTIC_MODEL,
            &cd.user_attributes,
        );
        env.namespace = cd.namespace.clone();
        env.extend_tparams(&cd.tparams.list);
        cd.recurse(&mut env, self.object());
    }

    fn visit_typedef(&mut self, env: &mut Env, td: &mut Typedef) {
        let mut env = env.clone();
        env.namespace = td.namespace.clone();
        env.extend_tparams(&td.tparams);
        td.recurse(&mut env, self.object());
    }

    fn visit_def(&mut self, env: &mut Env, def: &mut Def) {
        match &def {
            // need to handle it ourselves, because in visit_fun_ is
            // called both for toplevel functions and lambdas
            Def::Fun(f) => {
                let mut env = env.clone();
                env.namespace = f.namespace.clone();
                env.extend_tparams(&f.tparams);
                def.recurse(&mut env, self.object());
            }
            Def::SetNamespaceEnv(nsenv) => {
                env.namespace = (**nsenv).clone();
            }
            _ => def.recurse(env, self.object()),
        }
    }

    fn visit_method_(&mut self, env: &mut Env, m: &mut Method_) {
        let mut env = env.clone();
        env.extend_tparams(&m.tparams);
        m.recurse(&mut env, self.object());
    }

    // Codegen does not elaborate traits in the trait redeclaration node.
    // TODO: This should be changed if this feature is to be shipped.
    // Also change: class_method_trait_resolution in emit_class
    // T56629465
    fn visit_method_redeclaration(&mut self, env: &mut Env, mt: &mut MethodRedeclaration) {
        if env.in_codegen() {
            let old_trait = mt.trait_.clone();
            mt.recurse(env, self.object());
            mt.trait_ = old_trait;
        } else {
            mt.recurse(env, self.object());
        }
    }

    fn visit_pu_enum(&mut self, env: &mut Env, pue: &mut PuEnum) {
        let mut env = env.clone();
        env.extend_tparams_puenum(&pue.case_types);
        pue.recurse(&mut env, self.object());
    }

    fn visit_gconst(&mut self, env: &mut Env, gc: &mut Gconst) {
        let mut env = env.clone();
        env.namespace = gc.namespace.clone();
        gc.recurse(&mut env, self.object());
    }

    fn visit_file_attribute(&mut self, env: &mut Env, fa: &mut FileAttribute) {
        let mut env = env.clone();
        env.namespace = fa.namespace.clone();
        fa.recurse(&mut env, self.object());
    }

    fn visit_record_def(&mut self, env: &mut Env, rd: &mut RecordDef) {
        rd.name =
            namespaces::elaborate_id(&env.namespace, namespaces::ElaborateKind::Record, &rd.name);
        match &mut rd.extends {
            Some(Hint(_, h_)) => {
                let h = h_.as_mut();
                match h {
                    Hint_::Happly(sid, _hl) => {
                        let new_name = namespaces::elaborate_id(
                            &env.namespace,
                            namespaces::ElaborateKind::Record,
                            &sid,
                        );
                        std::mem::replace(sid, new_name);
                    }
                    _ => (),
                }
            }
            _ => (),
        }
        rd.recurse(env, self.object());
    }

    // I don't think we need to visit blocks because we got rid of let bindings :)

    fn visit_catch(&mut self, env: &mut Env, catch: &mut Catch) {
        let exception_sid = &mut catch.0;
        env.elaborate_type_name(exception_sid);
        catch.recurse(env, self.object());
    }

    // I don't think we need to visit stmts because we got rid of let bindings :)

    // Lfun and Efun as Expr_:: nodes so update the env in visit_expr_

    // Actually rewrites the names
    fn visit_expr_(&mut self, env: &mut Env, e: &mut Expr_) {
        // Sets env for lambdas
        let orig_in_ppl_flag = env.in_ppl;
        match e {
            Expr_::Efun(_) | Expr_::Lfun(_) => {
                env.in_ppl = false;
            }
            _ => (),
        }

        match e {
            Expr_::Call(c) => {
                let (call_type, func, targs, args, uargs) =
                    (&mut c.0, &mut c.1, &mut c.2, &mut c.3, &mut c.4);

                // Recurse first due to borrow order
                call_type.accept(env, self.object());
                targs.accept(env, self.object());
                args.accept(env, self.object());
                uargs.accept(env, self.object());

                if let Some(sid) = func.1.as_id_mut() {
                    if !(sn::special_functions::is_special_function(&sid.1)
                        || (sn::ppl_functions::is_reserved(&sid.1) && env.in_ppl))
                    {
                        sid.1 = namespaces::elaborate_id(
                            &env.namespace,
                            namespaces::ElaborateKind::Fun,
                            sid,
                        )
                        .1;
                        env.handle_special_calls(e);
                    }
                } else {
                    func.accept(env, self.object());
                }
            }
            Expr_::FunctionPointer(fp) => {
                let (name, targs) = (&mut fp.0, &mut fp.1);
                if let Some(sid) = name.1.as_id_mut() {
                    sid.1 = namespaces::elaborate_id(
                        &env.namespace,
                        namespaces::ElaborateKind::Fun,
                        sid,
                    )
                    .1;
                } else {
                    name.accept(env, self.object());
                }
                targs.accept(env, self.object());
            }
            Expr_::ObjGet(og) => {
                let (obj, expr, nullsafe) = (&mut og.0, &mut og.1, &mut og.2);
                if let Expr_::Id(..) = expr.1 {
                } else {
                    expr.accept(env, self.object());
                }

                obj.accept(env, self.object());
                nullsafe.accept(env, self.object());
            }
            Expr_::Id(sid) if !((sid.1 == "NAN" || sid.1 == "INF") && env.in_codegen()) => {
                sid.1 =
                    namespaces::elaborate_id(&env.namespace, namespaces::ElaborateKind::Const, sid)
                        .1;
            }
            Expr_::PUIdentifier(pui) => {
                let class_id = &mut pui.0;
                if let Some(e) = class_id.1.as_ciexpr_mut() {
                    if let Some(sid) = e.1.as_id_mut() {
                        env.elaborate_type_name(sid);
                    } else {
                        e.accept(env, self.object());
                    }
                } else {
                    class_id.accept(env, self.object());
                }
            }
            Expr_::New(n) => {
                let (class_id, targs, args, unpacked_el) = (&mut n.0, &mut n.1, &mut n.2, &mut n.3);
                if let Some(e) = class_id.1.as_ciexpr_mut() {
                    if let Some(sid) = e.1.as_id_mut() {
                        env.elaborate_type_name(sid);
                    } else {
                        e.accept(env, self.object());
                    }
                } else {
                    class_id.accept(env, self.object());
                }
                targs.accept(env, self.object());
                args.accept(env, self.object());
                unpacked_el.accept(env, self.object());
            }
            Expr_::Record(r) => {
                let (record_name, l) = (&mut r.0, &mut r.2);
                env.elaborate_type_name(record_name);
                l.accept(env, self.object());
            }
            Expr_::ClassConst(cc) => {
                let type_ = &mut cc.0;
                if let Some(e) = type_.1.as_ciexpr_mut() {
                    if let Some(sid) = e.1.as_id_mut() {
                        env.elaborate_type_name(sid);
                    } else {
                        e.accept(env, self.object());
                    }
                } else {
                    type_.accept(env, self.object());
                }
            }
            Expr_::ClassGet(cg) => {
                let (class_id, class_get_expr) = (&mut cg.0, &mut cg.1);
                if let Some(e) = class_id.1.as_ciexpr_mut() {
                    if let Some(sid) = e.1.as_id_mut() {
                        env.elaborate_type_name(sid);
                    } else {
                        e.accept(env, self.object());
                    }
                } else {
                    class_id.accept(env, self.object());
                }
                class_get_expr.accept(env, self.object());
            }
            Expr_::Xml(x) if !env.in_codegen() => {
                let (xml_id, attributes, el) = (&mut x.0, &mut x.1, &mut x.2);
                env.elaborate_type_name(xml_id);
                attributes.recurse(env, self.object());
                el.recurse(env, self.object());
            }
            _ => e.recurse(env, self.object()),
        }
        env.in_ppl = orig_in_ppl_flag;
    }

    fn visit_hint_(&mut self, env: &mut Env, hint: &mut Hint_) {
        fn is_rx(x: &str) -> bool {
            x == sn::rx::RX || x == sn::rx::RX_LOCAL || x == sn::rx::RX_SHALLOW
        }
        fn is_xhp_screwup(x: &str) -> bool {
            x == "Xhp" || x == ":Xhp" || x == "XHP"
        }
        match hint {
            Hint_::Happly(sid, _) if is_xhp_screwup(&sid.1) => (),
            Hint_::Happly(sid, hints) if is_rx(&sid.1) && hints.len() == 1 => match *hints[0].1 {
                Hint_::Hfun(_) => (),
                _ => {
                    env.elaborate_type_name(sid);
                }
            },
            Hint_::Happly(sid, _) if is_rx(&sid.1) => {
                env.elaborate_type_name(sid);
            }
            Hint_::Happly(sid, _) if is_reserved_type_hint(&sid.1) && !env.in_codegen() => (),
            Hint_::Happly(sid, _) => {
                env.elaborate_type_name(sid);
            }
            _ => (),
        }
        hint.recurse(env, self.object());
    }

    fn visit_shape_field_name(&mut self, env: &mut Env, sfn: &mut ShapeFieldName) {
        match sfn {
            ShapeFieldName::SFclassConst(id, _) => {
                env.elaborate_type_name(id);
            }
            _ => (),
        }
        sfn.recurse(env, self.object());
    }

    fn visit_user_attribute(&mut self, env: &mut Env, ua: &mut UserAttribute) {
        if !sn::user_attributes::is_reserved(&ua.name.1) {
            env.elaborate_type_name(&mut ua.name);
        }
        ua.recurse(env, self.object());
    }

    fn visit_insteadof_alias(&mut self, env: &mut Env, alias: &mut InsteadofAlias) {
        let (replacement_sid, orig_sids) = (&mut alias.0, &mut alias.2);
        env.elaborate_type_name(replacement_sid);
        for sid in orig_sids.iter_mut() {
            env.elaborate_type_name(sid);
        }
        alias.recurse(env, self.object());
    }

    fn visit_use_as_alias(&mut self, env: &mut Env, alias: &mut UseAsAlias) {
        let sid_option = &mut alias.0;
        match sid_option {
            Some(sid) => {
                env.elaborate_type_name(sid);
            }
            _ => (),
        }
    }

    fn visit_xhp_child(&mut self, env: &mut Env, child: &mut XhpChild) {
        match child {
            XhpChild::ChildName(sid)
                if !env.in_codegen()
                    && !sn::xhp::is_reserved(&sid.1)
                    && !sn::xhp::is_xhp_category(&sid.1) =>
            {
                env.elaborate_type_name(sid);
            }
            _ => (),
        }
        child.recurse(env, self.object());
    }
}

pub fn elaborate_program(e: ocamlrep::rc::RcOc<namespace_env::Env>, defs: &mut Program) {
    let mut env = Env::make(e);
    let mut visitor = ElaborateNamespacesVisitor {};
    for mut def in defs.into_iter() {
        visitor.visit_def(&mut env, &mut def);
    }
}
