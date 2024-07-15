#![allow(dead_code)]

use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash, RandomState},
    ops::Index,
    sync::Arc,
};

//#####################################################################################################################
// Source:https://docs.rs/fn-cache/1.1.1/src/fn_cache/hash_cache.rs.html
// Changed: removed requirement for f to implement Send and Sync
//#####################################################################################################################

pub struct HashCache<'f, I, O, S = RandomState>
where
    I: Eq + Hash,
{
    pub(crate) cache: HashMap<I, O, S>,
    f: Arc<dyn Fn(&mut Self, &I) -> O + 'f>,
}

impl<'f, I, O, S> HashCache<'f, I, O, S>
where
    I: Eq + Hash,
    S: BuildHasher,
{
    // Changed visibility from private to crate public
    pub(crate) fn get(&mut self, input: I) -> &O {
        if self.cache.contains_key(&input) {
            self.cache.index(&input)
        } else {
            let output = self.compute(&input);
            self.cache.entry(input).or_insert(output)
        }
    }
}

impl<'f, I, O> HashCache<'f, I, O, RandomState>
where
    I: Eq + Hash,
{
    /// Create a cache for the provided function. If the
    /// function stores references, the cache can only
    /// live as long as those references.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&I) -> O + 'f,
    {
        Self::recursive(move |_, i| f(i))
    }

    /// Create a cache for the provided recursive function.
    /// If the function stores references, the cache can
    /// only live as long as those references.
    pub fn recursive<F>(f: F) -> Self
    where
        F: Fn(&mut Self, &I) -> O + 'f,
    {
        HashCache {
            cache: HashMap::default(),
            f: Arc::new(f),
        }
    }
}

impl<'f, I, O, S> HashCache<'f, I, O, S>
where
    I: Eq + Hash,
    S: BuildHasher,
{
    fn compute(&mut self, input: &I) -> O {
        (self.f.clone())(self, input)
    }
}
