use clap::Parser;
use log::error;
use osmflatc::args::Args;
use osmflatc::run;

fn main() {
    let args = Args::parse();
    let level = match args.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .format_target(false)
        .format_module_path(false)
        .format_timestamp_nanos()
        .init();

    if let Err(e) = run(args) {
        error!("{e}");
        std::process::exit(1);
    }
}
