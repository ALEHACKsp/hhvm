// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
#![allow(unused_variables)]

mod print_env;
mod write;

use indexmap::IndexSet;
use itertools::Itertools;
pub use write::{Error, IoWrite, Result, Write};

use context::Context;
use core_utils_rust::add_ns;
use env::{local::Type as Local, Env as BodyEnv};
use escaper::escape;
use hhas_attribute_rust::{self as hhas_attribute, HhasAttribute};
use hhas_body_rust::HhasBody;
use hhas_class_rust::{self as hhas_class, HhasClass};
use hhas_constant_rust::HhasConstant;
use hhas_function_rust::HhasFunction;
use hhas_method_rust::HhasMethod;
use hhas_param_rust::HhasParam;
use hhas_pos_rust::Span;
use hhas_program_rust::HhasProgram;
use hhas_property_rust::HhasProperty;
use hhas_record_def_rust::{Field, HhasRecord};
use hhas_type::{constraint, Info as HhasTypeInfo};
use hhas_type_const::HhasTypeConstant;
use hhas_typedef_rust::Typedef as HhasTypedef;
use hhbc_ast_rust::*;
use hhbc_id_rust::{class, Id};
use hhbc_string_utils_rust::{
    float, integer, lstrip, quote_string, quote_string_with_escape, strip_global_ns, strip_ns,
    triple_quote_string, types,
};
use instruction_sequence_rust::InstrSeq;
use label_rust::Label;
use oxidized::{ast, ast_defs, doc_comment::DocComment};
use runtime::TypedValue;
use write::*;

use std::{borrow::Cow, convert::TryInto};

const ADATA_ARRAY_PREFIX: &str = "a";
const ADATA_VARRAY_PREFIX: &str = "y";
const ADATA_VEC_PREFIX: &str = "v";
const ADATA_DICT_PREFIX: &str = "D";
const ADATA_DARRAY_PREFIX: &str = "Y";
const ADATA_KEYSET_PREFIX: &str = "k";

pub mod context {
    use crate::write::*;
    use options::Options;
    use oxidized::relative_path::RelativePath;

    /// Indent is an abstraction of indentation. Configurable indentation
    /// and perf tweaking will be easier.
    struct Indent(usize);

    impl Indent {
        pub fn new() -> Self {
            Self(0)
        }

        pub fn inc(&mut self) {
            self.0 += 1;
        }

        pub fn dec(&mut self) {
            self.0 -= 1;
        }

        pub fn write<W: Write>(&self, w: &mut W) -> Result<(), W::Error> {
            Ok(for _ in 0..self.0 {
                w.write("  ")?;
            })
        }
    }

    pub struct Context<'a> {
        pub opts: &'a Options,
        pub path: Option<&'a RelativePath>,

        dump_symbol_refs: bool,
        indent: Indent,
        is_system_lib: bool,
    }

    impl<'a> Context<'a> {
        pub fn new(
            opts: &'a Options,
            path: Option<&'a RelativePath>,
            dump_symbol_refs: bool,
            is_system_lib: bool,
        ) -> Self {
            Self {
                opts,
                path,
                dump_symbol_refs,
                indent: Indent::new(),
                is_system_lib,
            }
        }

        pub fn dump_symbol_refs(&self) -> bool {
            self.dump_symbol_refs
        }

        /// Insert a newline with indentation
        pub fn newline<W: Write>(&self, w: &mut W) -> Result<(), W::Error> {
            newline(w)?;
            self.indent.write(w)
        }

        /// Start a new indented block
        pub fn block<W, F>(&mut self, w: &mut W, f: F) -> Result<(), W::Error>
        where
            W: Write,
            F: FnOnce(&mut Self, &mut W) -> Result<(), W::Error>,
        {
            self.indent.inc();
            let r = f(self, w);
            self.indent.dec();
            r
        }

        pub fn unblock<W, F>(&mut self, w: &mut W, f: F) -> Result<(), W::Error>
        where
            W: Write,
            F: FnOnce(&mut Self, &mut W) -> Result<(), W::Error>,
        {
            self.indent.dec();
            let r = f(self, w);
            self.indent.inc();
            r
        }

        /// Printing instruction list requies manually control indentation,
        /// where indent_inc/indent_dec are called
        pub fn indent_inc(&mut self) {
            self.indent.inc();
        }

        pub fn indent_dec(&mut self) {
            self.indent.dec();
        }

        pub fn is_system_lib(&self) -> bool {
            self.is_system_lib
        }
    }
}

struct ExprEnv<'e> {
    pub codegen_env: Option<&'e BodyEnv<'e>>,
    pub is_xhp: bool,
}

pub fn print_program<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    prog: &HhasProgram,
) -> Result<(), W::Error> {
    match ctx.path {
        Some(p) => {
            let p = &escape(p.to_absoute().to_str().ok_or(Error::InvalidUTF8)?);

            concat_str_by(w, " ", ["#", p, "starts here"])?;

            newline(w)?;

            newline(w)?;
            concat_str(w, [".filepath ", format!("\"{}\"", p).as_str(), ";"])?;

            newline(w)?;
            handle_not_impl(|| print_program_(ctx, w, prog))?;

            newline(w)?;
            concat_str_by(w, " ", ["#", p, "ends here"])?;

            newline(w)
        }
        None => {
            w.write("#starts here")?;

            newline(w)?;
            handle_not_impl(|| print_program_(ctx, w, prog))?;

            newline(w)?;
            w.write("#ends here")?;

            newline(w)
        }
    }
}

fn print_program_<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    prog: &HhasProgram,
) -> Result<(), W::Error> {
    let is_hh = if prog.is_hh { "1" } else { "0" };
    newline(w)?;
    write_list(w, &[".hh_file ", is_hh, ";"])?;
    newline(w)?;

    print_data_region(w, vec![])?;
    print_main(ctx, w, &prog.main)?;
    concat(w, &prog.functions, |w, f| print_fun_def(ctx, w, f))?;
    concat(w, &prog.record_defs, |w, rd| print_record_def(ctx, w, rd))?;
    concat(w, &prog.classes, |w, cd| print_class_def(ctx, w, cd))?;
    concat(w, &prog.typedefs, |w, td| print_typedef(ctx, w, td))?;
    print_file_attributes(ctx, w, &prog.file_attributes)?;

    if ctx.dump_symbol_refs() {
        return not_impl!();
    }
    Ok(())
}

fn print_typedef<W: Write>(ctx: &mut Context, w: &mut W, td: &HhasTypedef) -> Result<(), W::Error> {
    newline(w)?;
    w.write(".alias ")?;
    print_typedef_attributes(ctx, w, td)?;
    w.write(td.name.to_raw_string())?;
    w.write(" = ")?;
    print_typedef_info(w, &td.type_info)?;
    w.write(" ")?;
    wrap_by_triple_quotes(w, |w| print_adata(ctx, w, &td.type_structure))?;
    w.write(";")
}

fn print_typedef_attributes<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    td: &HhasTypedef,
) -> Result<(), W::Error> {
    let mut specials = vec![];
    if ctx.is_system_lib() {
        specials.push("persistent");
    }
    print_special_and_user_attrs(ctx, w, &specials[..], td.attributes.as_slice())
}

fn print_data_region_element<W: Write>(w: &mut W, fake_elem: usize) -> Result<(), W::Error> {
    not_impl!()
}

