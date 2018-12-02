#[macro_use]
extern crate flatdata;

// re-export what is needed from flatdata to use osmflat
pub use flatdata::{Archive, FileResourceStorage};

mod osmflat;

pub use osmflat::*;
