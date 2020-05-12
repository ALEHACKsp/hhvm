// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use oxidized::ToOxidized;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypingContKey<'a> {
    Next,
    Continue,
    Break,
    Catch,
    Do,
    Exit,
    Fallthrough,
    Finally,
    Goto(&'a str),
}

impl<'a> ToOxidized for TypingContKey<'a> {
    type Target = oxidized::typing_cont_key::TypingContKey;

    fn to_oxidized(&self) -> Self::Target {
        use oxidized::typing_cont_key::TypingContKey as C;
        match self {
            TypingContKey::Next => C::Next,
            TypingContKey::Continue => C::Continue,
            TypingContKey::Break => C::Break,
            TypingContKey::Catch => C::Catch,
            TypingContKey::Do => C::Do,
            TypingContKey::Exit => C::Exit,
            TypingContKey::Fallthrough => C::Fallthrough,
            TypingContKey::Finally => C::Finally,
            TypingContKey::Goto(l) => C::Goto(l.to_oxidized()),
        }
    }
}
