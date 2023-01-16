use ahash::AHashMap;

#[derive(Debug, Clone, Copy)]
struct TerminatedStringPtr {
    ptr: *const u8,
}

// We use this (unsafe) wrapper to get the most compact hashmap possible
// Using a "&'static str" would be bigger due to the length stored
// Using a String would allocate a lot of individual blocks
// Using a small-string-optimized structure would create large objects
impl TerminatedStringPtr {
    /// Safety:
    /// Requires the data pointed to to:
    /// * Be \0 terminated
    /// * Outlive TerminatedStringPtr
    unsafe fn from_ptr(ptr: *const u8) -> Self {
        Self { ptr }
    }

    fn as_bytes(&self) -> &[u8] {
        // Safety:
        // If constructed properly from a 0-terminated string that outlives this instance this is safe
        unsafe { std::ffi::CStr::from_ptr(self.ptr as *const i8).to_bytes() }
    }
}

impl PartialEq for TerminatedStringPtr {
    fn eq(&self, other: &TerminatedStringPtr) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl std::hash::Hash for TerminatedStringPtr {
    fn hash<H>(&self, h: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.as_bytes().hash(h)
    }
}

impl Eq for TerminatedStringPtr {}

impl std::borrow::Borrow<[u8]> for TerminatedStringPtr {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[derive(Debug, Default)]
pub struct StringTable {
    // Append only, we will never reallocate any data inside
    data: Vec<Vec<u8>>,

    // The hashmap references strings in the data block
    // Since we cannot prove to the compiler that the strings
    // will be "alive" long enough we have to manage lifetime ourselves
    indexed_data: AHashMap<TerminatedStringPtr, u64>,

    size_in_bytes: u64,
}

impl StringTable {
    pub fn new() -> Self {
        Default::default()
    }

    /// Inserts a string into string table and returns its index.
    ///
    /// If the string was already inserted before, the string is deduplicated
    /// and the index to the previous string is returned.
    pub fn insert(&mut self, s: &str) -> u64 {
        // Horrible news, we cannot use entry API since it does not support Borrow
        // See: https://github.com/rust-lang/rust/issues/56167
        if let Some(&idx) = self.indexed_data.get(s.as_bytes()) {
            return idx;
        }

        let idx = self.size_in_bytes;
        if self
            .data
            .last()
            .filter(|x| x.len() + s.len() < x.capacity()) // str-len + \0
            .is_none()
        {
            self.data
                .push(Vec::with_capacity((1024 * 1024 * 4).max(s.len() + 1)));
        }
        // unwrap is ok here, since we just ensured that there is always one entry
        let buffer = self.data.last_mut().unwrap();
        let pos = buffer.len();
        let ptr_before = buffer.as_ptr();
        buffer.extend(s.as_bytes());
        buffer.push(0);
        // Safety: We must never reallocate the buffer
        debug_assert_eq!(ptr_before, buffer.as_ptr());
        let key = unsafe {
            // convert back to str (safe since we know that it is valid UTF, it was created from a str)
            let key: &str = std::str::from_utf8_unchecked(&buffer[pos..]);
            // safe since we make sure to never reallocate/free any buffer
            let key_ptr = key.as_ptr();
            TerminatedStringPtr::from_ptr(key_ptr)
        };
        self.indexed_data.insert(key, idx);

        self.size_in_bytes += s.len() as u64 + 1;
        idx
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let Self {
            data,
            indexed_data,
            size_in_bytes,
        } = self;
        std::mem::drop(indexed_data);

        let mut result = Vec::new();
        result.reserve(size_in_bytes as usize);
        for buffer in data {
            result.extend(buffer); // also drops buffer
        }
        result
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

    #[test]
    fn test_large_insert() {
        let mut st = StringTable::new();
        assert_eq!(st.insert("hello"), 0);
        assert_eq!(st.insert(&str::repeat("x", 1024 * 1024 * 5)), 6);
        assert_eq!(st.insert("huh"), 1024 * 1024 * 5 + 1 + 6);
        assert_eq!(st.insert(&str::repeat("x", 1024 * 1024 * 5)), 6);
        assert_eq!(st.insert("hello"), 0);

        let bytes = st.into_bytes();
        assert_eq!(
            bytes,
            ("hello\0".to_string() + &str::repeat("x", 1024 * 1024 * 5) + "\0huh\0").as_bytes()
        );
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
        fn sequence_of_insert(ref seq in prop::collection::vec("[^\x00]*", 1..100))
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
