use super::graph::{DependencyGraph, EdgeType};
use crate::names::def_id_name;

use itertools::Itertools;

use rustc_hir::def::DefKind;
use rustc_middle::mir::visit::{TyContext, Visitor};
use rustc_middle::mir::Body;
use rustc_middle::ty::{GenericArg, GenericArgKind, ImplSubject, List, Ty, TyCtxt, TyKind};
use rustc_span::def_id::DefId;

/// MIR Visitor responsible for creating the dependency graph and comparing checksums
pub(crate) struct GraphVisitor<'tcx, 'g> {
    tcx: TyCtxt<'tcx>,
    graph: &'g mut DependencyGraph<String>,
    processed_instance: Option<(DefId, &'tcx List<GenericArg<'tcx>>)>,
}

impl<'tcx, 'g> GraphVisitor<'tcx, 'g> {
    pub(crate) fn new(
        tcx: TyCtxt<'tcx>,
        graph: &'g mut DependencyGraph<String>,
    ) -> GraphVisitor<'tcx, 'g> {
        GraphVisitor {
            tcx,
            graph,
            processed_instance: None,
        }
    }

    pub fn visit(&mut self, body: &'tcx Body<'tcx>, substs: &'tcx List<GenericArg<'tcx>>) {
        let def_id = body.source.def_id();
        self.processed_instance = Some((
            def_id,
            if cfg!(feature = "monomorphize_all") {
                substs
            } else {
                List::empty()
            },
        ));

        //##########################################################################################################
        // Visit body and contained promoted mir

        self.visit_body(body);

        for body in self.tcx.promoted_mir(def_id) {
            self.visit_body(body)
        }

        self.processed_instance = None;
    }

    fn get_outer(&self) -> (DefId, &'tcx List<GenericArg<'tcx>>) {
        self.processed_instance
            .expect("Cannot find currently analyzed body")
    }
}

