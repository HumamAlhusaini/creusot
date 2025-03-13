pub(crate) mod constant;
pub(crate) mod external;
#[allow(dead_code)]
pub(crate) mod fmir;
pub(crate) mod function;
pub(crate) mod pearlite;
pub(crate) mod specification;
pub(crate) mod traits;

use crate::{
    backend::{Why3Generator, is_trusted_item},
    contracts_items::{
        AreContractsLoaded, are_contracts_loaded, is_logic, is_no_translate, is_predicate, is_spec,
    },
    ctx::{self},
    error::{Error, InternalError},
    metadata,
    options::Output,
    translated_item::FileModule,
    validate::{
        validate_impls, validate_opacity, validate_purity, validate_terminates, validate_traits,
        validate_trusted,
    },
};
use ctx::TranslationCtx;
use rustc_hir::{def::DefKind, def_id::DefId};
use rustc_middle::ty::TyCtxt;
use rustc_span::DUMMY_SP;
use std::{fs::File, io::Write, path::PathBuf, time::Instant};
use why3::{
    Ident,
    declaration::{Attribute, Decl, Module},
    printer::{self, Print, pretty_blocks},
};

pub(crate) fn before_analysis(ctx: &mut TranslationCtx) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    match are_contracts_loaded(ctx.tcx) {
        AreContractsLoaded::Yes => {},
        AreContractsLoaded::No => ctx.fatal_error(DUMMY_SP, "The `creusot_contracts` crate is not loaded. You will not be able to verify any code using Creusot until you do so.").with_note("Don't forget to actually use creusot_contracts: `use creusot_contracts::*;`").emit(),
        AreContractsLoaded::MissingItems(missing) => {
            let mut message = String::from("The `creusot_contracts` crate is loaded, but the following items are missing: ");
            for (i, item) in missing.iter().enumerate() {
                if i != 0 {
                    message.push_str(", ");
                }
                message.push_str(item);
            }
            message.push_str(". Maybe your version of `creusot-contracts` is wrong?");
            ctx.fatal_error(DUMMY_SP, &message).emit()
        },
    }

    ctx.load_metadata();
    match ctx.load_extern_specs() {
        Ok(()) => {}
        Err(Error::MustPrint(msg)) => msg.emit(ctx.tcx),
        Err(Error::TypeCheck(_)) => ctx.tcx.dcx().abort_if_errors(),
    };

    for def_id in ctx.tcx.hir().body_owners() {
        // OK to ignore this error, because we abort after the loop.
        let _ = validate_purity(ctx, def_id);

        let def_id = def_id.to_def_id();
        if (is_spec(ctx.tcx, def_id) || is_predicate(ctx.tcx, def_id) || is_logic(ctx.tcx, def_id))
            && !is_trusted_item(ctx.tcx, def_id)
        {
            // OK to ignore this error, because we abort after the loop.
            let _ = ctx.term(def_id);
            let _ = validate_opacity(ctx, def_id);
        }
    }
    ctx.tcx.dcx().abort_if_errors();
    // OK to ignore this error, because we abort right after.
    let _ = validate_terminates(ctx);
    ctx.tcx.dcx().abort_if_errors();

    // Check that all trait laws are well-formed
    validate_traits(ctx);
    validate_impls(ctx);
    validate_trusted(ctx);

    debug!("before_analysis: {:?}", start.elapsed());
    Ok(())
}

fn should_translate(tcx: TyCtxt, mut def_id: DefId) -> bool {
    loop {
        if is_no_translate(tcx, def_id) {
            return false;
        }

        if tcx.is_closure_like(def_id) {
            def_id = tcx.parent(def_id);
        } else {
            return true;
        }
    }
}

