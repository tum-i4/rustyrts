use std::mem::transmute;

use super::defid_util::{get_def_id_post_test_fn, get_def_id_pre_test_fn, get_def_id_trace_fn};
use crate::constants::EDGE_CASES_NO_TRACE;
use log::{error, trace};
use rustc_abi::{Align, Size};
use rustc_ast::Mutability;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::{
    mir::{
        interpret::{Allocation, ConstValue, Pointer, Scalar},
        BasicBlock, BasicBlockData, Body, Constant, ConstantKind, Local, LocalDecl, Operand, Place,
        ProjectionElem, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
    },
    ty::{List, RegionKind, Ty, TyCtxt, TyKind, UintTy},
};
use rustc_span::Span;

#[cfg(unix)]
use super::defid_util::{get_def_id_post_main_fn, get_def_id_pre_main_fn};

//######################################################################################################################
// Functions for inserting locals

fn insert_local_ret<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) -> Local {
    let span = body.span;
    let ty_empty = tcx.mk_tup([].iter());
    let local_decl_1 = LocalDecl::new(ty_empty, span).immutable();
    let local_decls = &mut body.local_decls;
    let local_1 = local_decls.push(local_decl_1);
    local_1
}

fn insert_local_u8<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) -> (Local, Ty<'tcx>) {
    let span = body.span;
    let ty_u8 = tcx.mk_ty(TyKind::Uint(UintTy::U8));
    let region = tcx.mk_region(RegionKind::ReErased);
    let ty_ref_u8 = tcx.mk_mut_ref(region, ty_u8);
    let local_decl = LocalDecl::new(ty_ref_u8, span).immutable();
    let local_decls = &mut body.local_decls;
    let local = local_decls.push(local_decl);
    (local, ty_ref_u8)
}

fn insert_local_str<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) -> (Local, Ty<'tcx>) {
    let span = body.span;
    let ty_str = tcx.mk_ty(TyKind::Str);
    let region = tcx.mk_region(RegionKind::ReErased);
    let ty_ref_str = tcx.mk_imm_ref(region, ty_str);
    let local_decl = LocalDecl::new(ty_ref_str, span).immutable();
    let local_decls = &mut body.local_decls;
    let local = local_decls.push(local_decl);
    (local, ty_ref_str)
}

//######################################################################################################################
// Functions for inserting assign statements, e.g. (_52 <= const "foo::bar")

fn insert_assign_str<'tcx>(
    tcx: TyCtxt<'tcx>,
    local_str: Local,
    place_elem_list: &'tcx List<ProjectionElem<Local, Ty<'tcx>>>,
    content: &str,
    ty_ref_str: Ty<'tcx>,
    span: Span,
) -> (Statement<'tcx>, Place<'tcx>) {
    let const_assign_statement_str = {
        let place_str = Place {
            local: local_str,
            projection: place_elem_list,
        };

        let new_allocation = Allocation::from_bytes_byte_aligned_immutable(content.as_bytes());
        let interned_allocation = tcx.intern_const_alloc(new_allocation);
        let new_const_value = ConstValue::Slice {
            data: interned_allocation,
            start: 0,
            end: content.len(),
        };
        let new_literal = ConstantKind::Val(new_const_value, ty_ref_str);

        let new_constant = Constant {
            span,
            user_ty: None,
            literal: new_literal,
        };

        let new_operand = Operand::Constant(Box::new(new_constant));
        let new_rvalue = Rvalue::Use(new_operand);

        let const_assign_statement = Statement {
            source_info: SourceInfo::outermost(span),
            kind: StatementKind::Assign(Box::new((place_str, new_rvalue))),
        };

        const_assign_statement
    };

    let place_ref_str = Place {
        local: local_str,
        projection: place_elem_list,
    };

    (const_assign_statement_str, place_ref_str)
}

