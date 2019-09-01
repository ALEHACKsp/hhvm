// Copyright (c) 2019, Facebook, Inc.
// All rights reserved.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use oxidized::{
    map::Map,
    namespace_env::Env as NamespaceEnv,
    pos::Pos,
    prim_defs::Comment,
    relative_path::RelativePath,
    s_map::SMap,
    s_set::SSet,
    {aast, aast_defs, ast_defs, file_info},
};

use parser_rust::{
    indexed_source_text::IndexedSourceText, lexable_token::LexablePositionedToken,
    positioned_syntax::PositionedSyntaxTrait, source_text::SourceText, syntax::*, syntax_error,
    syntax_trait::SyntaxTrait,
};

use ocamlvalue_macro::Ocamlvalue;

use std::collections::HashSet;
use std::result::Result::{Err, Ok};

macro_rules! not_impl {
    () => {
        panic!("NOT IMPLEMENTED")
    };
}

macro_rules! aast {
    ($ty:ident) =>  {oxidized::aast::$ty};
    // NOTE: In <,> pattern, comma prevents rustfmt eating <>
    ($ty:ident<,>) =>  {oxidized::aast::$ty<Pos, (), (), ()>}
}

macro_rules! ret {
    ($ty:ty) => { std::result::Result<$ty, Error<Syntax<T, V>>> }
}

macro_rules! ret_aast {
    ($ty:ident) => { std::result::Result<aast!($ty), Error<Syntax<T, V>>> };
    ($ty:ident<,>) => { std::result::Result<aast!($ty<,>), Error<Syntax<T, V>>> }
}

#[derive(Debug, Clone)]
pub enum LiftedAwaitKind {
    LiftedFromStatement,
    LiftedFromConcurrent,
}

#[derive(Debug, Clone)]
pub struct LiftedAwaits {
    pub awaits: Vec<(Option<aast::Id>, aast!(Expr<,>))>,
    lift_kind: LiftedAwaitKind,
}

impl LiftedAwaits {
    fn lift_kind(&self) -> LiftedAwaitKind {
        self.lift_kind.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Env<'a> {
    codegen: bool,
    elaborate_namespaces: bool,
    include_line_comments: bool,
    keep_errors: bool,
    quick_mode: bool,
    /* Show errors even in quick mode. Does not override keep_errors. Hotfix
     * until we can properly set up saved states to surface parse errors during
     * typechecking properly. */
    show_all_errors: bool,
    lower_coroutines: bool,
    fail_open: bool,
    file_mode: file_info::Mode,
    top_level_statements: bool, /* Whether we are (still) considering TLSs*/

    pub saw_yield: bool, /* Information flowing back up */
    pub lifted_awaits: Option<LiftedAwaits>,
    pub tmp_var_counter: isize,
    /* Whether we've seen COMPILER_HALT_OFFSET. The value of COMPILER_HALT_OFFSET
      defaults to 0 if HALT_COMPILER isn't called.
      None -> COMPILER_HALT_OFFSET isn't in the source file
      Some 0 -> COMPILER_HALT_OFFSET is in the source file, but HALT_COMPILER isn't
      Some x -> COMPILER_HALT_OFFSET is in the source file,
                HALT_COMPILER is at x bytes offset in the file.
    */
    pub saw_compiler_halt_offset: Option<isize>,
    pub cls_reified_generics: HashSet<String>,
    pub in_static_method: bool,
    pub parent_maybe_reified: bool,
    /* This provides a generic mechanism to delay raising parsing errors;
     * since we're moving FFP errors away from CST to a stage after lowering
     * _and_ want to prioritize errors before lowering, the lowering errors
     * must be merely stored when the lowerer runs (until check for FFP runs (on AST)
     * and raised _after_ FFP error checking (unless we run the lowerer twice,
     * which would be expensive). */
    pub lowpri_errors: Vec<(Pos, String)>,

    pub indexed_source_text: &'a IndexedSourceText<'a>,
    pub auto_ns_map: &'a [(String, String)],
}

impl<'a> Env<'a> {
    pub fn make(indexed_source_text: &'a IndexedSourceText<'a>) -> Self {
        Env {
            codegen: false,
            elaborate_namespaces: true,
            include_line_comments: false,
            keep_errors: false,
            quick_mode: false,
            show_all_errors: false,
            lower_coroutines: true,
            fail_open: true,
            file_mode: file_info::Mode::Mpartial,
            top_level_statements: false,
            saw_yield: false,
            lifted_awaits: None,
            tmp_var_counter: 0,
            saw_compiler_halt_offset: None,
            cls_reified_generics: HashSet::new(),
            in_static_method: false,
            parent_maybe_reified: false,
            lowpri_errors: vec![],
            indexed_source_text,
            auto_ns_map: &[],
        }
    }

    fn file_mode(&self) -> file_info::Mode {
        self.file_mode
    }