fn print_data_region<W: Write>(w: &mut W, fake_adata: Vec<usize>) -> Result<(), W::Error> {
    concat(w, fake_adata, |w, i| print_data_region_element(w, *i))?;
    newline(w)
}

fn handle_not_impl<E: std::fmt::Debug, F: FnOnce() -> Result<(), E>>(f: F) -> Result<(), E> {
    let r = f();
    match &r {
        Err(Error::NotImpl(msg)) => {
            println!("#### NotImpl: {}", msg);
            eprintln!("NotImpl: {}", msg);
            Ok(())
        }
        _ => r,
    }
}

fn print_fun_def<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    fun_def: &HhasFunction,
) -> Result<(), W::Error> {
    let body = &fun_def.body;
    newline(w)?;
    w.write(".function ")?;
    if ctx
        .opts
        .hack_compiler_flags
        .contains(options::CompilerFlags::EMIT_GENERICS_UB)
    {
        print_upper_bounds(w, &body.upper_bounds)?;
    }
    print_fun_attrs(ctx, w, fun_def)?;
    if ctx.opts.source_map() {
        w.write(string_of_span(&fun_def.span))?;
        w.write(" ")?;
    }
    option(w, &body.return_type_info, print_type_info)?;
    w.write(" ")?;
    w.write(fun_def.name.to_raw_string())?;
    print_params(w, fun_def.body.env.as_ref(), fun_def.params())?;
    if fun_def.is_generator() {
        w.write(" isGenerator")?;
    }
    if fun_def.is_async() {
        w.write(" isAsync")?;
    }
    if fun_def.is_pair_generator() {
        w.write(" isPairGenerator")?;
    }
    if fun_def.rx_disabled() {
        w.write(" isRxDisabled")?;
    }
    w.write(" ")?;
    wrap_by_braces(w, |w| {
        ctx.block(w, |c, w| print_body(c, w, body))?;
        newline(w)
    })?;
    newline(w)
}

fn print_requirement<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    r: &(class::Type<'_>, hhas_class::TraitReqKind),
) -> Result<(), W::Error> {
    w.write("\n  .require ")?;
    match r {
        (name, hhas_class::TraitReqKind::MustExtend) => {
            w.write(format!("extends <{}>;", name.to_raw_string()))
        }
        (name, hhas_class::TraitReqKind::MustImplement) => {
            w.write(format!("implements <{}>;", name.to_raw_string()))
        }
    }
}

fn print_type_constant<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    c: &HhasTypeConstant,
) -> Result<(), W::Error> {
    w.write("\n  .const ")?;
    w.write(&c.name)?;
    w.write(" isType")?;
    match c.initializer.as_ref() {
        Some(init) => {
            w.write(" = \"\"\"")?;
            print_adata(ctx, w, init)?;
            w.write("\"\"\";")
        }
        None => w.write(";"),
    }
}

fn print_property_doc_comment<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    p: &HhasProperty,
) -> Result<(), W::Error> {
    if let Some(s) = p.doc_comment.as_ref() {
        w.write(triple_quote_string(&s.0))?;
        w.write(" ")?;
    }
    Ok(())
}

fn print_property_attributes<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    property: &HhasProperty,
) -> Result<(), W::Error> {
    let mut special_attributes = vec![];
    if property.is_late_init() {
        special_attributes.push("late_init")
    };
    if property.is_no_bad_redeclare() {
        special_attributes.push("no_bad_redeclare")
    };
    if property.initial_satisfies_tc() {
        special_attributes.push("initial_satisfies_tc")
    }
    if property.no_implicit_null() {
        special_attributes.push("no_implicit_null")
    }
    if property.has_system_initial() {
        special_attributes.push("sys_initial_val")
    }
    if property.is_const() {
        special_attributes.push("is_const")
    }
    if property.is_deep_init() {
        special_attributes.push("deep_init")
    }
    if property.is_lsb() {
        special_attributes.push("lsb")
    }
    if property.is_static() {
        special_attributes.push("static")
    }
    special_attributes.push(property.visibility.as_ref());
    special_attributes.reverse();

    w.write("[")?;
    concat_by(w, " ", &special_attributes, |w, a| w.write(a))?;
    if !special_attributes.is_empty() && !property.attributes.is_empty() {
        w.write(" ")?;
    }
    print_attributes(ctx, w, &property.attributes)?;
    w.write("] ")
}

fn print_property_type_info<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    p: &HhasProperty,
) -> Result<(), W::Error> {
    print_type_info(w, &p.type_info)?;
    w.write(" ")
}

fn print_property<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    class_def: &HhasClass,
    property: &HhasProperty,
) -> Result<(), W::Error> {
    newline(w)?;
    w.write("  .property ")?;
    print_property_attributes(ctx, w, property)?;
    print_property_doc_comment(ctx, w, property)?;
    print_property_type_info(ctx, w, property)?;
    w.write(property.name.to_raw_string())?;
    w.write(" =\n    ")?;
    let initial_value = property.initial_value.as_ref();
    if class_def.is_closure() || initial_value == Some(&TypedValue::Uninit) {
        w.write("uninit;")
    } else {
        w.write("\"\"\"")?;
        match initial_value {
            None => w.write("N;"),
            Some(value) => print_adata(ctx, w, &value),
        }?;
        w.write("\"\"\";")
    }
}

fn print_constant<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    c: &HhasConstant,
) -> Result<(), W::Error> {
    w.write("\n  .const ")?;
    w.write(c.name.to_raw_string())?;
    match c.value.as_ref() {
        Some(TypedValue::Uninit) => w.write(" = uninit")?,
        Some(value) => {
            w.write(" = \"\"\"")?;
            print_adata(ctx, w, value)?;
            w.write("\"\"\"")?
        }
        None => (),
    }
    w.write(";")
}

fn print_enum_ty<W: Write>(ctx: &mut Context, w: &mut W, c: &HhasClass) -> Result<(), W::Error> {
    if let Some(et) = c.enum_type.as_ref() {
        newline(w)?;
        w.write("  .enum_ty ")?;
        print_type_info_(w, true, et)?;
        w.write(";")?;
    }
    Ok(())
}

fn print_doc_comment<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    doc_comment: &Option<DocComment>,
) -> Result<(), W::Error> {
    if let Some(cmt) = doc_comment {
        ctx.newline(w)?;
        w.write(format!(".doc {};", triple_quote_string(&cmt.0)))?;
    }
    Ok(())
}

fn print_use_precedence<W: Write, X>(ctx: &mut Context, w: &mut W, _: X) -> Result<(), W::Error> {
    not_impl!()
}

fn print_use_alias<W: Write, X>(ctx: &mut Context, w: &mut W, _: X) -> Result<(), W::Error> {
    not_impl!()
}

fn print_method_trait_resolutions<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    (mtr, kind_as_tring): &(&ast::MethodRedeclaration, class::Type),
) -> Result<(), W::Error> {
    w.write(format!(
        "\n    {}::{} as strict ",
        kind_as_tring.to_raw_string(),
        mtr.method.1
    ))?;
    if mtr.fun_kind.is_async() {
        w.write("async ")?;
    }
    w.write("[")?;
    if mtr.final_ {
        w.write("final ")?;
    }
    w.write(mtr.visibility.to_string())?;
    if mtr.abstract_ {
        w.write(" abstract")?;
    }
    if mtr.static_ {
        w.write(" static")?;
    }
    w.write("] ")?;
    w.write(format!("{};", mtr.name.1))
}