fn insert_assign_u8<'tcx>(
    tcx: TyCtxt<'tcx>,
    local_u8: Local,
    place_elem_list: &'tcx List<ProjectionElem<Local, Ty<'tcx>>>,
    content: u8,
    ty_ref_u8: Ty<'tcx>,
    span: Span,
) -> (Statement<'tcx>, Place<'tcx>) {
    let const_assign_statement_u8 = {
        let place_u8 = Place {
            local: local_u8,
            projection: place_elem_list,
        };

        let content = [content];
        let new_allocation =
            Allocation::from_bytes(&content[..], Align::from_bytes(1).unwrap(), Mutability::Mut);
        let interned_allocation = tcx.intern_const_alloc(new_allocation);
        let memory_allocation = tcx.create_memory_alloc(interned_allocation);

        let new_ptr = Pointer::new(memory_allocation, Size::ZERO);
        let new_const_value = ConstValue::Scalar(Scalar::from_pointer(new_ptr, &tcx));

        let new_literal = ConstantKind::Val(new_const_value, ty_ref_u8);

        let new_constant = Constant {
            span,
            user_ty: None,
            literal: new_literal,
        };

        let new_operand = Operand::Constant(Box::new(new_constant));
        let new_rvalue = Rvalue::Use(new_operand);

        let const_assign_statement = Statement {
            source_info: SourceInfo::outermost(span),
            kind: StatementKind::Assign(Box::new((place_u8, new_rvalue))),
        };

        const_assign_statement
    };

    let place_ref_u8 = Place {
        local: local_u8,
        projection: place_elem_list,
    };

    (const_assign_statement_u8, place_ref_u8)
}

fn create_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    span: Span,
    args_vec: Vec<Operand<'tcx>>,
    local_ret: Local,
    place_elem_list: &'tcx List<ProjectionElem<Local, Ty<'tcx>>>,
    target: Option<BasicBlock>,
) -> Terminator<'tcx> {
    let func_subst = tcx.mk_substs([].iter());
    let func_ty = tcx.mk_ty(TyKind::FnDef(def_id, func_subst));
    let literal = ConstantKind::Val(ConstValue::ZeroSized, func_ty);

    let func_constant = Constant {
        span,
        user_ty: None,
        literal: literal,
    };
    let func_operand = Operand::Constant(Box::new(func_constant));

    let place_ret = Place {
        local: local_ret,
        projection: place_elem_list,
    };

    let terminator_kind = TerminatorKind::Call {
        func: func_operand,
        args: args_vec,
        destination: place_ret,
        target: target,
        cleanup: None,
        from_hir_call: false,
        fn_span: span,
    };

    let terminator = Terminator {
        source_info: SourceInfo::outermost(span),
        kind: terminator_kind,
    };

    terminator
}

//######################################################################################################################
// Functions and traits for inserting calls to rustyrts_dynamic_rlib

