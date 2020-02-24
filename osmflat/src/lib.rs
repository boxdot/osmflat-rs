#[macro_use]
extern crate flatdata;

// re-export what is needed from flatdata to use osmflat
pub use flatdata::{Archive, ArchiveBuilder, FileResourceStorage};

mod osmflat;
mod tags;

pub use osmflat::*;
pub use tags::*;

/// Foobar
pub use osmflat::Osm;
