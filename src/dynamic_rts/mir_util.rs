use std::mem::transmute;

use super::defid_util::{get_def_id_post_test_fn, get_def_id_pre_test_fn, get_def_id_trace_fn};
use crate::constants::EDGE_CASES_NO_TRACE;
use rustc_abi::HasDataLayout;
use rustc_abi::Size;
use rustc_ast::Mutability;
use rustc_data_structures::sorted_map::SortedMap;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::mir::interpret::CtfeProvenance;
use rustc_middle::{
    mir::{interpret::AllocId, CallSource, Const, ConstOperand, ConstValue, UnwindAction},
    ty::Region,
};
use rustc_middle::{
    mir::{
        interpret::{Allocation, Pointer, Scalar},
        BasicBlock, BasicBlockData, Body, Local, LocalDecl, Operand, Place, ProjectionElem,
        SourceInfo, Terminator, TerminatorKind,
    },
    ty::{List, RegionKind, Ty, TyCtxt, TyKind, TypeAndMut, UintTy},
};
use rustc_span::Span;
use tracing::{error, trace};

#[cfg(unix)]
use super::defid_util::{get_def_id_post_main_fn, get_def_id_pre_main_fn};

//######################################################################################################################
// Functions for inserting locals

fn insert_local_ret<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) -> Local {
    let span = body.span;
    let ty_empty = tcx.mk_ty_from_kind(TyKind::Tuple(List::empty()));
    let local_decl_1 = LocalDecl::new(ty_empty, span).immutable();
    let local_decls = &mut body.local_decls;

    local_decls.push(local_decl_1)
}

fn ty_bool(tcx: TyCtxt<'_>) -> Ty<'_> {
    let ty_bool = tcx.mk_ty_from_kind(TyKind::Bool);
    ty_bool
}

fn ty_str(tcx: TyCtxt<'_>) -> Ty<'_> {
    let ty_str = tcx.mk_ty_from_kind(TyKind::Str);
    let region = Region::new_from_kind(tcx, RegionKind::ReErased);
    let ty_ref_str = tcx.mk_ty_from_kind(TyKind::Ref(region, ty_str, Mutability::Not));
    ty_ref_str
}

fn ty_tuple_of_str_and_ptr(tcx: TyCtxt<'_>) -> Ty<'_> {
    let ty_str = tcx.mk_ty_from_kind(TyKind::Str);
    let ty_ptr = tcx.mk_ty_from_kind(TyKind::RawPtr(TypeAndMut {
        ty: tcx.mk_ty_from_kind(TyKind::Uint(UintTy::U64)),
        mutbl: Mutability::Mut,
    }));
    let region = Region::new_from_kind(tcx, RegionKind::ReErased);
    let ty_ref_str = tcx.mk_ty_from_kind(TyKind::Ref(region, ty_str, Mutability::Not));
    let list = tcx.mk_type_list(&[ty_ref_str, ty_ptr]);
    let ty_tuple = tcx.mk_ty_from_kind(TyKind::Tuple(list));
    let ty_ref_tuple = tcx.mk_ty_from_kind(TyKind::Ref(region, ty_tuple, Mutability::Mut));
    ty_ref_tuple
}

//######################################################################################################################
// Functions for inserting assign statements, e.g. (_52 <= const "foo::bar")

fn operand_str<'tcx>(
    tcx: TyCtxt<'tcx>,
    content: &str,
    ty_ref_str: Ty<'tcx>,
    span: Span,
) -> Operand<'tcx> {
    let new_allocation = Allocation::from_bytes_byte_aligned_immutable(content.as_bytes());
    let interned_allocation = tcx.mk_const_alloc(new_allocation);
    let new_const_value = ConstValue::Slice {
        data: interned_allocation,
        meta: content.len() as u64,
    };
    let new_literal = Const::Val(new_const_value, ty_ref_str);

    let new_constant = ConstOperand {
        span,
        user_ty: None,
        const_: new_literal,
    };

    Operand::Constant(Box::new(new_constant))
}

fn operand_bool(content: bool, ty_bool: Ty<'_>, span: Span) -> Operand<'_> {
    let new_const_value = ConstValue::Scalar(Scalar::from_bool(content));

    let new_literal = Const::Val(new_const_value, ty_bool);

    let new_constant = ConstOperand {
        span,
        user_ty: None,
        const_: new_literal,
    };

    Operand::Constant(Box::new(new_constant))
}

fn operand_tuple_of_str_and_ptr<'tcx>(
    tcx: TyCtxt<'tcx>,
    content_str: &str,
    content_ptr: u64,
    ty_tuple_of_str_and_ptr: Ty<'tcx>,
    span: Span,
) -> Operand<'tcx> {
    let str_allocation = Allocation::from_bytes_byte_aligned_immutable(content_str.as_bytes());
    let str_interned_allocation = tcx.mk_const_alloc(str_allocation);
    let str_memory_alloc = tcx.reserve_and_set_memory_alloc(str_interned_allocation);

    let tuple_allocation = Allocation::from_bytes(
        [
            [0x0; 8],
            content_str.len().to_ne_bytes(),
            content_ptr.to_ne_bytes(),
        ]
        .concat(),
        tcx.data_layout().aggregate_align.pref,
        Mutability::Mut,
    );

    let provenance_map = tuple_allocation.provenance();
    let map: &mut SortedMap<Size, AllocId> = unsafe { std::mem::transmute(provenance_map.ptrs()) };
    map.insert(Size::from_bytes(0), str_memory_alloc);

    let tuple_interned_allocation = tcx.mk_const_alloc(tuple_allocation);

    let tuple_memory_allocation = tcx.reserve_and_set_memory_alloc(tuple_interned_allocation);
    let provenance = CtfeProvenance::from(tuple_memory_allocation);

    let tuple_ptr = Pointer::new(provenance, Size::ZERO);
    let tuple_const_value = ConstValue::Scalar(Scalar::from_pointer(tuple_ptr, &tcx));

    let ref_tuple = Const::Val(tuple_const_value, ty_tuple_of_str_and_ptr);

    let tuple_constant = ConstOperand {
        span,
        user_ty: None,
        const_: ref_tuple,
    };

    Operand::Constant(Box::new(tuple_constant))
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
    let func_subst = tcx.mk_args(&[]);
    let func_ty = tcx.mk_ty_from_kind(TyKind::FnDef(def_id, func_subst));
    let literal = Const::Val(ConstValue::ZeroSized, func_ty);

    let func_constant = ConstOperand {
        span,
        user_ty: None,
        const_: literal,
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
        target,
        call_source: CallSource::Normal,
        fn_span: span,
        unwind: UnwindAction::Continue,
    };

    Terminator {
        source_info: SourceInfo::outermost(span),
        kind: terminator_kind,
    }
}

