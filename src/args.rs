use docopt::Docopt;

const USAGE: &'static str = "
Compiler from osm.pbf format to osm.flatdata format.

Usage:
  osm-flatdata <input> <output>
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub arg_input: String,
    pub arg_output: String,
}

pub fn parse_args() -> Args {
    Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit())
}
