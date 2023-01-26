use std::{collections::HashMap, str::FromStr};

/// Wrapper of HashMap to provide serialisation and deserialisation of checksums
pub(crate) struct Checksums {
    inner: HashMap<String, (u64, u64)>,
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
            output += &format!("{} - {},{}\n", name, first, second);
        }
        output
    }
}

impl FromStr for Checksums {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut output = Self::new();

        s.lines()
            .into_iter()
            .filter(|line| *line != "")
            .map(|line| line.split_once(" - "))
            .filter(|maybe| maybe.is_some())
            .map(|maybe| maybe.unwrap())
            .map(|(name, checksum)| {
                let maybe = checksum.split_once(",");
                if let Some((first, second)) = maybe {
                    return Some((name, (first, second)));
                } else {
                    return None;
                }
            })
            .filter(|maybe| maybe.is_some())
            .map(|maybe| maybe.unwrap())
            .map(|(name, (first_str, second_str))| {
                let first = first_str.parse();
                let second = second_str.parse();

                if first.is_ok() && second.is_ok() {
                    Some((name, (first.unwrap(), second.unwrap())))
                } else {
                    None
                }
            })
            .filter(|maybe| maybe.is_some())
            .map(|maybe| maybe.unwrap())
            .for_each(|(name, checksum)| {
                output.inner.insert(name.to_string(), checksum);
            });

        Ok(output)
    }
}