impl<'tcx, 'g> Visitor<'tcx> for GraphVisitor<'tcx, 'g> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let (outer, outer_substs) = self.get_outer();

        if cfg!(feature = "monomorphize_all") {
            let name_after_monomorphization = def_id_name(self.tcx, outer, outer_substs);
            let name_not_monomorphized = def_id_name(self.tcx, outer, &[]);

            self.graph.add_edge(
                name_after_monomorphization,
                name_not_monomorphized,
                EdgeType::Monomorphization,
            );
        }

        if let Some(impl_def) = self.tcx.impl_of_method(outer) {
            let tys = match self.tcx.impl_subject(impl_def) {
                ImplSubject::Trait(trait_ref) => {
                    let implementors = self.tcx.impl_item_implementor_ids(impl_def);

                    if !cfg!(monomorphize_all) {
                        // 8. fn in `trait` definition -> fn in trait impl (`impl <trait> for ..`)
                        for (trait_fn, impl_fn) in implementors {
                            if *impl_fn == outer {
                                self.graph.add_edge(
                                    def_id_name(self.tcx, *trait_fn, &[]), // No substs here
                                    def_id_name(self.tcx, outer, outer_substs),
                                    EdgeType::TraitImpl,
                                );
                            }
                        }
                    }

                    let mut acc = Vec::new();
                    for subst in trait_ref.substs {
                        if let GenericArgKind::Type(ty) = subst.unpack() {
                            acc.push(ty);
                        }
                    }
                    acc
                }
                ImplSubject::Inherent(ty) => {
                    vec![ty]
                }
            };

            for ty in tys {
                match ty.kind() {
                    // 6. abstract data type -> fn in (trait) impl (`impl <trait>? for ..`)
                    TyKind::Adt(adt_def, substs) => {
                        self.graph.add_edge(
                            def_id_name(self.tcx, adt_def.did(), substs),
                            def_id_name(self.tcx, outer, outer_substs),
                            EdgeType::AdtImpl,
                        );
                    }
                    // 7. trait -> fn in trait definition (`trait { ..`)
                    TyKind::Dynamic(predicates, _, _) => {
                        for binder in predicates.iter() {
                            let pred = binder.skip_binder();
                            let (def_id, substs) = match pred {
                                rustc_middle::ty::ExistentialPredicate::Trait(trait_ref) => {
                                    (trait_ref.def_id, trait_ref.substs)
                                }
                                rustc_middle::ty::ExistentialPredicate::Projection(trait_ref) => {
                                    (trait_ref.def_id, trait_ref.substs)
                                }
                                rustc_middle::ty::ExistentialPredicate::AutoTrait(def_id) => {
                                    (def_id, List::empty())
                                }
                            };

                            self.graph.add_edge(
                                def_id_name(self.tcx, def_id, substs),
                                def_id_name(self.tcx, outer, outer_substs),
                                EdgeType::TraitImpl,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        self.super_body(body);
    }

    fn visit_ty(&mut self, mut ty: Ty<'tcx>, ty_context: TyContext) {
        self.super_ty(ty);
        let (outer, outer_substs) = self.get_outer();

        if cfg!(feature = "monomorphize_all") {
            let param_env = self
                .tcx
                .param_env(outer)
                .with_reveal_all_normalized(self.tcx);
            ty = self
                .tcx
                .subst_and_normalize_erasing_regions(outer_substs, param_env, ty);
        }

        // Apparently, all this is not done in self.super_ty(ty)
        match ty.kind() {
            TyKind::Ref(_, ty, _) => self.visit_ty(*ty, clone_ty_context(&ty_context)),
            TyKind::Array(ty, _) => self.visit_ty(*ty, clone_ty_context(&ty_context)),
            TyKind::Slice(ty) => self.visit_ty(*ty, clone_ty_context(&ty_context)),
            TyKind::Tuple(tys) => {
                for ty in tys.iter() {
                    self.visit_ty(ty, clone_ty_context(&ty_context));
                }
            }
            _ => {}
        }

        for (def_id, substs, edge_type) in match ty.kind() {
            // 1. outer node  -> contained Closure
            TyKind::Closure(def_id, substs) => vec![(*def_id, *substs, EdgeType::Closure)],
            // 2. outer node  -> contained Generator
            TyKind::Generator(def_id, substs, _) => vec![(*def_id, *substs, EdgeType::Generator)],
            // 3. caller node  -> callee `fn`
            TyKind::FnDef(def_id, substs) => {
                if let DefKind::AssocFn = self.tcx.def_kind(def_id) {
                    vec![]
                } else {
                    vec![(*def_id, *substs, EdgeType::FnDef)]
                }
            }
            // 4. outer node -> referenced abstract data type (`struct` or `enum`)
            TyKind::Adt(adt_def, substs) => vec![(adt_def.did(), *substs, EdgeType::Adt)],
            // 5. outer node -> referenced trait
            TyKind::Dynamic(predicates, _, _) => {
                let mut acc = Vec::new();

                for binder in predicates.iter() {
                    let pred = binder.skip_binder();

                    let result: (DefId, &List<GenericArg>, EdgeType) = match pred {
                        rustc_middle::ty::ExistentialPredicate::Trait(trait_ref) => {
                            (trait_ref.def_id, trait_ref.substs, EdgeType::Trait)
                        }
                        rustc_middle::ty::ExistentialPredicate::Projection(trait_ref) => {
                            (trait_ref.def_id, trait_ref.substs, EdgeType::Trait)
                        }
                        rustc_middle::ty::ExistentialPredicate::AutoTrait(def_id) => {
                            (def_id, List::empty(), EdgeType::Trait)
                        }
                    };
                    acc.push(result);
                }
                acc
            }
            TyKind::Alias(_, ty) => vec![(ty.def_id, ty.substs, EdgeType::Adt)],
            _ => vec![],
        } {
            // We also want to visit the tys of substs here, to capture all traits and adts referenced
            for subst in substs {
                if let GenericArgKind::Type(ty) = subst.unpack() {
                    self.visit_ty(ty, clone_ty_context(&ty_context));
                }
            }

            if cfg!(feature = "monomorphize_all")
                || edge_type == EdgeType::Adt
                || edge_type == EdgeType::Trait
            {
                let mut all_substs = vec![substs];

                if !def_id.is_local() {
                    if let Some(upstream_mono) = self.tcx.upstream_monomorphizations_for(def_id) {
                        all_substs = upstream_mono.keys().map(|s| *s).collect_vec();
                    }
                }

                for substs in all_substs {
                    self.graph.add_edge(
                        def_id_name(self.tcx, outer, outer_substs),
                        def_id_name(self.tcx, def_id, substs),
                        edge_type,
                    );
                }
            } else {
                self.graph.add_edge(
                    def_id_name(self.tcx, outer, outer_substs),
                    def_id_name(self.tcx, def_id, &[]),
                    edge_type,
                );
            }
        }
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use log::info;
    use rustc_middle::mir::mono::MonoItem;
    use std::{fs, io::Error, path::PathBuf, string::String};
    use test_log::test;

    use rustc_errors::registry;
    use rustc_hash::{FxHashMap, FxHashSet};
    use rustc_session::config::{self, CheckCfg, OptLevel};
    use rustc_span::source_map;

    use crate::static_rts::graph::{DependencyGraph, EdgeType};

    use super::GraphVisitor;

    const TEST_DATA_PATH: &str = "test-data/static/src";

    fn load_test_code(file_name: &str) -> Result<String, Error> {
        let mut path_buf = PathBuf::from(TEST_DATA_PATH);
        path_buf.push(file_name);
        fs::read_to_string(path_buf.as_path())
    }

    fn compile_and_visit(file_name: &str) -> DependencyGraph<String> {
        let test_code = load_test_code(file_name).expect("Failed to load test code.");

        let config = rustc_interface::Config {
            opts: config::Options {
                test: true,
                optimize: OptLevel::No,
                ..config::Options::default()
            },
            crate_cfg: FxHashSet::default(),
            crate_check_cfg: CheckCfg::default(),
            input: config::Input::Str {
                name: source_map::FileName::Custom("main.rs".into()),
                input: test_code,
            },
            output_dir: None,
            output_file: None,
            file_loader: None,
            lint_caps: FxHashMap::default(),
            parse_sess_created: None,
            register_lints: None,
            override_queries: None,
            registry: registry::Registry::new(&rustc_error_codes::DIAGNOSTICS),
            make_codegen_backend: None,
        };

        let mut graph = DependencyGraph::new();

        rustc_interface::run_compiler(config, |compiler| {
            compiler.enter(|queries| {
                queries.global_ctxt().unwrap().enter(|tcx| {
                    let code_gen_units = tcx.collect_and_partition_mono_items(()).1;
                    let bodies = code_gen_units
                        .iter()
                        .flat_map(|c| c.items().keys())
                        .filter(|m| if let MonoItem::Fn(_) = m { true } else { false })
                        .map(|m| {
                            let MonoItem::Fn(instance) = m else {unreachable!()};
                            instance
                        })
                        .filter(|i: &&rustc_middle::ty::Instance| tcx.is_mir_available(i.def_id()))
                        .map(|i| (tcx.optimized_mir(i.def_id()), i.substs))
                        .collect_vec();

                    let mut visitor = GraphVisitor::new(tcx, &mut graph);

                    for (body, substs) in bodies {
                        visitor.visit(body, substs);
                    }
                })
            });
        });

        info!("{}", graph.to_string());
        graph
    }

    fn assert_contains_edge(
        graph: &DependencyGraph<String>,
        start: &str,
        end: &str,
        edge_type: &EdgeType,
    ) {
        let error_str = format!("Did not find edge {} -> {} ({:?})", start, end, edge_type);

        let start = graph
            .get_nodes()
            .iter()
            .find(|s| s.ends_with(start))
            .unwrap();

        let end = graph.get_nodes().iter().find(|s| s.ends_with(end)).unwrap();

        let maybe_edges = graph.get_edges_to(end);
        assert!(maybe_edges.is_some(), "{}", error_str);

        let edges = maybe_edges.unwrap();
        assert!(edges.contains_key(start), "{}", error_str);

        let edge_types = edges.get(start).unwrap();
        assert!(edge_types.contains(edge_type), "{}", error_str);
    }

    fn assert_does_not_contain_edge(
        graph: &DependencyGraph<String>,
        start: &str,
        end: &str,
        edge_type: &EdgeType,
    ) {
        let start = graph
            .get_nodes()
            .iter()
            .find(|s| s.ends_with(start))
            .unwrap();

        let end = graph.get_nodes().iter().find(|s| s.ends_with(end)).unwrap();

        let maybe_edges = graph.get_edges_to(end);
        if maybe_edges.is_some() {
            let edges = maybe_edges.unwrap();
            if edges.contains_key(start) {
                let edge_types = edges.get(start).unwrap();
                assert!(
                    !edge_types.contains(edge_type),
                    "Found unexpected edge {} -> {} ({:?})",
                    start,
                    end,
                    edge_type
                );
            }
        }
    }

    #[test]
    fn test_function_call() {
        let graph = compile_and_visit("call.rs");

        let start = "::test::test";
        let end = "::func";
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_closure() {
        let graph = compile_and_visit("closure.rs");

        let start = "::test::test";
        let end = "::test::test::{closure#0}";
        let edge_type = EdgeType::Closure;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_fndef() {
        let graph = compile_and_visit("fndef.rs");

        let start = "::test::test_indirect";
        let end = "::incr";
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = "::test::test_higher_order";
        let end = "::incr";
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    #[cfg(not(feature = "monomorphize_all"))]
    fn test_impls() {
        let graph = compile_and_visit("impls.rs");

        let edge_type = EdgeType::FnDef;

        let start = "::test::test_static";
        let end = "::Foo::new";
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = "::test::test_const";
        let end = "::Foo::get";
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = "::test::test_mut";
        let end = "::Foo::set";
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    #[cfg(not(feature = "monomorphize_all"))]
    fn test_traits() {
        let graph = compile_and_visit("traits.rs");

        println!("{}", graph.to_string());

        {
            let edge_type = EdgeType::FnDef;

            let start = "::test::test_direct";
            let end = "Animal::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::sound_generic";
            let end = "Animal::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::sound_dyn";
            let end = "Animal::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let edge_type = EdgeType::FnDef;

            let start = "::test::test_mut_direct";
            let end = "::Animal::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::set_treat_generic";
            let end = "::Animal::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::set_treat_dyn";
            let end = "Animal::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let edge_type = EdgeType::AdtImpl;

            let start = "::Lion";
            let end = "::<Lion as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start: &str = "::Dog";
            let end = "::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let edge_type = EdgeType::AdtImpl;

            let start = "::Lion";
            let end = "::<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start: &str = "::Dog";
            let end = "::<Dog as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }
    }

    #[test]
    #[cfg(feature = "monomorphize_all")]
    fn test_traits() {
        let graph = compile_and_visit("traits.rs");

        println!("{}", graph.to_string());

        {
            let edge_type = EdgeType::FnDef;

            let start = "::test::test_direct";
            let end = "<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::sound_generic::<Lion>";
            let end = "<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::sound_dyn";
            let end = "<dyn Animal as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let edge_type = EdgeType::FnDef;

            let start = "::test::test_mut_direct";
            let end = "::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::set_treat_generic::<Dog>";
            let end = "::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::set_treat_dyn";
            let end = "<dyn Animal as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let edge_type = EdgeType::AdtImpl;

            let start = "::Lion";
            let end = "::<Lion as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start: &str = "::Dog";
            let end = "::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let edge_type = EdgeType::AdtImpl;

            let start = "::Lion";
            let end = "::<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start: &str = "::Dog";
            let end = "::<Dog as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }
    }
}

fn clone_ty_context(context: &TyContext) -> TyContext {
    match context {
        TyContext::LocalDecl { local, source_info } => TyContext::LocalDecl {
            local: local.clone(),
            source_info: source_info.clone(),
        },
        TyContext::UserTy(span) => TyContext::UserTy(span.clone()),
        TyContext::ReturnTy(source_info) => TyContext::ReturnTy(source_info.clone()),
        TyContext::YieldTy(source_info) => TyContext::YieldTy(source_info.clone()),
        TyContext::Location(location) => TyContext::Location(location.clone()),
    }
}
