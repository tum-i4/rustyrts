use log::info;
use rustc_interface::interface;

#[allow(dead_code)]
pub(crate) fn print_compiler_config(config: &mut interface::Config) {
    info!("config.opts:");
    info!("    crate_types: {:?}", config.opts.crate_types);
    info!("    optimize: {:?}", config.opts.optimize);
    info!("    debug_assertions: {:?}", config.opts.debug_assertions);
    info!("    debug_info: {:?}", config.opts.debuginfo);
    info!("    lint_opts: {:?}", config.opts.lint_opts);
    info!("    lint_cap: {:?}", config.opts.lint_cap);
    info!("    describe_lints: {:?}", config.opts.describe_lints);
    info!("    output_types: {:?}", config.opts.output_types);
    info!("    search_paths: {:?}", config.opts.search_paths);
    info!("    libs: {:?}", config.opts.libs);
    info!("    maybe_sysroot: {:?}", config.opts.maybe_sysroot);
    info!("    target_triple: {:?}", config.opts.target_triple);
    info!("    test: {:?}", config.opts.test);
    info!("    error_format: {:?}", config.opts.error_format);
    info!("    diagnostic_width: {:?}", config.opts.diagnostic_width);
    info!("    incremental: {:?}", config.opts.incremental);
    info!("    assert_incr_state: {:?}", config.opts.assert_incr_state);
    info!("    prints: {:?}", config.opts.prints);
    info!("    crate_name: {:?}", config.opts.crate_name);
    info!("    unstable_features: {:?}", config.opts.unstable_features);
    info!("    actually_rustdoc: {:?}", config.opts.actually_rustdoc);
    info!("    trimmed_def_path: {:?}", config.opts.trimmed_def_paths);
    info!(
        "    cli_forced_codegen_units: {:?}",
        config.opts.cli_forced_codegen_units
    );
    info!(
        "    cli_forced_local_thinlto_off: {:?}",
        config.opts.cli_forced_local_thinlto_off
    );
    info!("    path_prefix: {:?}", config.opts.remap_path_prefix);
    info!(
        "    real_rust_source_base_dir: {:?}",
        config.opts.real_rust_source_base_dir
    );
    info!("    edition: {:?}", config.opts.edition);
    info!(
        "    json_artifact_notification: {:?}",
        config.opts.json_artifact_notifications
    );
    info!("    pretty: {:?}", config.opts.pretty);
    info!("    working_dir: {:?}", config.opts.working_dir);

    //     info!("    {:?}", config.opts.unstable_opts);
    //     info!("    {:?}", config.opts.cg);
    //     info!("    {:?}", config.opts.externs);

    info!("config.opts.cg");
    info!("    ar: {:?}", config.opts.cg.ar);
    info!("    code_model: {:?}", config.opts.cg.code_model);
    info!(
        "    control_flow_guard: {:?}",
        config.opts.cg.control_flow_guard
    );
    info!(
        "    debug_assertions: {:?}",
        config.opts.cg.debug_assertions
    );
    info!("    debuginfo: {:?}", config.opts.cg.debuginfo);
    info!(
        "    default_linker_libraries: {:?}",
        config.opts.cg.default_linker_libraries
    );
    info!("    embed_bitcode: {:?}", config.opts.cg.embed_bitcode);
    info!("    extra_filename: {:?}", config.opts.cg.extra_filename);
    info!(
        "    force_frame_pointers: {:?}",
        config.opts.cg.force_frame_pointers
    );
    info!(
        "    force_unwind_tables: {:?}",
        config.opts.cg.force_unwind_tables
    );
    info!("    incremental: {:?}", config.opts.cg.incremental);
    info!(
        "    inline_threshold: {:?}",
        config.opts.cg.inline_threshold
    );
    info!(
        "    instrument_coverage: {:?}",
        config.opts.cg.instrument_coverage
    );
    info!("    link_arg: {:?}", config.opts.cg.link_arg);
    info!("    link_args: {:?}", config.opts.cg.link_args);
    info!("    link_dead_code: {:?}", config.opts.cg.link_dead_code);
    info!(
        "    link_self_contained: {:?}",
        config.opts.cg.link_self_contained
    );
    info!("    linker: {:?}", config.opts.cg.linker);
    info!("    linker_flavor: {:?}", config.opts.cg.linker_flavor);
    info!(
        "    linker_plugin_lto: {:?}",
        config.opts.cg.linker_plugin_lto
    );
    info!("    llvm_args: {:?}", config.opts.cg.llvm_args);
    info!("    lto: {:?}", config.opts.cg.lto);
    info!("    metadata: {:?}", config.opts.cg.metadata);
    info!(
        "    no_prepopulate_passes: {:?}",
        config.opts.cg.no_prepopulate_passes
    );
    info!("    no_redzone: {:?}", config.opts.cg.no_redzone);
    info!("    no_stack_check: {:?}", config.opts.cg.no_stack_check);
    info!(
        "    no_vectorize_loops: {:?}",
        config.opts.cg.no_vectorize_loops
    );
    info!(
        "    no_vectorize_slp: {:?}",
        config.opts.cg.no_vectorize_slp
    );
    info!("    opt_level: {:?}", config.opts.cg.opt_level);
    info!("    overflow_checks: {:?}", config.opts.cg.overflow_checks);
    info!("    panic: {:?}", config.opts.cg.panic);
    info!("    passes: {:?}", config.opts.cg.passes);
    info!("    prefer_dynamic: {:?}", config.opts.cg.prefer_dynamic);
    info!(
        "    profile_generate: {:?}",
        config.opts.cg.profile_generate
    );
    info!("    profile_use: {:?}", config.opts.cg.profile_use);
    info!(
        "    relocation_model: {:?}",
        config.opts.cg.relocation_model
    );
    info!("    remark: {:?}", config.opts.cg.remark);
    info!("    rpath: {:?}", config.opts.cg.rpath);
    info!("    save_temps: {:?}", config.opts.cg.save_temps);
    info!("    soft_float: {:?}", config.opts.cg.soft_float);
    info!("    split_debuginfo: {:?}", config.opts.cg.split_debuginfo);
    info!("    strip: {:?}", config.opts.cg.strip);
    info!(
        "    symbol_mangling_version: {:?}",
        config.opts.cg.symbol_mangling_version
    );
    info!("    target_cpu: {:?}", config.opts.cg.target_cpu);
    info!("    target_feature: {:?}", config.opts.cg.target_feature);

    info!("config.opts.unstable_opts");
    info!(
        "    allow_features: {:?}",
        config.opts.unstable_opts.allow_features
    );
    info!(
        "    always_encode_mir: {:?}",
        config.opts.unstable_opts.always_encode_mir
    );
    info!(
        "    asm_comments: {:?}",
        config.opts.unstable_opts.asm_comments
    );
    info!(
        "    assert_incr_state: {:?}",
        config.opts.unstable_opts.assert_incr_state
    );
    info!(
        "    assume_incomplete_release: {:?}",
        config.opts.unstable_opts.assume_incomplete_release
    );
    info!(
        "    binary_dep_depinfo: {:?}",
        config.opts.unstable_opts.binary_dep_depinfo
    );
    info!(
        "    box_noalias: {:?}",
        config.opts.unstable_opts.box_noalias
    );
    info!(
        "    branch_protection: {:?}",
        config.opts.unstable_opts.branch_protection
    );
    info!(
        "    cf_protection: {:?}",
        config.opts.unstable_opts.cf_protection
    );
    info!(
        "    cgu_partitioning_strategy: {:?}",
        config.opts.unstable_opts.cgu_partitioning_strategy
    );
    info!(
        "    codegen_backend: {:?}",
        config.opts.unstable_opts.codegen_backend
    );
    info!(
        "    combine_cgu: {:?}",
        config.opts.unstable_opts.combine_cgu
    );
    info!("    crate_attr: {:?}", config.opts.unstable_opts.crate_attr);
    info!(
        "    debug_info_for_profiling: {:?}",
        config.opts.unstable_opts.debug_info_for_profiling
    );
    info!(
        "    debug_macros: {:?}",
        config.opts.unstable_opts.debug_macros
    );
    info!(
        "    deduplicate_diagnostics: {:?}",
        config.opts.unstable_opts.deduplicate_diagnostics
    );
    info!(
        "    dep_info_omit_d_target: {:?}",
        config.opts.unstable_opts.dep_info_omit_d_target
    );
    info!("    dep_tasks: {:?}", config.opts.unstable_opts.dep_tasks);
    info!(
        "    diagnostic_width: {:?}",
        config.opts.unstable_opts.diagnostic_width
    );
    info!("    dlltool: {:?}", config.opts.unstable_opts.dlltool);
    info!(
        "    dont_buffer_diagnostics: {:?}",
        config.opts.unstable_opts.dont_buffer_diagnostics
    );
    info!(
        "    drop_tracking: {:?}",
        config.opts.unstable_opts.drop_tracking
    );
    info!(
        "    dual_proc_macros: {:?}",
        config.opts.unstable_opts.dual_proc_macros
    );
    info!(
        "    dump_dep_graph: {:?}",
        config.opts.unstable_opts.dump_dep_graph
    );
    info!(
        "    dump_drop_tracking_cfg: {:?}",
        config.opts.unstable_opts.dump_drop_tracking_cfg
    );
    info!("    dump_mir: {:?}", config.opts.unstable_opts.dump_mir);
    info!(
        "    dump_mir_dataflow: {:?}",
        config.opts.unstable_opts.dump_mir_dataflow
    );
    info!(
        "    dump_mir_dir: {:?}",
        config.opts.unstable_opts.dump_mir_dir
    );
    info!(
        "    dump_mir_exclude_pass_number: {:?}",
        config.opts.unstable_opts.dump_mir_exclude_pass_number
    );
    info!(
        "    dump_mir_graphviz: {:?}",
        config.opts.unstable_opts.dump_mir_graphviz
    );
    info!(
        "    dump_mir_spanview: {:?}",
        config.opts.unstable_opts.dump_mir_spanview
    );
    info!(
        "    dump_mono_stats: {:?}",
        config.opts.unstable_opts.dump_mono_stats
    );
    info!(
        "    dump_mono_stats_format: {:?}",
        config.opts.unstable_opts.dump_mono_stats_format
    );
    info!(
        "    dwarf_version: {:?}",
        config.opts.unstable_opts.dwarf_version
    );
    info!("    dylib_lto: {:?}", config.opts.unstable_opts.dylib_lto);
    info!(
        "    emit_stack_sizes: {:?}",
        config.opts.unstable_opts.emit_stack_sizes
    );
    info!(
        "    emit_thin_lto: {:?}",
        config.opts.unstable_opts.emit_thin_lto
    );
    info!(
        "    export_executable_symbols: {:?}",
        config.opts.unstable_opts.export_executable_symbols
    );
    info!(
        "    extra_const_ub_checks: {:?}",
        config.opts.unstable_opts.extra_const_ub_checks
    );
    info!(
        "    fewer_names: {:?}",
        config.opts.unstable_opts.fewer_names
    );
    info!(
        "    force_unstable_if_unmarked: {:?}",
        config.opts.unstable_opts.force_unstable_if_unmarked
    );
    info!("    fuel: {:?}", config.opts.unstable_opts.fuel);
    info!(
        "    function_sections: {:?}",
        config.opts.unstable_opts.function_sections
    );
    info!(
        "    future_incompat_test: {:?}",
        config.opts.unstable_opts.future_incompat_test
    );
    info!(
        "    graphviz_dark_mode: {:?}",
        config.opts.unstable_opts.graphviz_dark_mode
    );
    info!(
        "    graphviz_font: {:?}",
        config.opts.unstable_opts.graphviz_font
    );
    info!("    hir_stats: {:?}", config.opts.unstable_opts.hir_stats);
    info!(
        "    human_readable_cgu_names: {:?}",
        config.opts.unstable_opts.human_readable_cgu_names
    );
    info!(
        "    identify_regions: {:?}",
        config.opts.unstable_opts.identify_regions
    );
    info!(
        "    incremental_ignore_spans: {:?}",
        config.opts.unstable_opts.incremental_ignore_spans
    );
    info!(
        "    incremental_info: {:?}",
        config.opts.unstable_opts.incremental_info
    );
    info!(
        "    incremental_relative_spans: {:?}",
        config.opts.unstable_opts.incremental_relative_spans
    );
    info!(
        "    incremental_verify_ich: {:?}",
        config.opts.unstable_opts.incremental_verify_ich
    );
    info!(
        "    inline_in_all_cgus: {:?}",
        config.opts.unstable_opts.inline_in_all_cgus
    );
    info!(
        "    inline_llvm: {:?}",
        config.opts.unstable_opts.inline_llvm
    );
    info!("    inline_mir: {:?}", config.opts.unstable_opts.inline_mir);
    info!(
        "    inline_mir_hint_threshold: {:?}",
        config.opts.unstable_opts.inline_mir_hint_threshold
    );
    info!(
        "    inline_mir_threshold: {:?}",
        config.opts.unstable_opts.inline_mir_threshold
    );
    info!(
        "    input_stats: {:?}",
        config.opts.unstable_opts.input_stats
    );
    info!(
        "    instrument_coverage: {:?}",
        config.opts.unstable_opts.instrument_coverage
    );
    info!(
        "    instrument_mcount: {:?}",
        config.opts.unstable_opts.instrument_mcount
    );
    info!(
        "    keep_hygiene_data: {:?}",
        config.opts.unstable_opts.keep_hygiene_data
    );
    info!(
        "    layout_seed: {:?}",
        config.opts.unstable_opts.layout_seed
    );
    info!(
        "    link_native_libraries: {:?}",
        config.opts.unstable_opts.link_native_libraries
    );
    info!("    link_only: {:?}", config.opts.unstable_opts.link_only);
    info!(
        "    llvm_plugins: {:?}",
        config.opts.unstable_opts.llvm_plugins
    );
    info!(
        "    llvm_time_trace: {:?}",
        config.opts.unstable_opts.llvm_time_trace
    );
    info!(
        "    location_detail: {:?}",
        config.opts.unstable_opts.location_detail
    );
    info!("    ls: {:?}", config.opts.unstable_opts.ls);
    info!(
        "    macro_backtrace: {:?}",
        config.opts.unstable_opts.macro_backtrace
    );
    info!(
        "    maximal_hir_to_mir_coverage: {:?}",
        config.opts.unstable_opts.maximal_hir_to_mir_coverage
    );
    info!(
        "    merge_functions: {:?}",
        config.opts.unstable_opts.merge_functions
    );
    info!("    meta_stats: {:?}", config.opts.unstable_opts.meta_stats);
    info!(
        "    mir_emit_retag: {:?}",
        config.opts.unstable_opts.mir_emit_retag
    );
    info!(
        "    mir_enable_passes: {:?}",
        config.opts.unstable_opts.mir_enable_passes
    );
    info!(
        "    mir_opt_level: {:?}",
        config.opts.unstable_opts.mir_opt_level
    );
    info!(
        "    mir_pretty_relative_line_numbers: {:?}",
        config.opts.unstable_opts.mir_pretty_relative_line_numbers
    );
    info!(
        "    move_size_limit: {:?}",
        config.opts.unstable_opts.move_size_limit
    );
    info!(
        "    mutable_noalias: {:?}",
        config.opts.unstable_opts.mutable_noalias
    );
    info!("    nll_facts: {:?}", config.opts.unstable_opts.nll_facts);
    info!(
        "    nll_facts_dir: {:?}",
        config.opts.unstable_opts.nll_facts_dir
    );
    info!(
        "    no_analysis: {:?}",
        config.opts.unstable_opts.no_analysis
    );
    info!("    no_codegen: {:?}", config.opts.unstable_opts.no_codegen);
    info!(
        "    no_generate_arange_section: {:?}",
        config.opts.unstable_opts.no_generate_arange_section
    );
    info!(
        "    no_jump_tables: {:?}",
        config.opts.unstable_opts.no_jump_tables
    );
    info!(
        "    no_leak_check: {:?}",
        config.opts.unstable_opts.no_leak_check
    );
    info!("    no_link: {:?}", config.opts.unstable_opts.no_link);
    info!(
        "    no_parallel_llvm: {:?}",
        config.opts.unstable_opts.no_parallel_llvm
    );
    info!(
        "    no_profiler_runtime: {:?}",
        config.opts.unstable_opts.no_profiler_runtime
    );
    info!(
        "    no_unique_section_names: {:?}",
        config.opts.unstable_opts.no_unique_section_names
    );
    info!(
        "    normalize_docs: {:?}",
        config.opts.unstable_opts.normalize_docs
    );
    info!("    oom: {:?}", config.opts.unstable_opts.oom);
    info!(
        "    osx_rpath_install_name: {:?}",
        config.opts.unstable_opts.osx_rpath_install_name
    );
    info!(
        "    packed_bundled_libs: {:?}",
        config.opts.unstable_opts.packed_bundled_libs
    );
    info!(
        "    panic_abort_tests: {:?}",
        config.opts.unstable_opts.panic_abort_tests
    );
    info!(
        "    panic_in_drop: {:?}",
        config.opts.unstable_opts.panic_in_drop
    );
    info!("    parse_only: {:?}", config.opts.unstable_opts.parse_only);
    info!("    perf_stats: {:?}", config.opts.unstable_opts.perf_stats);
    info!("    plt: {:?}", config.opts.unstable_opts.plt);
    info!("    polonius: {:?}", config.opts.unstable_opts.polonius);
    info!(
        "    polymorphize: {:?}",
        config.opts.unstable_opts.polymorphize
    );
    info!(
        "    pre_link_arg: {:?}",
        config.opts.unstable_opts.pre_link_arg
    );
    info!(
        "    pre_link_args: {:?}",
        config.opts.unstable_opts.pre_link_args
    );
    info!(
        "    precise_enum_drop_elaboration: {:?}",
        config.opts.unstable_opts.precise_enum_drop_elaboration
    );
    info!("    print_fuel: {:?}", config.opts.unstable_opts.print_fuel);
    info!(
        "    print_llvm_passes: {:?}",
        config.opts.unstable_opts.print_llvm_passes
    );
    info!(
        "    print_mono_items: {:?}",
        config.opts.unstable_opts.print_mono_items
    );
    info!(
        "    print_type_sizes: {:?}",
        config.opts.unstable_opts.print_type_sizes
    );
    info!(
        "    proc_macro_backtrace: {:?}",
        config.opts.unstable_opts.proc_macro_backtrace
    );
    info!(
        "    proc_macro_execution_strategy: {:?}",
        config.opts.unstable_opts.proc_macro_execution_strategy
    );
    info!("    profile: {:?}", config.opts.unstable_opts.profile);
    info!(
        "    profile_closures: {:?}",
        config.opts.unstable_opts.profile_closures
    );
    info!(
        "    profile_emit: {:?}",
        config.opts.unstable_opts.profile_emit
    );
    info!(
        "    profile_sample_use: {:?}",
        config.opts.unstable_opts.profile_sample_use
    );
    info!(
        "    profiler_runtime: {:?}",
        config.opts.unstable_opts.profiler_runtime
    );
    info!(
        "    query_dep_graph: {:?}",
        config.opts.unstable_opts.query_dep_graph
    );
    info!(
        "    randomize_layout: {:?}",
        config.opts.unstable_opts.randomize_layout
    );
    info!(
        "    relax_elf_relocations: {:?}",
        config.opts.unstable_opts.relax_elf_relocations
    );
    info!(
        "    relro_level: {:?}",
        config.opts.unstable_opts.relro_level
    );
    info!(
        "    remap_cwd_prefix: {:?}",
        config.opts.unstable_opts.remap_cwd_prefix
    );
    info!(
        "    report_delayed_bugs: {:?}",
        config.opts.unstable_opts.report_delayed_bugs
    );
    info!("    sanitizer: {:?}", config.opts.unstable_opts.sanitizer);
    info!(
        "    sanitizer_memory_track_origins: {:?}",
        config.opts.unstable_opts.sanitizer_memory_track_origins
    );
    info!(
        "    sanitizer_recover: {:?}",
        config.opts.unstable_opts.sanitizer_recover
    );
    info!(
        "    saturating_float_casts: {:?}",
        config.opts.unstable_opts.saturating_float_casts
    );
    info!(
        "    self_profile: {:?}",
        config.opts.unstable_opts.self_profile
    );
    info!(
        "    self_profile_counter: {:?}",
        config.opts.unstable_opts.self_profile_counter
    );
    info!(
        "    self_profile_events: {:?}",
        config.opts.unstable_opts.self_profile_events
    );
    info!(
        "    share_generics: {:?}",
        config.opts.unstable_opts.share_generics
    );
    info!("    show_span: {:?}", config.opts.unstable_opts.show_span);
    info!(
        "    simulate_remapped_rust_src_base: {:?}",
        config.opts.unstable_opts.simulate_remapped_rust_src_base
    );
    info!("    span_debug: {:?}", config.opts.unstable_opts.span_debug);
    info!(
        "    span_free_formats: {:?}",
        config.opts.unstable_opts.span_free_formats
    );
    info!(
        "    split_dwarf_inlining: {:?}",
        config.opts.unstable_opts.split_dwarf_inlining
    );
    info!(
        "    split_dwarf_kind: {:?}",
        config.opts.unstable_opts.split_dwarf_kind
    );
    info!(
        "    src_hash_algorithm: {:?}",
        config.opts.unstable_opts.src_hash_algorithm
    );
    info!(
        "    stack_protector: {:?}",
        config.opts.unstable_opts.stack_protector
    );
    info!(
        "    strict_init_checks: {:?}",
        config.opts.unstable_opts.strict_init_checks
    );
    info!("    strip: {:?}", config.opts.unstable_opts.strip);
    info!(
        "    symbol_mangling_version: {:?}",
        config.opts.unstable_opts.symbol_mangling_version
    );
    info!("    teach: {:?}", config.opts.unstable_opts.teach);
    info!("    temps_dir: {:?}", config.opts.unstable_opts.temps_dir);
    info!("    thinlto: {:?}", config.opts.unstable_opts.thinlto);
    info!(
        "    thir_unsafeck: {:?}",
        config.opts.unstable_opts.thir_unsafeck
    );
    info!("    threads: {:?}", config.opts.unstable_opts.threads);
    info!(
        "    time_llvm_passes: {:?}",
        config.opts.unstable_opts.time_llvm_passes
    );
    info!(
        "    time_passes: {:?}",
        config.opts.unstable_opts.time_passes
    );
    info!("    tls_model: {:?}", config.opts.unstable_opts.tls_model);
    info!(
        "    trace_macros: {:?}",
        config.opts.unstable_opts.trace_macros
    );
    info!(
        "    track_diagnostics: {:?}",
        config.opts.unstable_opts.track_diagnostics
    );
    info!(
        "    trait_solver: {:?}",
        config.opts.unstable_opts.trait_solver
    );
    info!(
        "    translate_additional_ftl: {:?}",
        config.opts.unstable_opts.translate_additional_ftl
    );
    info!(
        "    translate_directionality_markers: {:?}",
        config.opts.unstable_opts.translate_directionality_markers
    );
    info!(
        "    translate_lang: {:?}",
        config.opts.unstable_opts.translate_lang
    );
    info!(
        "    translate_remapped_path_to_local_path: {:?}",
        config
            .opts
            .unstable_opts
            .translate_remapped_path_to_local_path
    );
    info!(
        "    trap_unreachable: {:?}",
        config.opts.unstable_opts.trap_unreachable
    );
    info!(
        "    treat_err_as_bug: {:?}",
        config.opts.unstable_opts.treat_err_as_bug
    );
    info!(
        "    trim_diagnostic_paths: {:?}",
        config.opts.unstable_opts.trim_diagnostic_paths
    );
    info!("    tune_cpu: {:?}", config.opts.unstable_opts.tune_cpu);
    info!("    ui_testing: {:?}", config.opts.unstable_opts.ui_testing);
    info!(
        "    uninit_const_chunk_threshold: {:?}",
        config.opts.unstable_opts.uninit_const_chunk_threshold
    );
    info!(
        "    unleash_the_miri_inside_of_you: {:?}",
        config.opts.unstable_opts.unleash_the_miri_inside_of_you
    );
    info!("    unpretty: {:?}", config.opts.unstable_opts.unpretty);
    info!(
        "    unsound_mir_opts: {:?}",
        config.opts.unstable_opts.unsound_mir_opts
    );
    info!(
        "    unstable_options: {:?}",
        config.opts.unstable_opts.unstable_options
    );
    info!(
        "    use_ctors_section: {:?}",
        config.opts.unstable_opts.use_ctors_section
    );
    info!(
        "    validate_mir: {:?}",
        config.opts.unstable_opts.validate_mir
    );
    info!("    verbose: {:?}", config.opts.unstable_opts.verbose);
    info!(
        "    verify_llvm_ir: {:?}",
        config.opts.unstable_opts.verify_llvm_ir
    );
    info!(
        "    virtual_function_elimination: {:?}",
        config.opts.unstable_opts.virtual_function_elimination
    );
    info!(
        "    wasi_exec_model: {:?}",
        config.opts.unstable_opts.wasi_exec_model
    );
}
