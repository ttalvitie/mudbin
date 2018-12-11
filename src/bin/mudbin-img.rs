use mudbin::errors::*;
use mudbin::cli_common::{run_with_args, DefaultArgs};
use mudbin::create_image;

use std::path::Path;

use clap::{Arg, ArgMatches, SubCommand};

use error_chain::quick_main;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn run_create(args: &ArgMatches) -> Result<()> {
    let output_path = Path::new(args.value_of_os("output").unwrap());
    create_image(output_path)
}

fn run() -> Result<()> {
    let args = clap::App::new("mudbin-img")
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
                        .required(true)
                )
        );
    
    run_with_args(args, |args| {
        if let Some(args) = args.subcommand_matches("create") {
            run_create(args)?;
        }
        Ok(())
    })
}

quick_main!(run);
