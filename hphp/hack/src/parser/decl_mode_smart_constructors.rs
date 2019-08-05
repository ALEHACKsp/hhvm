/**
 * Copyright (c) 2019, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
*/
use crate::lexable_token::LexableToken;
use crate::parser_env::ParserEnv;
use crate::source_text::SourceText;
use crate::syntax::*;
use crate::syntax_smart_constructors::{StateType, SyntaxSmartConstructors};
use crate::token_kind::TokenKind;

pub struct State<S> {
    stack: Vec<bool>,
    phantom_s: std::marker::PhantomData<*const S>,
}
impl<S> Clone for State<S> {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            phantom_s: self.phantom_s,
        }
    }
}

impl<S> State<S> {
    /// Pops n times and returns the first popped element
    fn pop_n(&mut self, n: usize) -> bool {
        if self.stack.len() < n {
            panic!("invalid state");
        }
        let head = self.stack.pop().unwrap();
        self.stack.truncate(self.stack.len() - (n - 1)); // pop n-1 times efficiently
        head
    }

    fn push(&mut self, s: bool) {
        self.stack.push(s);
    }

    pub fn stack(&self) -> &Vec<bool> {
        &self.stack
    }
}

impl<'src, S> StateType<'src, S> for State<S> {
    fn initial(_: &ParserEnv, _: &SourceText<'src>) -> Self {
        Self {
            stack: vec![],
            phantom_s: std::marker::PhantomData,
        }
    }

    fn next(&mut self, inputs: &[&S]) {
        let st_todo = if self.stack.len() > inputs.len() {
            self.stack.split_off(self.stack.len() - inputs.len())
        } else {
            std::mem::replace(&mut self.stack, vec![])
        };
        let res = st_todo.into_iter().any(|b2| b2);
        self.push(res);
    }
}

pub use crate::decl_mode_smart_constructors_generated::*;

pub struct DeclModeSmartConstructors<S, Token, Value> {
    pub state: State<S>,
    phantom_token: std::marker::PhantomData<*const Token>,
    phantom_value: std::marker::PhantomData<*const Value>,
}
impl<'a, S, Token, Value> Clone for DeclModeSmartConstructors<S, Token, Value> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            phantom_token: self.phantom_token,
            phantom_value: self.phantom_value,
        }
    }
}

impl<'a, Token, Value>
    SyntaxSmartConstructors<'a, Syntax<Token, Value>, State<Syntax<Token, Value>>>
    for DeclModeSmartConstructors<Syntax<Token, Value>, Token, Value>
where
    Token: LexableToken,
    Value: SyntaxValueType<Token>,
{
    fn new(env: &ParserEnv, src: &SourceText<'a>) -> Self {
        Self {
            state: State::initial(env, src),
            phantom_token: std::marker::PhantomData,
            phantom_value: std::marker::PhantomData,
        }
    }

    fn make_yield_expression(&mut self, _r1: Self::R, _r2: Self::R) -> Self::R {
        self.state.pop_n(2);
        self.state.push(true);
        Self::R::make_missing(&self.state, 0)
    }

    fn make_yield_from_expression(&mut self, _r1: Self::R, _r2: Self::R, _r3: Self::R) -> Self::R {
        self.state.pop_n(3);
        self.state.push(true);
        Self::R::make_missing(&self.state, 0)
    }

    fn make_lambda_expression(
        &mut self,
        r1: Self::R,
        r2: Self::R,
        r3: Self::R,
        r4: Self::R,
        r5: Self::R,
        body: Self::R,
    ) -> Self::R {
        let saw_yield = self.state.pop_n(6);
        let body = replace_body(&self.state, body, saw_yield);
        self.state.push(false);
        Self::R::make_lambda_expression(&self.state, r1, r2, r3, r4, r5, body)
    }

    fn make_anonymous_function(
        &mut self,
        r1: Self::R,
        r2: Self::R,
        r3: Self::R,
        r4: Self::R,
        r5: Self::R,
        r6: Self::R,
        r7: Self::R,
        r8: Self::R,
        r9: Self::R,
        r10: Self::R,
        r11: Self::R,
        body: Self::R,
    ) -> Self::R {
        let saw_yield = self.state.pop_n(12);
        let body = replace_body(&self.state, body, saw_yield);
        self.state.push(false);
        Self::R::make_anonymous_function(
            &self.state,
            r1,
            r2,
            r3,
            r4,
            r5,
            r6,
            r7,
            r8,
            r9,
            r10,
            r11,
            body,
        )
    }

    fn make_awaitable_creation_expression(
        &mut self,
        r1: Self::R,
        r2: Self::R,
        r3: Self::R,
        body: Self::R,
    ) -> Self::R {
        let saw_yield = self.state.pop_n(4);
        let body = replace_body(&self.state, body, saw_yield);
        self.state.push(false);
        Self::R::make_awaitable_creation_expression(&self.state, r1, r2, r3, body)
    }

    fn make_methodish_declaration(
        &mut self,
        r1: Self::R,
        r2: Self::R,
        body: Self::R,
        r3: Self::R,
    ) -> Self::R {
        self.state.pop_n(1);
        let saw_yield = self.state.pop_n(3);
        let body = replace_body(&self.state, body, saw_yield);
        self.state.push(false);
        Self::R::make_methodish_declaration(&self.state, r1, r2, body, r3)
    }

    fn make_function_declaration(&mut self, r1: Self::R, r2: Self::R, body: Self::R) -> Self::R {
        let saw_yield = self.state.pop_n(3);
        let body = replace_body(&self.state, body, saw_yield);
        self.state.push(false);
        Self::R::make_function_declaration(&self.state, r1, r2, body)
    }
}

fn replace_body<Token, Value>(
    st: &State<Syntax<Token, Value>>,
    body: Syntax<Token, Value>,
    saw_yield: bool,
) -> Syntax<Token, Value>
where
    Token: LexableToken,
    Value: SyntaxValueType<Token>,
{
    match body.syntax {
        SyntaxVariant::CompoundStatement(children) => {
            let stmts = if saw_yield {
                let token = Token::make(TokenKind::Yield, 0, 0, vec![], vec![]);
                let yield_ = Syntax::<Token, Value>::make_token(st, token);
                Syntax::make_list(st, vec![yield_], 0)
            } else {
                Syntax::make_missing(st, 0)
            };
            Syntax::make_compound_statement(
                st,
                children.compound_left_brace,
                stmts,
                children.compound_right_brace,
            )
        }
        _ => body,
    }
}
