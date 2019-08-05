// Copyright (c) 2019, Facebook, Inc.
// All rights reserved.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

extern crate ocaml;
use parser_rust as parser;

use crate::ocaml_coroutine_state::OcamlCoroutineState;
use crate::ocaml_syntax::OcamlSyntax;

use ocaml::core::memory;
use ocaml::core::mlvalues::{empty_list, Color, Size, Tag, Value};

use std::iter::Iterator;

use parser::coroutine_smart_constructors::{CoroutineStateType, State as CoroutineState};
use parser::decl_mode_smart_constructors::State as DeclModeState;
use parser::file_mode::FileMode;
use parser::lexable_token::LexableToken;
use parser::minimal_syntax::MinimalValue;
use parser::minimal_token::MinimalToken;
use parser::minimal_trivia::MinimalTrivia;
use parser::positioned_syntax::PositionedValue;
use parser::positioned_token::PositionedToken;
use parser::positioned_trivia::PositionedTrivia;
use parser::smart_constructors::NoState;
use parser::source_text::SourceText;
use parser::syntax::*;
use parser::syntax_error::SyntaxError;
use parser::syntax_kind::SyntaxKind;
use parser::token_kind::TokenKind;
use parser::trivia_kind::TriviaKind;

extern "C" {
    fn ocamlpool_reserve_block(tag: Tag, size: Size) -> Value;
    fn ocamlpool_reserve_string(size: Size) -> Value;
    static mut ocamlpool_generation: usize;
    static ocamlpool_limit: *mut Value;
    static ocamlpool_bound: *mut Value;
    static mut ocamlpool_cursor: *mut Value;
    static ocamlpool_color: Color;
}

unsafe fn reserve_block(tag: Tag, size: Size) -> Value {
    let result = ocamlpool_cursor.offset(-(size as isize) - 1);
    if result < ocamlpool_limit {
        return ocamlpool_reserve_block(tag, size);
    }
    ocamlpool_cursor = result;
    *result = (tag as usize) | ocamlpool_color | (size << 10);
    return result.offset(1) as Value;
}

unsafe fn caml_set_field(obj: Value, index: usize, val: Value) {
    if (val & 1 == 1)
        || ((val as *const Value) >= ocamlpool_limit && (val as *const Value) <= ocamlpool_bound)
    {
        *(obj as *mut Value).offset(index as isize) = val;
    } else {
        memory::caml_initialize((obj as *mut Value).offset(index as isize), val);
    }
}

// Unsafe functions in this file should be called only:
// - while being called from OCaml process
// - between ocamlpool_enter / ocamlpool_leave invocations
pub unsafe fn caml_block(tag: Tag, fields: &[Value]) -> Value {
    let result = reserve_block(tag, fields.len());
    for (i, field) in fields.iter().enumerate() {
        caml_set_field(result, i, *field);
    }
    return result;
}

pub unsafe fn caml_tuple(fields: &[Value]) -> Value {
    caml_block(0, fields)
}

pub struct SerializationContext {
    source_text: Value,
}

impl SerializationContext {
    pub fn new(source_text: Value) -> Self {
        Self { source_text }
    }
}

pub trait ToOcaml {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value;
}

pub unsafe fn to_list<T>(values: &[T], context: &SerializationContext) -> Value
where
    T: ToOcaml,
{
    let mut res = empty_list();

    for v in values.iter().rev() {
        res = caml_tuple(&[v.to_ocaml(context), res])
    }
    res
}

// Not implementing ToOcaml for integer types, because Value itself is an integer too and it makes
// it too easy to accidentally treat a pointer to heap as integer and try double convert it
fn usize_to_ocaml(x: usize) -> Value {
    (x << 1) + 1
}

pub fn u8_to_ocaml(x: u8) -> Value {
    usize_to_ocaml(x as usize)
}

impl ToOcaml for bool {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        usize_to_ocaml(*self as usize)
    }
}

impl ToOcaml for Vec<bool> {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        to_list(&self, context)
    }
}

impl<S> ToOcaml for DeclModeState<S> {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        self.stack().to_ocaml(context)
    }
}

impl ToOcaml for TokenKind {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        u8_to_ocaml(self.ocaml_tag())
    }
}

impl ToOcaml for TriviaKind {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        u8_to_ocaml(self.ocaml_tag())
    }
}

impl ToOcaml for MinimalTrivia {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        // From full_fidelity_minimal_trivia.ml:
        // type t = {
        //   kind: Full_fidelity_trivia_kind.t;
        //   width: int
        // }
        let kind = self.kind.to_ocaml(context);
        let width = usize_to_ocaml(self.width);
        caml_tuple(&[kind, width])
    }
}