fn print_uses<W: Write>(ctx: &mut Context, w: &mut W, c: &HhasClass) -> Result<(), W::Error> {
    if c.uses.is_empty() && c.method_trait_resolutions.is_empty() {
        Ok(())
    } else {
        let unique_ids: IndexSet<&str> = c
            .uses
            .iter()
            .map(|e| strip_global_ns(e.to_raw_string()))
            .collect();
        let unique_ids: Vec<_> = unique_ids.into_iter().collect();

        newline(w)?;
        w.write("  .use ")?;
        concat_by(w, " ", unique_ids, |w, id| w.write(id))?;

        if c.use_aliases.is_empty()
            && c.use_precedences.is_empty()
            && c.method_trait_resolutions.is_empty()
        {
            w.write(";")
        } else {
            w.write(" {")?;
            for x in &c.use_precedences {
                print_use_precedence(ctx, w, x)?;
            }
            for x in &c.use_aliases {
                print_use_alias(ctx, w, x)?;
            }
            for x in &c.method_trait_resolutions {
                print_method_trait_resolutions(ctx, w, x)?;
            }
            newline(w)?;
            w.write("  }")
        }
    }
}

fn print_class_special_attributes<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    c: &HhasClass,
) -> Result<(), W::Error> {
    let user_attrs = &c.attributes;
    let is_system_lib = ctx.is_system_lib();

    let mut special_attributes: Vec<&str> = vec![];
    if c.needs_no_reifiedinit() {
        special_attributes.push("noreifiedinit")
    }
    if c.no_dynamic_props() {
        special_attributes.push("no_dynamic_props")
    }
    if c.is_const() {
        special_attributes.push("is_const")
    }
    if hhas_attribute::has_foldable(user_attrs) {
        special_attributes.push("foldable")
    }
    if is_system_lib {
        special_attributes.extend(&["unique", "builtin", "persistent"])
    }
    if hhas_attribute::has_dynamically_constructible(user_attrs) {
        special_attributes.push("dyn_constructible");
    }
    if !c.is_top() {
        special_attributes.push("nontop");
    }
    if c.is_closure() && !is_system_lib {
        special_attributes.push("unique");
    }
    if c.is_closure() {
        special_attributes.push("no_override");
    }
    if c.is_trait() {
        special_attributes.push("trait");
    }
    if c.is_interface() {
        special_attributes.push("interface");
    }
    if c.is_final() {
        special_attributes.push("final");
    }
    if c.is_sealed() {
        special_attributes.push("sealed");
    }
    if c.enum_type.is_some() {
        special_attributes.push("enum");
    }
    if c.is_abstract() {
        special_attributes.push("abstract");
    }
    if special_attributes.is_empty() && user_attrs.is_empty() {
        return Ok(());
    }

    w.write("[")?;
    special_attributes.reverse();
    concat_by(w, " ", &special_attributes, |w, a| w.write(a))?;
    if !special_attributes.is_empty() && !user_attrs.is_empty() {
        w.write(" ")?;
    }
    print_attributes(ctx, w, &user_attrs)?;
    w.write("] ")?;
    Ok(())
}

fn print_implements<W: Write>(
    w: &mut W,
    implements: &Vec<class::Type<'_>>,
) -> Result<(), W::Error> {
    if implements.is_empty() {
        return Ok(());
    }
    w.write(" implements (")?;
    concat_str_by(
        w,
        " ",
        implements
            .iter()
            .map(|x| x.to_raw_string())
            .collect::<Vec<_>>(),
    )?;
    w.write(")")
}

fn print_method_def<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    method_def: &HhasMethod,
) -> Result<(), W::Error> {
    newline(w)?;
    w.write("  .method ")?;
    w.write(method_def.name.to_raw_string())?;
    w.write(" {")?;
    w.write("  }")?;
    not_impl!()
}

fn print_class_def<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    class_def: &HhasClass,
) -> Result<(), W::Error> {
    newline(w)?;
    w.write(".class ")?;
    if ctx
        .opts
        .hack_compiler_flags
        .contains(options::CompilerFlags::EMIT_GENERICS_UB)
    {
        print_upper_bounds(w, &class_def.upper_bounds)?;
    }
    print_class_special_attributes(ctx, w, class_def)?;
    w.write(class_def.name.to_raw_string())?;
    if ctx.opts.source_map() {
        w.write(format!(" {}", string_of_span(&class_def.span)))?;
    }
    print_extends(w, class_def.base.as_ref().map(|x| x.to_raw_string()))?;
    print_implements(w, &class_def.implements)?;
    w.write(" {")?;
    ctx.block(w, |c, w| {
        print_doc_comment(c, w, &class_def.doc_comment)?;
        print_uses(c, w, class_def)?;
        print_enum_ty(c, w, class_def)?;
        for x in &class_def.requirements {
            print_requirement(c, w, x)?;
        }
        for x in &class_def.constants {
            print_constant(c, w, x)?;
        }
        for x in &class_def.type_constants {
            print_type_constant(c, w, x)?;
        }
        for x in &class_def.properties {
            print_property(c, w, class_def, x)?;
        }
        for m in &class_def.methods {
            print_method_def(c, w, m)?;
        }
        Ok(())
    })?;
    newline(w)?;
    w.write("}")?;
    newline(w)
}

fn pos_to_prov_tag(ctx: &Context, loc: &Option<ast_defs::Pos>) -> String {
    match loc {
        Some(_) if ctx.opts.array_provenance() => unimplemented!(),
        _ => "".into(),
    }
}

fn print_class_id<W: Write>(w: &mut W, id: &ClassId) -> Result<(), W::Error> {
    wrap_by_quotes(w, |w| w.write(escape(id.to_raw_string())))
}

fn print_const_id<W: Write>(w: &mut W, id: &ConstId) -> Result<(), W::Error> {
    wrap_by_quotes(w, |w| w.write(escape(id.to_raw_string())))
}

fn print_adata_id<W: Write>(w: &mut W, id: &AdataId) -> Result<(), W::Error> {
    concat_str(w, ["@", id.as_str()])
}

fn print_adata_mapped_argument<W: Write, F, V>(
    ctx: &mut Context,
    w: &mut W,
    col_type: &str,
    loc: &Option<ast_defs::Pos>,
    values: &Vec<V>,
    f: F,
) -> Result<(), W::Error>
where
    F: Fn(&mut Context, &mut W, &V) -> Result<(), W::Error>,
{
    w.write(format!(
        "{}:{}:{{{}",
        col_type,
        values.len(),
        pos_to_prov_tag(ctx, loc)
    ))?;
    for v in values {
        f(ctx, w, v)?
    }
    w.write(format!("}}"))
}

fn print_adata_collection_argument<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    col_type: &str,
    loc: &Option<ast_defs::Pos>,
    values: &Vec<TypedValue>,
) -> Result<(), W::Error> {
    print_adata_mapped_argument(ctx, w, col_type, loc, values, &print_adata)
}

