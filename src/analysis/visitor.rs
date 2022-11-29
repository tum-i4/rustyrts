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
        if let Some(ConstContext::ConstFn) | None =
            self.tcx.hir().body_const_context(def_id.expect_local())
        {
            self.visit_body(self.tcx.optimized_mir(def_id));
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
                                self.tcx.def_path_debug_str(caller),
                                self.tcx.def_path_debug_str(def_id),
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
                            self.tcx.def_path_debug_str(caller),
                            self.tcx.def_path_debug_str(def_id),
                            EdgeType::Unevaluated,
                        );
                    }
                    ConstantKind::Val(cons, _ty) => {
                        if let ConstValue::Scalar(Scalar::Ptr(ptr, _)) = cons {
                            if let GlobalAlloc::Static(def_id) =
                                self.tcx.global_alloc(ptr.provenance)
                            {
                                self.graph.add_edge(
                                    self.tcx.def_path_debug_str(caller),
                                    self.tcx.def_path_debug_str(def_id),
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
                            self.tcx.def_path_debug_str(caller),
                            self.tcx.def_path_debug_str(*def_id),
                            EdgeType::Closure,
                        );
                    }
                    TyKind::Generator(def_id, _, _) => {
                        self.graph.add_edge(
                            self.tcx.def_path_debug_str(caller),
                            self.tcx.def_path_debug_str(*def_id),
                            EdgeType::Generator,
                        );
                    }
                    TyKind::FnDef(def_id, _) => {
                        self.graph.add_edge(
                            self.tcx.def_path_debug_str(caller),
                            self.tcx.def_path_debug_str(*def_id),
                            EdgeType::FnDef,
                        );
                    }
                    _ => {}
                }
            }
        });
    }
}