impl ToOcaml for MinimalToken {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        let kind = self.kind().to_ocaml(context);
        let width = usize_to_ocaml(self.width());
        let leading = to_list(&self.leading, context);
        let trailing = to_list(&self.trailing, context);

        // From full_fidelity_minimal_token.ml:
        // type t = {
        //   kind: TokenKind.t;
        //   width: int;
        //   leading: Trivia.t list;
        //   trailing: Trivia.t list
        // }
        caml_tuple(&[kind, width, leading, trailing])
    }
}

impl ToOcaml for MinimalValue {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        // From full_fidelity_minimal_syntax.ml:
        // type t = { full_width: int }
        let full_width = usize_to_ocaml(self.full_width);
        caml_tuple(&[full_width])
    }
}

impl<Token, SyntaxValue> ToOcaml for Syntax<Token, SyntaxValue>
where
    Token: LexableToken + ToOcaml,
    SyntaxValue: SyntaxValueType<Token> + ToOcaml,
{
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        let value = self.value.to_ocaml(context);

        let syntax = match &self.syntax {
            SyntaxVariant::Missing => u8_to_ocaml(SyntaxKind::Missing.ocaml_tag()),
            SyntaxVariant::Token(token) => {
                let token_kind = token.kind();
                let token = token.to_ocaml(context);
                caml_block(SyntaxKind::Token(token_kind).ocaml_tag(), &[token])
            }
            SyntaxVariant::SyntaxList(l) => {
                let l = to_list(l, context);
                caml_block(SyntaxKind::SyntaxList.ocaml_tag(), &[l])
            }
            _ => {
                let tag = self.kind().ocaml_tag() as u8;
                // This could be much more readable by constructing a vector of children and
                // passing it to caml_block, but the cost of this intermediate vector allocation is
                // too big
                let n = Self::fold_over_children(&|_, n| n + 1, 0, &self.syntax);
                let result = reserve_block(tag, n);
                // Similarly, fold_over_children() avoids intermediate allocation done by children()
                Self::fold_over_children(
                    &|field, i| {
                        let field = field.to_ocaml(context);
                        caml_set_field(result, i, field);
                        i + 1
                    },
                    0,
                    &self.syntax,
                );
                result
            }
        };
        caml_tuple(&[syntax, value])
    }
}

impl ToOcaml for PositionedTrivia {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        // From full_fidelity_positioned_trivia.ml:
        // type t = {
        //   kind: TriviaKind.t;
        //   source_text : SourceText.t;
        //   offset : int;
        //   width : int
        // }
        caml_tuple(&[
            self.kind.to_ocaml(context),
            context.source_text,
            usize_to_ocaml(self.offset),
            usize_to_ocaml(self.width),
        ])
    }
}

// TODO (kasper): we replicate LazyTrivia memory saving bit-packing when converting from Rust to
// OCaml values, but Rust values themselves are not packed. We should consider porting this
// optimization there too.
fn trivia_kind_mask(kind: TriviaKind) -> usize {
    1 << (62 - (kind.ocaml_tag()))
}

fn build_lazy_trivia(trivia_list: &[PositionedTrivia], acc: Option<usize>) -> Option<usize> {
    trivia_list
        .iter()
        .fold(acc, |acc, trivia| match (acc, trivia.kind) {
            (None, _) | (_, TriviaKind::AfterHaltCompiler) | (_, TriviaKind::ExtraTokenError) => {
                None
            }
            (Some(mask), kind) => Some(mask | trivia_kind_mask(kind)),
        })
}

unsafe fn get_forward_pointer(token: &PositionedToken) -> Value {
    if token.0.ocamlpool_generation == ocamlpool_generation {
        return token.0.ocamlpool_forward_pointer;
    } else {
        return ocaml::core::mlvalues::UNIT;
    }
}

unsafe fn set_forward_pointer(token: &PositionedToken, value: Value) {
    *std::mem::transmute::<&usize, *mut Value>(&token.0.ocamlpool_forward_pointer) = value;
    *std::mem::transmute::<&usize, *mut usize>(&token.0.ocamlpool_generation) =
        ocamlpool_generation;
}

impl ToOcaml for PositionedToken {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        let res = get_forward_pointer(self);
        if res != ocaml::core::mlvalues::UNIT {
            return res;
        }

        let kind = self.kind().to_ocaml(context);
        let offset = usize_to_ocaml(self.offset());
        let leading_width = usize_to_ocaml(self.leading_width());
        let width = usize_to_ocaml(self.width());
        let trailing_width = usize_to_ocaml(self.trailing_width());

