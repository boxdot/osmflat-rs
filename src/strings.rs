use std::collections::{hash_map, HashMap};

#[derive(Debug, Default)]
pub struct StringTable {
    indexed_data: HashMap<String, u64>,
    contiguous_data: Vec<(String, u64)>,
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
    pub fn insert<S: Into<String>>(&mut self, s: S) -> u64 {
        // Horrible news, we need to clone s, even we would not need to:
        // https://github.com/rust-lang/rust/pull/50821
        let s = s.into();
        let s_len = s.len();
        match self.indexed_data.entry(s) {
            hash_map::Entry::Occupied(entry) => *entry.into_mut(),
            hash_map::Entry::Vacant(entry) => {
                let idx = *entry.insert(self.size_in_bytes);
                self.size_in_bytes = self.size_in_bytes + s_len as u64 + 1;
                idx
            }
        }
    }

    /// Pushes a string to the end of string table and returns its index.
    ///
    /// The string is always pushed into the string table regardless of it was
    /// already inserted or not. Use this method for creating contiguous
    /// sequences of strings.
    pub fn push<S: Into<String>>(&mut self, s: S) -> u64 {
        let idx = self.size_in_bytes;
        let s: String = s.into();
        self.size_in_bytes += s.len() as u64 + 1;
        self.indexed_data.entry(s.clone()).or_insert(idx);
        self.contiguous_data.push((s, idx));
        idx as u64
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let Self {
            indexed_data,
            contiguous_data,
            size_in_bytes,
            ..
        } = self;

        let mut index: Vec<(&String, &u64)> = indexed_data.iter().collect();
        index.reserve(contiguous_data.len());
        for (s, idx) in &contiguous_data {
            index.push((s, idx));
        }
        index.sort_by_key(|(_, &idx)| idx);
        index.dedup_by_key(|(_, &idx)| idx);

        let mut data = Vec::new();
        data.reserve(size_in_bytes as usize);
        for (s, _) in index {
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
    fn test_simple_insert_push() {
        let mut st = StringTable::new();
        assert_eq!(st.push("hello"), 0);
        assert_eq!(st.insert("world"), 6);
        assert_eq!(st.insert("world"), 6);
        assert_eq!(st.push("!"), 6 + 6);
        assert_eq!(st.insert("!"), 6 + 6);
        assert_eq!(st.insert("!"), 6 + 6);

        let bytes = st.into_bytes();
        println!("{}", ::std::str::from_utf8(&bytes).unwrap());
        assert_eq!(bytes, b"hello\0world\0!\0");
    }

    proptest! {
        #[test]
        fn test_push(ref v in prop::collection::vec(".*", 1..100)) {
            let mut st = StringTable::new();
            let mut index = 0;
            for elt in v {
                assert_eq!(st.push(elt.clone()), index as u64);
                index += elt.len() + 1;
            }
            let bytes = st.into_bytes();
            let original_bytes: Vec<u8> = v.clone()
                .into_iter()
                .map(|s| s.into_bytes())
                .fold(Vec::new(), |mut acc, elt| {
                    acc.extend(elt);
                    acc.push(b'\0');
                    acc
                });
            assert_eq!(bytes, original_bytes);
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StringTableOp {
        Push,
        Insert,
    }

    #[derive(Debug, Default)]
    struct ReferenceStringTable {
        words: HashSet<String>,
        data: Vec<u8>,
    }

    impl ReferenceStringTable {
        fn push(&mut self, input: String) {
            self.data.extend(input.as_bytes());
            self.data.push(b'\0');
            self.words.insert(input);
        }

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
        fn sequence_of_insert_push(
            ref seq in prop::collection::vec(
                (prop::sample::select(
                    vec![StringTableOp::Push, StringTableOp::Insert]),
                "\\PC*") , 1..100
            )
        )
        {
            let mut st = StringTable::new();
            let mut reference_st = ReferenceStringTable::default();
            for (op, input) in seq {
                match *op {
                    StringTableOp::Push => {
                        st.push(input.clone());
                        reference_st.push(input.clone());
                    }
                    StringTableOp::Insert => {
                        st.insert(input.clone());
                        reference_st.insert(input.clone());
                    }
                }
            }
            assert_eq!(st.into_bytes(), reference_st.data);
        }
    }
}