pub trait Traceable<'tcx> {
    fn insert_trace(
        &mut self,
        tcx: TyCtxt<'tcx>,
        name: &str,
        cache_str: &mut Option<(Local, Ty<'tcx>)>,
        cache_u8: &mut Option<(Local, Ty<'tcx>)>,
        cache_ret: &mut Option<Local>,
    );

    fn insert_pre_test(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>);

    #[cfg(unix)]
    fn insert_pre_main(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>);

    fn insert_post_test(
        &mut self,
        tcx: TyCtxt<'tcx>,
        name: &str,
        cache_str: &mut Option<(Local, Ty<'tcx>)>,
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
    );

    #[cfg(unix)]
    fn check_calls_to_exit(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>);

    #[cfg(unix)]
    fn insert_post_main(
        &mut self,
        tcx: TyCtxt<'tcx>,
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
    );
}

impl<'tcx> Traceable<'tcx> for Body<'tcx> {
    fn insert_trace(
        &mut self,
        tcx: TyCtxt<'tcx>,
        name: &str,
        cache_str: &mut Option<(Local, Ty<'tcx>)>,
        cache_u8: &mut Option<(Local, Ty<'tcx>)>,
        cache_ret: &mut Option<Local>,
    ) {
        if !EDGE_CASES_NO_TRACE.iter().any(|c| name.ends_with(c)) {
            trace!(
                "Inserting trace(\"{}\") into {:?}",
                name,
                self.source.def_id()
            );

            let Some(def_id_trace_fn) = get_def_id_trace_fn(tcx) else {
                error!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
                return;
            };

            let local_ret = *cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));
            let (local_str, ty_ref_str) =
                *cache_str.get_or_insert_with(|| insert_local_str(tcx, self));
            let (local_u8, ty_ref_u8) = *cache_u8.get_or_insert_with(|| insert_local_u8(tcx, self));

            let span = self.span;

            //*******************************************************
            // Create assign statements

            let place_elem_list = tcx.mk_place_elems([].iter());

            let (assign_statement_str, place_ref_str) =
                insert_assign_str(tcx, local_str, place_elem_list, name, ty_ref_str, span);

            let (assign_statement_u8, place_ref_u8) =
                insert_assign_u8(tcx, local_u8, place_elem_list, 0u8, ty_ref_u8, span);

            //*******************************************************
            // Create new basic block

            let basic_blocks = self.basic_blocks.as_mut();

            let mut args_vec = Vec::new();
            args_vec.push(Operand::Move(place_ref_str));
            args_vec.push(Operand::Move(place_ref_u8));
            let terminator = create_call(
                tcx,
                def_id_trace_fn,
                span,
                args_vec,
                local_ret,
                place_elem_list,
                Some(basic_blocks.next_index()), // After we swap bbs later, this will point to the original bb0
            );

            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
            new_basic_block_data.statements.push(assign_statement_str);
            new_basic_block_data.statements.push(assign_statement_u8);

            let index = basic_blocks.push(new_basic_block_data);

            // Swap bb0 and the new basic block
            basic_blocks.swap(BasicBlock::from_usize(0), index);
        }
    }

    fn insert_pre_test(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>) {
        trace!("Inserting pre_test() into {:?}", self.source.def_id());

        let Some(def_id_pre_fn) = get_def_id_pre_test_fn(tcx) else {
            error!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
            return;
        };

        let local_ret = *cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

        let span = self.span;

        let place_elem_list = tcx.mk_place_elems([].iter());

        //*******************************************************
        // Create new basic block

        let basic_blocks = self.basic_blocks.as_mut();

        let args_vec = Vec::with_capacity(0);
        let terminator = create_call(
            tcx,
            def_id_pre_fn,
            span,
            args_vec,
            local_ret,
            place_elem_list,
            Some(basic_blocks.next_index()), // After we swap bbs later, this will point to the original bb0
        );

        let new_basic_block_data = BasicBlockData::new(Some(terminator));

        let index = basic_blocks.push(new_basic_block_data);

        // Swap bb0 and the new basic block
        basic_blocks.swap(BasicBlock::from_usize(0), index);
    }

    #[cfg(unix)]
    fn insert_pre_main(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>) {
        trace!("Inserting pre_main() into {:?}", self.source.def_id());

        let Some(def_id_pre_fn) = get_def_id_pre_main_fn(tcx) else {
            error!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
            return;
        };

        cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

        let local_ret = cache_ret.unwrap();

        let span = self.span;

        let place_elem_list = tcx.mk_place_elems([].iter());

        //*******************************************************
        // Create new basic block

        // Clone former bb0
        let basic_blocks = self.basic_blocks.as_mut();

        let args_vec = Vec::with_capacity(0);
        let terminator = create_call(
            tcx,
            def_id_pre_fn,
            span,
            args_vec,
            local_ret,
            place_elem_list,
            Some(basic_blocks.next_index()), // After we swap bbs later, this will point to the original bb0
        );

        let new_basic_block_data = BasicBlockData::new(Some(terminator));

        let index = basic_blocks.push(new_basic_block_data);

        // Swap bb0 and the new basic block
        basic_blocks.swap(BasicBlock::from_usize(0), index);
    }

    fn insert_post_test(
        &mut self,
        tcx: TyCtxt<'tcx>,
        name: &str,
        cache_str: &mut Option<(Local, Ty<'tcx>)>,
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
    ) {
        trace!("Inserting post_test() into {:?}", self.source.def_id());

        let Some(def_id_post_fn) = get_def_id_post_test_fn(tcx) else {
            return;
        };

        let len = self.basic_blocks.len();
        for i in (0..len).rev() {
            let terminator_kind: &mut TerminatorKind =
            // SAFETY: We need to forcefully mutate this TerminatorKind to change its cleanup attribute
            // IMPORTANT: Do not use it after any modifications to basic_blocks (may corrupt heap)
            unsafe { transmute(&self.basic_blocks.get(BasicBlock::from_usize(i)).unwrap().terminator().kind) };

            match terminator_kind {
                TerminatorKind::Return | TerminatorKind::Resume => {
                    cache_str.get_or_insert_with(|| insert_local_str(tcx, self));
                    cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                    let local_ret = cache_ret.unwrap();
                    let (local_str, ty_ref_str) = cache_str.unwrap();

                    let span = self.span;

                    //*******************************************************
                    // Create assign statements

                    let place_elem_list = tcx.mk_place_elems([].iter());
                    let (assign_statement_str, place_ref_str) =
                        insert_assign_str(tcx, local_str, place_elem_list, name, ty_ref_str, span);

                    //*******************************************************
                    // Create new basic block

                    let basic_blocks = self.basic_blocks.as_mut();

                    let mut args_vec = Vec::new();
                    args_vec.push(Operand::Move(place_ref_str));
                    let terminator = create_call(
                        tcx,
                        def_id_post_fn,
                        span,
                        args_vec,
                        local_ret,
                        place_elem_list,
                        Some(basic_blocks.next_index()), // After we swap bbs later, this will point to the original bb_i
                    );

                    let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                    new_basic_block_data.statements.push(assign_statement_str);

                    if let TerminatorKind::Resume = terminator_kind {
                        new_basic_block_data.is_cleanup = true;
                    }

                    let index = basic_blocks.push(new_basic_block_data);

                    // Swap bb_i and the new basic block
                    basic_blocks.swap(BasicBlock::from_usize(i), index);
                }
                TerminatorKind::Call { cleanup, .. }
                | TerminatorKind::Assert { cleanup, .. }
                | TerminatorKind::InlineAsm { cleanup, .. }
                    if cleanup.is_none()
                        && !self
                            .basic_blocks
                            .get(BasicBlock::from_usize(i))
                            .unwrap()
                            .is_cleanup =>
                {
                    if let Some(call_bb) = cache_call {
                        cleanup.replace(*call_bb);
                    } else {
                        cache_str.get_or_insert_with(|| insert_local_str(tcx, self));
                        cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                        let local_ret = cache_ret.unwrap();
                        let (local_str, ty_ref_str) = cache_str.unwrap();

                        let span = self.span;

                        let basic_blocks = self.basic_blocks.as_mut();

                        // At this index, we will insert the call to rustyrts_post_test()
                        cleanup.replace(basic_blocks.next_index());

                        //*******************************************************
                        // Insert new bb to resume unwinding

                        let resume_bb = {
                            let terminator = Terminator {
                                source_info: SourceInfo::outermost(span),
                                kind: TerminatorKind::Resume,
                            };

                            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                            new_basic_block_data.is_cleanup = true;

                            basic_blocks.push(new_basic_block_data)
                        };

                        //*******************************************************
                        // Create assign statements

                        let place_elem_list = tcx.mk_place_elems([].iter());
                        let (assign_statement_str, place_ref_str) = insert_assign_str(
                            tcx,
                            local_str,
                            place_elem_list,
                            name,
                            ty_ref_str,
                            span,
                        );

                        //*******************************************************
                        // Create new basic block

                        let mut args_vec = Vec::new();
                        args_vec.push(Operand::Move(place_ref_str));
                        let terminator = create_call(
                            tcx,
                            def_id_post_fn,
                            span,
                            args_vec,
                            local_ret,
                            place_elem_list,
                            Some(basic_blocks.next_index()), // After swapping bbs, this will point to resume
                        );

                        let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                        new_basic_block_data.statements.push(assign_statement_str);
                        new_basic_block_data.is_cleanup = true;

                        let index = basic_blocks.push(new_basic_block_data);

                        // Swap bbs to order nicely (first call, then resume)
                        basic_blocks.swap(index, resume_bb);

                        cache_call.replace(resume_bb); // At this index we now find the bb containing the call
                    }
                }
                _ => (),
            }
        }
    }

    #[cfg(unix)]
    fn check_calls_to_exit(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>) {
        use super::defid_util::get_def_id_exit_fn;

        let Some(def_id_post_fn) = get_def_id_post_main_fn(tcx) else {
            return;
        };

        let Some(def_id_exit) = get_def_id_exit_fn(tcx) else {
            return;
        };

        let len = self.basic_blocks.len();
        for i in 0..len {
            let is_cleanup = self
                .basic_blocks
                .get(BasicBlock::from_usize(i))
                .unwrap()
                .is_cleanup;

            let terminator_kind: &TerminatorKind = &self
                .basic_blocks
                .get(BasicBlock::from_usize(i))
                .unwrap()
                .terminator()
                .kind;

            if let TerminatorKind::Call { func, .. } = terminator_kind {
                if let Operand::Constant(boxed_def_id) = func {
                    if let ConstantKind::Val(ConstValue::ZeroSized, func_ty) = boxed_def_id.literal
                    {
                        if let TyKind::FnDef(def_id, _) = func_ty.kind() {
                            if *def_id == def_id_exit {
                                // We found a call to std::process::exit()

                                cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                                let local_ret = cache_ret.unwrap();

                                let span = self.span;

                                let place_elem_list = tcx.mk_place_elems([].iter());

                                //*******************************************************
                                // Create new basic block

                                let basic_blocks = self.basic_blocks.as_mut();

                                let args_vec = Vec::with_capacity(0);
                                let terminator = create_call(
                                    tcx,
                                    def_id_post_fn,
                                    span,
                                    args_vec,
                                    local_ret,
                                    place_elem_list,
                                    Some(basic_blocks.next_index()), // After we swap bbs later, this will point to the original bb_i
                                );

                                let mut new_basic_block_data =
                                    BasicBlockData::new(Some(terminator));

                                if is_cleanup {
                                    new_basic_block_data.is_cleanup = true;
                                }

                                let index = basic_blocks.push(new_basic_block_data);

                                // Swap bb_i and the new basic block
                                basic_blocks.swap(BasicBlock::from_usize(i), index);
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(unix)]
    fn insert_post_main(
        &mut self,
        tcx: TyCtxt<'tcx>,
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
    ) {
        use crate::{constants::EDGE_CASE_FROM_RESIDUAL, names::def_id_name};

        trace!("Inserting post_main() into {:?}", self.source.def_id());

        let Some(def_id_post_fn) = get_def_id_post_main_fn(tcx) else {
            return;
        };

        let len = self.basic_blocks.len();
        for i in 0..len {
            let terminator_kind: &mut TerminatorKind =
            // SAFETY: We need to forcefully mutate this TerminatorKind to change its cleanup attribute
            // IMPORTANT: Do not write to it after any modifications to basic_blocks (may corrupt heap)
            unsafe { transmute( &self
                .basic_blocks
                .get(BasicBlock::from_usize(i))
                .unwrap()
                .terminator()
                .kind) };

            match terminator_kind {
                TerminatorKind::Return | TerminatorKind::Resume => {
                    cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                    let local_ret = cache_ret.unwrap();

                    let span = self.span;

                    let place_elem_list = tcx.mk_place_elems([].iter());

                    //*******************************************************
                    // Create new basic block

                    let basic_blocks = self.basic_blocks.as_mut();

                    let args_vec = Vec::with_capacity(0);
                    let terminator = create_call(
                        tcx,
                        def_id_post_fn,
                        span,
                        args_vec,
                        local_ret,
                        place_elem_list,
                        Some(basic_blocks.next_index()), // After we swap bbs later, this will point to the original bb_i
                    );

                    let mut new_basic_block_data = BasicBlockData::new(Some(terminator));

                    if let TerminatorKind::Resume = terminator_kind {
                        new_basic_block_data.is_cleanup = true;
                    }

                    let index = basic_blocks.push(new_basic_block_data);

                    // Swap bb_i and the new basic block
                    basic_blocks.swap(BasicBlock::from_usize(i), index);
                }
                TerminatorKind::Call { cleanup, .. }
                | TerminatorKind::Assert { cleanup, .. }
                | TerminatorKind::InlineAsm { cleanup, .. }
                    if cleanup.is_none()
                        && !self
                            .basic_blocks
                            .get(BasicBlock::from_usize(i))
                            .unwrap()
                            .is_cleanup =>
                {
                    let terminator_kind = &self
                        .basic_blocks
                        .get(BasicBlock::from_usize(i))
                        .unwrap()
                        .terminator()
                        .kind;

                    if let TerminatorKind::Call { func, .. } = terminator_kind {
                        if def_id_name(tcx, func.const_fn_def().unwrap().0, &[], false, true)
                            .split_once("::")
                            .map(|(_, second)| second == EDGE_CASE_FROM_RESIDUAL)
                            .unwrap_or(false)
                        {
                            // EDGE CASE: if the unwind attribute of a call to this function is inserted,
                            // llvm will throw an error and abort compilation
                            continue;
                        }
                    }

                    if let Some(call_bb) = cache_call {
                        cleanup.replace(*call_bb);
                    } else {
                        cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                        let local_ret = cache_ret.unwrap();

                        let span = self.span;

                        let basic_blocks = self.basic_blocks.as_mut();

                        // At this index, we will insert the call to rustyrts_post_main()
                        cleanup.replace(basic_blocks.next_index());

                        //*******************************************************
                        // Insert new bb to resume

                        let resume_bb = {
                            let terminator = Terminator {
                                source_info: SourceInfo::outermost(span),
                                kind: TerminatorKind::Resume,
                            };

                            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                            new_basic_block_data.is_cleanup = true;

                            basic_blocks.push(new_basic_block_data)
                        };

                        //*******************************************************
                        // Create new basic block

                        let place_elem_list = tcx.mk_place_elems([].iter());

                        let args_vec = Vec::with_capacity(0);
                        let terminator = create_call(
                            tcx,
                            def_id_post_fn,
                            span,
                            args_vec,
                            local_ret,
                            place_elem_list,
                            Some(basic_blocks.next_index()), // After swapping bbs, this will point to resume
                        );

                        let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                        new_basic_block_data.is_cleanup = true;

                        let index = basic_blocks.push(new_basic_block_data);

                        // Swap bbs to order nicely (first call, then resume)
                        basic_blocks.swap(index, resume_bb);

                        cache_call.replace(resume_bb); // At this index we now find the bb containing the call
                    }
                }
                _ => (),
            }
        }
    }
}