// TODO: Move the main loop out of `translation.rs`
pub(crate) fn after_analysis(ctx: TranslationCtx) -> Result<(), Box<dyn std::error::Error>> {
    let mut why3 = Why3Generator::new(ctx);

    let start = Instant::now();
    for def_id in why3.hir().body_owners() {
        let def_id = def_id.to_def_id();

        if !should_translate(why3.tcx, def_id) {
            info!("Skipping {:?}", def_id);
            continue;
        }

        if why3.def_kind(def_id) == DefKind::AnonConst {
            continue;
        }

        info!("Translating body {:?}", def_id);
        // Ok to ignore, because we call `abort_if_errors` at the end of the next loop
        let _ = why3.translate(def_id);
    }

    for impls in why3.all_local_trait_impls(()).values() {
        for impl_id in impls {
            // Ok to ignore, because we call `abort_if_errors` at the end of the loop
            let _ = why3.translate(impl_id.to_def_id());
        }
    }
    why3.tcx.dcx().abort_if_errors();

    debug!("after_analysis_translate: {:?}", start.elapsed());
    let start = Instant::now();

    if why3.dcx().has_errors().is_some() {
        return Err(Box::new(InternalError("Failed to generate correct why3")));
    }

    if why3.should_export() {
        metadata::dump_exports(&mut why3);
    }

    if why3.should_compile() {
        use crate::run_why3::run_why3;

        let output_target = why3.opts.output.clone();
        let prefix = why3.opts.prefix.clone();
        let modules = why3.modules();
        let modules = modules.flat_map(|item| item.modules());

        let file = print_crate(output_target, prefix, modules)?;
        run_why3(&why3, file);
    }
    debug!("after_analysis_dump: {:?}", start.elapsed());

    Ok(())
}

pub enum OutputHandle {
    Directory(PathBuf, Vec<Ident>), // One file per Coma module, second component is a prefix for all files
    File(Box<dyn Write>),           // Monolithic output
}

fn module_output(modl: &FileModule, output: &mut OutputHandle) -> std::io::Result<()> {
    match output {
        OutputHandle::Directory(dir, prefix) => {
            let mut path = dir.clone();
            path.push(modl.path.file_name(&*prefix));
            path.set_extension("coma");
            let prefix = path.parent().unwrap();
            std::fs::create_dir_all(prefix).unwrap();
            modular_output(modl, &mut std::io::BufWriter::new(File::create(path).unwrap()))
        }
        OutputHandle::File(w) => monolithic_output(modl, &mut *w),
    }
}

fn show_attribute(attr: &Attribute) -> String {
    match attr {
        Attribute::Attr(contents) => format!("@{}", contents),
        Attribute::NamedSpan(name) => format!("%#{}", name),
        Attribute::Span(file, sline, scol, eline, ecol) => {
            format!("#\"{}\" {} {} {} {}", file, sline, scol, eline, ecol)
        }
    }
}

fn modular_output<T: Write>(modl: &FileModule, out: &mut T) -> std::io::Result<()> {
    let FileModule { path: _, modl: Module { name: _, decls, attrs, meta } } = modl;
    let attrs = attrs.iter().map(|attr| Decl::Comment(show_attribute(attr)));
    let meta = meta.iter().map(|s| Decl::Comment(s.clone()));
    let decls: Vec<Decl> = attrs.chain(meta).chain(decls.iter().cloned()).collect();
    pretty_blocks(&decls, &printer::ALLOC).1.render(120, out)?;
    writeln!(out)?;
    Ok(())
}

fn monolithic_output<T: Write>(modl: &FileModule, out: &mut T) -> std::io::Result<()> {
    modl.modl.pretty(&printer::ALLOC).1.render(120, out)?;
    writeln!(out)?;
    Ok(())
}

// Remove coma files in the `verif/{krate}/` directory to avoid obsolete files left after
// (re)moving functions in source code.
// We don't want to just `remove_dir_all()` because it may contain
// `proof.json`, `why3session.xml`, and `why3shapes.xml` files that users want to preserve.
fn remove_coma_files(dir: &PathBuf) -> std::io::Result<()> {
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                remove_coma_files(&path)?;
                let _ = std::fs::remove_dir(path); // remove the directory if it's empty, do nothing otherwise
            } else if path.extension().map_or(false, |ext| ext == "coma") {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}

fn print_crate<I: Iterator<Item = FileModule>>(
    output_target: Output,
    prefix: Vec<Ident>,
    modules: I,
) -> std::io::Result<Option<PathBuf>> {
    let (root, mut output) = match output_target {
        Output::Directory(dir) => {
            let mut outdir = dir.clone();
            for m in &prefix {
                outdir.push(m.as_str());
            }
            remove_coma_files(&outdir)?;
            (Some(dir.clone()), OutputHandle::Directory(dir, prefix))
        }
        Output::File(ref f) => {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            (
                Some(f.clone()),
                OutputHandle::File(Box::new(std::io::BufWriter::new(File::create(f)?))),
            )
        }
        Output::Stdout => (None, OutputHandle::File(Box::new(std::io::stdout()))),
        Output::None => return Ok(None),
    };

    for modl in modules {
        module_output(&modl, &mut output)?;
    }

    //flush the buffer before running why3
    drop(output);

    Ok(root)
}
