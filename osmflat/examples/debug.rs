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

use osmflat::{iter_tags, FileResourceStorage, Osm, RelationMembersRef};
use structopt::StructOpt;

use std::path::PathBuf;
use std::str::{self, Utf8Error};

#[derive(Debug)]
struct Header<'ar> {
    #[allow(unused)]
    bbox: (f64, f64, f64, f64),
    #[allow(unused)]
    writingprogram: &'ar str,
    #[allow(unused)]
    source: &'ar str,
    #[allow(unused)]
    replication_timestamp: i64,
    #[allow(unused)]
    replication_sequence_number: i64,
    #[allow(unused)]
    replication_base_url: &'ar str,
}

#[derive(Debug)]
struct Node<'ar> {
    #[allow(unused)]
    id: Option<u64>,
    #[allow(unused)]
    lat: f64,
    #[allow(unused)]
    lon: f64,
    #[allow(unused)]
    tags: Vec<(&'ar str, &'ar str)>,
}

#[derive(Debug)]
struct Way<'ar> {
    #[allow(unused)]
    id: Option<u64>,
    #[allow(unused)]
    tags: Vec<(&'ar str, &'ar str)>,
    #[allow(unused)]
    nodes: Vec<Option<u64>>,
}

#[derive(Debug)]
struct Relation<'ar> {
    #[allow(unused)]
    id: Option<u64>,
    #[allow(unused)]
    tags: Vec<(&'ar str, &'ar str)>,
    #[allow(unused)]
    members: Vec<Member<'ar>>,
}

#[derive(Debug)]
struct Member<'ar> {
    #[allow(unused)]
    r#type: Type,
    #[allow(unused)]
    idx: Option<u64>,
    #[allow(unused)]
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
                        r#type: Type::Node,
                        idx: m.node_idx(),
                        role: strings.substring(m.role_idx() as usize)?,
                    },
                    RelationMembersRef::WayMember(m) => Member {
                        r#type: Type::Way,
                        idx: m.way_idx(),
                        role: strings.substring(m.role_idx() as usize)?,
                    },
                    RelationMembersRef::RelationMember(m) => Member {
                        r#type: Type::Relation,
                        idx: m.relation_idx(),
                        role: strings.substring(m.role_idx() as usize)?,
                    },
                };
                Ok(res)
            })
    }
}

#[derive(StructOpt, Debug)]
struct Args {
    /// Input osmflat archive
    #[structopt(parse(from_os_str))]
    input: PathBuf,
    /// Output PNG filename
    #[structopt(
        help = "Which types to print: (n)odes, (w)ays, or (r)elations",
        default_value = "nwr"
    )]
    types: String,
    #[structopt(long, help = "Amount of entities to print")]
    num: Option<usize>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::from_args();
    let archive = Osm::open(FileResourceStorage::new(args.input))?;

    let header = archive.header();
    let strings = archive.stringtable();

    let scale_coord = |x| x as f64 / header.coord_scale() as f64;

    // print header
    let header = Header {
        bbox: (
            scale_coord(header.bbox_left()),
            scale_coord(header.bbox_right()),
            scale_coord(header.bbox_top()),
            scale_coord(header.bbox_bottom()),
        ),
        writingprogram: strings.substring(header.writingprogram_idx() as usize)?,
        source: strings.substring(header.source_idx() as usize)?,
        replication_timestamp: header.replication_timestamp(),
        replication_sequence_number: header.replication_sequence_number(),
        replication_base_url: strings.substring(header.replication_base_url_idx() as usize)?,
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
    let mut node_ids = archive.ids().map(|x| x.nodes()).into_iter().flatten();
    if args.types.contains('n') {
        for node in archive.nodes().iter().take(args.num.unwrap_or(usize::MAX)) {
            let node = Node {
                id: node_ids.next().map(|x| x.value()),
                lat: scale_coord(node.lat()),
                lon: scale_coord(node.lon()),
                tags: collect_utf8_tags(node.tags()),
            };

            println!("{:#?}", node);
        }
    }

    // print ways
    let nodes_index = archive.nodes_index();
    let mut way_ids = archive.ids().map(|x| x.ways()).into_iter().flatten();
    if args.types.contains('w') {
        for way in archive.ways().iter().take(args.num.unwrap_or(usize::MAX)) {
            let way = Way {
                id: way_ids.next().map(|x| x.value()),
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
    let mut relation_ids = archive.ids().map(|x| x.ways()).into_iter().flatten();
    if args.types.contains('r') {
        for (relation_idx, relation) in archive.relations()[..3]
            .iter()
            .take(args.num.unwrap_or(usize::MAX))
            .enumerate()
        {
            let members: Result<Vec<_>, _> = Member::new_slice(&archive, relation_idx).collect();
            let relation = Relation {
                id: relation_ids.next().map(|x| x.value()),
                tags: collect_utf8_tags(relation.tags()),
                members: members?,
            };

            println!("{:#?}", relation);
        }
    }

    Ok(())
}
