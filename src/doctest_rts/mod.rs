use std::{
    collections::HashSet,
    fs::read,
    hash::Hasher,
    path::{Path, PathBuf},
    sync::Arc,
};

use cargo::{
    core::{
        compiler::{Compilation, CompileKind, Doctest},
        Workspace,
    },
    util::path_args,
};
use log::{debug, trace};
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::{fx::FxIndexMap, stable_hasher::StableHasher};
use rustc_errors::FatalError;
use rustc_hir::CRATE_HIR_ID;
use rustc_session::{
    config::{host_triple, parse_externs, CodegenOptions, Options, RustcOptGroup},
    search_paths::SearchPath,
    EarlyDiagCtxt,
};
use rustc_span::def_id::{CRATE_DEF_ID, LOCAL_CRATE};

use crate::{
    checksums::{insert_hashmap, Checksums},
    doctest_rts::rustdoc::{scrape_test_config, Collector, ErrorCodes, HirCollector},
    fs_utils::{get_checksums_path, write_to_file, CacheKind},
};

use self::rustdoc::{GlobalTestOptions, LangString};

mod rustdoc;

pub fn run_analysis_doctests(
    ws: &Workspace<'_>,
    compilation: &Compilation<'_>,
    target_dir: PathBuf,
) -> HashSet<String> {
    let path = {
        let mut path = target_dir;
        path.push(Into::<&str>::into(CacheKind::Doctests));
        path
    };

    let crate_name = "doctests";
    let crate_id = 0;

    //#################################################################################################################
    // 1. Calculate checksums of doctests
    // 2. Extract names of tests

    let (_tests, new_checksums) = checksums_and_test_names(ws, compilation);

    //#################################################################################################################
    // Import checksums

    let old_checksums = {
        let checksums_path_buf = get_checksums_path(path.clone(), crate_name, crate_id);

        let maybe_checksums = read(checksums_path_buf);

        if let Ok(checksums) = maybe_checksums {
            Checksums::from(checksums.as_slice())
        } else {
            Checksums::new()
        }
    };

    //#################################################################################################################
    // 3. Calculate names of changed doctests

    let mut changed_doctests = HashSet::new();

    // We only consider nodes from the new revision
    for name in new_checksums.keys() {
        trace!("Checking {}", name);
        let changed = {
            let maybe_new = new_checksums.get(name);
            let maybe_old = old_checksums.get(name);

            match (maybe_new, maybe_old) {
                (None, _) => unreachable!(),
                (Some(_), None) => true,
                (Some(new), Some(old)) => new != old,
            }
        };

        if changed {
            debug!("Changed due to doctest checksums: {}", name);
            changed_doctests.insert(name.clone());
        }
    }

    //#################################################################################################################
    // Export new checksums

    write_to_file(
        Into::<Vec<u8>>::into(&new_checksums),
        path.clone(),
        |buf| get_checksums_path(buf, crate_name, crate_id),
        false,
    );

    changed_doctests
}

