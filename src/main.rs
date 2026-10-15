mod cli;
mod cms;
mod commands;
mod config;
mod error;
mod forgejo;
mod git;
mod timeutil;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();
    let config = config::Config::load();

    let result = match cli.command {
        Commands::Ci { all: true, recent, .. } => commands::ci::run_all(recent, cli.json, &config),
        Commands::Ci { watch, timeout, .. } => commands::ci::run(watch, timeout, cli.json, &config),
        Commands::Cms { command } => commands::cms::run(command, cli.json, &config),
        Commands::Auth { command } => commands::auth::run(command, cli.json, &config),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