fn print_adata_dict_collection_argument<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    col_type: &str,
    loc: &Option<ast_defs::Pos>,
    pairs: &Vec<(TypedValue, TypedValue)>,
) -> Result<(), W::Error> {
    print_adata_mapped_argument(ctx, w, col_type, loc, pairs, |ctx, w, (v1, v2)| {
        print_adata(ctx, w, v1)?;
        print_adata(ctx, w, v2)
    })
}

fn print_adata<W: Write>(ctx: &mut Context, w: &mut W, tv: &TypedValue) -> Result<(), W::Error> {
    match tv {
        TypedValue::Uninit => w.write("uninit"),
        TypedValue::Null => w.write("N;"),
        TypedValue::String(s) => w.write(format!("s:{}:{};", s.len(), quote_string_with_escape(s))),
        TypedValue::Float(f) => w.write(format!("d:{};", float::to_string(*f))),
        TypedValue::Int(i) => w.write(format!("i:{};", i)),
        // TODO: The False case seems to sometimes be b:0 and sometimes i:0.  Why?
        TypedValue::Bool(false) => w.write("b:0;"),
        TypedValue::Bool(true) => w.write("b:1;"),
        TypedValue::Dict((pairs, loc)) => {
            print_adata_dict_collection_argument(ctx, w, ADATA_DICT_PREFIX, loc, pairs)
        }
        TypedValue::Vec((values, loc)) => {
            print_adata_collection_argument(ctx, w, ADATA_VEC_PREFIX, loc, values)
        }
        TypedValue::DArray((pairs, loc)) => {
            print_adata_dict_collection_argument(ctx, w, ADATA_DARRAY_PREFIX, loc, pairs)
        }
        TypedValue::Array(pairs) => {
            print_adata_dict_collection_argument(ctx, w, ADATA_ARRAY_PREFIX, &None, pairs)
        }
        TypedValue::Keyset(values) => {
            print_adata_collection_argument(ctx, w, ADATA_KEYSET_PREFIX, &None, values)
        }
        TypedValue::VArray((values, loc)) => {
            print_adata_collection_argument(ctx, w, ADATA_VARRAY_PREFIX, loc, values)
        }
        TypedValue::HhasAdata(_) => not_impl!(),
    }
}

fn print_attribute<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    a: &HhasAttribute,
) -> Result<(), W::Error> {
    w.write(format!(
        "\"{}\"(\"\"\"{}:{}:{{",
        a.name,
        ADATA_VARRAY_PREFIX,
        a.arguments.len()
    ))?;
    concat(w, &a.arguments, |w, arg| print_adata(ctx, w, arg))?;
    w.write("}\"\"\")")
}

fn print_attributes<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    al: impl AsRef<[HhasAttribute]>,
) -> Result<(), W::Error> {
    // Adjust for underscore coming before alphabet
    let al: Vec<&HhasAttribute> = al
        .as_ref()
        .iter()
        .sorted_by_key(|a| (!a.name.starts_with("__"), &a.name))
        .collect();
    concat_by(w, " ", &al, |w, a| print_attribute(ctx, w, a))
}

fn print_file_attributes<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    al: &Vec<HhasAttribute>,
) -> Result<(), W::Error> {
    if al.is_empty() {
        return Ok(());
    }
    newline(w)?;
    w.write(".file_attributes [")?;
    print_attributes(ctx, w, al)?;
    w.write("] ;")?;
    newline(w)
}

fn print_main<W: Write>(ctx: &mut Context, w: &mut W, body: &HhasBody) -> Result<(), W::Error> {
    w.write(".main ")?;
    if ctx.opts.source_map() {
        w.write("(1,1) ")?;
    }
    wrap_by_braces(w, |w| {
        ctx.block(w, |c, w| print_body(c, w, body))?;
        newline(w)
    })?;
    newline(w)
}

fn is_bareword_char(c: &u8) -> bool {
    match *c {
        b'_' | b'.' | b'$' | b'\\' => true,
        c => (c >= b'0' && c <= b'9') || (c >= b'a' && c <= b'z') || (c >= b'A' && c <= b'Z'),
    }
}

fn print_body<W: Write>(ctx: &mut Context, w: &mut W, body: &HhasBody) -> Result<(), W::Error> {
    print_doc_comment(ctx, w, &body.doc_comment)?;
    if body.is_memoize_wrapper {
        ctx.newline(w)?;
        w.write(".ismemoizewrapper;")?;
    }
    if body.is_memoize_wrapper_lsb {
        ctx.newline(w)?;
        w.write(".ismemoizewrapperlsb;")?;
    }
    if body.num_iters > 0 {
        ctx.newline(w)?;
        w.write(format!(".number {};", body.num_iters))?;
    }
    if !body.decl_vars.is_empty() {
        ctx.newline(w)?;
        w.write(".declvars ")?;
        concat_by(w, " ", &body.decl_vars, |w, var| {
            if var.as_bytes().iter().all(is_bareword_char) {
                w.write(var)
            } else {
                wrap_by_quotes(w, |w| w.write(escaper::escape(var)))
            }
        })?;
        w.write(";")?;
    }
    print_instructions(ctx, w, &body.body_instrs)
}

fn print_instructions<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    instr_seq: &InstrSeq,
) -> Result<(), W::Error> {
    use Instruct::*;
    use InstructTry::*;
    for instr in instr_seq.iter() {
        match instr {
            ISpecialFlow(_) => return not_impl!(),
            IComment(_) => {
                // indetation = 0
                newline(w)?;
                print_instr(w, instr)?;
            }
            ILabel(_) => ctx.unblock(w, |c, w| {
                c.newline(w)?;
                print_instr(w, instr)
            })?,
            ITry(TryCatchBegin) => {
                ctx.newline(w)?;
                print_instr(w, instr)?;
                ctx.indent_inc();
            }
            ITry(TryCatchMiddle) => ctx.unblock(w, |c, w| {
                c.newline(w)?;
                print_instr(w, instr)
            })?,
            ITry(TryCatchEnd) => {
                ctx.indent_dec();
                ctx.newline(w)?;
                print_instr(w, instr)?;
            }
            _ => {
                ctx.newline(w)?;
                print_instr(w, instr)?;
            }
        }
    }
    Ok(())
}

fn print_instr<W: Write>(w: &mut W, instr: &Instruct) -> Result<(), W::Error> {
    fn print_call<W: Write>(w: &mut W, call: &InstructCall) -> Result<(), W::Error> {
        use InstructCall as I;
        match call {
            I::FCallBuiltin(n, un, io, id) => concat_str_by(
                w,
                " ",
                [
                    "FCallBuiltin",
                    n.to_string().as_str(),
                    un.to_string().as_str(),
                    io.to_string().as_str(),
                    quote_string(id).as_str(),
                ],
            ),
            _ => return not_impl!(),
        }
    }

    use Instruct::*;
    use InstructBasic as IB;
    match instr {
        IIterator(_) => not_impl!(),
        IBasic(b) => w.write(match b {
            IB::Nop => "Nop",
            IB::EntryNop => "EntryNop",
            IB::PopC => "PopC",
            IB::PopU => "PopU",
            IB::Dup => "Dup",
        }),
        ILitConst(lit) => print_lit_const(w, lit),
        IOp(op) => print_op(w, op),
        IContFlow(cf) => print_control_flow(w, cf),
        ICall(c) => print_call(w, c),
        IMisc(misc) => print_misc(w, misc),
        IGet(_) => not_impl!(),
        IMutator(mutator) => print_mutator(w, mutator),
        ILabel(l) => {
            print_label(w, l)?;
            w.write(":")
        }
        IIsset(_) => not_impl!(),
        IBase(_) => not_impl!(),
        IFinal(_) => not_impl!(),
        ITry(_) => not_impl!(),
        IComment(s) => concat_str_by(w, " ", ["#", s.as_str()]),
        ISrcLoc(_) => not_impl!(),
        IAsync(_) => not_impl!(),
        IGenerator(gen) => print_gen_creation_execution(w, gen),
        IIncludeEvalDefine(ed) => print_include_eval_define(w, ed),
        IGenDelegation(_) => not_impl!(),
        _ => Err(Error::fail("invalid instruction")),
    }
}

