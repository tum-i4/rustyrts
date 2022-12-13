use log::debug;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::ConstContext;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::mir::visit::{TyContext, Visitor};
use rustc_middle::mir::{self, Body, Local, Location, Terminator, TerminatorKind};
use rustc_middle::mir::{ConstantKind, LocalDecl};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use std::cell::RefCell;

use crate::graph::graph::{DependencyGraph, EdgeType};

use super::util::def_path_debug_str_custom;

thread_local! {
    static PROCESSED_BODY: RefCell<Option<DefId>>  = RefCell::new(None);
}

pub(crate) struct GraphVisitor<'tcx, 'g> {
    tcx: TyCtxt<'tcx>,
    graph: &'g mut DependencyGraph<String>,
}

impl<'tcx, 'g> GraphVisitor<'tcx, 'g> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        graph: &'g mut DependencyGraph<String>,
    ) -> GraphVisitor<'tcx, 'g> {
        GraphVisitor { tcx, graph }
    }

    pub fn visit(&mut self, def_id: DefId) {
        let has_body = self
            .tcx
            .hir()
            .maybe_body_owned_by(def_id.expect_local())
            .is_some();

        debug!("Processing {}", def_path_debug_str_custom(self.tcx, def_id));

        if has_body {
            debug!("... with body");
            if let Some(ConstContext::ConstFn) | None =
                self.tcx.hir().body_const_context(def_id.expect_local())
            {
                let body = self.tcx.optimized_mir(def_id);

                //let target = "rust_out::test::test_higher_order";
                //let actual = def_path_debug_str_custom(self.tcx, def_id);
                //if actual == target {
                //    debug!("{:?}", body);
                //}

                self.visit_body(body);
            }
        }
    }

    pub fn process_traits(&mut self) {
        for (_, impls) in self.tcx.all_local_trait_impls(()) {
            for def_id in impls {
                let implementors = self.tcx.impl_item_implementor_ids(def_id.to_def_id());
                for (&trait_fn, &impl_fn) in implementors {
                    self.graph.add_edge(
                        def_path_debug_str_custom(self.tcx, trait_fn),
                        def_path_debug_str_custom(self.tcx, impl_fn),
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

        self.graph
            .add_node(def_path_debug_str_custom(self.tcx, def_id));

        PROCESSED_BODY.with(|processed| {
            processed.replace(Some(def_id));
            self.super_body(body);
            processed.take();
        });
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        self.super_terminator(terminator, location);

        PROCESSED_BODY.with(|processed| {
            if let Some(outer) = *processed.borrow() {
                if let TerminatorKind::Call { func, .. } = &terminator.kind {
                    if let Some((def_id, _)) = func.const_fn_def() {
                        let def_kind = self.tcx.def_kind(def_id);

                        if let DefKind::Fn | DefKind::AssocFn = def_kind {
                            self.graph.add_edge(
                                def_path_debug_str_custom(self.tcx, outer),
                                def_path_debug_str_custom(self.tcx, def_id),
                                EdgeType::Call,
                            );
                        }
                    }
                }
            }
        });
    }

    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        let literal = constant.literal;

        PROCESSED_BODY.with(|processed| {
            if let Some(outer) = *processed.borrow() {
                match literal {
                    ConstantKind::Unevaluated(content, _ty) => {
                        let def_id = content.def.did;

                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, outer),
                            def_path_debug_str_custom(self.tcx, def_id),
                            EdgeType::Unevaluated,
                        );
                    }
                    ConstantKind::Val(cons, ty) => {
                        match cons {
                            ConstValue::Scalar(Scalar::Ptr(ptr, _)) => {
                                match self.tcx.global_alloc(ptr.provenance) {
                                    GlobalAlloc::Static(def_id) => {
                                        let (accessor, accessed) = (
                                            def_path_debug_str_custom(self.tcx, outer),
                                            def_path_debug_str_custom(self.tcx, def_id),
                                        );

                                        if ty.is_mutable_ptr() {
                                            // If the ptr is mut, we also add an edge in the reverse direction
                                            self.graph.add_edge(
                                                accessed.clone(),
                                                accessor.clone(),
                                                EdgeType::Scalar,
                                            );
                                        }
                                        // Since we do not know if a mut ptr is read, we unconditionally add an edge here
                                        self.graph.add_edge(accessor, accessed, EdgeType::Scalar);
                                    }
                                    GlobalAlloc::Function(instance) => {
                                        let def_id = instance.def_id();
                                        let (accessor, accessed) = (
                                            def_path_debug_str_custom(self.tcx, outer),
                                            def_path_debug_str_custom(self.tcx, def_id),
                                        );
                                        // TODO: find out if this is useful at all
                                        self.graph.add_edge(accessor, accessed, EdgeType::FnPtr);
                                    }
                                    _ => (),
                                }
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                }
            }
        });
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _: TyContext) {
        PROCESSED_BODY.with(|processed| {
            if let Some(outer) = *processed.borrow() {
                match ty.kind() {
                    TyKind::Closure(def_id, _) => {
                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, outer),
                            def_path_debug_str_custom(self.tcx, *def_id),
                            EdgeType::Closure,
                        );
                    }
                    TyKind::Generator(def_id, _, _) => {
                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, outer),
                            def_path_debug_str_custom(self.tcx, *def_id),
                            EdgeType::Generator,
                        );
                    }
                    TyKind::FnDef(def_id, _) => {
                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, outer),
                            def_path_debug_str_custom(self.tcx, *def_id),
                            EdgeType::FnDef,
                        );
                    }
                    _ => {}
                }
            }
        });
    }
}

