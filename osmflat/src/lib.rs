#[macro_use]
extern crate flatdata;

// re-export what is needed from flatdata to use osmflat
pub use flatdata::{Archive, ArchiveBuilder, FileResourceStorage};

mod osmflat;

pub use crate::osmflat::*;

/// Helper function to iterate through tags from osmflat.
#[inline]
pub fn tags<'a>(
    archive: &'a osmflat::Osm,
    range: std::ops::Range<u64>,
) -> impl Iterator<Item = Result<(&'a str, &'a str), std::str::Utf8Error>> + 'a + Clone {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    range.map(move |idx| {
        let tag = tags.at(tags_index.at(idx as usize).value() as usize);
        let key = strings.substring(tag.key_idx() as usize)?;
        let val = strings.substring(tag.value_idx() as usize)?;
        Ok((key, val))
    })
}

/// Helper function to iterate through tags from osmflat.
#[inline]
pub fn tags_raw<'a>(
    archive: &'a osmflat::Osm,
    range: std::ops::Range<u64>,
) -> impl Iterator<Item = (&'a [u8], &'a [u8])> + 'a + Clone {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    range.map(move |idx| {
        let tag = tags.at(tags_index.at(idx as usize).value() as usize);
        let key = strings.substring_raw(tag.key_idx() as usize);
        let val = strings.substring_raw(tag.value_idx() as usize);
        (key, val)
    })
}

/// Helper function to iterate through tags from osmflat.
#[inline]
pub fn get_tag_raw<'a>(
    archive: &'a osmflat::Osm,
    mut range: std::ops::Range<u64>,
    key: &[u8],
) -> Option<&'a [u8]> {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    range.find_map(move |idx| {
        let tag = tags.at(tags_index.at(idx as usize).value() as usize);
        let key_block = &strings.as_bytes()[tag.key_idx() as usize..];
        if key_block.starts_with(key) && *key_block.get(key.len()).unwrap_or(&0) == 0 {
            Some(strings.substring_raw(tag.value_idx() as usize))
        } else {
            None
        }
    })
}

/// Helper function to iterate through tags from osmflat.
#[inline]
pub fn get_tag<'a>(
    archive: &'a osmflat::Osm,
    range: std::ops::Range<u64>,
    key: &[u8],
) -> Result<Option<&'a str>, std::str::Utf8Error> {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    for idx in range {
        let tag = tags.at(tags_index.at(idx as usize).value() as usize);
        let key_block = &strings.as_bytes()[tag.key_idx() as usize..];
        if key_block.starts_with(key) && *key_block.get(key.len()).unwrap_or(&0) == 0 {
            return Ok(Some(strings.substring(tag.value_idx() as usize)?));
        }
    }
    Ok(None)
}

#[inline]
pub fn tag_matches<'a>(
    archive: &'a osmflat::Osm,
    range: std::ops::Range<u64>,
    key: &[u8],
    value: &[u8],
) -> bool {
    let tags = archive.tags();
    let tags_index = archive.tags_index();
    let strings = archive.stringtable();

    let matches = |idx, value| {
        let block = &strings.as_bytes()[idx as usize..];
        return block.starts_with(value) && *block.get(value.len()).unwrap_or(&0) == 0;
    };

    for idx in range {
        let tag = tags.at(tags_index.at(idx as usize).value() as usize);
        if matches(tag.key_idx(), key) {
            return matches(tag.value_idx(), value);
        }
    }
    return false;
}
