#![allow(dead_code)]

// The purpose of this crate is, to verify that static RustyRTS can handle calls to functions via indirection over std
// This ensures that the custom sysroot used by static rustyrts is working correctly

use std::hash::{Hash, Hasher};
use std::{fmt::Debug, hash::DefaultHasher};

struct Inner {
    data: u32,
}

#[derive(Debug, Hash)]
struct Outer {
    data: Inner,
}

impl Debug for Inner {
    #[cfg(not(feature = "changes_debug"))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Inner")
    }

    #[cfg(feature = "changes_debug")]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Unexpected")
    }
}

impl Hash for Inner {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write("Foo".as_bytes());

        #[cfg(not(feature = "changes_hash"))]
        state.write_u32(self.data);
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug() {
        let sut = Outer {
            data: Inner { data: 1 },
        };
        assert_eq!(format!("{:?}", sut), "Outer { data: Inner }")
    }

    #[test]
    fn test_hash() {
        let sut1 = Outer {
            data: Inner { data: 1 },
        };

        let sut2 = Outer {
            data: Inner { data: 2 },
        };

        assert_ne!(calculate_hash(&sut1), calculate_hash(&sut2));
    }
}
