// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.
use crate::typing_env_return_info;
pub use crate::typing_env_types::*;
use decl_provider_rust as decl_provider;
use typing_defs_rust as typing_defs;
use typing_defs_rust::typing_make_type::TypeBuilder;

use oxidized::relative_path::RelativePath;

pub fn empty_global_env<'a>(
    builder: &'a TypeBuilder<'a>,
    provider: &'a dyn decl_provider::DeclProvider,
    file: RelativePath,
) -> Genv<'a> {
    Genv {
        file,
        tcopt: oxidized::global_options::GlobalOptions::default(),
        params: LocalIdMap::new(),
        return_info: typing_env_return_info::TypingEnvReturnInfo {
            explicit: false,
            mutable: false,
            void_to_rx: false,
            disposable: false,
            type_: typing_defs::PossiblyEnforcedTy {
                enforced: false,
                type_: builder.nothing(builder.mk_rnone()),
            },
        },
        builder,
        provider,
    }
}