fn print_mutator<W: Write>(w: &mut W, mutator: &InstructMutator) -> Result<(), W::Error> {
    use InstructMutator as M;
    match mutator {
        M::SetL(local) => {
            w.write("SetL ")?;
            print_local(w, local)
        }
        _ => not_impl!(),
    }
}

fn print_gen_creation_execution<W: Write>(
    w: &mut W,
    gen: &GenCreationExecution,
) -> Result<(), W::Error> {
    use GenCreationExecution as G;
    match gen {
        G::CreateCont => w.write("CreateCont"),
        G::ContEnter => w.write("ContEnter"),
        G::ContRaise => w.write("ContRaise"),
        G::Yield => w.write("Yield"),
        G::YieldK => w.write("YieldK"),
        G::ContCheck(CheckStarted::IgnoreStarted) => w.write("ContCheck IgnoreStarted"),
        G::ContCheck(CheckStarted::CheckStarted) => w.write("ContCheck CheckStarted"),
        G::ContValid => w.write("ContValid"),
        G::ContKey => w.write("ContKey"),
        G::ContGetReturn => w.write("ContGetReturn"),
        G::ContCurrent => w.write("ContCurrent"),
    }
}

fn print_misc<W: Write>(w: &mut W, misc: &InstructMisc) -> Result<(), W::Error> {
    use InstructMisc as M;
    match misc {
        M::This => w.write("This"),
        M::CheckThis => w.write("CheckThis"),
        M::FuncNumArgs => w.write("FuncNumArgs"),
        M::ChainFaults => w.write("ChainFaults"),
        M::VerifyRetTypeC => w.write("VerifyRetTypeC"),
        M::VerifyRetTypeTS => w.write("VerifyRetTypeTS"),
        M::Self_ => w.write("Self_"),
        M::Parent => w.write("Parent"),
        M::LateBoundCls => w.write("LateBoundCls"),
        M::ClassName => w.write("ClassName"),
        M::RecordReifiedGeneric => w.write("RecordReifiedGeneric"),
        M::CheckReifiedGenericMismatch => w.write("CheckReifiedGenericMismatch"),
        M::NativeImpl => w.write("NativeImpl"),
        M::AKExists => w.write("AKExists"),
        M::Idx => w.write("Idx"),
        M::ArrayIdx => w.write("ArrayIdx"),
        M::BreakTraceHint => w.write("BreakTraceHint"),
        M::CGetCUNop => w.write("CGetCUNop"),
        M::UGetCUNop => w.write("UGetCUNop"),
        M::LockObj => w.write("LockObj"),
        _ => not_impl!(),
    }
}

fn print_include_eval_define<W: Write>(
    w: &mut W,
    ed: &InstructIncludeEvalDefine,
) -> Result<(), W::Error> {
    use InstructIncludeEvalDefine::*;
    match ed {
        Incl => w.write("Incl"),
        InclOnce => w.write("InclOnce"),
        Req => w.write("Req"),
        ReqOnce => w.write("ReqOnce"),
        ReqDoc => w.write("ReqDoc"),
        Eval => w.write("Eval"),
        DefCls(n) => concat_str_by(w, " ", ["DefCls", n.to_string().as_str()]),
        DefClsNop(n) => concat_str_by(w, " ", ["DefClsNop", n.to_string().as_str()]),
        DefRecord(n) => concat_str_by(w, " ", ["DefRecord", n.to_string().as_str()]),
        DefCns(id) => {
            w.write("DefCns ")?;
            print_hhbc_id(w, id)
        }
        DefTypeAlias(id) => w.write(format!("DefTypeAlias {}", id)),
    }
}

fn print_control_flow<W: Write>(w: &mut W, cf: &InstructControlFlow) -> Result<(), W::Error> {
    use InstructControlFlow as CF;
    match cf {
        CF::Jmp(l) => {
            w.write("Jmp ")?;
            print_label(w, l)
        }
        CF::JmpNS(l) => {
            w.write("JmpNS ")?;
            print_label(w, l)
        }
        CF::JmpZ(l) => {
            w.write("JmpZ ")?;
            print_label(w, l)
        }
        CF::JmpNZ(l) => {
            w.write("JmpNZ ")?;
            print_label(w, l)
        }
        CF::RetC => w.write("RetC"),
        CF::RetCSuspended => w.write("RetCSuspended"),
        CF::RetM(p) => not_impl!(),
        CF::Throw => w.write("Throw"),
        CF::Switch(_, _, _) => not_impl!(),
        CF::SSwitch(_) => not_impl!(),
    }
}

