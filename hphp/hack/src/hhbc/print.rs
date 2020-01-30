// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
#![allow(unused_variables)]

mod print_env;
mod write;

pub use write::{Error, IoWrite, Result, Write};

use escaper::escape;
use hhas_program_rust::HhasProgram;
use hhas_record_def_rust::{Field, HhasRecord};
use hhbc_id_rust::Id;
use oxidized::relative_path::RelativePath;
use write::*;

pub fn print_program<W: Write>(
    path: Option<&RelativePath>,
    dump_symbol_refs: bool,
    w: &mut W,
    prog: &HhasProgram,
) -> Result<(), W::Error> {
    match path {
        Some(p) => {
            let p = escape(p.to_absoute().to_str().ok_or(Error::InvalidUTF8)?);
            let p = p.as_str();

            write_list_by(w, " ", &["#", p, "starts here"])?;
            newline(w)?;
            newline(w)?;

            write_list(w, &[".filepath ", format!("\"{}\"", p).as_str(), ";"])?;
            newline(w)?;

            print_program_(path, dump_symbol_refs, w, prog)?;
            newline(w)?;

            write_list_by(w, " ", &["#", p, "ends here"])?;
            newline(w)
        }
        None => {
            w.write("#starts here")?;
            newline(w)?;

            print_program_(path, dump_symbol_refs, w, prog)?;
            newline(w)?;

            w.write("#ends here")?;
            newline(w)
        }
    }
}

fn print_program_<W: Write>(
    path: Option<&RelativePath>,
    dump_symbol_refs: bool,
    w: &mut W,
    prog: &HhasProgram,
) -> Result<(), W::Error> {
    let is_hh = if prog.is_hh { "1" } else { "0" };
    newline(w)?;
    write_list(w, &["hh_file ", is_hh, ";"])?;

    newline(w)?;
    write_map(w, print_record_def, prog.record_defs.iter())?;

    if dump_symbol_refs {
        unimplemented!("hrust");
    }
    Ok(())
}

fn print_extends<W: Write>(w: &mut W, base: Option<&str>) -> Result<(), W::Error> {
    match base {
        None => Ok(()),
        Some(b) => write_list_by(w, " ", &["extends", b]),
    }
}

fn print_record_field<W: Write>(w: &mut W, field: &Field) -> Result<(), W::Error> {
    newline(w)
}

fn print_record_def<W: Write>(w: &mut W, record: &HhasRecord) -> Result<(), W::Error> {
    newline(w)?;
    if record.is_abstract {
        write_list_by(w, " ", &[".record", record.name.to_raw_string()])?;
    } else {
        write_list_by(w, " ", &[".record", "[final]", record.name.to_raw_string()])?;
    }
    w.write(" ")?;
    print_extends(w, record.base.as_ref().map(|b| b.to_raw_string()))?;
    w.write(" ")?;

    wrap_by_braces(w, |w| {
        write_map(w, print_record_field, record.fields.iter())?;
        newline(w)
    })?;
    newline(w)
}