    fn should_surface_error(&self) -> bool {
        (!self.quick_mode || self.show_all_errors) && self.keep_errors
    }

    fn is_typechecker(&self) -> bool {
        !self.codegen
    }

    fn codegen(&self) -> bool {
        self.codegen
    }

    fn source_text(&self) -> &SourceText<'a> {
        self.indexed_source_text.source_text
    }

    fn lower_coroutines(&self) -> bool {
        self.lower_coroutines
    }

    fn fail_open(&self) -> bool {
        self.fail_open
    }
}

pub enum Error<Node> {
    APIMissingSyntax(String, Node),
    LowererInvariantFailure(String, String),
    Failwith(String),
}

#[derive(Ocamlvalue)]
pub struct Result {
    ast: aast!(Program<,>),
    comments: Vec<(Pos, Comment)>,
}

use parser_core_types::syntax::SyntaxVariant::*;

pub trait Lowerer<'a, T, V>
where
    T: LexablePositionedToken<'a>,
    Syntax<T, V>: PositionedSyntaxTrait,
    V: SyntaxValueWithKind,
{
    fn make_empty_ns_env(env: &Env) -> NamespaceEnv {
        NamespaceEnv::empty(Vec::from(env.auto_ns_map), env.codegen())
    }

    fn mode_annotation(mode: file_info::Mode) -> file_info::Mode {
        match mode {
            file_info::Mode::Mphp => file_info::Mode::Mdecl,
            m => m,
        }
    }

    fn p_pos(node: &Syntax<T, V>, env: &Env) -> Pos {
        node.position_exclusive(env.indexed_source_text)
            .unwrap_or(Pos::make_none())
    }

    fn raise_parsing_error(node: &Syntax<T, V>, env: &mut Env, msg: &str) {
        not_impl!()
    }

    fn raise_parsing_error_pos(pos: Pos, env: &mut Env, msg: &str) {
        // TODO: enable should_surface_errors
        if env.codegen() && !env.lower_coroutines() {
            env.lowpri_errors.push((pos, String::from(msg)))
        }
    }

    #[inline]
    fn failwith<N>(msg: &str) -> ret!(N) {
        Err(Error::Failwith(String::from(msg)))
    }

    #[inline]
    fn text(node: &Syntax<T, V>, env: &Env) -> String {
        String::from(node.text(env.source_text()))
    }

    fn missing_syntax<N>(
        fallback: Option<N>,
        expecting: &'static str,
        node: &Syntax<T, V>,
        env: &Env,
    ) -> ret!(N) {
        //TODO:
        let pos = Self::p_pos(node, env);
        Err(Error::LowererInvariantFailure(
            String::from(""),
            String::from(""),
        ))
    }

    fn mp_optional<S>(
        p: &dyn Fn(&Syntax<T, V>, &Env) -> ret!(S),
        node: &Syntax<T, V>,
        env: &Env,
    ) -> ret!(Option<S>) {
        match &node.syntax {
            Missing => Ok(None),
            _ => p(node, env).map(Some),
        }
    }

    fn pos_qualified_name(node: &Syntax<T, V>, env: &Env) -> ret_aast!(Sid) {
        not_impl!()
    }

    fn pos_name(node: &Syntax<T, V>, env: &mut Env) -> ret_aast!(Sid) {
        match &node.syntax {
            QualifiedName(_) => Self::pos_qualified_name(node, env),
            SimpleTypeSpecifier(child) => Self::pos_name(&child.simple_type_specifier, env),
            _ => {
                let name = node.text(env.indexed_source_text.source_text);
                if name == "__COMPILER_HALT_OFFSET__" {
                    env.saw_compiler_halt_offset = Some(0);
                }
                let p = if name == "__LINE__" {
                    Pos::make_none()
                } else {
                    Self::p_pos(node, env)
                };
                Ok(ast_defs::Id(p, String::from(name)))
            }
        }
    }

    fn as_list(node: &Syntax<T, V>) -> Vec<&Syntax<T, V>> {
        fn strip_list_item<T1, V1>(node: &Syntax<T1, V1>) -> &Syntax<T1, V1> {
            match node {
                Syntax {
                    syntax: ListItem(i),
                    ..
                } => &i.list_item,
                x => x,
            }
        }

        match node {
            Syntax {
                syntax: SyntaxList(synl),
                ..
            } => synl.iter().map(strip_list_item).collect(),
            Syntax {
                syntax: Missing, ..
            } => vec![],
            syn => vec![syn],
        }
    }

    fn p_hint(node: &Syntax<T, V>, env: &Env) -> ret_aast!(Hint) {
        not_impl!()
    }

    fn p_simple_initializer(node: &Syntax<T, V>, env: &Env) -> ret_aast!(Expr<,>) {
        match &node.syntax {
            SimpleInitializer(child) => Self::p_expr(&child.simple_initializer_value, env),
            _ => Self::missing_syntax(None, "simple initializer", node, env),
        }
    }

    fn p_expr(node: &Syntax<T, V>, env: &Env) -> ret_aast!(Expr<,>) {
        not_impl!()
    }

    fn p_stmt(node: &Syntax<T, V>, env: &mut Env) -> ret_aast!(Stmt<,>) {
        // TODO: clear_statement_scope & extract_and_push_docblock
        let pos = Self::p_pos(node, env);
        match &node.syntax {
            MarkupSection(_) => Self::p_markup(node, env),
            _ => not_impl!(),
        }
    }

    fn is_hashbang(text: &Syntax<T, V>) -> bool {
        not_impl!()
    }

    fn p_markup(node: &Syntax<T, V>, env: &mut Env) -> ret_aast!(Stmt<,>) {
        match &node.syntax {
            MarkupSection(child) => {
                let markup_prefix = &child.markup_prefix;
                let markup_text = &child.markup_text;
                let markup_expression = &child.markup_expression;
                let pos = Self::p_pos(node, env);
                let has_dot_hack_extension = pos.filename().ends_with(".hack");
                if has_dot_hack_extension {
                    Self::raise_parsing_error(node, env, &syntax_error::error1060);
                } else if markup_prefix.value.is_missing()
                    && markup_text.width() > 0
                    && !Self::is_hashbang(&markup_text)
                {
                    Self::raise_parsing_error(node, env, &syntax_error::error1001);
                }
                let expr = match &markup_expression.syntax {
                    Missing => None,
                    ExpressionStatement(e) => {
                        Some(Self::p_expr(&e.expression_statement_expression, env)?)
                    }
                    _ => Self::failwith("expression expected")?,
                };
                let stmt_ = aast::Stmt_::Markup((pos.clone(), Self::text(&markup_text, env)), expr);
                Ok(aast::Stmt(pos, Box::new(stmt_)))
            }
            _ => Self::failwith("invalid node"),
        }
    }

    fn p_def(node: &Syntax<T, V>, env: &mut Env) -> ret!(Vec<aast!(Def<,>)>) {
        match &node.syntax {
            ConstDeclaration(child) => {
                let ty = &child.const_type_specifier;
                let decls = Self::as_list(&child.const_declarators);
                let mut defs = vec![];
                for decl in decls.iter() {
                    let def = match &decl.syntax {
                        ConstantDeclarator(child) => {
                            let name = &child.constant_declarator_name;
                            let init = &child.constant_declarator_initializer;
                            let gconst = aast::Gconst {
                                annotation: (),
                                mode: Self::mode_annotation(env.file_mode()),
                                name: Self::pos_name(name, env)?,
                                type_: Self::mp_optional(&Self::p_hint, ty, env)?,
                                value: Self::p_simple_initializer(init, env)?,
                                namespace: Self::make_empty_ns_env(env),
                                span: Self::p_pos(node, env),
                            };
                            aast::Def::Constant(gconst)
                        }
                        _ => Self::missing_syntax(None, "constant declaration", decl, env)?,
                    };
                    defs.push(def);
                }
                Ok(defs)
            }
            InclusionDirective(child)
                if env.file_mode() != file_info::Mode::Mdecl
                    && env.file_mode() != file_info::Mode::Mphp
                    || env.codegen() =>
            {
                let expr = Self::p_expr(&child.inclusion_expression, env)?;
                Ok(vec![aast::Def::Stmt(aast::Stmt(
                    Self::p_pos(node, env),
                    Box::new(aast::Stmt_::Expr(expr)),
                ))])
            }
            _ => Ok(vec![aast::Def::Stmt(Self::p_stmt(node, env)?)]),
        }
    }

    fn p_program(node: &Syntax<T, V>, env: &mut Env) -> ret_aast!(Program<,>) {
        let nodes = Self::as_list(node);
        let mut acc = vec![];
        for i in 0..nodes.len() {
            match nodes[i] {
                // TODO: handle Halt
                Syntax {
                    syntax: EndOfFile(_),
                    ..
                } => break,
                node => acc.append(&mut Self::p_def(node, env)?),
            }
        }
        // TODO: post process
        Ok(acc)
    }

    fn p_script(node: &Syntax<T, V>, env: &mut Env) -> ret_aast!(Program<,>) {
        match &node.syntax {
            Script(children) => Self::p_program(&children.script_declarations, env),
            _ => Self::missing_syntax(None, "script", node, env),
        }
    }

    fn lower(env: &mut Env<'a>, script: &Syntax<T, V>) -> ::std::result::Result<Result, String> {
        let comments = vec![];
        let ast_result = Self::p_script(script, env);
        // TODO: handle error
        let ast = match ast_result {
            Ok(ast) => ast,
            // TODO: add msg
            Err(_) => return Err(String::from("ERROR")),
        };
        Ok(Result { ast, comments })
    }
}