        let lazy_trivia_mask = Some(0);
        let lazy_trivia_mask = build_lazy_trivia(&self.leading(), lazy_trivia_mask);
        let lazy_trivia_mask = build_lazy_trivia(&self.trailing(), lazy_trivia_mask);

        let trivia = match lazy_trivia_mask {
            Some(mask) => usize_to_ocaml(mask),
            None => {
                //( Trivia.t list * Trivia.t list)
                let leading = to_list(self.leading(), context);
                let trailing = to_list(self.trailing(), context);
                caml_tuple(&[leading, trailing])
            }
        };
        // From full_fidelity_positioned_token.ml:
        // type t = {
        //   kind: TokenKind.t;
        //   source_text: SourceText.t;
        //   offset: int; (* Beginning of first trivia *)
        //   leading_width: int;
        //   width: int; (* Width of actual token, not counting trivia *)
        //   trailing_width: int;
        //   trivia: LazyTrivia.t;
        // }
        let res = caml_tuple(&[
            kind,
            context.source_text,
            offset,
            leading_width,
            width,
            trailing_width,
            trivia,
        ]);
        set_forward_pointer(self, res);
        res
    }
}

const TOKEN_VALUE_VARIANT: u8 = 0;
const TOKEN_SPAN_VARIANT: u8 = 1;
const MISSING_VALUE_VARIANT: u8 = 2;

impl ToOcaml for PositionedValue {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        match self {
            PositionedValue::TokenValue(t) => {
                let token = t.to_ocaml(context);
                // TokenValue of  ...
                caml_block(TOKEN_VALUE_VARIANT, &[token])
            }
            PositionedValue::TokenSpan { left, right } => {
                let left = left.to_ocaml(context);
                let right = right.to_ocaml(context);
                // TokenSpan { left: Token.t; right: Token.t }
                caml_block(TOKEN_SPAN_VARIANT, &[left, right])
            }
            PositionedValue::Missing { offset } => {
                let offset = usize_to_ocaml(*offset);
                // Missing of {...}
                caml_block(MISSING_VALUE_VARIANT, &[context.source_text, offset])
            }
        }
    }
}

impl ToOcaml for Option<FileMode> {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        match self {
            None => usize_to_ocaml(0),
            Some(x) => {
                let tag: u8 = match x {
                    FileMode::Mphp => 0,
                    FileMode::Mdecl => 1,
                    FileMode::Mstrict => 2,
                    FileMode::Mpartial => 3,
                    FileMode::Mexperimental => 4,
                };
                caml_tuple(&[u8_to_ocaml(tag)])
            }
        }
    }
}

impl ToOcaml for SyntaxError {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        // type error_type = ParseError | RuntimeError
        //
        // type t = {
        //   child        : t option;
        //   start_offset : int;
        //   end_offset   : int;
        //   error_type   : error_type;
        //   message      : string;
        // }

        let child = usize_to_ocaml(0); // None
        let start_offset = usize_to_ocaml(self.start_offset);
        let end_offset = usize_to_ocaml(self.end_offset);
        let error_type = usize_to_ocaml(0); // ParseError

        let m = self.message.as_bytes();
        let message = ocamlpool_reserve_string(m.len());
        let mut str_ = ocaml::Str::from(ocaml::Value::new(message));
        str_.data_mut().copy_from_slice(m);

        caml_tuple(&[child, start_offset, end_offset, error_type, message])
    }
}

impl ToOcaml for NoState {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        ocaml::core::mlvalues::UNIT
    }
}

/// Blanket implementation for states of Smart Constructors that need to access SourceText;
/// such SC by convention wrap their state into a pair (State, &SourceText).
impl<'a, T> ToOcaml for (T, SourceText<'a>)
where
    T: ToOcaml,
{
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        // don't serialize .1 (source text) as it is not part the real state we care about
        self.0.to_ocaml(_context)
    }
}

impl<'a, S> ToOcaml for CoroutineState<'a, S> {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        self.seen_ppl().to_ocaml(_context)
    }
}

impl<'a, S> ToOcaml for OcamlCoroutineState<'a, S> {
    unsafe fn to_ocaml(&self, context: &SerializationContext) -> Value {
        self.seen_ppl().to_ocaml(context)
    }
}

impl<V> ToOcaml for OcamlSyntax<V> {
    unsafe fn to_ocaml(&self, _context: &SerializationContext) -> Value {
        self.syntax
    }
}