#[cfg(test)]
mod test {
    use log::info;
    use std::fmt::Display;
    use std::hash::Hash;
    use std::{fs, io::Error, path::PathBuf, string::String};
    use test_log::test;

    use rustc_errors::registry;
    use rustc_hash::{FxHashMap, FxHashSet};
    use rustc_session::config::{self, CheckCfg};
    use rustc_span::source_map;

    use crate::graph::graph::DependencyGraph;
    use crate::graph::graph::EdgeType;

    use super::GraphVisitor;

    const TEST_DATA_PATH: &str = "test-data/src";
    const CRATE_PREFIX: &str = "rust_out";

    fn load_test_code(file_name: &str) -> Result<String, Error> {
        let mut path_buf = PathBuf::from(TEST_DATA_PATH);
        path_buf.push(file_name);
        fs::read_to_string(path_buf.as_path())
    }

    fn compile_and_visit(file_name: &str) -> DependencyGraph<String> {
        let test_code = load_test_code(file_name).expect("Failed to load test code.");

        let config = rustc_interface::Config {
            // Command line options
            opts: config::Options {
                test: true,
                ..config::Options::default()
            },
            // cfg! configuration in addition to the default ones
            crate_cfg: FxHashSet::default(), // FxHashSet<(String, Option<String>)>
            crate_check_cfg: CheckCfg::default(), // CheckCfg
            input: config::Input::Str {
                name: source_map::FileName::Custom("main.rs".into()),
                input: test_code,
            },
            input_path: None,
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
                queries.global_ctxt().unwrap().take().enter(|tcx| {
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

    fn assert_contains_edge<T: Eq + Hash + Clone + Display>(
        graph: &DependencyGraph<T>,
        start: &T,
        end: &T,
        edge_type: &EdgeType,
    ) {
        let error_str = format!("Did not find edge {} -> {} ({:?})", start, end, edge_type);

        let maybe_edges = graph.get_edges_to(end);
        assert!(maybe_edges.is_some(), "{}", error_str);

        let edges = maybe_edges.unwrap();
        assert!(edges.contains_key(start), "{}", error_str);

        let edge_types = edges.get(&start).unwrap();
        assert!(edge_types.contains(edge_type), "{}", error_str);
    }

    fn assert_does_not_contain_edge<T: Eq + Hash + Clone + Display>(
        graph: &DependencyGraph<T>,
        start: &T,
        end: &T,
        edge_type: &EdgeType,
    ) {
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

        let start = format!("{CRATE_PREFIX}::test::test");
        let end = format!("{CRATE_PREFIX}::func");
        let edge_type = EdgeType::Call;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_closure() {
        let graph = compile_and_visit("closure.rs");

        let start = format!("{CRATE_PREFIX}::test::test");
        let end = format!("{CRATE_PREFIX}::test::test#1::{{closure#0}}");
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
            let end = format!("{CRATE_PREFIX}::FOO");

            let start = format!("{CRATE_PREFIX}::test::test_direct_read");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = format!("{CRATE_PREFIX}::test::test_indirect_ptr_read");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = format!("{CRATE_PREFIX}::test::test_indirect_ref_read");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            // mut ptr, additional edge in reverse direction
            let start = format!("{CRATE_PREFIX}::BAR");

            let end = format!("{CRATE_PREFIX}::test::test_direct_write");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);

            let end = format!("{CRATE_PREFIX}::test::test_indirect_ptr_write");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);

            let end = format!("{CRATE_PREFIX}::test::test_indirect_ref_write");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);

            let end = format!("{CRATE_PREFIX}::test::test_indirect_ref_write");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_contains_edge(&graph, &end, &start, &edge_type);
        }
    }

    #[test]
    fn test_unealuated() {
        let graph = compile_and_visit("unevaluated.rs");

        let start = format!("{CRATE_PREFIX}::test::test_const_read");
        let end = format!("{CRATE_PREFIX}::BAZ");
        let edge_type = EdgeType::Unevaluated;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_fndef() {
        let graph = compile_and_visit("fndef.rs");

        let start = format!("{CRATE_PREFIX}::test::test_indirect");
        let end = format!("{CRATE_PREFIX}::incr");
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = format!("{CRATE_PREFIX}::test::test_higher_order");
        let end = format!("{CRATE_PREFIX}::incr");
        let edge_type = EdgeType::FnDef;
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_impls() {
        let graph = compile_and_visit("impls.rs");

        let edge_type = EdgeType::Call;

        let start = format!("{CRATE_PREFIX}::test::test_static");
        let end = format!("{CRATE_PREFIX}::{{impl#0}}::new");
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = format!("{CRATE_PREFIX}::test::test_const");
        let end = format!("{CRATE_PREFIX}::{{impl#1}}::get");
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

        let start = format!("{CRATE_PREFIX}::test::test_mut");
        let end = format!("{CRATE_PREFIX}::{{impl#1}}::set");
        assert_contains_edge(&graph, &start, &end, &edge_type);
        assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
    }

    #[test]
    fn test_traits() {
        let graph = compile_and_visit("traits.rs");

        {
            let end = format!("{CRATE_PREFIX}::Animal::sound");
            let edge_type = EdgeType::Call;

            let start = format!("{CRATE_PREFIX}::test::test_direct");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = format!("{CRATE_PREFIX}::sound_generic");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = format!("{CRATE_PREFIX}::sound_dyn");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let end = format!("{CRATE_PREFIX}::Animal::set_treat");
            let edge_type = EdgeType::Call;

            let start = format!("{CRATE_PREFIX}::test::test_mut_direct");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = format!("{CRATE_PREFIX}::set_treat_generic");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let start = format!("{CRATE_PREFIX}::set_treat_dyn");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let start = format!("{CRATE_PREFIX}::Animal::set_treat");
            let edge_type = EdgeType::Impl;

            let end = format!("{CRATE_PREFIX}::{{impl#0}}::set_treat");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let end = format!("{CRATE_PREFIX}::{{impl#1}}::set_treat");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }

        {
            let start = format!("{CRATE_PREFIX}::Animal::sound");
            let edge_type = EdgeType::Impl;

            let end = format!("{CRATE_PREFIX}::{{impl#0}}::sound");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

            let end = format!("{CRATE_PREFIX}::{{impl#1}}::sound");
            assert_contains_edge(&graph, &start, &end, &edge_type);
            assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
        }
    }
}
