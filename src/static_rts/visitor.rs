use std::collections::HashSet;

use crate::constants::SUFFIX_DYN;
use crate::names::def_id_name;
use log::{info, warn};
use rustc_middle::mir::interpret::{AllocId, ConstValue, GlobalAlloc, Scalar};
use rustc_middle::{mir::Body, ty::Instance};
use rustc_middle::{
    mir::{
        visit::{TyContext, Visitor},
        ConstantKind,
    },
    ty::ParamEnv,
};
use rustc_middle::{
    mir::{Constant, Location},
    ty::{GenericArg, InstanceDef, List, Ty, TyCtxt, TyKind},
};
use rustc_span::def_id::DefId;

pub(crate) struct ResolvingVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
    acc: HashSet<String>,
    visited: HashSet<(DefId, &'tcx List<GenericArg<'tcx>>)>,
    processed: (DefId, &'tcx List<GenericArg<'tcx>>),
}

impl<'tcx, 'g> ResolvingVisitor<'tcx> {
    pub(crate) fn find_dependencies(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>) -> HashSet<String> {
        let def_id = body.source.def_id();
        let param_env = tcx.param_env(def_id).with_reveal_all_normalized(tcx);
        let mut resolver = ResolvingVisitor {
            tcx,
            param_env,
            acc: HashSet::new(),
            visited: HashSet::new(),
            processed: (def_id, List::identity_for_item(tcx, def_id)),
        };

        resolver.visit_body(body);
        for body in tcx.promoted_mir(def_id) {
            resolver.visit_body(body)
        }
        resolver.acc
    }

    fn visit(&mut self, def_id: DefId, substs: &'tcx List<GenericArg<'tcx>>, context: Context) {
        if self.visited.insert((def_id, substs)) {
            if let Context::CodeGen = context {
                self.acc.insert(def_id_name(self.tcx, def_id, false, true));
            }
            if self.tcx.is_mir_available(def_id) {
                let old_processed = self.processed;
                self.processed = (def_id, substs);

                let body = match context {
                    Context::CodeGen => self.tcx.optimized_mir(def_id),
                    Context::Static => self.tcx.mir_for_ctfe(def_id),
                };

                self.visit_body(body);
                for body in self.tcx.promoted_mir(def_id) {
                    self.visit_body(body)
                }
                self.processed = old_processed;
            }
        }
    }
}

enum Context {
    CodeGen,
    Static,
}

enum Dependency {
    Static,
    Dynamic,
    Drop,
    Contained,
}

