use super::graph::{DependencyGraph, EdgeType};
use crate::names::def_id_name;

use itertools::Itertools;
use rustc_hir::ConstContext;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::mir::visit::{TyContext, Visitor};
use rustc_middle::mir::ConstantKind;
use rustc_middle::mir::{self, Body, Location};
use rustc_middle::ty::{GenericArgKind, ImplSubject, Instance, Ty, TyCtxt, TyKind};

/// MIR Visitor responsible for creating the dependency graph and comparing checksums
pub(crate) struct GraphVisitor<'tcx, 'g> {
    tcx: TyCtxt<'tcx>,
    graph: &'g mut DependencyGraph<String>,
    monomorphize_all: bool,
    processed_instance: Option<&'tcx Instance<'tcx>>,
}

impl<'tcx, 'g> GraphVisitor<'tcx, 'g> {
    pub(crate) fn new(
        tcx: TyCtxt<'tcx>,
        graph: &'g mut DependencyGraph<String>,
        monomorphize_all: bool,
    ) -> GraphVisitor<'tcx, 'g> {
        GraphVisitor {
            tcx,
            graph,
            monomorphize_all,
            processed_instance: None,
        }
    }

    pub fn visit(&mut self, instance: &'tcx Instance<'tcx>) {
        let def_id = instance.def_id();

        if def_id.is_local() {
            let body = match self.tcx.hir().body_const_context(def_id.expect_local()) {
                Some(ConstContext::ConstFn) | None => self.tcx.optimized_mir(def_id),
                Some(ConstContext::Static(..)) | Some(ConstContext::Const) => {
                    self.tcx.mir_for_ctfe(def_id)
                }
            };

            let old_processed_instance = self.processed_instance.replace(instance);
            //##########################################################################################################
            // Visit body and contained promoted mir
            self.visit_body(body);

            for body in self.tcx.promoted_mir(def_id) {
                self.visit_body(body)
            }
            self.processed_instance = old_processed_instance;
        }
    }
}

