use std::path::PathBuf;

/// Compiler of Open Street Data from osm.pbf format to osm.flatdata format
#[derive(Debug, StructOpt)]
#[structopt(name = "osmflatc")]
pub struct Args {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u8,

    /// Input OSM pbf file
    #[structopt(name = "input", parse(from_os_str))]
    pub input: PathBuf,

    /// Output directory for OSM flatdata archive
    #[structopt(name = "output", parse(from_os_str))]
    pub output: PathBuf,
}
