use super::graph::{DependencyGraph, EdgeType};
use crate::callbacks_shared::TEST_MARKER;
use crate::constants::SUFFIX_DYN;
use crate::names::def_id_name;
use log::warn;
use rustc_hir::def::DefKind;
use rustc_hir::AttributeMap;
use rustc_middle::mir::visit::{TyContext, Visitor};
use rustc_middle::mir::Body;
use rustc_middle::ty::{GenericArg, Instance, InstanceDef, List, Ty, TyCtxt, TyKind};
use rustc_span::def_id::DefId;

/// MIR Visitor responsible for creating the dependency graph and comparing checksums
pub(crate) struct GraphVisitor<'tcx, 'g> {
    tcx: TyCtxt<'tcx>,
    graph: &'g mut DependencyGraph<String>,
    processed_instance: Option<(DefId, &'tcx List<GenericArg<'tcx>>)>,

    #[cfg(not(feature = "monomorphize"))]
    original_substs: Option<&'tcx List<GenericArg<'tcx>>>,
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

            #[cfg(not(feature = "monomorphize"))]
            original_substs: None,
        }
    }

    pub fn visit(&mut self, body: &'tcx Body<'tcx>, substs: &'tcx List<GenericArg<'tcx>>) {
        let def_id = body.source.def_id();

        #[cfg(feature = "monomorphize")]
        {
            self.processed_instance = Some((def_id, substs));
        }
        #[cfg(not(feature = "monomorphize"))]
        {
            self.processed_instance = Some((def_id, List::empty()));
            self.original_substs = Some(substs);
        }

        //##########################################################################################################
        // Visit body and contained promoted mir

        self.visit_body(body);

        for body in self.tcx.promoted_mir(def_id) {
            self.visit_body(body)
        }

        self.processed_instance = None;

        #[cfg(not(feature = "monomorphize"))]
        {
            self.original_substs = None;
        }
    }

    fn get_outer(&self) -> (DefId, &'tcx List<GenericArg<'tcx>>) {
        self.processed_instance
            .expect("Cannot find currently analyzed body")
    }

    #[cfg(not(feature = "monomorphize"))]
    fn get_orig(&self) -> &'tcx List<GenericArg<'tcx>> {
        self.original_substs
            .expect("Cannot find currently original substs")
    }
}

