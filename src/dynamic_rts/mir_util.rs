use std::mem::transmute;

use super::defid_util::{get_def_id_post_fn, get_def_id_pre_fn, get_def_id_trace_fn};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::{
    mir::{
        interpret::{Allocation, ConstValue},
        BasicBlockData, Body, Constant, ConstantKind, Local, LocalDecl, Operand, Place, Rvalue,
        SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
    },
    ty::{RegionKind, Ty, TyCtxt, TyKind},
};

pub fn insert_local_ret<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) -> Local {
    let span = body.span;
    let ty_empty = tcx.mk_tup([].iter());
    let local_decl_1 = LocalDecl::new(ty_empty, span).immutable();
    let local_decls = &mut body.local_decls;
    let local_1 = local_decls.push(local_decl_1);
    local_1
}

pub fn insert_locals_str<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &mut Body<'tcx>,
) -> (Local, Local, Ty<'tcx>) {
    let span = body.span;

    let ty_str = tcx.mk_ty(TyKind::Str);
    let region = tcx.mk_region(RegionKind::ReErased);
    let ty_ref = tcx.mk_imm_ref(region, ty_str);

    let local_decl_2 = LocalDecl::new(ty_ref, span).immutable();
    let local_decl_3 = LocalDecl::new(ty_ref, span).immutable();

    let local_decls = &mut body.local_decls;

    let local_2 = local_decls.push(local_decl_2);
    let local_3 = local_decls.push(local_decl_3);

    (local_2, local_3, ty_ref)
}

pub fn insert_trace<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, name: &str) {
    let Some(def_id_trace_fn) = get_def_id_trace_fn(tcx) else {
        eprintln!("Crate {} will not be traced.", tcx.crate_name(LOCAL_CRATE));
        return;
    };

    let local_ret = insert_local_ret(tcx, body);
    let (local_1, local_2, ty_ref_str) = insert_locals_str(tcx, body);

    let content = name.as_bytes();
    let span = body.span;

    //*******************************************************
    // Create assign statements

    let place_elem_list = tcx.intern_place_elems(&[]);

    let (const_assign_statement_str, ref_assign_statement_str) = {
        let place_str = Place {
            local: local_2,
            projection: place_elem_list,
        };

        let new_allocation = Allocation::from_bytes_byte_aligned_immutable(content);
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
            source_info: SourceInfo::outermost(body.span),
            kind: StatementKind::Assign(Box::new((place_str, new_rvalue))),
        };

        let place_ref = Place {
            local: local_1,
            projection: place_elem_list,
        };

        let new_rvalue_ref = Rvalue::Use(Operand::Copy(place_str));

        let ref_assign_statement = Statement {
            source_info: SourceInfo::outermost(body.span),
            kind: StatementKind::Assign(Box::new((place_ref, new_rvalue_ref))),
        };

        (const_assign_statement, ref_assign_statement)
    };

    //*******************************************************
    // Create new basic block

    let index_vec = body.basic_blocks.as_mut();

    let first_basic_block_data = index_vec.raw.get(0).unwrap();

    // Clone former bb0
    let basic_block = index_vec.push(first_basic_block_data.clone());

    let func_subst = tcx.mk_substs([].iter());
    let func_ty = tcx.mk_ty(TyKind::FnDef(def_id_trace_fn, func_subst));
    let literal = ConstantKind::Val(ConstValue::ZeroSized, func_ty);

    let func_constant = Constant {
        span,
        user_ty: None,
        literal: literal,
    };
    let func_operand = Operand::Constant(Box::new(func_constant));

    let place_ref_str = Place {
        local: local_1,
        projection: place_elem_list,
    };

    let mut args_vec = Vec::new();
    args_vec.push(Operand::Move(place_ref_str));

    let place_ret = Place {
        local: local_ret,
        projection: place_elem_list,
    };

    let terminator_kind = TerminatorKind::Call {
        func: func_operand,
        args: args_vec,
        destination: place_ret,
        target: Some(basic_block),
        cleanup: None,
        from_hir_call: false,
        fn_span: span,
    };

    let terminator = Terminator {
        source_info: SourceInfo::outermost(span),
        kind: terminator_kind,
    };

    let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
    new_basic_block_data
        .statements
        .push(const_assign_statement_str);
    new_basic_block_data
        .statements
        .push(ref_assign_statement_str);

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

    let index_vec = body.basic_blocks.as_mut();

    let first_basic_block_data = index_vec.raw.get(0).unwrap();

    // Clone former bb0
    let basic_block = index_vec.push(first_basic_block_data.clone());

    let func_subst = tcx.mk_substs([].iter());
    let func_ty = tcx.mk_ty(TyKind::FnDef(def_id_pre_fn, func_subst));
    let literal = ConstantKind::Val(ConstValue::ZeroSized, func_ty);

    let func_constant = Constant {
        span,
        user_ty: None,
        literal,
    };
    let func_operand = Operand::Constant(Box::new(func_constant));

    let args_vec = Vec::new();

    let place_ret = Place {
        local: local_ret,
        projection: place_elem_list,
    };

    let terminator_kind = TerminatorKind::Call {
        func: func_operand,
        args: args_vec,
        destination: place_ret,
        target: Some(basic_block),
        cleanup: None,
        from_hir_call: false,
        fn_span: span,
    };

    let terminator = Terminator {
        source_info: SourceInfo::outermost(span),
        kind: terminator_kind,
    };

    let new_basic_block_data = BasicBlockData::new(Some(terminator));

    *body.basic_blocks.as_mut().raw.get_mut(0).unwrap() = new_basic_block_data;
}

