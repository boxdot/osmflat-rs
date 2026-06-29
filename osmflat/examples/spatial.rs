use clap::Parser;
use osmflat::{
    find_nodes_by_bounding_box, find_relations_by_bounding_box, find_ways_by_bounding_box,
    iter_tags, FileResourceStorage, Osm,
};
use std::path::PathBuf;

#[derive(Debug, clap::ValueEnum, Clone, Copy)]
enum OsmType {
    Node,
    Way,
    Relation,
}

/// Queries an osmflat archive by bounding box
#[derive(Debug, Parser)]
#[command(allow_negative_numbers = true)]
struct Args {
    /// input osmflat archive
    input: PathBuf,
    /// left bound of the longitude
    #[clap(long)]
    lon_min: f64,
    /// right bound of the longitude
    #[clap(long)]
    lon_max: f64,
    /// lower bound of the latitude
    #[clap(long)]
    lat_min: f64,
    /// upper bound of the latitude
    #[clap(long)]
    lat_max: f64,
    /// One of node, way, relation
    #[clap(long)]
    osm_type: OsmType,
}

const DIVIDER: &str = "------------------------";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = Args::parse();

    let resource = FileResourceStorage::new(opts.input.clone());
    let archive = Osm::open(resource)?;

    match opts.osm_type {
        OsmType::Node => {
            output_nodes(&archive, &opts);
        }
        OsmType::Way => {
            output_ways(&archive, &opts);
        }
        OsmType::Relation => {
            output_relations(&archive, &opts);
        }
    }

    Ok(())
}

fn output_nodes(archive: &Osm, opts: &Args) {
    let coord_scale = archive.header().coord_scale() as f64;

    for node in find_nodes_by_bounding_box(
        archive,
        opts.lon_min,
        opts.lat_min,
        opts.lon_max,
        opts.lat_max,
    ) {
        println!("{}", DIVIDER);
        println!(
            "Node lon={} lat={}",
            node.lon() as f64 / coord_scale,
            node.lat() as f64 / coord_scale
        );
        for (k, v) in iter_tags(archive, node.tags()) {
            println!(
                "{}={}",
                std::str::from_utf8(k).unwrap(),
                std::str::from_utf8(v).unwrap()
            );
        }
    }
}

fn output_ways(archive: &Osm, opts: &Args) {
    for way in find_ways_by_bounding_box(
        archive,
        opts.lon_min,
        opts.lat_min,
        opts.lon_max,
        opts.lat_max,
    ) {
        println!("{}", DIVIDER);
        println!("Way with {} nodes", way.refs().count());
        for (k, v) in iter_tags(archive, way.tags()) {
            println!(
                "{}={}",
                std::str::from_utf8(k).unwrap(),
                std::str::from_utf8(v).unwrap()
            );
        }
    }
}

fn output_relations(archive: &Osm, opts: &Args) {
    let coord_scale = archive.header().coord_scale() as f64;

    for relation in find_relations_by_bounding_box(
        archive,
        opts.lon_min,
        opts.lat_min,
        opts.lon_max,
        opts.lat_max,
    ) {
        println!("{}", DIVIDER);
        println!(
            "Relation bbox lon=[{}, {}] lat=[{}, {}]",
            relation.min_lon() as f64 / coord_scale,
            relation.max_lon() as f64 / coord_scale,
            relation.min_lat() as f64 / coord_scale,
            relation.max_lat() as f64 / coord_scale,
        );
        for (k, v) in iter_tags(archive, relation.tags()) {
            println!(
                "{}={}",
                std::str::from_utf8(k).unwrap(),
                std::str::from_utf8(v).unwrap()
            );
        }
    }
}
