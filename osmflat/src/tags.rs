//! All functions in this module operate on raw bytes for performance reasons.
//! It is easy to combine these with `std::str::from_utf8` family of functions,
//! to lift them to operate on `str`.

use crate::Osm;
use std::ops::Range;

/// Returns an iterator over tags specified by `range`.
///
/// When searching for a tag by key consider to use `find_tag` which
/// performs better.
#[inline]
pub fn iter_tags(archive: &Osm, range: Range<u64>) -> impl Iterator<Item = (&[u8], &[u8])> + Clone {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    range.map(move |idx| {
        let tag = &tags[tags_index[idx as usize].value() as usize];
        let key = strings.substring_raw(tag.key_idx() as usize);
        let val = strings.substring_raw(tag.value_idx() as usize);
        (key, val)
    })
}

/// Finds the first tag in the given `range` which satisfies the predicate
/// applied to the key and value and returns the corresponding value.
///
/// Note that the predicate function is called on the whole key block and value
/// block. These are zero (`\0`) divided blocks of bytes that start at the key
/// resp. value, and contain the rest string data. In particular, the len of
/// the block is *not* the len of the key resp. value. The user is responsible
/// to check or find the zero terminator.
#[inline]
pub fn find_tag_by(
    archive: &Osm,
    mut range: Range<u64>,
    mut predicate: impl FnMut(&[u8], &[u8]) -> bool,
) -> Option<&[u8]> {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    range.find_map(move |idx| {
        let tag = &tags[tags_index[idx as usize].value() as usize];
        let key_block = &strings.as_bytes()[tag.key_idx() as usize..];
        let value_block = &strings.as_bytes()[tag.value_idx() as usize..];
        if predicate(key_block, value_block) {
            Some(strings.substring_raw(tag.value_idx() as usize))
        } else {
            None
        }
    })
}

/// Finds a tag by its key in the given `range` and returns the corresponding
/// value.
#[inline]
pub fn find_tag<'a>(archive: &'a Osm, range: Range<u64>, key: &[u8]) -> Option<&'a [u8]> {
    find_tag_by(archive, range, |key_block, _| {
        key_block.starts_with(key) && *key_block.get(key.len()).unwrap_or(&0) == 0
    })
}

/// Checks if there is a tag in `range` with a given `key` and `value`.
#[inline]
pub fn has_tag(archive: &Osm, range: Range<u64>, key: &[u8], value: &[u8]) -> bool {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    let matches = |idx, value| {
        let block = &strings.as_bytes()[idx as usize..];
        block.starts_with(value) && *block.get(value.len()).unwrap_or(&0) == 0
    };

    for idx in range {
        let tag = &tags[tags_index[idx as usize].value() as usize];
        if matches(tag.key_idx(), key) {
            return matches(tag.value_idx(), value);
        }
    }
    false
}