fn print_lit_const<W: Write>(w: &mut W, lit: &InstructLitConst) -> Result<(), W::Error> {
    use InstructLitConst as LC;
    match lit {
        LC::Null => w.write("Null"),
        LC::Int(i) => concat_str_by(w, " ", ["Int", i.to_string().as_str()]),
        LC::String(s) => {
            w.write("String ")?;
            wrap_by_quotes(w, |w| w.write(escape(s)))
        }
        LC::True => w.write("True"),
        LC::False => w.write("False"),
        LC::Double(d) => concat_str_by(w, " ", ["Double", d.as_str()]),
        LC::AddElemC => w.write("AddElemC"),
        LC::AddNewElemC => w.write("AddNewElemC"),
        LC::NewPair => w.write("NewPair"),
        LC::File => w.write("File"),
        LC::Dir => w.write("Dir"),
        LC::Method => w.write("Method"),
        LC::FuncCred => w.write("FuncCred"),
        LC::Array(id) => {
            w.write("Array ")?;
            print_adata_id(w, id)
        }
        LC::Dict(id) => {
            w.write("Dict ")?;
            print_adata_id(w, id)
        }
        LC::Keyset(id) => {
            w.write("Keyset ")?;
            print_adata_id(w, id)
        }
        LC::Vec(id) => {
            w.write("Vec ")?;
            print_adata_id(w, id)
        }
        LC::NewArray(i) => concat_str_by(w, " ", ["NewArray", i.to_string().as_str()]),
        LC::NewMixedArray(i) => concat_str_by(w, " ", ["NewMixedArray", i.to_string().as_str()]),
        LC::NewDictArray(i) => concat_str_by(w, " ", ["NewDictArray", i.to_string().as_str()]),
        LC::NewDArray(i) => concat_str_by(w, " ", ["NewDArray", i.to_string().as_str()]),
        LC::NewPackedArray(i) => concat_str_by(w, " ", ["NewPackedArray", i.to_string().as_str()]),
        LC::NewVArray(i) => concat_str_by(w, " ", ["NewVArray", i.to_string().as_str()]),
        LC::NewVecArray(i) => concat_str_by(w, " ", ["NewVecArray", i.to_string().as_str()]),
        LC::NewKeysetArray(i) => concat_str_by(w, " ", ["NewKeysetArray", i.to_string().as_str()]),
        LC::NewLikeArrayL(local, i) => {
            w.write("NewLikeArrayL ")?;
            print_local(w, local)?;
            w.write(" ")?;
            w.write(i.to_string().as_str())
        }
        LC::NewStructArray(l) => {
            w.write("NewStructArray ")?;
            wrap_by_angle(w, |w| print_shape_fields(w, l))
        }
        LC::NewStructDArray(l) => {
            w.write("NewStructDArray ")?;
            wrap_by_angle(w, |w| print_shape_fields(w, l))
        }
        LC::NewStructDict(l) => {
            w.write("NewStructDict ")?;
            wrap_by_angle(w, |w| print_shape_fields(w, l))
        }
        LC::NewRecord(cid, l) => {
            w.write("NewRecord ")?;
            print_class_id(w, cid)?;
            wrap_by_angle(w, |w| print_shape_fields(w, l))
        }
        LC::NewRecordArray(cid, l) => {
            w.write("NewRecordArray ")?;
            print_class_id(w, cid)?;
            wrap_by_angle(w, |w| print_shape_fields(w, l))
        }
        LC::CnsE(id) => {
            w.write("CnsE ")?;
            print_const_id(w, id)
        }
        LC::ClsCns(id) => {
            w.write("ClsCns ")?;
            print_const_id(w, id)
        }
        LC::ClsCnsD(const_id, cid) => {
            w.write("ClsCnsD ")?;
            print_const_id(w, const_id)?;
            print_class_id(w, cid)
        }
        LC::NewCol(ct) => {
            w.write("NewCol ")?;
            print_collection_type(w, ct)
        }
        LC::ColFromArray(ct) => {
            w.write("ColFromArray ")?;
            print_collection_type(w, ct)
        }
        _ => not_impl!(),
    }
}

fn print_collection_type<W: Write>(w: &mut W, ct: &CollectionType) -> Result<(), W::Error> {
    use CollectionType as CT;
    match ct {
        CT::Vector => w.write("Vector"),
        CT::Map => w.write("Map"),
        CT::Set => w.write("Set"),
        CT::Pair => w.write("Pair"),
        CT::ImmVector => w.write("ImmVector"),
        CT::ImmMap => w.write("ImmMap"),
        CT::ImmSet => w.write("ImmSet"),
    }
}

fn print_shape_fields<W: Write>(w: &mut W, sf: &Vec<String>) -> Result<(), W::Error> {
    concat_by(w, " ", sf, |w, f| wrap_by_quotes(w, |w| w.write(escape(f))))
}

fn print_op<W: Write>(w: &mut W, op: &InstructOperator) -> Result<(), W::Error> {
    use InstructOperator as I;
    match op {
        I::Fatal(fatal_op) => print_fatal_op(w, fatal_op),
        _ => not_impl!(),
    }
}

fn print_fatal_op<W: Write>(w: &mut W, f: &FatalOp) -> Result<(), W::Error> {
    match f {
        FatalOp::Parse => w.write("Fatal Parse"),
        FatalOp::Runtime => w.write("Fatal Runtime"),
        FatalOp::RuntimeOmitFrame => w.write("Fatal RuntimeOmitFrame"),
    }
}

fn print_params<W: Write>(
    w: &mut W,
    body_env: Option<&BodyEnv>,
    params: &[HhasParam],
) -> Result<(), W::Error> {
    wrap_by_paren(w, |w| {
        concat_by(w, ", ", params, |w, i| print_param(w, body_env, i))
    })
}

fn print_param<W: Write>(
    w: &mut W,
    body_env: Option<&BodyEnv>,
    param: &HhasParam,
) -> Result<(), W::Error> {
    print_param_user_attributes(w, param)?;
    if param.is_inout {
        w.write("inout ")?;
    }
    if param.is_variadic {
        w.write("...")?;
    }
    option(w, &param.type_info, |w, ty| {
        print_type_info(w, ty)?;
        w.write(" ")
    })?;
    w.write(&param.name)?;
    option(w, &param.default_value, |w, i| {
        print_param_default_value(w, body_env, i)
    })
}

fn print_param_default_value<W: Write>(
    w: &mut W,
    body_env: Option<&BodyEnv>,
    default_val: &(Label, ast::Expr),
) -> Result<(), W::Error> {
    let expr_env = ExprEnv {
        codegen_env: body_env,
        is_xhp: false,
    };
    w.write(" = ")?;
    print_label(w, &default_val.0)?;
    wrap_by_paren(w, |w| {
        wrap_by_triple_quotes(w, |w| print_expr(w, &expr_env, &default_val.1))
    })
}

fn print_label<W: Write>(w: &mut W, label: &Label) -> Result<(), W::Error> {
    match label {
        Label::Regular(id) => {
            w.write("L")?;
            print_int(w, id)
        }
        Label::DefaultArg(id) => {
            w.write("DV")?;
            print_int(w, id)
        }
        Label::Named(id) => w.write(id),
    }
}

fn print_local<W: Write>(w: &mut W, local: &Local) -> Result<(), W::Error> {
    match local {
        Local::Unnamed(id) => {
            w.write("_")?;
            print_int(w, id)
        }
        Local::Named(id) => w.write(id),
    }
}

fn print_int<W: Write>(w: &mut W, i: &usize) -> Result<(), W::Error> {
    // TODO(shiqicao): avoid allocating intermediate string
    w.write(format!("{}", i))
}

fn print_key_value<W: Write>(
    w: &mut W,
    env: &ExprEnv,
    k: &ast::Expr,
    v: &ast::Expr,
) -> Result<(), W::Error> {
    print_expr(w, env, k)?;
    w.write(" => ")?;
    print_expr(w, env, v)
}

fn print_afield<W: Write>(w: &mut W, env: &ExprEnv, afield: &ast::Afield) -> Result<(), W::Error> {
    use ast::Afield as A;
    match afield {
        A::AFvalue(e) => print_expr(w, env, &e),
        A::AFkvalue(k, v) => print_key_value(w, env, &k, &v),
    }
}

fn print_afields<W: Write>(
    w: &mut W,
    env: &ExprEnv,
    afields: impl AsRef<[ast::Afield]>,
) -> Result<(), W::Error> {
    concat_by(w, ", ", afields, |w, i| print_afield(w, env, i))
}

fn print_uop<W: Write>(w: &mut W, op: ast::Uop) -> Result<(), W::Error> {
    use ast::Uop as U;
    w.write(match op {
        U::Utild => "~",
        U::Unot => "!",
        U::Uplus => "+",
        U::Uminus => "-",
        U::Uincr => "++",
        U::Udecr => "--",
        U::Usilence => "@",
        U::Upincr | U::Updecr => {
            return Err(Error::fail(
                "string_of_uop - should have been captures earlier",
            ))
        }
    })
}

