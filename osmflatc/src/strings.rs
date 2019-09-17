use inlinable_string::{InlinableString, StringExt};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct StringTable {
    indexed_data: HashMap<InlinableString, u64>,
    size_in_bytes: u64,
}

impl StringTable {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn next_index(&self) -> u64 {
        self.size_in_bytes
    }

    /// Inserts a string into string table and returns its index.
    ///
    /// If the string was already inserted before, the string is deduplicated
    /// and the index to the previous string is returned.
    pub fn insert(&mut self, s: &str) -> u64 {
        // Horrible news, we cannot use entry API since it does not support Borrow
        // See: https://github.com/rust-lang/rust/issues/56167
        if let Some(&idx) = self.indexed_data.get(s) {
            return idx;
        }

        let idx = self.size_in_bytes;
        self.indexed_data.insert(s.into(), idx);
        self.size_in_bytes += s.len() as u64 + 1;
        idx
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut index: Vec<(&InlinableString, &u64)> = self.indexed_data.iter().collect();
        index.sort_by_key(|(_, &idx)| idx);

        let mut data = Vec::new();
        data.reserve(self.size_in_bytes as usize);
        for (s, &idx) in index {
            assert!(data.len() as u64 == idx);
            data.extend(s.as_bytes());
            data.push(b'\0');
        }
        data
    }
}

#[cfg(test)]
mod test {
    use super::StringTable;
    use proptest::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn test_simple_insert() {
        let mut st = StringTable::new();
        assert_eq!(st.insert("hello"), 0);
        assert_eq!(st.insert("world"), 6);
        assert_eq!(st.insert("world"), 6);
        assert_eq!(st.insert("!"), 6 + 6);
        assert_eq!(st.insert("!"), 6 + 6);
        assert_eq!(st.insert("!"), 6 + 6);

        let bytes = st.into_bytes();
        println!("{}", ::std::str::from_utf8(&bytes).unwrap());
        assert_eq!(bytes, b"hello\0world\0!\0");
    }

    #[derive(Debug, Default)]
    struct ReferenceStringTable {
        words: HashSet<String>,
        data: Vec<u8>,
    }

    impl ReferenceStringTable {
        fn insert(&mut self, input: String) {
            if !self.words.contains(&input) {
                self.words.insert(input.clone());
                self.data.extend(input.as_bytes());
                self.data.push(b'\0');
            }
        }
    }

    proptest! {
        #[test]
        fn sequence_of_insert(ref seq in prop::collection::vec(".*", 1..100))
        {
            let mut st = StringTable::new();
            let mut reference_st = ReferenceStringTable::default();
            for input in seq {
                st.insert(input);
                reference_st.insert(input.into());
            }
            assert_eq!(st.into_bytes(), reference_st.data);
        }
    }
}
