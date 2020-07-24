// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use crate::typing_env_return_info;
use crate::typing_make_type::TypeBuilder;
use crate::typing_per_cont_env::{PerContEntry, TypingPerContEnv};
use arena_trait::Arena;
use decl_provider_rust::DeclProvider;
use ocamlrep::{Allocator, OpaqueValue, ToOcamlRep};
use oxidized::pos::Pos as OwnedPos;
use oxidized::{relative_path, typechecker_options};
use oxidized_by_ref::pos::Pos;
use std::cell::RefCell;
use typing_ast_rust::typing_inference_env::InferenceEnv;

pub use typing_collections_rust::*;
pub use typing_defs_rust::typing_logic::{SubtypeProp, *};
pub use typing_defs_rust::{Ty, *};

pub struct Env<'a> {
    pub ident_counter: isize,
    pub function_pos: &'a Pos<'a>,
    pub fresh_typarams: SSet<'a>,
    pub lenv: LocalEnv<'a>,
    pub genv: Genv<'a>,
    pub log_levels: SMap<'a, isize>,
    pub inference_env: InferenceEnv<'a>,
}

impl<'a> Env<'a> {
    pub fn new<'b>(function_pos: &'b OwnedPos, genv: Genv<'a>) -> Self {
        let function_pos = Pos::from_oxidized_in(function_pos, genv.builder.bumpalo());
        Env {
            ident_counter: 0,
            function_pos,
            fresh_typarams: SSet::empty(),
            inference_env: InferenceEnv::new(genv.builder),
            lenv: LocalEnv::initial_local(genv.builder),
            genv,
            log_levels: SMap::empty(),
        }
    }

    pub fn builder(&self) -> &'a TypeBuilder<'a> {
        self.genv.builder
    }

    pub fn bld(&self) -> &'a TypeBuilder<'a> {
        self.builder()
    }

    pub fn provider(&self) -> &'a dyn DeclProvider {
        self.genv.provider
    }

    pub fn ast_pos(&self, pos: &OwnedPos) -> &'a Pos<'a> {
        Pos::from_oxidized_with_file_in(
            pos,
            self.function_pos.filename(),
            self.genv.builder.bumpalo(),
        )
    }

    pub fn set_function_pos(&mut self, pos: &OwnedPos) {
        self.function_pos = Pos::from_oxidized_in(pos, self.genv.builder.bumpalo());
    }

    pub fn set_return_type(&mut self, ty: Ty<'a>) {
        self.genv.return_info.type_.type_ = ty;
    }
}

impl<'a> Env<'a> {
    pub fn to_oxidized<'b>(
        &'b self,
        arena: &'b impl Arena,
    ) -> oxidized_by_ref::typing_env_types::Env<'b>
    where
        'a: 'b,
    {
        let Env {
            ident_counter: _,
            function_pos,
            fresh_typarams: _,
            lenv,
            genv,
            log_levels: _,
            inference_env,
        } = self;
        oxidized_by_ref::typing_env_types::Env {
            function_pos,
            fresh_typarams: Default::default(), // TODO(hrust)
            lenv: lenv.to_oxidized(arena),
            genv: genv.to_oxidized(),
            decl_env: (),
            in_loop: false,
            in_try: false,
            in_case: false,
            inside_constructor: false,
            inside_ppl_class: false,
            global_tpenv: oxidized_by_ref::type_parameter_env::TypeParameterEnv {
                tparams: Default::default(),
                consistent: true,
            },
            log_levels: Default::default(), // TODO(hrust)
            inference_env: inference_env.to_oxidized(arena),
            allow_wildcards: false,
            big_envs: RefCell::new(&[]),
            pessimize: false,
        }
    }
}

impl ToOcamlRep for Env<'_> {
    fn to_ocamlrep<'a, A: Allocator>(&self, alloc: &'a A) -> OpaqueValue<'a> {
        let arena = bumpalo::Bump::new();
        self.to_oxidized(&arena).to_ocamlrep(alloc)
    }
}

pub struct Genv<'a> {
    pub tcopt: typechecker_options::TypecheckerOptions,
    pub return_info: typing_env_return_info::TypingEnvReturnInfo<'a>,
    pub params: Map<'a, LocalId<'a>, (Ty<'a>, ParamMode)>,
    pub file: relative_path::RelativePath,
    pub builder: &'a TypeBuilder<'a>,
    pub provider: &'a dyn DeclProvider,
}

impl<'a> Genv<'a> {
    pub fn to_oxidized(&self) -> oxidized_by_ref::typing_env_types::Genv<'a> {
        // TODO(hrust) most fields of Genv
        oxidized_by_ref::typing_env_types::Genv {
            tcopt: oxidized::global_options::GlobalOptions::default(),
            return_: oxidized_by_ref::typing_env_return_info::TypingEnvReturnInfo {
                type_: oxidized_by_ref::typing_defs::PossiblyEnforcedTy {
                    enforced: true,
                    type_: oxidized_by_ref::typing_defs_core::Ty(Reason::none(), &Ty_::Tmixed),
                },
                disposable: false,
                mutable: false,
                explicit: true,
                void_to_rx: false,
            },
            params: Default::default(),
            condition_types: Default::default(),
            parent: None,
            self_: None,
            static_: false,
            fun_kind: oxidized_by_ref::ast_defs::FunKind::FSync,
            val_kind: oxidized_by_ref::typing_defs::ValKind::Lval,
            fun_mutable: None,
            file: oxidized_by_ref::relative_path::RelativePath::make(
                oxidized_by_ref::relative_path::Prefix::Root,
                "",
            ),
        }
    }
}

#[derive(Debug)]
pub struct LocalEnv<'a> {
    pub per_cont_env: TypingPerContEnv<'a>,
    // TODO(hrust): mutability
    // TODO(hrust): reactivity
    // TODO(hrust): using vars
}

impl<'a> LocalEnv<'a> {
    pub fn initial_local<A: Arena>(arena: &'a A) -> Self {
        LocalEnv {
            per_cont_env: TypingPerContEnv::initial_locals(arena, PerContEntry::empty()),
        }
    }
}

impl<'a> LocalEnv<'a> {
    pub fn to_oxidized<'b>(
        &self,
        arena: &'b impl Arena,
    ) -> oxidized_by_ref::typing_env_types::LocalEnv<'b>
    where
        'a: 'b,
    {
        let LocalEnv { per_cont_env } = self;
        oxidized_by_ref::typing_env_types::LocalEnv {
            per_cont_env: per_cont_env.to_oxidized(arena),
            local_mutability: Default::default(),
            local_reactive: oxidized_by_ref::typing_env_types::Reactivity::Nonreactive,
            local_using_vars: Default::default(),
        }
    }
}

pub use oxidized_by_ref::local_id::LocalId;
