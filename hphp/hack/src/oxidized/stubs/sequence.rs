// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use ocamlrep::OcamlRep;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Sequence<T>(T);

impl<T: OcamlRep> OcamlRep for Sequence<T> {
    fn to_ocamlrep<'a, A: ocamlrep::Allocator<'a>>(&self, _alloc: &A) -> ocamlrep::Value<'a> {
        unimplemented!()
    }

    fn from_ocamlrep(_value: ocamlrep::Value<'_>) -> Result<Self, ocamlrep::FromError> {
        unimplemented!()
    }
}
