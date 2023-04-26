use crate::rustc_data_structures::stable_hasher::HashStable;
use lazy_static::lazy_static;
use regex::bytes::Regex;
use rustc_data_structures::stable_hasher::StableHasher;
use rustc_middle::{mir::Body, ty::TyCtxt};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// Wrapper of HashMap to provide serialization and deserialization of checksums
#[derive(Eq, PartialEq, Debug)]
pub(crate) struct Checksums {
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

    pub fn inner(&self) -> &HashMap<String, HashSet<(u64, u64)>> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut HashMap<String, HashSet<(u64, u64)>> {
        &mut self.inner
    }
}

impl ToString for Checksums {
    fn to_string(&self) -> String {
        let mut output = String::new();

        for (name, checksums) in &self.inner {
            for (first, second) in checksums {
                output += &format!(
                    "{} - {}{}\n",
                    name,
                    // SAFETY: This is intentional. Checksums are not necessarily valid utf8.
                    unsafe { String::from_utf8_unchecked(Vec::from(first.to_ne_bytes())) },
                    unsafe { String::from_utf8_unchecked(Vec::from(second.to_ne_bytes())) },
                );
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
            .into_iter()
            .map(|m| m.as_bytes())
            .map(|line| {
                // spilt into name and checksum
                (
                    line.get(..line.len() - 20).unwrap(), // name ends 20 byte before end of line
                    line.get(line.len() - 17..line.len() - 1).unwrap(), // checksums begin 17 bytes before end + removing \n at the end
                )
            })
            .map(|(name, checksum)| (std::str::from_utf8(&name).unwrap(), checksum.split_at(8))) // parse name and split checksum
            .map(|(name, (first_str, second_str))| {
                // Parse checksums from two [u8,8] in two u64s
                let first = u64::from_ne_bytes(first_str.try_into().unwrap());
                let second = u64::from_ne_bytes(second_str.try_into().unwrap());
                (name, (first, second))
            })
            .for_each(|(name, checksum)| {
                insert_hashmap(&mut output.inner, name.to_string(), checksum);
            });
        output
    }
}

/// Function to obtain a stable checksum of a MIR body
pub(crate) fn get_checksum<'tcx>(tcx: TyCtxt<'tcx>, body: &Body) -> (u64, u64) {
    let mut hash = (0, 0);

    tcx.with_stable_hashing_context(|ref mut context| {
        // We use the hashing mechanism provided by the compiler to obtain a hash of a MIR body,
        // that is stable beyond the compiler session

        context.without_hir_bodies(|context| {
            context.while_hashing_spans(false, |context| {
                let mut hasher = StableHasher::new();
                body.hash_stable(context, &mut hasher);
                hash = hasher.finalize();
            })
        });
    });

    hash
}

pub(crate) fn insert_hashmap<K: Hash + Eq + Clone, V: Hash + Eq>(
    map: &mut HashMap<K, HashSet<V>>,
    key: K,
    value: V,
) {
    if let None = map.get(&key) {
        map.insert(key.clone(), HashSet::new()).unwrap_or_default();
    }
    map.get_mut(&key).unwrap().insert(value);
}

#[cfg(test)]
mod teest {

    use crate::checksums::insert_hashmap;

    use super::Checksums;
    use test_log::test;

    #[test]
    pub fn test_checksum_deserialization() {
        let mut checksums = Checksums::new();
        let mut inner = checksums.inner_mut();

        insert_hashmap(&mut inner, "node1".to_string(), (100000000000006, 0));
        insert_hashmap(&mut inner, "node2".to_string(), (2, 100000000000005));
        insert_hashmap(&mut inner, "node3".to_string(), (3, 100000000000004));
        insert_hashmap(&mut inner, "node4".to_string(), (4, 100000000000003));
        insert_hashmap(&mut inner, "node5".to_string(), (5, u64::MAX - 1));
        insert_hashmap(&mut inner, "node6".to_string(), (6, u64::MAX));

        let serialized = checksums.to_string();
        let deserialized = Checksums::from(serialized.as_bytes());

        assert_eq!(checksums, deserialized);
    }

    #[test]
    pub fn test_checksum_deserialization_empty() {
        let checksums = Checksums::new();

        let serialized = checksums.to_string();
        let deserialized = Checksums::from(serialized.as_bytes());

        assert_eq!(checksums, deserialized);
    }
}
