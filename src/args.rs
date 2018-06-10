use docopt::Docopt;

const USAGE: &str = "
Compiler of Open Street Data from osm.pbf format to osm.flatdata format.

Usage:
  osmflatc <input> <output> [-v...]
  osmflatc (-h | --help)
  osmflatc --version

Options:
  -v --verbose  Verbosity.
  -h --help     Show this screen.
  --version     Show version.
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub arg_input: String,
    pub arg_output: String,
    pub flag_verbose: usize,
}

pub fn parse_args() -> Args {
    Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit())
}
