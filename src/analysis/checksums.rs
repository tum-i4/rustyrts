use lazy_static::lazy_static;
use regex::bytes::Regex;
use std::collections::HashMap;

/// Wrapper of HashMap to provide serialisation and deserialisation of checksums
#[derive(Eq, PartialEq, Debug)]
pub(crate) struct Checksums {
    inner: HashMap<String, (u64, u64)>, // key: name of node - value: checksum of length 128 bit in two u64s
}

impl Checksums {
    pub fn new() -> Self {
        Checksums {
            inner: HashMap::new(),
        }
    }

    pub fn inner(&mut self) -> &mut HashMap<String, (u64, u64)> {
        &mut self.inner
    }
}

impl ToString for Checksums {
    fn to_string(&self) -> String {
        let mut output = String::new();

        for (name, (first, second)) in &self.inner {
            output += &format!(
                "{} - {}{}\n",
                name,
                unsafe { String::from_utf8_unchecked(Vec::from(first.to_ne_bytes())) },
                unsafe { String::from_utf8_unchecked(Vec::from(second.to_ne_bytes())) },
            );
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
                output.inner.insert(name.to_string(), checksum);
            });
        output
    }
}

#[cfg(test)]
mod teest {
    use regex::bytes::Regex;
    use std::str::FromStr;
    use test_log::test;

    use log::debug;

    use super::Checksums;

    #[test]
    pub fn test_checksum_deserialization() {
        let mut checksums = Checksums::new();
        let inner = checksums.inner();

        inner.insert("node1".to_string(), (100000000000006, 0));
        inner.insert("node2".to_string(), (2, 100000000000005));
        inner.insert("node3".to_string(), (3, 100000000000004));
        inner.insert("node4".to_string(), (4, 100000000000003));
        inner.insert("node5".to_string(), (5, u64::MAX - 1));
        inner.insert("node6".to_string(), (6, u64::MAX));

        let serialized = checksums.to_string();
        let deserialized = Checksums::from(serialized.as_bytes());

        assert_eq!(checksums, deserialized);
    }
}
