use std::path::PathBuf;

/// Compiler of Open Street Data from osm.pbf format to osm.flatdata format
#[derive(Debug, clap::Parser)]
#[clap(about, version, author)]
pub struct Args {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(short, long, parse(from_occurrences))]
    pub verbose: u8,

    /// Input OSM pbf file
    pub input: PathBuf,

    /// Output directory for OSM flatdata archive
    pub output: PathBuf,

    /// Whether to compile the optional ids subs
    #[structopt(long = "ids")]
    pub ids: bool,
}
