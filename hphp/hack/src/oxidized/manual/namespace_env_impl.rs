// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use crate::gen::namespace_env::Env;
use crate::s_map::SMap;

use hh_autoimport_rust as hh_autoimport;

impl Env {
    pub fn empty(
        auto_ns_map: Vec<(String, String)>,
        is_codegen: bool,
        disable_xhp_element_mangling: bool,
    ) -> Self {
        Env {
            ns_uses: hh_autoimport::NAMESPACES_MAP.clone(),
            class_uses: hh_autoimport::TYPES_MAP.clone(),
            fun_uses: hh_autoimport::FUNCS_MAP.clone(),
            const_uses: hh_autoimport::CONSTS_MAP.clone(),
            record_def_uses: SMap::new(),
            name: None,
            auto_ns_map,
            is_codegen,
            disable_xhp_element_mangling,
        }
    }
}

impl std::default::Default for Env {
    fn default() -> Self {
        Self::empty(vec![], false, false)
    }
}
