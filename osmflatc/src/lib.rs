//! Library surface of the `osmflatc` compiler.
//!
//! The binary (`src/main.rs`) drives the end-to-end pbf -> flatdata conversion,
//! but the reusable building blocks live here so that integration tests and
//! benchmarks (which can only see a crate's *library* API) can exercise the
//! internal processing stages -- e.g. serializing a single synthetic
//! `PrimitiveBlock` into the temporary RocksDB layout.

use std::collections::hash_map;
use std::io;
use std::str;

use ahash::AHashMap;
use indicatif::ProgressStyle;

pub mod args;
pub mod error;
pub mod osmpbf;
pub mod processing;
mod run;
pub mod stats;
pub mod strings;

pub use run::run;

use crate::error::OsmFlatcError;
use crate::strings::StringTable;

/// Boxed error type used throughout the compiler.
pub type Error = Box<dyn std::error::Error>;

/// Number of entities written to RocksDB between progress-bar updates.
pub const BATCH_SIZE: usize = 5000;

#[derive(PartialEq, Eq, Copy, Clone)]
struct I40 {
    x: [u8; 5],
}

impl I40 {
    fn from_u64(x: u64) -> Self {
        let x = x.to_le_bytes();
        debug_assert_eq!((x[5], x[6], x[7]), (0, 0, 0));
        Self {
            x: [x[0], x[1], x[2], x[3], x[4]],
        }
    }

    fn to_u64(self) -> u64 {
        let extented = [
            self.x[0], self.x[1], self.x[2], self.x[3], self.x[4], 0, 0, 0,
        ];
        u64::from_le_bytes(extented)
    }
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for I40 {
    fn hash<H>(&self, h: &mut H)
    where
        H: std::hash::Hasher,
    {
        // We manually implement Hash like this, since [u8; 5] is slower to hash
        // than u64 for some/many hash functions
        self.to_u64().hash(h)
    }
}

/// Holds tags external vector and deduplicates tags.
pub struct TagSerializer<'a> {
    tags: flatdata::ExternalVector<'a, osmflat::Tag>,
    tags_index: flatdata::ExternalVector<'a, osmflat::TagIndex>,
    dedup: AHashMap<(I40, I40), I40>, // deduplication table: (key_idx, val_idx) -> pos
}

impl<'a> TagSerializer<'a> {
    /// Start writing tags and the tag index into `builder`.
    pub fn new(builder: &'a osmflat::OsmBuilder) -> io::Result<Self> {
        Ok(Self {
            tags: builder.start_tags()?,
            tags_index: builder.start_tags_index()?,
            dedup: AHashMap::new(),
        })
    }

    /// Append a `(key_idx, value_idx)` tag, deduplicating identical tags.
    pub fn serialize(&mut self, key_idx: u64, val_idx: u64) -> Result<(), Error> {
        let idx = match self
            .dedup
            .entry((I40::from_u64(key_idx), I40::from_u64(val_idx)))
        {
            hash_map::Entry::Occupied(entry) => entry.get().to_u64(),
            hash_map::Entry::Vacant(entry) => {
                let idx = self.tags.len() as u64;
                let tag = self.tags.grow()?;
                tag.set_key_idx(key_idx);
                tag.set_value_idx(val_idx);
                entry.insert(I40::from_u64(idx));
                idx
            }
        };

        let tag_index = self.tags_index.grow()?;
        tag_index.set_value(idx);

        Ok(())
    }

    /// Index of the next tag-index entry that would be written.
    pub fn next_index(&self) -> u64 {
        self.tags_index.len() as u64
    }

    /// Finalize the tag vectors, panicking on a write error.
    pub fn close(self) {
        if let Err(e) = self.tags.close() {
            panic!("failed to close tags: {}", e);
        }
        if let Err(e) = self.tags_index.close() {
            panic!("failed to close tags index: {}", e);
        }
    }
}

/// adds all strings in a table to the lookup and returns a vectors of
/// references to be used instead
pub fn add_string_table(
    pbf_stringtable: &osmpbf::StringTable,
    stringtable: &mut StringTable,
) -> Result<Vec<u64>, OsmFlatcError> {
    let mut result = Vec::with_capacity(pbf_stringtable.s.len());
    for x in &pbf_stringtable.s {
        let string = str::from_utf8(x)?;
        result.push(stringtable.insert(string));
    }
    Ok(result)
}

/// Shared progress-bar style for the conversion stages.
pub fn pb_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:>24} [{bar:23}] {pos}/{len}: {per_sec} {elapsed}/{eta}")
        .unwrap()
        .progress_chars("=> ")
}
