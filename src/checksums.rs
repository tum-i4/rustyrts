use crate::rustc_data_structures::stable_hasher::HashStable;
use lazy_static::lazy_static;
use regex::bytes::Regex;
use rustc_data_structures::stable_hasher::StableHasher;
use rustc_middle::mir::interpret::ConstAllocation;
use rustc_middle::mir::Body;
use rustc_middle::ty::{ScalarInt, TyCtxt, VtblEntry};
use rustc_span::sym;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::{
    collections::{HashMap, HashSet},
    hash::Hasher,
};

/// Wrapper of HashMap to provide serialization and deserialization of checksums
/// (Newtype Pattern)
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Checksums {
    inner: HashMap<String, HashSet<(u64, u64)>>, // key: name of node - value: checksum(s) of length 128 bit in two u64s
}

//##### Explanation of why we have multiple checksums per node:
// There may in fact be multiple bodies with the same def_path and different checksums
// (def_paths only differ in a disambiguator that is unfortunately NOT stable across compiler sessions)
// The only solution is to aggregate all available checksums

impl Checksums {
    pub fn new() -> Self {
        Checksums {
            inner: HashMap::new(),
        }
    }
}

impl Default for Checksums {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Checksums {
    type Target = HashMap<String, HashSet<(u64, u64)>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Checksums {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<&Checksums> for Vec<u8> {
    fn from(val: &Checksums) -> Self {
        let mut output = Vec::new();

        for (name, checksums) in &val.inner {
            for (first, second) in checksums {
                output.extend(name.as_bytes());
                output.extend(" - ".as_bytes());
                output.extend(Vec::from(first.to_ne_bytes()));
                output.extend(Vec::from(second.to_ne_bytes()));
                output.push(b'\n');
            }
        }
        output
    }
}

impl From<&[u8]> for Checksums {
    fn from(value: &[u8]) -> Self {
        let mut output = Self::new();

        lazy_static! {
            static ref REGEX_LINE: Regex = Regex::new(r"(?-u).* - [\s\S]{16}\n").unwrap();
        }

        REGEX_LINE
            .find_iter(value)
            .map(|m| m.as_bytes())
            .map(|line| {
                // spilt into name and checksum
                (
                    line.get(..line.len() - 20).unwrap(), // name ends 20 byte before end of line
                    line.get(line.len() - 17..line.len() - 1).unwrap(), // checksums begin 17 bytes before end + removing \n at the end
                )
            })
            .map(|(name, checksum)| (std::str::from_utf8(name).unwrap(), checksum.split_at(8))) // parse name and split checksum
            .map(|(name, (first_str, second_str))| {
                // Parse checksums from two [u8,8] in two u64s
                let first = u64::from_ne_bytes(first_str.try_into().unwrap());
                let second = u64::from_ne_bytes(second_str.try_into().unwrap());
                (name, (first, second))
            })
            .for_each(|(name, checksum)| {
                insert_hashmap(&mut output.inner, &name.to_string(), checksum);
            });
        output
    }
}

/// Function to obtain a stable checksum of a MIR body
pub(crate) fn get_checksum_body(tcx: TyCtxt<'_>, body: &Body) -> (u64, u64) {
    let mut hash = (0, 0);

    tcx.with_stable_hashing_context(|ref mut context| {
        // We use the hashing mechanism provided by the compiler to obtain a hash of a MIR body,
        // that is stable beyond the compiler session

        context.without_hir_bodies(|context| {
            context.while_hashing_spans(false, |context| {
                let mut hasher = StableHasher::new();
                if tcx
                    .get_attr(body.source.def_id(), sym::should_panic)
                    .is_some()
                {
                    hasher.write("should_panic".as_bytes());
                }
                if tcx.get_attr(body.source.def_id(), sym::ignore).is_some() {
                    hasher.write("ignore".as_bytes());
                }
                body.hash_stable(context, &mut hasher);
                hash = hasher.finalize();
            })
        });
    });

    hash
}

/// Function to obtain a stable checksum of a vtable entry
pub(crate) fn get_checksum_vtbl_entry<'tcx>(
    tcx: TyCtxt<'tcx>,
    entry: &VtblEntry<'tcx>,
) -> (u64, u64) {
    let mut hash = (0, 0);

    tcx.with_stable_hashing_context(|ref mut context| {
        // We use the hashing mechanism provided by the compiler to obtain a hash of a MIR body,
        // that is stable beyond the compiler session

        context.without_hir_bodies(|context| {
            context.while_hashing_spans(false, |context| {
                let mut hasher = StableHasher::new();
                entry.hash_stable(context, &mut hasher);
                hash = hasher.finalize();
            })
        });
    });

    hash
}

/// Function to obtain a stable checksum of a global alloc
pub(crate) fn get_checksum_const_allocation<'tcx>(
    tcx: TyCtxt<'tcx>,
    alloc: &ConstAllocation<'tcx>,
) -> (u64, u64) {
    let mut hash = (0, 0);

    tcx.with_stable_hashing_context(|ref mut context| {
        // We use the hashing mechanism provided by the compiler to obtain a hash of a MIR body,
        // that is stable beyond the compiler session

        context.without_hir_bodies(|context| {
            context.while_hashing_spans(false, |context| {
                let mut hasher = StableHasher::new();
                alloc.hash_stable(context, &mut hasher);
                hash = hasher.finalize();
            })
        });
    });

    hash
}

/// Function to obtain a stable checksum of a scalar int
pub(crate) fn get_checksum_scalar_int(tcx: TyCtxt<'_>, scalar_int: &ScalarInt) -> (u64, u64) {
    let mut hash = (0, 0);

    tcx.with_stable_hashing_context(|ref mut context| {
        // We use the hashing mechanism provided by the compiler to obtain a hash of a MIR body,
        // that is stable beyond the compiler session

        context.without_hir_bodies(|context| {
            context.while_hashing_spans(false, |context| {
                let mut hasher = StableHasher::new();
                scalar_int.hash_stable(context, &mut hasher);
                hash = hasher.finalize();
            })
        });
    });

    hash
}

pub(crate) fn insert_hashmap<K: Hash + Eq + Clone, V: Hash + Eq>(
    map: &mut HashMap<K, HashSet<V>>,
    key: &K,
    value: V,
) {
    if map.get(key).is_none() {
        map.insert(key.clone(), HashSet::new()).unwrap_or_default();
    }
    map.get_mut(key).unwrap().insert(value);
}

#[cfg(test)]
mod test {

    use super::Checksums;
    use crate::checksums::insert_hashmap;

    #[test]
    pub fn test_checksum_deserialization() {
        let mut checksums = Checksums::new();

        insert_hashmap(&mut checksums, &"node1".to_string(), (100000000000006, 0));
        insert_hashmap(&mut checksums, &"node2".to_string(), (2, 100000000000005));
        insert_hashmap(&mut checksums, &"node3".to_string(), (3, 100000000000004));
        insert_hashmap(&mut checksums, &"node4".to_string(), (4, 100000000000003));
        insert_hashmap(&mut checksums, &"node5".to_string(), (5, u64::MAX - 1));
        insert_hashmap(&mut checksums, &"node6".to_string(), (6, u64::MAX));

        let serialized: Vec<u8> = (&checksums).into();
        let deserialized = Checksums::from(serialized.as_slice());

        assert_eq!(checksums, deserialized);
    }

    #[test]
    pub fn test_checksum_deserialization_empty() {
        let checksums = Checksums::new();

        let serialized: Vec<u8> = (&checksums).into();
        let deserialized = Checksums::from(serialized.as_slice());

        assert_eq!(checksums, deserialized);
    }
}