// Adapted from: https://github.com/rust-lang/rust/blob/b71fa82d786ae1b5866510f1b3a7e5b7e1890e4c/src/librustdoc/doctest.rs#L92
fn checksums_and_test_names(
    ws: &Workspace<'_>,
    compilation: &Compilation<'_>,
) -> (Vec<String>, Checksums) {
    let dcx = EarlyDiagCtxt::new(rustc_session::config::ErrorOutputType::HumanReadable(
        rustc_errors::emitter::HumanReadableErrorType::Default(rustc_errors::ColorConfig::Auto),
    ));

    let mut checksums = Checksums::new();
    let mut test_names = Vec::new();

    for doctest_info in &compilation.to_doc_test {
        let Doctest {
            args,
            unstable_opts: _,
            unit,
            linker: _,
            script_meta,
            env,
        } = doctest_info;

        //##############
        // Preparing arguments

        let input = {
            let (arg, _cwd) = path_args(ws, unit);
            arg
        };

        let search_paths = {
            let mut search_paths = Vec::new();
            for &rust_dep in &[
                &compilation.deps_output[&unit.kind],
                &compilation.deps_output[&CompileKind::Host],
            ] {
                let search_path =
                    SearchPath::from_cli_opt(&dcx, &format!("dependency={}", rust_dep.display()));
                search_paths.push(search_path);
            }
            for native_dep in compilation.native_dirs.iter() {
                let search_path = SearchPath::from_cli_opt(&dcx, native_dep.to_str().unwrap());
                search_paths.push(search_path);
            }
            search_paths
        };

        let crate_types = unit
            .target
            .rustc_crate_types()
            .into_iter()
            .map(conversion_crate_type)
            .collect();

        let matches = {
            let mut options = rustc_session::getopts::Options::new();

            // Options adapted from:
            // https://github.com/rust-lang/rust/blob/abb95639ef2b837dbfe7b5d18f51fadda29711cb/src/librustdoc/lib.rs#L219

            let stable: fn(_, fn(&mut rustc_session::getopts::Options) -> &mut _) -> _ =
                RustcOptGroup::stable;
            let unstable: fn(_, fn(&mut rustc_session::getopts::Options) -> &mut _) -> _ =
                RustcOptGroup::unstable;

            let opts = vec![
                unstable("enable-per-target-ignores", |o| {
                    o.optflagmulti(
                        "",
                        "enable-per-target-ignores",
                        "parse ignore-foo for ignoring doctests on a per-target basis",
                    )
                }),
                stable("C", |o| {
                    o.optmulti(
                        "C",
                        "codegen",
                        "pass a codegen option to rustc",
                        "OPT[=VALUE]",
                    )
                }),
                stable("error-format", |o| {
                    o.optopt(
                        "",
                        "error-format",
                        "How errors and other messages are produced",
                        "human|json|short",
                    )
                }),
                stable("cfg", |o| {
                    o.optmulti("", "cfg", "pass a --cfg to rustc", "")
                }),
                stable("check-cfg", |o| {
                    o.optmulti("", "check-cfg", "pass a --check-cfg to rustc", "")
                }),
                stable("extern", |o| {
                    o.optmulti("", "extern", "pass an --extern to rustc", "NAME[=PATH]")
                }),
                unstable("Z", |o| {
                    o.optmulti(
                        "Z",
                        "",
                        "unstable / perma-unstable options (only on nightly build)",
                        "FLAG",
                    )
                }),
            ];

            for option in opts {
                (option.apply)(&mut options);
            }

            options.parse(args).unwrap()
        };

        let mut logical_env = FxIndexMap::default();
        for (k, v) in env {
            logical_env.insert(k.clone(), v.clone().into_string().unwrap());
        }
        if let Some(metadata) = script_meta {
            if let Some(items) = compilation.extra_env.get(metadata) {
                for (k, v) in items {
                    logical_env.insert(k.clone(), v.clone());
                }
            }
        }

        let target = {
            if let CompileKind::Target(target) = unit.kind {
                Some(target.rustc_target())
            } else {
                None
            }
        };

        let codegen_options = CodegenOptions::build(&dcx, &matches);
        let unstable_opts = rustc_session::config::UnstableOptions::build(&dcx, &matches);
        let externs = parse_externs(&dcx, &matches, &unstable_opts);

        let check_cfgs = matches.opt_strs("check-cfg");

        let enable_per_target_ignores = matches.opt_present("enable-per-target-ignores");

        //##############
        // Extracting doctests

        let input = rustc_session::config::Input::File(input);

        let sessopts = Options {
            maybe_sysroot: rustc_session::filesearch::get_or_default_sysroot().ok(),
            search_paths,
            crate_types,
            lint_opts: Vec::new(),
            lint_cap: Some(rustc_session::lint::Allow),
            cg: codegen_options,
            externs,
            unstable_features: rustc_feature::UnstableFeatures::from_environment(Some(
                unit.target.crate_name().as_str(),
            )),
            actually_rustdoc: true,
            edition: convert_edition(unit.target.edition()),
            target_triple: convert_target(target),
            crate_name: Some(unit.target.crate_name()),
            logical_env,
            ..Options::default()
        };

        let mut cfgs = matches.opt_strs("cfg");
        cfgs.push("doc".to_owned());
        cfgs.push("doctest".to_owned());

        let config = rustc_interface::Config {
            opts: sessopts,
            crate_cfg: cfgs,
            crate_check_cfg: check_cfgs,
            input,
            output_file: None,
            output_dir: None,
            file_loader: None,
            locale_resources: rustc_driver::DEFAULT_LOCALE_RESOURCES,
            lint_caps: FxHashMap::default(),
            parse_sess_created: None,
            hash_untracked_state: None,
            register_lints: None,
            override_queries: None,
            make_codegen_backend: None,
            registry: rustc_driver::diagnostics_registry(),
            ice_file: None,
            using_internal_features: Arc::default(),
            expanded_args: Vec::new(),
        };

        let _nocapture = false;
        let _json_unused_externs = false;

        let tests = rustc_interface::interface::run_compiler(config, |compiler| {
            compiler.enter(|queries| {
                let collector = queries.global_ctxt().unwrap().enter(|tcx| {
                    let crate_attrs = tcx.hir().attrs(CRATE_HIR_ID);

                    let opts = scrape_test_config(crate_attrs);
                    let mut collector = Collector::new(
                        tcx.crate_name(LOCAL_CRATE).to_string(),
                        false,
                        opts,
                        Some(compiler.sess.parse_sess.clone_source_map()),
                        None,
                        enable_per_target_ignores,
                    );

                    let mut hir_collector = HirCollector::new(
                        &compiler.sess,
                        &mut collector,
                        tcx.hir(),
                        ErrorCodes::from(compiler.sess.opts.unstable_features.is_nightly_build()),
                        tcx,
                    );
                    hir_collector.visit_testable(
                        "".to_string(),
                        CRATE_DEF_ID,
                        tcx.hir().span(CRATE_HIR_ID),
                        |this| tcx.hir().walk_toplevel_module(this),
                    );

                    collector
                });
                if compiler.sess.dcx().has_errors().is_some() {
                    FatalError.raise();
                }

                collector.tests
            })
        });

        for (name, test) in tests {
            let (prefix, _) = name
                .rsplit_once(" (line")
                .expect("Found malformed doctest name");
            let name = {
                if let Some(prefix) = prefix.strip_suffix(" -") {
                    prefix
                } else {
                    prefix
                }
            }
            .to_string();

            insert_hashmap(&mut checksums, &name, get_checksum_doctest(test));
            test_names.push(name);
        }
    }

    (test_names, checksums)
}