pub fn insert_post<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, name: &str) {
    let Some(def_id_post_fn) = get_def_id_post_fn(tcx) else {
        return;
    };

    let bb_calling_test_fn = body.basic_blocks.raw.get(0).unwrap();

    let terminator_kind: &mut TerminatorKind =
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
        let (local_1, local_2, ty_ref_str) = insert_locals_str(tcx, body);

        let content = name.as_bytes();
        let span = body.span;

        //*******************************************************
        // Determine next bb in unwinding

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
        let place_str = Place {
            local: local_2,
            projection: place_elem_list,
        };

        let new_allocation = Allocation::from_bytes_byte_aligned_immutable(content);
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
            source_info: SourceInfo::outermost(body.span),
            kind: StatementKind::Assign(Box::new((place_str, new_rvalue))),
        };

        let place_ref = Place {
            local: local_1,
            projection: place_elem_list,
        };

        let new_rvalue_ref = Rvalue::Use(Operand::Copy(place_str));

        let ref_assign_statement = Statement {
            source_info: SourceInfo::outermost(body.span),
            kind: StatementKind::Assign(Box::new((place_ref, new_rvalue_ref))),
        };

        //*******************************************************
        // Create new basic block

        let func_subst = tcx.mk_substs([].iter());
        let func_ty = tcx.mk_ty(TyKind::FnDef(def_id_post_fn, func_subst));
        let literal = ConstantKind::Val(ConstValue::ZeroSized, func_ty);

        let func_constant = Constant {
            span,
            user_ty: None,
            literal: literal,
        };
        let func_operand = Operand::Constant(Box::new(func_constant));

        let mut args_vec = Vec::new();
        args_vec.push(Operand::Move(place_ref));

        let place_ret = Place {
            local: local_ret,
            projection: place_elem_list,
        };

        let terminator_kind = TerminatorKind::Call {
            func: func_operand,
            args: args_vec,
            destination: place_ret,
            target: Some(resume_bb), // the next cleanup bb is inserted here
            cleanup: None,
            from_hir_call: false,
            fn_span: span,
        };

        let terminator = Terminator {
            source_info: SourceInfo::outermost(span),
            kind: terminator_kind,
        };

        let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
        new_basic_block_data.is_cleanup = true;
        new_basic_block_data.statements.push(const_assign_statement);
        new_basic_block_data.statements.push(ref_assign_statement);

        let new_bb = body.basic_blocks.as_mut().push(new_basic_block_data);

        // here we insert the call to rustyrts_post_fn() into cleanup
        *cleanup = Some(new_bb);
    }

    let len = body.basic_blocks.raw.len();
    for i in 1..len {
        let terminator_kind = &body.basic_blocks.raw.get(i).unwrap().terminator().kind;

        if let TerminatorKind::Return = terminator_kind {
            let local_ret = insert_local_ret(tcx, body);
            let (local_1, local_2, ty_ref_str) = insert_locals_str(tcx, body);

            let content = name.as_bytes();
            let span = body.span;

            //*******************************************************
            // Create assign statements

            let place_elem_list = tcx.intern_place_elems(&[]);
            let place_str = Place {
                local: local_2,
                projection: place_elem_list,
            };

            let new_allocation = Allocation::from_bytes_byte_aligned_immutable(content);
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
                source_info: SourceInfo::outermost(body.span),
                kind: StatementKind::Assign(Box::new((place_str, new_rvalue))),
            };

            let place_ref = Place {
                local: local_1,
                projection: place_elem_list,
            };

            let new_rvalue_ref = Rvalue::Use(Operand::Copy(place_str));

            let ref_assign_statement = Statement {
                source_info: SourceInfo::outermost(body.span),
                kind: StatementKind::Assign(Box::new((place_ref, new_rvalue_ref))),
            };

            //*******************************************************
            // Create new basic block

            let index_vec = body.basic_blocks.as_mut();
            let old_basic_block = index_vec.raw.get(i).unwrap();

            // Clone former basic_block
            let basic_block = index_vec.push(old_basic_block.clone());

            let func_subst = tcx.mk_substs([].iter());
            let func_ty = tcx.mk_ty(TyKind::FnDef(def_id_post_fn, func_subst));
            let literal = ConstantKind::Val(ConstValue::ZeroSized, func_ty);

            let func_constant = Constant {
                span,
                user_ty: None,
                literal: literal,
            };
            let func_operand = Operand::Constant(Box::new(func_constant));

            let mut args_vec = Vec::new();
            args_vec.push(Operand::Move(place_ref));

            let place_ret = Place {
                local: local_ret,
                projection: place_elem_list,
            };

            let terminator_kind = TerminatorKind::Call {
                func: func_operand,
                args: args_vec,
                destination: place_ret,
                target: Some(basic_block),
                cleanup: None,
                from_hir_call: false,
                fn_span: span,
            };

            let terminator = Terminator {
                source_info: SourceInfo::outermost(span),
                kind: terminator_kind,
            };

            let mut new_basic_block_data = BasicBlockData::new(Some(terminator));
            new_basic_block_data.statements.push(const_assign_statement);
            new_basic_block_data.statements.push(ref_assign_statement);

            *body.basic_blocks.as_mut().raw.get_mut(i).unwrap() = new_basic_block_data;
        }
    }
}
