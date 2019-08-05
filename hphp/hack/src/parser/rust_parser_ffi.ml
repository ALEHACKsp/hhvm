(**
 * Copyright (c) 2016, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)
module SourceText = Full_fidelity_source_text
module SyntaxError = Full_fidelity_syntax_error
module MinimalSyntax = Full_fidelity_minimal_syntax
module Env = Full_fidelity_parser_env
module PositionedSyntax = Full_fidelity_positioned_syntax

(* We could pass ParserEnv, but the less complicated structs we need to
 * synchronize on the boundary between Rust and OCaml, the better. *)
type parser_opts = (
  bool * (* is_experimental mode *)
  bool * (* hhvm_compat_mode *)
  bool * (* php5_compat_mode *)
  bool * (* codegen *)
  bool (* allow_new_attribute_syntax *)
)
let env_to_opts env = (
  (Env.is_experimental_mode env),
  (Env.hhvm_compat_mode env),
  (Env.php5_compat_mode env),
  (Env.codegen env),
  (Env.allow_new_attribute_syntax env)
)
let set_global_lexer_env _ =
  (* Parsing of file sets up global variables in lexer module. Those variables
   * are then accessed even after parsing, with the assumption that they have
   * not changed since the tree was created (which must accidentally be true,
   * lucky us).
   * Going through Rust parser would bypass setting those variables and produces
   * incorrect results. I'll just set them here directly to maintain the same
   * behavior. *)
  Full_fidelity_lexer.Env.set
    ~rust:true

exception RustException of string

external parse_mode: SourceText.t -> FileInfo.mode option = "rust_parse_mode"

external parse_minimal:
  SourceText.t ->
  parser_opts ->
  unit * MinimalSyntax.t * SyntaxError.t list = "parse_minimal"

let parse_minimal text env =
  set_global_lexer_env env;
  parse_minimal text (env_to_opts env)

external parse_positioned:
  SourceText.t ->
  parser_opts ->
  unit * PositionedSyntax.t * SyntaxError.t list = "parse_positioned"

let parse_positioned text env =
  set_global_lexer_env env;
  parse_positioned text (env_to_opts env)

external parse_positioned_with_decl_mode_sc:
  SourceText.t ->
  parser_opts ->
  bool list * PositionedSyntax.t * SyntaxError.t list = "parse_positioned_with_decl_mode_sc"

let parse_positioned_with_decl_mode_sc text env =
  set_global_lexer_env env;
  parse_positioned_with_decl_mode_sc text (env_to_opts env)

external parse_positioned_with_coroutine_sc:
   SourceText.t ->
   parser_opts ->
   bool * PositionedSyntax.t * SyntaxError.t list = "parse_positioned_with_coroutine_sc"

let parse_positioned_with_coroutine_sc text env =
  set_global_lexer_env env;
  parse_positioned_with_coroutine_sc text (env_to_opts env)

let init () =
  Callback.register_exception "rust exception" (RustException "");
  Full_fidelity_minimal_syntax.rust_parse_ref := parse_minimal;
  Full_fidelity_positioned_syntax.rust_parse_ref := parse_positioned;
  Full_fidelity_positioned_syntax.rust_parse_with_coroutine_sc_ref :=
    parse_positioned_with_coroutine_sc;
  Full_fidelity_positioned_syntax.rust_parse_with_decl_mode_sc_ref :=
    parse_positioned_with_decl_mode_sc;
  ()