impl<'tcx, 'g> Visitor<'tcx> for GraphVisitor<'tcx, 'g> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let Some(outer) = self.processed_instance else {panic!("Cannot find currently analyzed body")};

        if self.monomorphize_all {
            let name_after_monomorphization = def_id_name(self.tcx, outer.def_id(), outer.substs);
            let name_not_monomorphized = def_id_name(self.tcx, outer.def_id(), &[]);

            self.graph.add_edge(
                name_after_monomorphization,
                name_not_monomorphized,
                EdgeType::Monomorphization,
            );
        }

        if let Some(impl_def) = self.tcx.impl_of_method(outer.def_id()) {
            // 7. abstract data type -> fn in trait impl (`impl <trait> for ..`)
            if let ImplSubject::Trait(trait_ref) = self.tcx.impl_subject(impl_def) {
                for subst in trait_ref.substs {
                    if let GenericArgKind::Type(ty) = subst.unpack() {
                        let param_env = self
                            .tcx
                            .param_env(outer.def_id())
                            .with_reveal_all_normalized(self.tcx);
                        let ty = self.tcx.subst_and_normalize_erasing_regions(
                            outer.substs,
                            param_env,
                            ty,
                        );

                        if let TyKind::Adt(adt_def, substs) = ty.kind() {
                            self.graph.add_edge(
                                def_id_name(self.tcx, adt_def.did(), substs),
                                def_id_name(self.tcx, outer.def_id(), outer.substs),
                                EdgeType::Impl,
                            );
                        }
                    }
                }
            }
        }

        self.super_body(body);
    }

    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);
        let Some(outer) = self.processed_instance else {panic!("Cannot find currently analyzed body")};
        let outer_substs = if self.monomorphize_all {
            outer.substs.as_slice()
        } else {
            &[]
        };

        match constant.literal {
            ConstantKind::Unevaluated(content, _ty) => {
                // 5. borrowing node -> `const var`
                // This takes care of borrows of e.g. "const var: u64"
                let def_id = content.def.did;

                self.graph.add_edge(
                    def_id_name(self.tcx, outer.def_id(), outer_substs),
                    def_id_name(self.tcx, def_id, &[]),
                    EdgeType::Unevaluated,
                );
            }
            ConstantKind::Val(cons, _ty) => {
                match cons {
                    ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                        match self.tcx.global_alloc(ptr.provenance) {
                            GlobalAlloc::Static(def_id) => {
                                // 6. borrowing node -> `static var` or `static mut var`
                                // This takes care of borrows of e.g. "static var: u64"
                                let (accessor, accessed) = (
                                    def_id_name(self.tcx, outer.def_id(), outer_substs),
                                    def_id_name(self.tcx, def_id, &[]),
                                );

                                // Since we assume that a borrow is actually read, we always add an edge here
                                self.graph
                                    .add_edge(accessor.clone(), accessed, EdgeType::Scalar);
                            }
                            GlobalAlloc::Function(instance) => {
                                // TODO: I have not yet found out when this is useful, but since there is a defId stored in here, it might be important
                                // Perhaps this refers to extern fns?
                                let instance_substs = if self.monomorphize_all {
                                    instance.substs.as_slice()
                                } else {
                                    &[]
                                };
                                let (accessor, accessed) = (
                                    def_id_name(self.tcx, outer.def_id(), outer_substs),
                                    def_id_name(self.tcx, instance.def_id(), instance_substs),
                                );
                                self.graph
                                    .add_edge(accessor.clone(), accessed, EdgeType::FnPtr);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ => (),
        }
    }

    fn visit_ty(&mut self, mut ty: Ty<'tcx>, _: TyContext) {
        self.super_ty(ty);
        let Some(outer) = self.processed_instance else {panic!("Cannot find currently analyzed body")};

        let param_env = self
            .tcx
            .param_env(outer.def_id())
            .with_reveal_all_normalized(self.tcx);
        ty = self
            .tcx
            .subst_and_normalize_erasing_regions(outer.substs, param_env, ty);

        if let Some((def_id, substs, edge_type)) = match ty.kind() {
            // 1. outer node  -> contained Closure
            TyKind::Closure(def_id, substs) => Some((*def_id, substs, EdgeType::Closure)),
            // 2. outer node  -> contained Generator
            TyKind::Generator(def_id, substs, _) => Some((*def_id, substs, EdgeType::Generator)),
            // 3. caller node  -> callee `fn`
            TyKind::FnDef(def_id, substs) => Some((*def_id, substs, EdgeType::FnDef)),
            // 4. outer node -> referenced abstract data type (`struct` or `enum`)
            TyKind::Adt(adt_def, substs) => Some((adt_def.did(), substs, EdgeType::Adt)),
            _ => None,
        } {
            if self.monomorphize_all {
                let mut all_substs = vec![substs];

                if !def_id.is_local() {
                    if let Some(upstream_mono) = self.tcx.upstream_monomorphizations_for(def_id) {
                        all_substs = upstream_mono.keys().collect_vec();
                    }
                }

                for substs in all_substs {
                    self.graph.add_edge(
                        def_id_name(self.tcx, outer.def_id(), outer.substs),
                        def_id_name(self.tcx, def_id, substs),
                        edge_type,
                    );
                }
            } else if edge_type == EdgeType::Adt {
                let mut all_substs = vec![substs];

                if !def_id.is_local() {
                    if let Some(upstream_mono) = self.tcx.upstream_monomorphizations_for(def_id) {
                        all_substs = upstream_mono.keys().collect_vec();
                    }
                }

                for substs in all_substs {
                    self.graph.add_edge(
                        def_id_name(self.tcx, outer.def_id(), &[]),
                        def_id_name(self.tcx, def_id, substs),
                        edge_type,
                    );
                }
            } else {
                self.graph.add_edge(
                    def_id_name(self.tcx, outer.def_id(), &[]),
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
                    let instances = code_gen_units
                        .iter()
                        .flat_map(|c| c.items().keys())
                        .filter(|m| if let MonoItem::Fn(_) = m { true } else { false })
                        .map(|m| {
                            let MonoItem::Fn(instance) = m else {unreachable!()};
                            instance
                        })
                        .collect_vec();

                    let mut visitor = GraphVisitor::new(tcx, &mut graph, true);

                    for instance in instances {
                        visitor.visit(instance);
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
    fn test_scalar() {
        let graph = compile_and_visit("scalar.rs");
        let edge_type = EdgeType::Scalar;

        {
            // const ptr, no edge in reverse direction
            let end = "::FOO";

            let start = "::test::test_direct_read";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::test::test_indirect_ptr_read";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::test::test_indirect_ref_read";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            // mut ptr, additional edge in reverse direction
            let start = "::BAR";

            let end = "::test::test_direct_write";
            //assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);

            let end = "::test::test_indirect_ptr_write";
            //assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);

            let end = "::test::test_indirect_ref_write";
            //assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);

            let end = "::test::test_indirect_ref_write";
            //assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);
        }
    }

    #[test]
    fn test_unevaluated() {
        let graph = compile_and_visit("unevaluated.rs");

        let start = "::test::test_const_read";
        let end = "::BAZ";
        let edge_type = EdgeType::Unevaluated;
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

        //{
        //    let start = "::Animal::set_treat";
        //    let edge_type = EdgeType::Impl;
        //
        //    let end = "::<Lion as Animal>::set_treat";
        //    assert_contains_edge(&graph, &start, &end, &edge_type);
        //    assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        //
        //    let end = "::<Dog as Animal>::set_treat";
        //    assert_contains_edge(&graph, &start, &end, &edge_type);
        //    assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        //}

        //{
        //    let start = "::Animal::sound";
        //    let edge_type = EdgeType::Impl;
        //
        //    let end = "::<Lion as Animal>::sound";
        //    assert_contains_edge(&graph, &start, &end, &edge_type);
        //    assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        //
        //    let end = "::<Dog as Animal>::sound";
        //    assert_contains_edge(&graph, &start, &end, &edge_type);
        //    assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        //}
    }
}
