use super::graph::{DependencyGraph, EdgeType};
use crate::names::def_id_name;
use rustc_hir::def_id::DefId;
use rustc_hir::ConstContext;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::mir::visit::{TyContext, Visitor};
use rustc_middle::mir::ConstantKind;
use rustc_middle::mir::{self, Body, Location};
use rustc_middle::ty::{GenericArgKind, ImplSubject, Ty, TyCtxt, TyKind};

/// MIR Visitor responsible for creating the dependency graph and comparing checksums
pub(crate) struct GraphVisitor<'tcx, 'g> {
    tcx: TyCtxt<'tcx>,
    graph: &'g mut DependencyGraph<String>,
    processed_def_id: Option<DefId>,
}

impl<'tcx, 'g> GraphVisitor<'tcx, 'g> {
    pub(crate) fn new(
        tcx: TyCtxt<'tcx>,
        graph: &'g mut DependencyGraph<String>,
    ) -> GraphVisitor<'tcx, 'g> {
        GraphVisitor {
            tcx,
            graph,
            processed_def_id: None,
        }
    }

    pub fn visit(&mut self, def_id: DefId) {
        let has_body = self
            .tcx
            .hir()
            .maybe_body_owned_by(def_id.expect_local())
            .is_some();

        if has_body {
            let body = match self.tcx.hir().body_const_context(def_id.expect_local()) {
                Some(ConstContext::ConstFn) | None => self.tcx.optimized_mir(def_id),
                Some(ConstContext::Static(..)) | Some(ConstContext::Const) => {
                    self.tcx.mir_for_ctfe(def_id)
                }
            };

            //##########################################################################################################
            // Visit body
            self.visit_body(body);
        }
    }

    pub(crate) fn process_traits(&mut self) {
        for (_, impls) in self.tcx.all_local_trait_impls(()) {
            for def_id in impls {
                let implementors = self.tcx.impl_item_implementor_ids(def_id.to_def_id());

                if let ImplSubject::Trait(trait_ref) = self.tcx.impl_subject(def_id.to_def_id()) {
                    for subst in trait_ref.substs {
                        if let GenericArgKind::Type(ty) = subst.unpack() {
                            if let TyKind::Adt(adt_def, _) = ty.kind() {
                                for (_, &impl_fn) in implementors {
                                    self.graph.add_edge(
                                        def_id_name(self.tcx, adt_def.did()),
                                        def_id_name(self.tcx, impl_fn),
                                        EdgeType::Impl,
                                    );
                                }
                            }
                        }
                    }
                }

                for (&trait_fn, &impl_fn) in implementors {
                    self.graph.add_edge(
                        def_id_name(self.tcx, trait_fn),
                        def_id_name(self.tcx, impl_fn),
                        EdgeType::Impl,
                    );
                }
            }
        }
    }
}

impl<'tcx, 'g> Visitor<'tcx> for GraphVisitor<'tcx, 'g> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let def_id = body.source.instance.def_id();

        self.graph.add_node(def_id_name(self.tcx, def_id));

        let old_processed_body = self.processed_def_id.replace(def_id);
        self.super_body(body);
        self.processed_def_id = old_processed_body;
    }

    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);
        let Some(outer) = self.processed_def_id else {panic!("Cannot find currently analyzed body")};

        match constant.literal {
            ConstantKind::Unevaluated(content, _ty) => {
                // This takes care of borrows of e.g. "const var: u64"
                let def_id = content.def.did;

                self.graph.add_edge(
                    def_id_name(self.tcx, outer),
                    def_id_name(self.tcx, def_id),
                    EdgeType::Unevaluated,
                );
            }
            ConstantKind::Val(cons, _ty) => {
                match cons {
                    ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                        match self.tcx.global_alloc(ptr.provenance) {
                            GlobalAlloc::Static(def_id) => {
                                // This takes care of borrows of e.g. "static var: u64"
                                let (accessor, accessed_maybe_more) =
                                    (def_id_name(self.tcx, outer), def_id_name(self.tcx, def_id));

                                // // This is not necessary since for a node that writes into a variable,
                                // // there must exist a path from test to this node already
                                //
                                //if ty.is_mutable_ptr() {
                                //    // If the borrow is mut, we also add an edge in the reverse direction
                                //    self.graph.add_edge(
                                //        accessed.clone(),
                                //        accessor.clone(),
                                //        EdgeType::Scalar,
                                //    );
                                //}

                                // Since we assume that a borrow is actually read, we always add an edge here
                                self.graph.add_edge(
                                    accessor.clone(),
                                    accessed_maybe_more,
                                    EdgeType::Scalar,
                                );
                            }
                            GlobalAlloc::Function(instance) => {
                                // TODO: I have not yet found out when this is useful, but since there is a defId stored in here, it might be important
                                // Perhaps this refers to extern fns?
                                let def_id = instance.def_id();
                                let (accessor, accessed_maybe_more) =
                                    (def_id_name(self.tcx, outer), def_id_name(self.tcx, def_id));
                                self.graph.add_edge(
                                    accessor.clone(),
                                    accessed_maybe_more,
                                    EdgeType::FnPtr,
                                );
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

    fn visit_ty(&mut self, ty: Ty<'tcx>, _: TyContext) {
        self.super_ty(ty);
        let Some(outer) = self.processed_def_id else {panic!("Cannot find currently analyzed body")};

        match ty.kind() {
            TyKind::Closure(def_id, _) => {
                self.graph.add_edge(
                    def_id_name(self.tcx, outer),
                    def_id_name(self.tcx, *def_id),
                    EdgeType::Closure,
                );
            }
            TyKind::Generator(def_id, _, _) => {
                self.graph.add_edge(
                    def_id_name(self.tcx, outer),
                    def_id_name(self.tcx, *def_id),
                    EdgeType::Generator,
                );
            }
            TyKind::FnDef(def_id, _) => {
                self.graph.add_edge(
                    def_id_name(self.tcx, outer),
                    def_id_name(self.tcx, *def_id),
                    EdgeType::FnDef,
                );
            }
            TyKind::Adt(adt_def, _) => {
                self.graph.add_edge(
                    def_id_name(self.tcx, outer),
                    def_id_name(self.tcx, adt_def.did()),
                    EdgeType::Adt,
                );
            }
            //TyKind::Foreign(def_id) => {
            //    // this has effectively no impact because we do not track modifications of extern types
            //    self.graph.add_edge(
            //        def_path_debug_str_custom(self.tcx, outer),
            //        def_path_debug_str_custom(self.tcx, *def_id),
            //        EdgeType::Foreign,
            //    );
            //}
            _ => {}
        }
    }
}

#[cfg(test)]
mod test {
    use log::info;
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
                    let mut visitor = GraphVisitor::new(tcx, &mut graph);

                    for def_id in tcx.iter_local_def_id() {
                        visitor.visit(def_id.to_def_id());
                    }
                    visitor.process_traits();
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
            let end = "::Animal::sound";
            let edge_type = EdgeType::FnDef;

            let start = "::test::test_direct";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::sound_generic";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::sound_dyn";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let end = "::Animal::set_treat";
            let edge_type = EdgeType::FnDef;

            let start = "::test::test_mut_direct";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::set_treat_generic";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = "::set_treat_dyn";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let start = "::Animal::set_treat";
            let edge_type = EdgeType::Impl;

            let end = "::<Lion as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let end = "::<Dog as Animal>::set_treat";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let start = "::Animal::sound";
            let edge_type = EdgeType::Impl;

            let end = "::<Lion as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let end = "::<Dog as Animal>::sound";
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }
    }
}
