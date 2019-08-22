#[macro_use]
extern crate flatdata;

// re-export what is needed from flatdata to use osmflat
pub use flatdata::{Archive, FileResourceStorage};

mod osmflat;

pub use crate::osmflat::*;

/// Helper function to iterate through tags from osmflat.
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