fn print_key_values<W: Write>(
    w: &mut W,
    env: &ExprEnv,
    kvs: impl AsRef<[(ast::Expr, ast::Expr)]>,
) -> Result<(), W::Error> {
    concat_by(w, ", ", kvs, |w, (k, v)| print_key_value(w, env, k, v))
}

fn print_expr<W: Write>(
    w: &mut W,
    env: &ExprEnv,
    ast::Expr(p, expr): &ast::Expr,
) -> Result<(), W::Error> {
    fn adjust_id<'a>(env: &ExprEnv, id: &'a String) -> String {
        let s: Cow<'a, str> = match env.codegen_env {
            Some(env) => {
                if env.namespace.name.is_none()
                    && id
                        .as_bytes()
                        .iter()
                        .rposition(|c| *c == b'\\')
                        .map_or(true, |i| i < 1)
                {
                    strip_global_ns(id).into()
                } else {
                    add_ns(id)
                }
            }
            _ => id.into(),
        };
        // TODO(shiqicao): avoid allocating string, fix escape
        escaper::escape(s.as_ref())
    }
    use ast::Expr_ as E_;
    match expr {
        E_::Id(id) => w.write(adjust_id(env, &id.1)),
        E_::Lvar(lid) => w.write(escaper::escape(&(lid.1).1)),
        E_::Float(f) => not_impl!(),
        E_::Int(i) => {
            w.write(integer::to_decimal(i.as_str()).map_err(|_| Error::fail("ParseIntError"))?)
        }
        E_::String(s) => not_impl!(),
        E_::Null => w.write("NULL"),
        E_::True => w.write("true"),
        E_::False => w.write("false"),
        // For arrays and collections, we are making a conscious decision to not
        // match HHMV has HHVM's emitter has inconsistencies in the pretty printer
        // https://fburl.com/tzom2qoe
        E_::Array(afl) => wrap_by_(w, "array(", ")", |w| print_afields(w, env, afl)),
        E_::Collection(c) if (c.0).1 == "vec" || (c.0).1 == "dict" || (c.0).1 == "keyset" => {
            w.write(&(c.0).1)?;
            wrap_by_square(w, |w| print_afields(w, env, &c.2))
        }
        E_::Collection(c) => {
            let name = strip_ns((c.0).1.as_str());
            let name = types::fix_casing(&name);
            match name {
                "Set" | "Pair" | "Vector" | "Map" | "ImmSet" | "ImmVector" | "ImmMap" => {
                    w.write("HH\\\\")?;
                    w.write(name)?;
                    wrap_by_(w, " {", "}", |w| {
                        Ok(if !c.2.is_empty() {
                            w.write(" ")?;
                            print_afields(w, env, &c.2)?;
                            w.write(" ")?;
                        })
                    })
                }
                _ => Err(Error::fail(format!(
                    "Default value for an unknow collection - {}",
                    name
                ))),
            }
        }
        E_::Shape(_) | E_::Binop(_) | E_::Call(_) | E_::New(_) => not_impl!(),
        E_::Record(r) => {
            w.write(lstrip(adjust_id(env, &(r.0).1).as_str(), "\\\\"))?;
            print_key_values(w, env, &r.2)
        }
        E_::ClassConst(cc) => {
            if let Some(e1) = (cc.0).1.as_ciexpr() {
                not_impl!()
            } else {
                Err(Error::fail("TODO: Only expected CIexpr in class_const"))
            }
        }
        E_::Unop(u) => match u.0 {
            ast::Uop::Upincr => {
                print_expr(w, env, &u.1)?;
                w.write("++")
            }
            ast::Uop::Updecr => {
                print_expr(w, env, &u.1)?;
                w.write("--")
            }
            _ => {
                print_uop(w, u.0)?;
                print_expr(w, env, &u.1)
            }
        },
        E_::ObjGet(og) => {
            print_expr(w, env, &og.0)?;
            w.write(match og.2 {
                ast::OgNullFlavor::OGNullthrows => "->",
                ast::OgNullFlavor::OGNullsafe => "\\?->",
            })?;
            print_expr(w, env, &og.1)
        }
        E_::Clone(e) => {
            w.write("clone ")?;
            print_expr(w, env, e)
        }
        E_::ArrayGet(ag) => not_impl!(),
        E_::String2(ss) => concat_by(w, " . ", ss, |w, s| print_expr(w, env, s)),
        E_::PrefixedString(s) => {
            w.write(&s.0)?;
            w.write(" . ")?;
            print_expr(w, env, &s.1)
        }
        E_::Eif(eif) => {
            print_expr(w, env, &eif.0)?;
            w.write(" \\? ")?;
            option(w, &eif.1, |w, etrue| print_expr(w, env, etrue))?;
            w.write(" : ")?;
            print_expr(w, env, &eif.2)
        }
        E_::BracedExpr(e) => wrap_by_braces(w, |w| print_expr(w, env, e)),
        E_::ParenthesizedExpr(e) => wrap_by_paren(w, |w| print_expr(w, env, e)),
        E_::Cast(c) => {
            wrap_by_paren(w, |w| print_hint(w, &c.0))?;
            print_expr(w, env, &c.1)
        }
        E_::Pipe(p) => {
            print_expr(w, env, &p.1)?;
            w.write(" |> ")?;
            print_expr(w, env, &p.2)
        }
        E_::Is(i) => {
            print_expr(w, env, &i.0)?;
            w.write(" is ")?;
            print_hint(w, &i.1)
        }
        E_::As(a) => {
            print_expr(w, env, &a.0)?;
            w.write(if a.2 { " ?as " } else { " as " })?;
            print_hint(w, &a.1)
        }
        E_::Varray(va) => wrap_by_(w, "varray[", "]", |w| {
            concat_by(w, ", ", &va.1, |w, e| print_expr(w, env, e))
        }),
        E_::Darray(da) => wrap_by_(w, "darray[", "]", |w| print_key_values(w, env, &da.1)),
        E_::List(l) => wrap_by_(w, "list(", ")", |w| {
            concat_by(w, ", ", l, |w, i| print_expr(w, env, i))
        }),
        E_::Yield(y) => {
            w.write("yield ")?;
            print_afield(w, env, y)
        }
        E_::Await(e) => {
            w.write("await ")?;
            print_expr(w, env, e)
        }
        E_::YieldBreak => w.write("return"),
        E_::YieldFrom(e) => {
            w.write("yield from ")?;
            print_expr(w, env, e)
        }
        E_::Import(i) => {
            print_import_flavor(w, &i.0)?;
            w.write(" ")?;
            print_expr(w, env, &i.1)
        }
        E_::Xml(_) => not_impl!(),
        E_::Efun(_) => not_impl!(),
        E_::Omitted => Ok(()),
        E_::Lfun(_) => Err(Error::fail(
            "expected Lfun to be converted to Efun during closure conversion print_expr",
        )),
        E_::Suspend(_) | E_::Callconv(_) | E_::ExprList(_) => {
            Err(Error::fail("illegal default value"))
        },
        _ => Err(Error::fail(
            "TODO Unimplemented: We are missing a lot of cases in the case match. Delete this catchall"
        ))
    }
}

fn print_hint<W: Write>(w: &mut W, hint: &ast::Hint) -> Result<(), W::Error> {
    not_impl!()
}