impl<'tcx, 'g> Visitor<'tcx> for GraphVisitor<'tcx, 'g> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let (outer, outer_substs) = self.get_outer();

        if outer.is_local() {
            let attrs = &self.tcx.hir_crate(()).owners[self
                .tcx
                .local_def_id_to_hir_id(outer.expect_local())
                .owner
                .def_id]
                .as_owner()
                .map_or(AttributeMap::EMPTY, |o| &o.attrs)
                .map;

            for (_, list) in attrs.iter() {
                for attr in *list {
                    if attr.name_or_empty().to_ident_string() == TEST_MARKER {
                        let def_path = def_id_name(self.tcx, outer, outer_substs, false, false);
                        let trimmed_def_path =
                            def_id_name(self.tcx, outer, outer_substs, false, true);

                        self.graph.add_edge(
                            def_path[0..def_path.len() - 13].to_string(),
                            trimmed_def_path[0..trimmed_def_path.len() - 13].to_string(),
                            EdgeType::Trimmed,
                        )
                    }
                }
            }
        }

        #[cfg(feature = "monomorphize")]
        {
            let name_after_monomorphization =
                def_id_name(self.tcx, outer, outer_substs, false, true);
            let name_not_monomorphized = def_id_name(self.tcx, outer, &[], false, true);

            self.graph.add_edge(
                name_after_monomorphization,
                name_not_monomorphized,
                EdgeType::Monomorphization,
            );
        }

        if let DefKind::AssocFn = self.tcx.def_kind(outer) {
            let name = def_id_name(self.tcx, outer, &outer_substs, false, true);

            // 5. (only associated functions) function + !dyn -> function
            self.graph
                .add_edge(name.clone() + SUFFIX_DYN, name, EdgeType::DynFn)
        }

        if let Some(impl_def) = self.tcx.impl_of_method(outer) {
            if let Some(_) = self.tcx.impl_trait_ref(impl_def) {
                let implementors = self.tcx.impl_item_implementor_ids(impl_def);

                // 4. function in `trait` definition + !dyn -> function in trait impl (`impl <trait> for ..`) + !dyn
                for (trait_fn, impl_fn) in implementors {
                    if *impl_fn == outer {
                        let name_trait_fn =
                            def_id_name(self.tcx, *trait_fn, &[], false, true) + SUFFIX_DYN; // No substs here, even with monomorphize
                        let name_impl_fn =
                            def_id_name(self.tcx, outer, outer_substs, false, true) + SUFFIX_DYN;

                        self.graph
                            .add_edge(name_trait_fn, name_impl_fn, EdgeType::TraitImpl);
                    }
                }
            }
        }

        self.super_body(body);
    }

    #[allow(unused_mut)]
    fn visit_ty(&mut self, mut ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);
        let (outer, mut outer_substs) = self.get_outer();
        let outer_name = def_id_name(self.tcx, outer, outer_substs, false, true);

        #[cfg(not(feature = "monomorphize"))]
        let orig_substs = self.get_orig();

        #[cfg(feature = "monomorphize")]
        let orig_substs = outer_substs;

        // 6. function -> destructor (`drop()` function) of referenced abstract datatype
        if let Some(adt_def) = ty.ty_adt_def() {
            if let Some(destructor) = self.tcx.adt_destructor(adt_def.did()) {
                self.graph.add_edge(
                    outer_name.clone(),
                    def_id_name(self.tcx, destructor.did, &[], false, true),
                    EdgeType::Drop,
                );
            }
        }

        #[allow(unused_variables)]
        if let Some((def_id, substs, edge_type)) = match ty.kind() {
            // 1. function  -> contained Closure
            TyKind::Closure(def_id, substs) => Some((*def_id, *substs, EdgeType::Closure)),
            // 2. function  -> contained Generator
            TyKind::Generator(def_id, substs, _) => Some((*def_id, *substs, EdgeType::Generator)),

            TyKind::FnDef(_def_id, _substs) => {
                // We need to resolve ty here, to precisely resolve statically dispatched function calls
                let param_env = self
                    .tcx
                    .param_env(outer)
                    .with_reveal_all_normalized(self.tcx);
                ty = self
                    .tcx
                    .subst_and_normalize_erasing_regions(orig_substs, param_env, ty);

                let TyKind::FnDef(mut def_id, mut substs) = ty.kind() else {unreachable!()};

                let maybe_resolved = if let Ok(Some(instance)) =
                    Instance::resolve(self.tcx, param_env, def_id, &substs)
                {
                    match instance.def {
                        InstanceDef::Virtual(def_id, _) => {
                            // 3.4 caller function -> callee `fn` + !dyn (for functions in `trait {..} called by dynamic dispatch)
                            Some(Some((def_id, List::empty(), EdgeType::FnDefDyn)))
                            // No substs here, even with monomorphize
                        }
                        InstanceDef::Item(item) if !self.tcx.is_closure(instance.def_id()) => {
                            // Assign resolved function
                            def_id = item.did;
                            substs = instance.substs;
                            None
                        }
                        _ => Some(None),
                    }
                } else {
                    warn!(
                        "Failed to resolve instance, may lead to unsafe behavior {:?} - {:?}",
                        def_id, substs
                    );
                    None
                };

                maybe_resolved.unwrap_or_else(|| {
                    if let DefKind::AssocFn = self.tcx.def_kind(def_id) {
                        if let Some(_trait_def_id) = self.tcx.trait_of_item(def_id) {
                            // 3.1 caller function -> callee `fn` (for functions in `trait {..})
                            Some((def_id, substs, EdgeType::FnDefTrait))
                        } else {
                            // 3.2 caller function -> callee `fn` (for assoc `fn`s in `impl .. {..})
                            Some((def_id, substs, EdgeType::FnDefImpl))
                        }
                    } else {
                        // 3.3 caller function  -> callee `fn` (for non-assoc `fn`s, i.e. not inside `impl .. {..}`)
                        Some((def_id, substs, EdgeType::FnDef))
                    }
                })
            }
            _ => None,
        } {
            #[cfg(feature = "monomorphize")]
            {
                let mut name = def_id_name(self.tcx, def_id, substs, false, true);
                if edge_type == EdgeType::FnDefDyn {
                    name += SUFFIX_DYN;
                }

                self.graph.add_edge(outer_name.clone(), name, edge_type);
            }

            #[cfg(not(feature = "monomorphize"))]
            {
                let mut name = def_id_name(self.tcx, def_id, &[], false, true);
                if edge_type == EdgeType::FnDefDyn {
                    name += SUFFIX_DYN;
                }

                self.graph.add_edge(outer_name.clone(), name, edge_type);
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

    use crate::constants::SUFFIX_DYN;
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

        let start = graph.get_nodes().iter().find(|s| **s == start).unwrap();

        let end = graph.get_nodes().iter().find(|s| **s == end).unwrap();

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

        let start = "rust_out::test";
        let end = "rust_out::func";
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_closure() {
        let graph = compile_and_visit("closure.rs");

        let start = "rust_out::test";
        let end = "rust_out::test::{closure#0}";
        let edge_type = EdgeType::Closure;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_fndef() {
        let graph = compile_and_visit("fndef.rs");

        let start = "rust_out::test_indirect";
        let end = "rust_out::incr";
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = "rust_out::test_higher_order";
        let end = "rust_out::incr";
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    #[cfg(not(feature = "monomorphize"))]
    fn test_impls() {
        let graph = compile_and_visit("impls.rs");

        let edge_type = EdgeType::FnDefImpl;
        let end: &str = "rust_out::Foo::new";

        let start = "rust_out::test_static";
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = "rust_out::test_const";
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = "rust_out::test_mut";
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    #[cfg(not(feature = "monomorphize"))]
    fn test_traits() {
        let graph = compile_and_visit("traits.rs");

        println!("{}", graph.to_string());

        {
            let start = "rust_out::test_direct";
            let end = "rust_out::<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::sound_generic";
            let end = "rust_out::<Dog as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::sound_dyn";
            let end = "rust_out::Animal::sound".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefDyn);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefDyn);

            let start = "rust_out::Animal::sound".to_owned() + SUFFIX_DYN;
            let end = "rust_out::<Lion as Animal>::sound".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::TraitImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::TraitImpl);

            let start = "rust_out::Animal::walk".to_owned() + SUFFIX_DYN;
            let end = "rust_out::Animal::walk";
            assert_contains_edge(&graph, &start, &end, &EdgeType::DynFn);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::DynFn);
        }

        {
            let start = "rust_out::test_mut_direct";
            let end = "rust_out::<Lion as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::set_treat_generic";
            let end = "rust_out::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::set_treat_dyn";
            let end = "rust_out::Animal::set_treat".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefDyn);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefDyn);

            let start = "rust_out::Animal::set_treat".to_owned() + SUFFIX_DYN;
            let end = "rust_out::<Dog as Animal>::set_treat".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::TraitImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::TraitImpl);
        }
    }

    #[test]
    #[cfg(feature = "monomorphize")]
    fn test_traits() {
        let graph = compile_and_visit("traits.rs");

        println!("{}", graph.to_string());

        {
            let start = "rust_out::test_direct";
            let end = "rust_out::<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::sound_generic::<Lion>";
            let end = "rust_out::<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::sound_dyn";
            let end = "rust_out::Animal::sound".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefDyn);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefDyn);

            let start = "rust_out::Animal::sound".to_owned() + SUFFIX_DYN;
            let end = "rust_out::<Lion as Animal>::sound".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::TraitImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::TraitImpl);

            let start = "rust_out::<Lion as Animal>::walk".to_owned() + SUFFIX_DYN;
            let end = "rust_out::<Lion as Animal>::walk";
            assert_contains_edge(&graph, &start, &end, &EdgeType::DynFn);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::DynFn);
        }

        {
            let start = "rust_out::test_mut_direct";
            let end = "rust_out::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::set_treat_generic::<Dog>";
            let end = "rust_out::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefImpl);

            let start = "rust_out::set_treat_dyn";
            let end = "rust_out::Animal::set_treat".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefDyn);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefDyn);

            let start = "rust_out::Animal::set_treat".to_owned() + SUFFIX_DYN;
            let end = "rust_out::<Dog as Animal>::set_treat".to_owned() + SUFFIX_DYN;
            assert_contains_edge(&graph, &start, &end, &EdgeType::TraitImpl);
            assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::TraitImpl);
        }
    }
}
