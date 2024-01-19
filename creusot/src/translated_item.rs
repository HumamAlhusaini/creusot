use indexmap::IndexMap;
use rustc_hir::def_id::DefId;

use why3::declaration::{Decl, Module};

pub enum TranslatedItem {
    Logic {
        // A module which contains the contract with an opaque function symbol
        interface: Module,
        // A dummy module which just contains the function symbol with no contract
        stub: Module,
        // A module which contains the function body but uses stubs for all called functions
        modl: Module,
        // A module which contains the function body (and contract) but uses the actual bodies of all called functions
        proof_modl: Option<Module>,
        has_axioms: bool,
    },
    Closure {
        ty_modl: Module,
        interface: Module,
        modl: Option<Module>,
    },
    Program {
        interface: Module,
        modl: Option<Module>,
    },
    Trait {},
    Impl {
        modl: Module, // Refinement of traits,
    },
    AssocTy {
        modl: Module,
    },
    Constant {
        stub: Module,
        modl: Module,
    },
    // Types can not have dependencies yet, as Why3 does not yet have applicative clones
    Type {
        modl: Vec<Module>,
        accessors: IndexMap<DefId, IndexMap<DefId, Decl>>,
    },
    TyInv {
        modl: Module,
    },
}

impl<'a> TranslatedItem {
    pub(crate) fn modules(self) -> Box<dyn Iterator<Item = Module>> {
        use std::iter;
        use TranslatedItem::*;
        match self {
            Logic { proof_modl, .. } => Box::new(
                // iter::once(stub)
                // .chain(iter::once(interface))
                // .chain(iter::once(modl))
                proof_modl.into_iter(), // .chain(proof_modl.into_iter()),
            ),
            Program { modl, .. } => Box::new(modl.into_iter()),
            Trait { .. } => Box::new(iter::empty()),
            Impl { modl, .. } => Box::new(iter::once(modl)),
            AssocTy { .. } => Box::new(iter::empty()),
            Constant { stub, modl, .. } => {
                Box::new(std::iter::once(stub).chain(std::iter::once(modl)))
            }
            Type { mut modl, accessors, .. } => {
                modl[0].decls.extend(accessors.values().flat_map(|v| v.values()).cloned());

                Box::new(modl.into_iter())
            }
            Closure { ty_modl, modl, .. } => Box::new(iter::once(ty_modl).chain(modl.into_iter())),
            TyInv { .. } => Box::new(iter::empty()),
        }
    }
}
