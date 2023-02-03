use std::path::PathBuf;

use clap::Parser;

/// Compiler of Open Street Data from osm.pbf format to osm.flatdata format
#[derive(Debug, Parser)]
#[clap(about, version, author)]
pub struct Args {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[clap(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Input OSM pbf file
    pub input: PathBuf,

    /// Output directory for OSM flatdata archive
    pub output: PathBuf,

    /// Whether to compile the optional ids subs
    #[arg(long = "ids")]
    pub ids: bool,
}
