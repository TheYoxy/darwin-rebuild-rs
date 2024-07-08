pub mod cli;
pub mod initialize_panic_handler;
#[cfg_attr(debug_assertions, path = "logging_debug.rs")]
#[cfg_attr(not(debug_assertions), path = "logging.rs")]
pub mod logging;
pub mod macros;
pub mod nix_commands;
mod runner;

const DEFAULT_PROFILE: &str = "/nix/var/nix/profiles/system";

fn main() -> color_eyre::Result<()> {
  use clap::Parser;
  use runner::runnable::Runnable;

  initialize_panic_handler::initialize_panic_handler()?;

  let args = cli::Cli::parse();
  logging::setup_logging(args.verbose)?;

  let build_args = runner::nix_darwin_runner::NixDarwinRunner::new(&args)?;
  build_args.run()
}
