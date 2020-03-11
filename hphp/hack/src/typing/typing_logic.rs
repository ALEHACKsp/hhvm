// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use bumpalo::collections::Vec;

use crate::typing_defs_core::{InternalType, Ty};

pub enum SubtypePropEnum<'a> {
    Coerce(Ty<'a>, Ty<'a>),
    IsSubtype(InternalType<'a>, InternalType<'a>),
    Conj(Vec<'a, SubtypeProp<'a>>),
    Disj(Vec<'a, SubtypeProp<'a>>),
}
pub type SubtypeProp<'a> = &'a SubtypePropEnum<'a>;
