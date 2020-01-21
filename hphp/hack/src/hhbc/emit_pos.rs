// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use env::emitter::Emitter;
use instruction_sequence_rust::InstrSeq;
use options::EvalFlags;
use oxidized::pos::Pos;
use std::convert::TryInto;

pub fn emit_pos(emitter: &Emitter, pos: &Pos) -> InstrSeq {
    if emitter
        .options()
        .eval_flags
        .contains(EvalFlags::DISASSEMBLER_SOURCE_MAPPING)
        && !pos.is_none()
    {
        let (line_begin, line_end, col_begin, col_end) = pos.info_pos_extended();
        InstrSeq::make_srcloc(
            line_begin.try_into().unwrap(),
            line_end.try_into().unwrap(),
            col_begin.try_into().unwrap(),
            col_end.try_into().unwrap(),
        )
    } else {
        InstrSeq::Empty
    }
}

pub fn emit_pos_then(emitter: &Emitter, pos: &Pos, instrs: InstrSeq) -> InstrSeq {
    InstrSeq::gather(vec![emit_pos(emitter, pos), instrs])
}