fn print_import_flavor<W: Write>(w: &mut W, flavor: &ast::ImportFlavor) -> Result<(), W::Error> {
    use ast::ImportFlavor as F;
    w.write(match flavor {
        F::Include => "include",
        F::Require => "require",
        F::IncludeOnce => "include_once",
        F::RequireOnce => "require_once",
    })
}

fn print_param_user_attributes<W: Write>(w: &mut W, param: &HhasParam) -> Result<(), W::Error> {
    match &param.user_attributes[..] {
        [] => Ok(()),
        _ => not_impl!(),
    }
}

fn string_of_span(&Span(line_begin, line_end): &Span) -> String {
    format!("({},{})", line_begin, line_end)
}

fn print_fun_attrs<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    f: &HhasFunction,
) -> Result<(), W::Error> {
    use hhas_attribute::*;
    let user_attrs = &f.attributes;
    let mut special_attrs = vec![];
    if let Ok(attr) = f.rx_level.try_into() {
        special_attrs.push(attr);
    }
    if has_meth_caller(user_attrs) {
        special_attrs.push("builtin");
        special_attrs.push("is_meth_caller");
    }
    if f.is_interceptable() {
        special_attrs.push("interceptable");
    }
    if has_foldable(user_attrs) {
        special_attrs.push("foldable");
    }
    if has_provenance_skip_frame(user_attrs) {
        special_attrs.push("prov_skip_frame");
    }
    if f.is_no_injection() {
        special_attrs.push("no_injection");
    }
    if !f.is_top() {
        special_attrs.push("nontop");
    }
    if ctx.is_system_lib() || (has_dynamically_callable(user_attrs) && !f.is_memoize_impl()) {
        special_attrs.push("dyn_callable")
    }
    print_special_and_user_attrs(ctx, w, &special_attrs, user_attrs)
}

fn print_special_and_user_attrs<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    specials: &[&str],
    users: &[HhasAttribute],
) -> Result<(), W::Error> {
    if !users.is_empty() || !specials.is_empty() {
        wrap_by_square(w, |w| {
            concat_str_by(w, " ", specials)?;
            if !specials.is_empty() && !users.is_empty() {
                w.write(" ")?;
            }
            print_attributes(ctx, w, users)
        })?;
        w.write(" ")?;
    }
    Ok(())
}

fn print_upper_bounds<W: Write>(
    w: &mut W,
    ubs: impl AsRef<[(String, Vec<HhasTypeInfo>)]>,
) -> Result<(), W::Error> {
    wrap_by_braces(w, |w| concat_by(w, ", ", ubs, print_upper_bound))
}

fn print_upper_bound<W: Write>(
    w: &mut W,
    (id, tys): &(String, Vec<HhasTypeInfo>),
) -> Result<(), W::Error> {
    wrap_by_paren(w, |w| {
        concat_str_by(w, " ", [id.as_str(), "as", ""])?;
        concat_by(w, ", ", &tys, print_type_info)
    })
}

fn print_type_info<W: Write>(w: &mut W, ti: &HhasTypeInfo) -> Result<(), W::Error> {
    print_type_info_(w, false, ti)
}

fn print_type_flags<W: Write>(w: &mut W, flag: constraint::Flags) -> Result<(), W::Error> {
    let mut first = true;
    let mut print_space = |w: &mut W| -> Result<(), W::Error> {
        if !first {
            w.write(" ")
        } else {
            Ok(first = false)
        }
    };
    use constraint::Flags as F;
    if flag.contains(F::DISPLAY_NULLABLE) {
        print_space(w)?;
        w.write("display_nullable")?;
    }
    if flag.contains(F::EXTENDED_HINT) {
        print_space(w)?;
        w.write("extended_hint")?;
    }
    if flag.contains(F::NULLABLE) {
        print_space(w)?;
        w.write("nullable")?;
    }

    if flag.contains(F::SOFT) {
        print_space(w)?;
        w.write("soft")?;
    }
    if flag.contains(F::TYPE_CONSTANT) {
        print_space(w)?;
        w.write("type_constant")?;
    }

    if flag.contains(F::TYPE_VAR) {
        print_space(w)?;
        w.write("type_var")?;
    }

    if flag.contains(F::UPPERBOUND) {
        print_space(w)?;
        w.write("upperbound")?;
    }
    Ok(())
}

fn print_type_info_<W: Write>(w: &mut W, is_enum: bool, ti: &HhasTypeInfo) -> Result<(), W::Error> {
    let print_quote_str = |w: &mut W, opt: &Option<String>| {
        option_or(
            w,
            opt,
            |w, s: &String| wrap_by_quotes(w, |w| w.write(escape(s.as_ref()))),
            "N",
        )
    };

    wrap_by_angle(w, |w| {
        print_quote_str(w, &ti.user_type)?;
        w.write(" ")?;
        if !is_enum {
            print_quote_str(w, &ti.type_constraint.name)?;
            w.write(" ")?;
        }
        print_type_flags(w, ti.type_constraint.flags)
    })
}

fn print_typedef_info<W: Write>(w: &mut W, ti: &HhasTypeInfo) -> Result<(), W::Error> {
    wrap_by_angle(w, |w| {
        w.write(quote_string(
            ti.type_constraint.name.as_ref().map_or("", |n| n.as_str()),
        ))?;
        let flags = ti.type_constraint.flags & constraint::Flags::NULLABLE;
        if !flags.is_empty() {
            wrap_by(w, " ", |w| {
                print_type_flags(w, ti.type_constraint.flags & constraint::Flags::NULLABLE)
            })?;
        }
        Ok(())
    })
}

fn print_extends<W: Write>(w: &mut W, base: Option<&str>) -> Result<(), W::Error> {
    match base {
        None => Ok(()),
        Some(b) => concat_str_by(w, " ", [" extends", b]),
    }
}

fn print_record_field<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    Field(name, type_info, intial_value): &Field,
) -> Result<(), W::Error> {
    ctx.newline(w)?;
    w.write(".property ")?;
    match intial_value {
        Some(_) => w.write("[public] ")?,
        None => w.write("[public sys_initial_val] ")?,
    }
    print_type_info(w, type_info)?;
    concat_str_by(w, " ", ["", name, "="])?;

    ctx.block(w, |c, w| {
        c.newline(w)?;
        match intial_value {
            None => w.write("uninit")?,
            Some(value) => wrap_by_triple_quotes(w, |w| print_adata(c, w, value))?,
        }
        w.write(";")
    })
}

fn print_record_def<W: Write>(
    ctx: &mut Context,
    w: &mut W,
    record: &HhasRecord,
) -> Result<(), W::Error> {
    newline(w)?;
    if record.is_abstract {
        concat_str_by(w, " ", [".record", record.name.to_raw_string()])?;
    } else {
        concat_str_by(w, " ", [".record", "[final]", record.name.to_raw_string()])?;
    }
    print_extends(w, record.base.as_ref().map(|b| b.to_raw_string()))?;
    w.write(" ")?;

    wrap_by_braces(w, |w| {
        ctx.block(w, |c, w| {
            concat(w, &record.fields, |w, rf| print_record_field(c, w, rf))
        })?;
        ctx.newline(w)
    })?;
    newline(w)
}

fn print_hhbc_id<'a, W: Write, I: Id<'a>>(w: &mut W, id: &I) -> Result<(), W::Error> {
    wrap_by_quotes(w, |w| w.write(escape(id.to_raw_string())))
}
