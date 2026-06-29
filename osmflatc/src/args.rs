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

    /// Also write the optional ids sub-archive (original OSM ids)
    #[arg(long = "ids")]
    pub ids: bool,

    /// Also write the reverse id index (OSM id -> archive index) for fast
    /// id-based lookups. Implies `--ids`.
    #[arg(long = "reverse-ids")]
    pub reverse_ids: bool,

    /// Scratch RocksDB directory; use a fast SSD with ample space [default:
    /// output's parent]
    #[arg(long = "scratch-dir")]
    pub scratch_dir: Option<PathBuf>,

    /// Scratch RocksDB block cache (MiB); raise (e.g. 8192) for planet-scale
    #[arg(long = "block-cache-mb", default_value_t = 512)]
    pub block_cache_mb: usize,

    /// Scratch RocksDB write buffer (MiB); raise for higher write throughput
    #[arg(long = "write-buffer-mb", default_value_t = 128)]
    pub write_buffer_mb: usize,

    /// Max open SST files RocksDB may cache (-1 = unlimited). Must stay below
    /// the process open-file limit (raise it with `ulimit -n`); lower this
    /// if you hit "Too many open files"
    #[arg(long = "max-open-files", default_value_t = -1)]
    pub max_open_files: i32,
}