//######################################################################################################################
// Functions and traits for inserting calls to rustyrts_dynamic_rlib

pub trait Traceable<'tcx> {
    fn insert_trace(&mut self, tcx: TyCtxt<'tcx>, name: &str, cache_ret: &mut Option<Local>);

    fn insert_pre_test(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>);

    #[cfg(unix)]
    fn insert_pre_main(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>);

    fn insert_post_test(
        &mut self,
        tcx: TyCtxt<'tcx>,
        name: &str,
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
        append: bool,
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
    fn insert_trace(&mut self, tcx: TyCtxt<'tcx>, name: &str, cache_ret: &mut Option<Local>) {
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

            let span = self.span;
            let place_elem_list = tcx.mk_place_elems(&[]);

            let local_ret = *cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));
            let ty_tuple = ty_tuple_of_str_and_ptr(tcx);

            //*******************************************************
            // Create new basic block

            let basic_blocks = self.basic_blocks.as_mut();

            let operand_tuple_of_str_and_ptr =
                operand_tuple_of_str_and_ptr(tcx, name, u64::MAX, ty_tuple, span);

            let args_vec = vec![operand_tuple_of_str_and_ptr];
            let terminator = create_call(
                tcx,
                def_id_trace_fn,
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
    }

    fn insert_pre_test(&mut self, tcx: TyCtxt<'tcx>, cache_ret: &mut Option<Local>) {
        trace!("Inserting pre_test() into {:?}", self.source.def_id());

        let Some(def_id_pre_fn) = get_def_id_pre_test_fn(tcx) else {
            error!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
            return;
        };

        let local_ret = *cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

        let span = self.span;

        let place_elem_list = tcx.mk_place_elems(&[]);

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

        let place_elem_list = tcx.mk_place_elems(&[]);

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
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
        append: bool,
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
                TerminatorKind::Return | TerminatorKind::UnwindResume => {
                    cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                    let place_elem_list = tcx.mk_place_elems(&[]);
                    let span = self.span;

                    let local_ret = cache_ret.unwrap();
                    let ty_ref_str = ty_str(tcx);
                    let ty_bool = ty_bool(tcx);

                    //*******************************************************
                    // Create new basic block

                    let basic_blocks = self.basic_blocks.as_mut();

                    let operand_str = operand_str(tcx, name, ty_ref_str, span);
                    let operand_bool = operand_bool(append, ty_bool, span);

                    let args_vec = vec![operand_str, operand_bool];
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

                    if let TerminatorKind::UnwindResume = terminator_kind {
                        new_basic_block_data.is_cleanup = true;
                    }

                    let index = basic_blocks.push(new_basic_block_data);

                    // Swap bb_i and the new basic block
                    basic_blocks.swap(BasicBlock::from_usize(i), index);
                }
                TerminatorKind::Call { unwind, .. }
                | TerminatorKind::Assert { unwind, .. }
                | TerminatorKind::InlineAsm { unwind, .. }
                    if *unwind == UnwindAction::Continue
                        && !self
                            .basic_blocks
                            .get(BasicBlock::from_usize(i))
                            .unwrap()
                            .is_cleanup =>
                {
                    if let Some(call_bb) = cache_call {
                        *unwind = UnwindAction::Cleanup(*call_bb);
                    } else {
                        cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                        let place_elem_list = tcx.mk_place_elems(&[]);
                        let span = self.span;

                        let local_ret = cache_ret.unwrap();
                        let ty_ref_str = ty_str(tcx);
                        let ty_bool = ty_bool(tcx);

                        let basic_blocks = self.basic_blocks.as_mut();

                        // At this index, we will insert the call to rustyrts_post_test()
                        *unwind = UnwindAction::Cleanup(basic_blocks.next_index());

                        //*******************************************************
                        // Insert new bb to resume unwinding

                        let resume_bb = {
                            let terminator = Terminator {
                                source_info: SourceInfo::outermost(span),
                                kind: TerminatorKind::UnwindResume,
                            };

                            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                            new_basic_block_data.is_cleanup = true;

                            basic_blocks.push(new_basic_block_data)
                        };

                        //*******************************************************
                        // Create assign statements

                        //*******************************************************
                        // Create new basic block

                        let operand_str = operand_str(tcx, name, ty_ref_str, span);
                        let operand_bool = operand_bool(append, ty_bool, span);

                        let args_vec = vec![operand_str, operand_bool];
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

            if let TerminatorKind::Call {
                func: Operand::Constant(boxed_def_id),
                ..
            } = terminator_kind
            {
                if let Const::Val(ConstValue::ZeroSized, func_ty) = boxed_def_id.const_ {
                    if let TyKind::FnDef(def_id, _) = func_ty.kind() {
                        if *def_id == def_id_exit {
                            // We found a call to std::process::exit()

                            cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                            let local_ret = cache_ret.unwrap();

                            let span = self.span;

                            let place_elem_list = tcx.mk_place_elems(&[]);

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

    #[cfg(unix)]
    fn insert_post_main(
        &mut self,
        tcx: TyCtxt<'tcx>,
        cache_ret: &mut Option<Local>,
        cache_call: &mut Option<BasicBlock>,
    ) {
        use crate::names::def_id_name;

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
                TerminatorKind::Return | TerminatorKind::UnwindResume => {
                    cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                    let local_ret = cache_ret.unwrap();

                    let span = self.span;

                    let place_elem_list = tcx.mk_place_elems(&[]);

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

                    if let TerminatorKind::UnwindResume = terminator_kind {
                        new_basic_block_data.is_cleanup = true;
                    }

                    let index = basic_blocks.push(new_basic_block_data);

                    // Swap bb_i and the new basic block
                    basic_blocks.swap(BasicBlock::from_usize(i), index);
                }
                TerminatorKind::Call { unwind, .. }
                | TerminatorKind::Assert { unwind, .. }
                | TerminatorKind::InlineAsm { unwind, .. }
                    if *unwind == UnwindAction::Continue
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

                    if let TerminatorKind::Call {
                        destination, func, ..
                    } = terminator_kind
                    {
                        if let Some(local) = destination.as_local() {
                            if local.as_usize() == 0 {
                                // LLVM terminates with an error when calls that directly return something from main() are extended with a Resume terminator
                                continue;
                            }
                        }

                        // HACK: in tracing, injecting a cleanup into a particular terminator results in an LLVM error
                        if func.const_fn_def().is_some_and(|(def, _)| {
                            def_id_name(tcx, def, false, true).contains("__is_enabled")
                        }) {
                            continue;
                        }
                    }

                    if let Some(call_bb) = cache_call {
                        *unwind = UnwindAction::Cleanup(*call_bb);
                    } else {
                        cache_ret.get_or_insert_with(|| insert_local_ret(tcx, self));

                        let local_ret = cache_ret.unwrap();

                        let span = self.span;

                        let basic_blocks = self.basic_blocks.as_mut();

                        // At this index, we will insert the call to rustyrts_post_main()
                        *unwind = UnwindAction::Cleanup(basic_blocks.next_index());

                        //*******************************************************
                        // Insert new bb to resume

                        let resume_bb = {
                            let terminator = Terminator {
                                source_info: SourceInfo::outermost(span),
                                kind: TerminatorKind::UnwindResume,
                            };

                            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
                            new_basic_block_data.is_cleanup = true;

                            basic_blocks.push(new_basic_block_data)
                        };

                        //*******************************************************
                        // Create new basic block

                        let place_elem_list = tcx.mk_place_elems(&[]);

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
