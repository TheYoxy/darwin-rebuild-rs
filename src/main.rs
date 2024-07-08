pub mod build_runner;
pub mod cli;
pub mod initialize_panic_handler;
pub mod macros;
pub mod nix_commands;
#[cfg_attr(debug_assertions, path = "logging_debug.rs")]
#[cfg_attr(not(debug_assertions), path = "logging.rs")]
pub mod logging;

const DEFAULT_PROFILE: &str = "/nix/var/nix/profiles/system";

fn main() -> color_eyre::Result<()> {
  use build_runner::{BuildRunner, Runnable};
  use clap::Parser;

  initialize_panic_handler::initialize_panic_handler()?;

  let args = cli::Cli::parse();
  logging::setup_logging(args.verbose)?;

  let build_args = BuildRunner::new(&args);
  build_args.run()
}
