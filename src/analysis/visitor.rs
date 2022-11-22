use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::ConstContext;
use rustc_middle::mir::interpret::{ConstValue, GlobalAlloc, Scalar};
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::{self, Body, Local, Location, Terminator, TerminatorKind};
use rustc_middle::mir::{ConstantKind, LocalDecl};
use rustc_middle::ty::{TyCtxt, TyKind};

pub struct GraphVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    processed_body: Option<DefId>,
}

impl<'tcx> GraphVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> GraphVisitor<'tcx> {
        GraphVisitor {
            tcx,
            processed_body: None,
        }
    }

    pub fn visit(&mut self) {
        let tcx = self.tcx;

        println!("digraph {{");

        for def_id in tcx.mir_keys(()) {
            println!("\"{}\"", tcx.def_path_debug_str(def_id.to_def_id()))
        }

        for def_id in tcx.mir_keys(()) {
            if let Some(ConstContext::ConstFn) | None = tcx.hir().body_const_context(*def_id) {
                self.visit_body(tcx.optimized_mir(def_id.to_def_id()));
            }
        }

        println!("}}");
    }
}

impl<'tcx> Visitor<'tcx> for GraphVisitor<'tcx> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        let def_id = body.source.instance.def_id();

        self.processed_body = Some(def_id);
        self.super_body(body);
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        self.super_terminator(terminator, location);

        let caller = self.processed_body.unwrap();
        if let TerminatorKind::Call { func, .. } = &terminator.kind {
            if let Some((def_id, _)) = func.const_fn_def() {
                let def_kind = self.tcx.def_kind(def_id);

                if let DefKind::Fn = def_kind {
                    println!(
                        "\"{}\" -> \"{}\"",
                        self.tcx.def_path_debug_str(caller),
                        self.tcx.def_path_debug_str(def_id)
                    );
                }
            }
        }
    }

    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>, _location: Location) {
        let literal = constant.literal;

        let caller = self.processed_body.unwrap();
        match literal {
            ConstantKind::Unevaluated(content, _ty) => {
                let def_id = content.def.did;

                println!(
                    "\"{}\" -> \"{}\" // Const + {:?}",
                    self.tcx.def_path_debug_str(caller),
                    self.tcx.def_path_debug_str(def_id),
                    _ty.kind()
                );
            }
            ConstantKind::Val(cons, _ty) => {
                if let ConstValue::Scalar(Scalar::Ptr(ptr, _)) = cons {
                    if let GlobalAlloc::Static(def_id) = self.tcx.global_alloc(ptr.provenance) {
                        println!(
                            "\"{}\" -> \"{}\" // GlobalAlloc + {:?}",
                            self.tcx.def_path_debug_str(caller),
                            self.tcx.def_path_debug_str(def_id),
                            _ty.kind()
                        );
                    }
                }
            }
            _ => (),
        }
    }

    fn visit_local_decl(&mut self, _local: Local, local_decl: &LocalDecl<'tcx>) {
        let ty = local_decl.ty;
        let kind = ty.kind();

        let caller = self.processed_body.unwrap();

        match kind {
            TyKind::Closure(def_id, _) => {
                println!(
                    "\"{}\" -> \"{}\" // Closure",
                    self.tcx.def_path_debug_str(caller),
                    self.tcx.def_path_debug_str(*def_id)
                );
            }
            TyKind::Generator(def_id, _, _) => {
                println!(
                    "\"{}\" -> \"{}\" // Generator",
                    self.tcx.def_path_debug_str(caller),
                    self.tcx.def_path_debug_str(*def_id)
                );
            }
            TyKind::FnDef(def_id, _) => {
                println!(
                    "\"{}\" -> \"{}\" // FnDef",
                    self.tcx.def_path_debug_str(caller),
                    self.tcx.def_path_debug_str(*def_id)
                );
            }
            _ => {}
        }
    }
}
