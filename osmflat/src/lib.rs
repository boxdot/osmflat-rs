#![deny(missing_docs)]
#![allow(clippy::all)] // generated code is not clippy friendly

//! Flat OpenStreetMap (OSM) data format providing an efficient *random* data
//! access through [memory mapped files].
//!
//! The data format is described and implemented in [flatdata]. The [schema]
//! describes the fundamental OSM data structures: nodes, ways, relations and
//! tags as simple non-nested data structures. The relations between these are
//! expressed through indexes.
//!
//! ## Examples
//!
//! Open a flatdata archive (compiled from pbf with [`osmflatc`]) and iterate
//! through nodes:
//!
//! ```rust,no_run
//! use osmflat::{FileResourceStorage, Osm};
//!
//! fn main() {
//!     let storage = FileResourceStorage::new("path/to/archive.osm.flatdata");
//!     let archive = Osm::open(storage).unwrap();
//!
//!     for node in archive.nodes().iter() {
//!         println!("{:?}", node);
//!     }
//! }
//! ```
//!
//! For more examples, see the [examples] directory.
//!
//! [flatdata]: https://github.com/heremaps/flatdata
//! [schema]: https://github.com/boxdot/osmflat-rs/blob/master/flatdata/osm.flatdata
//! [memory mapped files]: https://en.wikipedia.org/wiki/Memory-mapped_file
//! [`osmflatc`]: https://github.com/boxdot/osmflat-rs/tree/master/osmflatc
//! [examples]: https://github.com/boxdot/osmflat-rs/tree/master/osmflat/examples

// generated osm module
include!("exp_gen.rs");

mod tags;

pub use crate::osm::*;
pub use crate::tags::*;

// re-export what is needed from flatdata to use osmflat
pub use flatdata::FileResourceStorage;
#[cfg(feature = "tar")]
pub use flatdata::TarArchiveResourceStorage;