impl<'tcx> Visitor<'tcx> for ResolvingVisitor<'tcx> {
    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        self.super_constant(constant, location);

        match constant.literal {
            ConstantKind::Ty(_) => {}
            ConstantKind::Unevaluated(..) => {}
            ConstantKind::Val(cons, _) => {
                let alloc_ids = match cons {
                    ConstValue::Scalar(Scalar::Ptr(ptr, ..)) => {
                        vec![ptr.provenance]
                    }
                    ConstValue::ByRef { alloc, offset: _ }
                    | ConstValue::Slice {
                        data: alloc,
                        start: _,
                        end: _,
                    } => alloc
                        .inner()
                        .provenance()
                        .provenances()
                        .collect::<Vec<AllocId>>(),
                    _ => vec![],
                };

                for alloc_id in alloc_ids {
                    match self.tcx.global_alloc(alloc_id) {
                        GlobalAlloc::Function(instance) => {
                            info!("Found fn ptr {:?}", instance);
                            self.visit(instance.def_id(), instance.substs, Context::CodeGen);
                        }
                        GlobalAlloc::Static(def_id) => {
                            self.visit(
                                def_id,
                                List::identity_for_item(self.tcx, def_id),
                                Context::Static,
                            );
                        }
                        _ => {}
                    }
                }
            }
        };
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);

        let (_outer_def_id, outer_substs) = self.processed;

        let maybe_dependency_drop = {
            ty.ty_adt_def().and_then(|adt_def| {
                self.tcx.adt_destructor(adt_def.did()).map(|destructor| {
                    (
                        destructor.did,
                        List::identity_for_item(self.tcx, destructor.did),
                        Dependency::Drop,
                    )
                })
            })
        };

        let maybe_dependency_other = {
            let maybe_normalized_ty = match *ty.kind() {
                TyKind::Closure(..) | TyKind::Generator(..) | TyKind::FnDef(..) => self
                    .tcx
                    .try_subst_and_normalize_erasing_regions(outer_substs, self.param_env, ty)
                    .ok(),
                _ => None,
            };

            maybe_normalized_ty.and_then(|ty| match *ty.kind() {
                TyKind::Closure(def_id, substs) => Some((def_id, substs, Dependency::Contained)),
                TyKind::Generator(def_id, substs, _) => {
                    Some((def_id, substs, Dependency::Contained))
                }
                TyKind::FnDef(def_id, substs) => {
                    match Instance::resolve(self.tcx, self.param_env, def_id, substs) {
                        Ok(Some(instance)) if !self.tcx.is_closure(instance.def_id()) => {
                            match instance.def {
                                InstanceDef::Item(item) => Some((
                                    item.def_id_for_type_of(),
                                    instance.substs,
                                    Dependency::Static,
                                )),
                                InstanceDef::Virtual(def_id, _) => {
                                    Some((def_id, substs, Dependency::Dynamic))
                                }
                                InstanceDef::ReifyShim(def_id) => Some((
                                    def_id,
                                    List::identity_for_item(self.tcx, def_id),
                                    Dependency::Static,
                                )),
                                InstanceDef::FnPtrShim(_def_id, ty) => match *ty.kind() {
                                    TyKind::FnDef(def_id, substs) => {
                                        let resolved = Instance::resolve(
                                            self.tcx,
                                            self.param_env,
                                            def_id,
                                            substs,
                                        );

                                        if let Ok(Some(instance)) = resolved {
                                            if let InstanceDef::Item(item) = instance.def {
                                                return Some((
                                                    item.def_id_for_type_of(),
                                                    instance.substs,
                                                    Dependency::Static,
                                                ));
                                            } else {
                                                warn!("Found something else {:?}", instance.def);
                                            }
                                        }
                                        None
                                    }
                                    _ => None,
                                },
                                InstanceDef::DropGlue(def_id, maybe_ty) => {
                                    if let Some(ty) = maybe_ty {
                                        self.visit_ty(ty, _ty_context);
                                    }
                                    Some((def_id, substs, Dependency::Static))
                                }
                                InstanceDef::CloneShim(def_id, ty) => {
                                    self.visit_ty(ty, _ty_context);
                                    Some((def_id, substs, Dependency::Static))
                                }

                                InstanceDef::Intrinsic(def_id)
                                | InstanceDef::VTableShim(def_id)
                                | InstanceDef::ClosureOnceShim {
                                    call_once: def_id,
                                    track_caller: _,
                                } => Some((def_id, substs, Dependency::Static)),
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            })
        };

        let maybe_dependency = maybe_dependency_other.or(maybe_dependency_drop);

        if let Some((def_id, _substs, Dependency::Dynamic)) = maybe_dependency {
            self.acc
                .insert(def_id_name(self.tcx, def_id, false, true) + SUFFIX_DYN);
            if let Some(trait_def) = self.tcx.trait_of_item(def_id) {
                let trait_impls = self.tcx.trait_impls_of(trait_def);

                let non_blanket_impls = trait_impls
                    .non_blanket_impls()
                    .values()
                    .flat_map(|impls| impls.iter());
                let blanket_impls = trait_impls.blanket_impls().iter();

                for impl_def in blanket_impls.chain(non_blanket_impls) {
                    let implementors = self.tcx.impl_item_implementor_ids(impl_def);
                    for (_trait_fn, impl_fn) in implementors {
                        self.visit(
                            *impl_fn,
                            List::identity_for_item(self.tcx, *impl_fn),
                            Context::CodeGen,
                        );
                    }
                }
            }
        }

        if let Some((def_id, substs, _dependency)) = maybe_dependency {
            self.visit(def_id, substs, Context::CodeGen);
        }
    }
}

// #[cfg(test)]
// mod test {
//     use itertools::Itertools;
//     use log::info;
//     use rustc_middle::mir::mono::MonoItem;
//     use std::{fs, io::Error, path::PathBuf, string::String};
//     use test_log::test;

//     use rustc_errors::registry;
//     use rustc_hash::{FxHashMap, FxHashSet};
//     use rustc_session::config::{self, CheckCfg, OptLevel};
//     use rustc_span::source_map;

//     use crate::constants::SUFFIX_DYN;
//     use crate::static_rts::graph::{DependencyGraph, EdgeType};

//     // use super::GraphVisitor;

//     const TEST_DATA_PATH: &str = "test-data/static/src";

//     fn load_test_code(file_name: &str) -> Result<String, Error> {
//         let mut path_buf = PathBuf::from(TEST_DATA_PATH);
//         path_buf.push(file_name);
//         fs::read_to_string(path_buf.as_path())
//     }

//     fn compile_and_visit(file_name: &str) -> DependencyGraph<String> {
//         let test_code = load_test_code(file_name).expect("Failed to load test code.");

//         let config = rustc_interface::Config {
//             opts: config::Options {
//                 test: true,
//                 optimize: OptLevel::No,
//                 ..config::Options::default()
//             },
//             crate_cfg: FxHashSet::default(),
//             crate_check_cfg: CheckCfg::default(),
//             input: config::Input::Str {
//                 name: source_map::FileName::Custom("main.rs".into()),
//                 input: test_code,
//             },
//             output_dir: None,
//             output_file: None,
//             file_loader: None,
//             lint_caps: FxHashMap::default(),
//             parse_sess_created: None,
//             register_lints: None,
//             override_queries: None,
//             registry: registry::Registry::new(&rustc_error_codes::DIAGNOSTICS),
//             make_codegen_backend: None,
//         };

//         rustc_interface::run_compiler(config, |compiler| {
//             compiler.enter(|queries| {
//                 queries.global_ctxt().unwrap().enter(|tcx| {
//                     let code_gen_units = tcx.collect_and_partition_mono_items(()).1;
//                     let bodies = code_gen_units
//                         .iter()
//                         .flat_map(|c| c.items().keys())
//                         .filter(|m| if let MonoItem::Fn(_) = m { true } else { false })
//                         .map(|m| {
//                             let MonoItem::Fn(instance) = m else {unreachable!()};
//                             instance
//                         })
//                         .filter(|i: &&rustc_middle::ty::Instance| tcx.is_mir_available(i.def_id()))
//                         .map(|i| (tcx.optimized_mir(i.def_id()), i.substs))
//                         .collect_vec();

//                     // let mut visitor = GraphVisitor::new(tcx, &mut graph);

//                     for (body, _substs) in bodies {
//                         // visitor.visit(body);
//                     }
//                 })
//             });
//         });
//     }

//     fn assert_contains_edge(
//         graph: &DependencyGraph<String>,
//         start: &str,
//         end: &str,
//         edge_type: &EdgeType,
//     ) {
//         let error_str = format!("Did not find edge {} -> {} ({:?})", start, end, edge_type);

//         let start = graph.get_nodes().iter().find(|s| **s == start).unwrap();

//         let end = graph.get_nodes().iter().find(|s| **s == end).unwrap();

//         let maybe_edges = graph.get_edges_to(end);
//         assert!(maybe_edges.is_some(), "{}", error_str);

//         let edges = maybe_edges.unwrap();
//         assert!(edges.contains_key(start), "{}", error_str);

//         let edge_types = edges.get(start).unwrap();
//         assert!(edge_types.contains(edge_type), "{}", error_str);
//     }

//     fn assert_does_not_contain_edge(
//         graph: &DependencyGraph<String>,
//         start: &str,
//         end: &str,
//         edge_type: &EdgeType,
//     ) {
//         let start = graph
//             .get_nodes()
//             .iter()
//             .find(|s| s.ends_with(start))
//             .unwrap();

//         let end = graph.get_nodes().iter().find(|s| s.ends_with(end)).unwrap();

//         let maybe_edges = graph.get_edges_to(end);
//         if maybe_edges.is_some() {
//             let edges = maybe_edges.unwrap();
//             if edges.contains_key(start) {
//                 let edge_types = edges.get(start).unwrap();
//                 assert!(
//                     !edge_types.contains(edge_type),
//                     "Found unexpected edge {} -> {} ({:?})",
//                     start,
//                     end,
//                     edge_type
//                 );
//             }
//         }
//     }

//     #[test]
//     fn test_function_call() {
//         let graph = compile_and_visit("call.rs");

//         let start = "rust_out::test";
//         let end = "rust_out::func";
//         let edge_type = EdgeType::FnDef;
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
//     }

//     #[test]
//     fn test_closure() {
//         let graph = compile_and_visit("closure.rs");

//         let start = "rust_out::test";
//         let end = "rust_out::test::{closure#0}";
//         let edge_type = EdgeType::Closure;
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
//     }

//     #[test]
//     fn test_fndef() {
//         let graph = compile_and_visit("fndef.rs");

//         let start = "rust_out::test_indirect";
//         let end = "rust_out::incr";
//         let edge_type = EdgeType::FnDef;
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

//         let start = "rust_out::test_higher_order";
//         let end = "rust_out::incr";
//         let edge_type = EdgeType::FnDef;
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
//     }

//     #[test]
//     fn test_impls() {
//         let graph = compile_and_visit("impls.rs");

//         let edge_type = EdgeType::FnDef;
//         let end: &str = "rust_out::Foo::new";

//         let start = "rust_out::test_static";
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

//         let start = "rust_out::test_const";
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);

//         let start = "rust_out::test_mut";
//         assert_contains_edge(&graph, &start, &end, &edge_type);
//         assert_does_not_contain_edge(&graph, &end, &start, &edge_type);
//     }

//     #[test]
//     fn test_traits() {
//         let graph = compile_and_visit("traits.rs");

//         println!("{}", graph.to_string());

//         {
//             let start = "rust_out::test_direct";
//             let end = "rust_out::<Lion as Animal>::sound";
//             assert_contains_edge(&graph, &start, &end, &EdgeType::FnDef);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDef);

//             // let start = "rust_out::sound_generic";
//             // let end = "rust_out::<Dog as Animal>::sound";
//             // assert_contains_edge(&graph, &start, &end, &EdgeType::FnDef);
//             // assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDef);

//             let start = "rust_out::sound_dyn";
//             let end = "rust_out::Animal::sound".to_owned() + SUFFIX_DYN;
//             assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefDyn);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefDyn);

//             let start = "rust_out::Animal::sound".to_owned() + SUFFIX_DYN;
//             let end = "rust_out::<Lion as Animal>::sound".to_owned() + SUFFIX_DYN;
//             assert_contains_edge(&graph, &start, &end, &EdgeType::TraitImpl);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::TraitImpl);

//             let start = "rust_out::Animal::walk".to_owned() + SUFFIX_DYN;
//             let end = "rust_out::Animal::walk";
//             assert_contains_edge(&graph, &start, &end, &EdgeType::DynFn);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::DynFn);
//         }

//         {
//             let start = "rust_out::test_mut_direct";
//             let end = "rust_out::<Lion as Animal>::set_treat";
//             assert_contains_edge(&graph, &start, &end, &EdgeType::FnDef);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDef);

//             // let start = "rust_out::set_treat_generic";
//             // let end = "rust_out::<Dog as Animal>::set_treat";
//             // assert_contains_edge(&graph, &start, &end, &EdgeType::FnDef);
//             // assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDef);

//             let start = "rust_out::set_treat_dyn";
//             let end = "rust_out::Animal::set_treat".to_owned() + SUFFIX_DYN;
//             assert_contains_edge(&graph, &start, &end, &EdgeType::FnDefDyn);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::FnDefDyn);

//             let start = "rust_out::Animal::set_treat".to_owned() + SUFFIX_DYN;
//             let end = "rust_out::<Dog as Animal>::set_treat".to_owned() + SUFFIX_DYN;
//             assert_contains_edge(&graph, &start, &end, &EdgeType::TraitImpl);
//             assert_does_not_contain_edge(&graph, &end, &start, &EdgeType::TraitImpl);
//         }
//     }
// }
