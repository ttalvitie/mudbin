use mudbin::create_image;
use mudbin::errors::*;

use std::path::Path;

use clap::{Arg, ArgMatches, SubCommand};

use error_chain::quick_main;

use log;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

trait DefaultArgs {
    fn default_args(self) -> Self;
}

impl<'a, 'b> DefaultArgs for clap::App<'a, 'b> {
    fn default_args(self) -> Self {
        self.arg(Arg::with_name("v").short("v").multiple(true).help(
            "Increase level of verbosity of the stderr output (specify multiple to increase more)",
        ))
    }
}

struct StderrLogger {
    level_filter: log::LevelFilter,
}

impl log::Log for StderrLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{} {}: {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

impl StderrLogger {
    fn init(level_filter: log::LevelFilter) -> Result<()> {
        log::set_boxed_logger(Box::new(StderrLogger { level_filter }))
            .map(|()| log::set_max_level(level_filter))
            .chain_err(|| "Could not set up logger")
    }
}

fn run_create(args: &ArgMatches) -> Result<()> {
    let output_path = Path::new(args.value_of_os("output").unwrap());
    create_image(output_path)
}

fn run() -> Result<()> {
    let args = clap::App::new("mudbin")
        .version(VERSION)
        .setting(clap::AppSettings::SubcommandRequired)
        .setting(clap::AppSettings::GlobalVersion)
        .subcommand(
            SubCommand::with_name("create")
                .about("Creates a new virtual machine root disk image")
                .default_args()
                .arg(
                    Arg::with_name("output")
                        .help("Path to the output image file")
                        .required(true),
                ),
        );

    let args = match args.get_matches_safe() {
        Err(e) => match e.kind {
            clap::ErrorKind::HelpDisplayed => {
                println!("{}", e.message);
                return Ok(());
            }
            clap::ErrorKind::VersionDisplayed => return Ok(()),
            _ => return Err(e).chain_err(|| "Parsing command line arguments failed"),
        },
        Ok(args) => args,
    };

    let mut verbosity = args.occurrences_of("v");
    if let Some(sub_args) = args.subcommand().1 {
        verbosity += sub_args.occurrences_of("v");
    };
    let log_level_filter = match verbosity {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    StderrLogger::init(log_level_filter)?;

    if let Some(args) = args.subcommand_matches("create") {
        run_create(args)?;
    }

    Ok(())
}

quick_main!(run);