fn convert_target(
    target: Option<cargo::util::interning::InternedString>,
) -> rustc_target::spec::TargetTriple {
    match target {
        Some(target) if target.ends_with(".json") => {
            let path = Path::new(&target);
            rustc_target::spec::TargetTriple::from_path(path)
                .unwrap_or_else(|_| panic!("target file {path:?} does not exist"))
        }
        Some(target) => rustc_target::spec::TargetTriple::TargetTriple(target.to_string()),
        _ => rustc_target::spec::TargetTriple::from_triple(host_triple()),
    }
}

fn convert_edition(edition: cargo::core::Edition) -> rustc_span::edition::Edition {
    match edition {
        cargo::core::Edition::Edition2015 => rustc_span::edition::Edition::Edition2015,
        cargo::core::Edition::Edition2018 => rustc_span::edition::Edition::Edition2018,
        cargo::core::Edition::Edition2021 => rustc_span::edition::Edition::Edition2021,
        cargo::core::Edition::Edition2024 => rustc_span::edition::Edition::Edition2024,
    }
}

fn conversion_crate_type(t: cargo::core::compiler::CrateType) -> rustc_session::config::CrateType {
    match t {
        cargo::core::compiler::CrateType::Bin => rustc_session::config::CrateType::Executable,
        cargo::core::compiler::CrateType::Lib => rustc_session::config::CrateType::Rlib,
        cargo::core::compiler::CrateType::Rlib => rustc_session::config::CrateType::Rlib,
        cargo::core::compiler::CrateType::Dylib => rustc_session::config::CrateType::Dylib,
        cargo::core::compiler::CrateType::Cdylib => rustc_session::config::CrateType::Cdylib,
        cargo::core::compiler::CrateType::Staticlib => rustc_session::config::CrateType::Staticlib,
        cargo::core::compiler::CrateType::ProcMacro => rustc_session::config::CrateType::ProcMacro,
        cargo::core::compiler::CrateType::Other(_) => todo!(),
    }
}

fn get_checksum_doctest(test: (String, GlobalTestOptions, LangString)) -> (u64, u64) {
    let mut hasher = StableHasher::new();

    let (code, global_options, lang_string) = test;

    hasher.write(b"code");
    hasher.write(code.as_bytes());

    hasher.write(b"global_options");
    if global_options.no_crate_inject {
        hasher.write(b"no_crate_inject");
    }
    for attr in global_options.attrs {
        hasher.write(attr.as_bytes());
    }

    hasher.write(b"lang_string");
    if lang_string.should_panic {
        hasher.write(b"should_panic");
    }
    if lang_string.no_run {
        hasher.write(b"no_run");
    }
    match lang_string.ignore {
        rustdoc::Ignore::All => hasher.write(b"All"),
        rustdoc::Ignore::None => hasher.write(b"None"),
        rustdoc::Ignore::Some(ignores) => {
            hasher.write(b"Some");
            for ignore in ignores {
                hasher.write(ignore.as_bytes());
            }
        }
    }
    if lang_string.rust {
        hasher.write(b"rust");
    }
    if lang_string.test_harness {
        hasher.write(b"test_harness");
    }
    if lang_string.compile_fail {
        hasher.write(b"compile_fail");
    }
    hasher.write(b"codes");
    for code in lang_string.error_codes {
        hasher.write(code.as_bytes());
    }
    if let Some(edition) = lang_string.edition {
        hasher.write(edition.to_string().as_bytes());
    }
    hasher.write(b"added_classes");
    for class in lang_string.added_classes {
        hasher.write(class.as_bytes());
    }
    hasher.write(b"unknown");
    for u in lang_string.unknown {
        hasher.write(u.as_bytes());
    }

    hasher.finalize()
}
