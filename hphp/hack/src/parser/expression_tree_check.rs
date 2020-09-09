// Copyright (c) 2020, Facebook, Inc.
// All rights reserved.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use oxidized::{
    aast,
    aast_defs::CallType,
    aast_visitor::{visit, AstParams, Node, Visitor},
    ast_defs::*,
    pos::Pos,
};
use parser_core_types::syntax_error::SyntaxError;

struct Context {
    in_expression_tree: bool,
}

struct Checker {
    errors: Vec<SyntaxError>,
}

impl Checker {
    fn syntax_error_at(&mut self, p: &Pos) {
        let (start_offset, end_offset) = p.info_raw();
        self.errors.push(SyntaxError::make(
            start_offset,
            end_offset,
            "Unsupported syntax for expression trees.".into(),
        ));
    }
}

impl<'ast> Visitor<'ast> for Checker {
    type P = AstParams<Context, ()>;

    fn object(&mut self) -> &mut dyn Visitor<'ast, P = Self::P> {
        self
    }

    fn visit_stmt(
        &mut self,
        c: &mut Context,
        s: &'ast aast::Stmt<Pos, (), (), ()>,
    ) -> Result<(), ()> {
        use aast::Stmt_::*;

        match &s.1 {
            Expr(_) => {}
            Return(_) => {}
            If(_) => {}
            // Primarily used for if without else, but also used for standalone ;.
            Noop => {}
            // Ban any other statement syntax inside expression trees.
            _ => {
                if c.in_expression_tree {
                    self.syntax_error_at(&s.0);
                    // Don't recurse, so we don't also produce syntax errors for child nodes.
                    return Ok(());
                }
            }
        }

        s.recurse(c, self)
    }

    fn visit_expr(&mut self, c: &mut Context, e: &aast::Expr<Pos, (), (), ()>) -> Result<(), ()> {
        use aast::Expr_::*;

        // Ensure the context tracks whether we're in a backtick.
        match &e.1 {
            ExpressionTree(_) => {
                c.in_expression_tree = true;
                let res = e.recurse(c, self);
                c.in_expression_tree = false;
                return res;
            }
            _ => {
                // If we're not in a backtick, all syntax is allowed.
                if !c.in_expression_tree {
                    return e.recurse(c, self);
                }
            }
        }

        if c.in_expression_tree {
            let valid_syntax = match &e.1 {
                // Allow integer and string literals.
                Int(_) => true,
                String(_) => true,
                // Allow local variables $foo.
                Lvar(_) => true,
                // Only allow + for binary operators.
                Binop(bop) => match **bop {
                    (Bop::Plus, _, _) => true,
                    _ => false,
                },
                // Allow simple function calls.
                Call(call) => match &**call {
                    // Ban call_user_func(...)
                    (CallType::CuserFunc, _, _, _, _) => false,
                    // Ban variadic calls foo(...$x);
                    (CallType::Cnormal, _, _, _, Some(_)) => false,
                    // Ban generic type arguments foo<X, Y>();
                    (CallType::Cnormal, _, targs, _, _) if !targs.is_empty() => false,
                    (CallType::Cnormal, recv, _targs, args, _variadic) => {
                        // Only allow direct function calls, so allow
                        // foo(), but don't allow (foo())().
                        match recv.1 {
                            Id(_) => {
                                // Recurse on the arguments manually,
                                // so we don't end up visiting the
                                // Id() node. This is awkward because
                                // Id nodes can represent global
                                // constants or function names.
                                args.accept(c, self)?;
                                return Ok(());
                            }
                            _ => false,
                        }
                    }
                },
                // Allow lambdas () ==> { ... } but not PHP-style function() { ... }
                Lfun(lf) => {
                    // Don't allow parameters with default values.
                    for param in &lf.0.params {
                        if param.expr.is_some() {
                            self.syntax_error_at(&param.pos);
                            return Ok(());
                        }
                    }
                    true
                }
                // Ban all other expression syntax possibilities.
                _ => false,
            };

            if valid_syntax {
                e.recurse(c, self)
            } else {
                self.syntax_error_at(&e.0);
                // Don't recurse, so we don't also produce syntax errors for child nodes.
                Ok(())
            }
        } else {
            e.recurse(c, self)
        }
    }
}

pub fn check_program(program: &aast::Program<Pos, (), (), ()>) -> Vec<SyntaxError> {
    let mut checker = Checker { errors: vec![] };
    let mut context = Context {
        in_expression_tree: false,
    };

    visit(&mut checker, &mut context, program).unwrap();
    checker.errors
}
