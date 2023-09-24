use std::collections::HashSet;

use crate::constants::SUFFIX_DYN;
use crate::names::def_id_name;
use rustc_middle::ty::{GenericArg, InstanceDef, List, Ty, TyCtxt, TyKind};
use rustc_middle::{
    mir::visit::{TyContext, Visitor},
    ty::ParamEnv,
};
use rustc_middle::{mir::Body, ty::Instance};
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

    fn visit(&mut self, def_id: DefId, substs: &'tcx List<GenericArg<'tcx>>) {
        if self.visited.insert((def_id, substs)) {
            self.acc.insert(def_id_name(self.tcx, def_id, false, true));
            if self.tcx.is_mir_available(def_id) {
                let old_processed = self.processed;
                self.processed = (def_id, substs);

                let body = self.tcx.optimized_mir(def_id);
                self.visit_body(body);
                for body in self.tcx.promoted_mir(def_id) {
                    self.visit_body(body)
                }
                self.processed = old_processed;
            }
        }
    }
}

enum Dependency {
    Static,
    Dynamic,
    Drop,
    Contained,
}

impl<'tcx> Visitor<'tcx> for ResolvingVisitor<'tcx> {
    fn visit_ty(&mut self, ty: Ty<'tcx>, _ty_context: TyContext) {
        self.super_ty(ty);

        let (_def_id, outer_substs) = self.processed;

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
                                InstanceDef::Virtual(def_id, _)
                                | InstanceDef::ReifyShim(def_id) => {
                                    Some((def_id, substs, Dependency::Dynamic))
                                }
                                InstanceDef::FnPtrShim(def_id, ty) => {
                                    self.visit_ty(ty, _ty_context);
                                    Some((def_id, substs, Dependency::Static))
                                }
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

        if let Some((def_id, substs, Dependency::Dynamic)) = maybe_dependency {
            self.acc
                .insert(def_id_name(self.tcx, def_id, false, true) + SUFFIX_DYN);
            if let Some(impl_def) = self.tcx.impl_of_method(def_id) {
                if let Some(_) = self.tcx.impl_trait_ref(impl_def) {
                    let implementors = self.tcx.impl_item_implementor_ids(impl_def);
                    for (_trait_fn, impl_fn) in implementors {
                        if *impl_fn == def_id {
                            self.visit(*impl_fn, substs);
                        }
                    }
                }
            }
        }

        if let Some((def_id, substs, _dependency)) = maybe_dependency {
            self.visit(def_id, substs);
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
