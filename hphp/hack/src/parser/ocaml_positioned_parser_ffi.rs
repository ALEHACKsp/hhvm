// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

rust_parser_ffi::parse!(
    ocaml_parse_positioned,
    ocaml_positioned_parser::parse_script
);
