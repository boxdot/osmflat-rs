//! Dumps the contents of the input archive in a debug format.
//!
//! Demonstrates
//!
//! * iteration through all fundamental types
//! * accessing of fields and following of references
//!
//! LICENSE
//!
//! The code in this example file is released into the Public Domain.

use osmflat::{iter_tags, Archive, FileResourceStorage, Osm, RelationMembersRef, COORD_SCALE};

use std::fmt;
use std::str::{self, Utf8Error};

/// Represents fixed point coordinates stored in OSM
#[derive(Clone, Copy)]
struct FixedI64(i64);

impl FixedI64 {
    fn value(self) -> f64 {
        self.0 as f64 / COORD_SCALE as f64
    }
}

impl fmt::Debug for FixedI64 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value: f64 = self.value();
        write!(f, "{}", value)
    }
}

#[derive(Debug)]
struct Header<'ar> {
    bbox: (FixedI64, FixedI64, FixedI64, FixedI64),
    required_features: Vec<&'ar str>,
    optional_features: Vec<&'ar str>,
    writingprogram: &'ar str,
    source: &'ar str,
    osmosis_replication_timestamp: i64,
    osmosis_replication_sequence_number: i64,
    osmosis_replication_base_url: &'ar str,
}

#[derive(Debug)]
struct Node<'ar> {
    id: i64,
    lat: FixedI64,
    lon: FixedI64,
    tags: Vec<(&'ar str, &'ar str)>,
}

#[derive(Debug)]
struct Way<'ar> {
    id: i64,
    tags: Vec<(&'ar str, &'ar str)>,
    nodes: Vec<Option<u64>>,
}

#[derive(Debug)]
struct Relation<'ar> {
    id: i64,
    tags: Vec<(&'ar str, &'ar str)>,
    members: Vec<Member<'ar>>,
}

#[derive(Debug)]
struct Member<'ar> {
    type_: Type,
    idx: Option<u64>,
    role: &'ar str,
}

#[derive(Debug)]
enum Type {
    Node,
    Way,
    Relation,
}

impl<'ar> Member<'ar> {
    fn new_slice(
        archive: &'ar Osm,
        relation_idx: usize,
    ) -> impl Iterator<Item = Result<Member<'ar>, Utf8Error>> {
        let strings = archive.stringtable();
        archive
            .relation_members()
            .at(relation_idx as usize)
            .map(move |member| {
                let res = match member {
                    RelationMembersRef::NodeMember(m) => Member {
                        type_: Type::Node,
                        idx: m.node_idx(),
                        role: strings.substring(m.role_idx() as usize)?,
                    },
                    RelationMembersRef::WayMember(m) => Member {
                        type_: Type::Way,
                        idx: m.way_idx(),
                        role: strings.substring(m.role_idx() as usize)?,
                    },
                    RelationMembersRef::RelationMember(m) => Member {
                        type_: Type::Relation,
                        idx: m.relation_idx(),
                        role: strings.substring(m.role_idx() as usize)?,
                    },
                };
                Ok(res)
            })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).take(2);
    let archive_dir = args.next().ok_or(
        "USAGE: debug <osmflat-archive> [TYPES] \
         TYPES can be any combination of 'n', 'w', 'r' (default: 'nwr').",
    )?;
    let types = args.next().unwrap_or_else(|| "nrw".to_string());
    let archive = Osm::open(FileResourceStorage::new(archive_dir))?;

    let header = archive.header();
    let strings = archive.stringtable();

    let required_features: Result<Vec<_>, _> = (header.required_feature_first_idx() as usize..)
        .take(header.required_features_size() as usize)
        .map(|idx| strings.substring(idx))
        .collect();
    let optional_features: Result<Vec<_>, _> = (header.optional_feature_first_idx() as usize..)
        .take(header.optional_features_size() as usize)
        .map(|idx| strings.substring(idx))
        .collect();

    // print header
    let header = Header {
        bbox: (
            FixedI64(header.bbox_left()),
            FixedI64(header.bbox_right()),
            FixedI64(header.bbox_top()),
            FixedI64(header.bbox_bottom()),
        ),
        required_features: required_features?,
        optional_features: optional_features?,
        writingprogram: strings.substring(header.writingprogram_idx() as usize)?,
        source: strings.substring(header.source_idx() as usize)?,
        osmosis_replication_timestamp: header.osmosis_replication_timestamp(),
        osmosis_replication_sequence_number: header.osmosis_replication_sequence_number(),
        osmosis_replication_base_url: strings
            .substring(header.osmosis_replication_base_url_idx() as usize)?,
    };
    println!("{:#?}", header);

    let collect_utf8_tags = |tags| -> Vec<(&str, &str)> {
        iter_tags(&archive, tags)
            .filter_map(|(k, v)| match (str::from_utf8(k), str::from_utf8(v)) {
                (Ok(k), Ok(v)) => Some((k, v)),
                _ => None,
            })
            .collect()
    };

    // print nodes
    if types.contains('n') {
        for node in &archive.nodes()[..3] {
            let node = Node {
                id: node.id(),
                lat: FixedI64(node.lat()),
                lon: FixedI64(node.lon()),
                tags: collect_utf8_tags(node.tags()),
            };

            println!("{:#?}", node);
        }
    }

    // print ways
    let nodes_index = archive.nodes_index();
    if types.contains('w') {
        for way in archive.ways() {
            let way = Way {
                id: way.id(),
                tags: collect_utf8_tags(way.tags()),
                nodes: way
                    .refs()
                    .map(|idx| nodes_index[idx as usize].value())
                    .collect(),
            };

            println!("{:#?}", way);
        }
    }

    // print relations
    if types.contains('r') {
        for (relation_idx, relation) in archive.relations()[..3].iter().enumerate() {
            let members: Result<Vec<_>, _> = Member::new_slice(&archive, relation_idx).collect();
            let relation = Relation {
                id: relation.id(),
                tags: collect_utf8_tags(relation.tags()),
                members: members?,
            };

            println!("{:#?}", relation);
        }
    }

    Ok(())
}
