//! The `planroom` binary: the CLI and the composition root.

use clap::{CommandFactory, Parser};

/// A shared, living map of a software system for people and agents.
#[derive(Parser)]
#[command(name = "planroom", version)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
    // No subcommands yet; with no arguments, print the help.
    Cli::command().print_help().expect("help is written to stdout");
}
