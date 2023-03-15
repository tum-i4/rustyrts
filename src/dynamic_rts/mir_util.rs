use std::mem::transmute;

use super::defid_util::{get_def_id_post_fn, get_def_id_pre_fn, get_def_id_trace_fn};
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
// Functions for inserting calls to rustyrts_dynamic_rlib

pub fn insert_trace<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, name: &str) {
    let Some(def_id_trace_fn) = get_def_id_trace_fn(tcx) else {
        eprintln!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
        return;
    };

    let local_ret = insert_local_ret(tcx, body);
    let (local_str, ty_ref_str) = insert_local_str(tcx, body);
    let (local_u8, ty_ref_u8) = insert_local_u8(tcx, body);

    let span = body.span;

    //*******************************************************
    // Create assign statements

    let place_elem_list = tcx.intern_place_elems(&[]);

    let (assign_statement_str, place_ref_str) =
        insert_assign_str(tcx, local_str, place_elem_list, name, ty_ref_str, span);

    let (assign_statement_u8, place_ref_u8) =
        insert_assign_u8(tcx, local_u8, place_elem_list, 0u8, ty_ref_u8, span);

    //*******************************************************
    // Create new basic block

    // Clone former bb0
    let index_vec = body.basic_blocks.as_mut();
    let first_basic_block_data = index_vec.raw.get(0).unwrap();
    let basic_block = index_vec.push(first_basic_block_data.clone());

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
        Some(basic_block),
    );

    let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
    new_basic_block_data.statements.push(assign_statement_str);
    new_basic_block_data.statements.push(assign_statement_u8);

    *body.basic_blocks.as_mut().raw.get_mut(0).unwrap() = new_basic_block_data;
}

pub fn insert_pre<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let Some(def_id_pre_fn) = get_def_id_pre_fn(tcx) else {
        eprintln!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
        return;
    };

    let local_ret = insert_local_ret(tcx, body);
    let span = body.span;

    let place_elem_list = tcx.intern_place_elems(&[]);

    //*******************************************************
    // Create new basic block

    // Clone former bb0
    let index_vec = body.basic_blocks.as_mut();
    let first_basic_block_data = index_vec.raw.get(0).unwrap();
    let basic_block = index_vec.push(first_basic_block_data.clone());

    let args_vec = Vec::new();
    let terminator = create_call(
        tcx,
        def_id_pre_fn,
        span,
        args_vec,
        local_ret,
        place_elem_list,
        Some(basic_block),
    );

    let new_basic_block_data = BasicBlockData::new(Some(terminator));

    *body.basic_blocks.as_mut().raw.get_mut(0).unwrap() = new_basic_block_data;
}

pub fn insert_post<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, name: &str) {
    let Some(def_id_post_fn) = get_def_id_post_fn(tcx) else {
        return;
    };

    let bb_calling_test_fn = body.basic_blocks.raw.get(0).unwrap();

    let terminator_kind: &mut TerminatorKind =
        // SAFETY: We need to forcefully mutate this TerminatorKind to change its cleanup attribute
        unsafe { transmute(&bb_calling_test_fn.terminator().kind) };

    if let TerminatorKind::Call {
        func: _,
        args: _,
        destination: _,
        target: _,
        cleanup,
        from_hir_call: _,
        fn_span: _,
    } = terminator_kind
    {
        let local_ret = insert_local_ret(tcx, body);
        let (local_str, ty_ref_str) = insert_local_str(tcx, body);

        let span = body.span;

        //*******************************************************
        // Determine next bb in unwinding
        // (If not present, we insert a new one containing a Resume terminator)

        let resume_bb = cleanup.take().unwrap_or_else(|| {
            let terminator = Terminator {
                source_info: SourceInfo::outermost(span),
                kind: TerminatorKind::Resume,
            };

            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
            new_basic_block_data.is_cleanup = true;

            let new_bb = body.basic_blocks.as_mut().push(new_basic_block_data);
            new_bb
        });

        //*******************************************************
        // Create assign statements

        let place_elem_list = tcx.intern_place_elems(&[]);
        let (assign_statement_str, place_ref_str) =
            insert_assign_str(tcx, local_str, place_elem_list, name, ty_ref_str, span);

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
            Some(resume_bb), // the next cleanup bb is inserted here
        );

        let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
        new_basic_block_data.is_cleanup = true;
        new_basic_block_data.statements.push(assign_statement_str);

        let new_bb = body.basic_blocks.as_mut().push(new_basic_block_data);

        // here we insert the call to rustyrts_post_fn() into cleanup
        *cleanup = Some(new_bb);
    }

    let len = body.basic_blocks.raw.len();
    for i in 1..len {
        let terminator_kind = &body.basic_blocks.raw.get(i).unwrap().terminator().kind;

        if let TerminatorKind::Return = terminator_kind {
            let local_ret = insert_local_ret(tcx, body);
            let (local_str, ty_ref_str) = insert_local_str(tcx, body);

            let span = body.span;

            //*******************************************************
            // Create assign statements

            let place_elem_list = tcx.intern_place_elems(&[]);
            let (assign_statement_str, place_ref_str) =
                insert_assign_str(tcx, local_str, place_elem_list, name, ty_ref_str, span);

            //*******************************************************
            // Create new basic block

            // Clone former basic_block
            let index_vec = body.basic_blocks.as_mut();
            let old_basic_block = index_vec.raw.get(i).unwrap();
            let basic_block = index_vec.push(old_basic_block.clone());

            let mut args_vec = Vec::new();
            args_vec.push(Operand::Move(place_ref_str));
            let terminator = create_call(
                tcx,
                def_id_post_fn,
                span,
                args_vec,
                local_ret,
                place_elem_list,
                Some(basic_block),
            );

            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
            new_basic_block_data.statements.push(assign_statement_str);

            *body.basic_blocks.as_mut().raw.get_mut(i).unwrap() = new_basic_block_data;
        }
    }
}
