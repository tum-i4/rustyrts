use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::ConstContext;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::{self, Body, Local, Location, Terminator, TerminatorKind};
use rustc_middle::mir::{ConstantKind, LocalDecl};
use rustc_middle::ty::{TyCtxt, TyKind};
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
        if self
            .tcx
            .hir()
            .maybe_body_owned_by(def_id.expect_local())
            .is_some()
        {
            if let Some(ConstContext::ConstFn) | None =
                self.tcx.hir().body_const_context(def_id.expect_local())
            {
                self.visit_body(self.tcx.optimized_mir(def_id));
            }
        }
    }
}

impl<'tcx, 'g> Visitor<'tcx> for GraphVisitor<'tcx, 'g> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let def_id = body.source.instance.def_id();

        PROCESSED_BODY.with(|processed| {
            processed.replace(Some(def_id));
            self.super_body(body);
            processed.take();
        });
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        self.super_terminator(terminator, location);

        PROCESSED_BODY.with(|processed| {
            if let Some(caller) = *processed.borrow() {
                if let TerminatorKind::Call { func, .. } = &terminator.kind {
                    if let Some((def_id, _)) = func.const_fn_def() {
                        let def_kind = self.tcx.def_kind(def_id);

                        if let DefKind::Fn = def_kind {
                            self.graph.add_edge(
                                def_path_debug_str_custom(self.tcx, caller),
                                def_path_debug_str_custom(self.tcx, def_id),
                                EdgeType::Call,
                            );
                        }
                    }
                }
            }
        });
    }

    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>, _location: Location) {
        let literal = constant.literal;

        PROCESSED_BODY.with(|processed| {
            if let Some(caller) = *processed.borrow() {
                match literal {
                    ConstantKind::Unevaluated(content, _ty) => {
                        let def_id = content.def.did;

                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, caller),
                            def_path_debug_str_custom(self.tcx, def_id),
                            EdgeType::Unevaluated,
                        );
                    }
                    ConstantKind::Val(cons, _ty) => {
                        if let ConstValue::Scalar(Scalar::Ptr(ptr, _)) = cons {
                            if let GlobalAlloc::Static(def_id) =
                                self.tcx.global_alloc(ptr.provenance)
                            {
                                self.graph.add_edge(
                                    def_path_debug_str_custom(self.tcx, caller),
                                    def_path_debug_str_custom(self.tcx, def_id),
                                    EdgeType::Scalar,
                                );
                            }
                        }
                    }
                    _ => (),
                }
            }
        });
    }

    fn visit_local_decl(&mut self, _local: Local, local_decl: &LocalDecl<'tcx>) {
        let ty = local_decl.ty;
        let kind = ty.kind();

        PROCESSED_BODY.with(|processed| {
            if let Some(caller) = *processed.borrow() {
                match kind {
                    TyKind::Closure(def_id, _) => {
                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, caller),
                            def_path_debug_str_custom(self.tcx, *def_id),
                            EdgeType::Closure,
                        );
                    }
                    TyKind::Generator(def_id, _, _) => {
                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, caller),
                            def_path_debug_str_custom(self.tcx, *def_id),
                            EdgeType::Generator,
                        );
                    }
                    TyKind::FnDef(def_id, _) => {
                        self.graph.add_edge(
                            def_path_debug_str_custom(self.tcx, caller),
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
    use std::hash::Hash;
    use std::{fs, io::Error, path::PathBuf, string::String};

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

        println!("Path: {}", path_buf.as_path().to_str().unwrap());
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
                })
            });
        });

        graph
    }

    fn assert_contains_edge<T: Eq + Hash + Clone>(
        graph: &DependencyGraph<T>,
        start: T,
        end: T,
        edge_type: EdgeType,
    ) {
        let maybe_edges = graph.get_edges_to(start);
        assert!(maybe_edges.is_some());

        let edges = maybe_edges.unwrap();
        assert!(edges.contains_key(&end));

        let edge_types = edges.get(&end).unwrap();
        assert!(edge_types.contains(&edge_type));
    }

    #[test]
    fn test_function_call() {
        let graph = compile_and_visit("function_call.rs");

        println!("{}", graph.to_string());

        let end = String::from(format!("{CRATE_PREFIX}::test::test"));
        let start = String::from(format!("{CRATE_PREFIX}::func"));

        assert_contains_edge(&graph, start, end, EdgeType::Call);
    }
}
