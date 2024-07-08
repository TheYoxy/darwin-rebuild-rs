pub mod build_runner;
pub mod cli;
pub mod initialize_panic_handler;
pub mod macros;
pub mod nix_commands;

const DEFAULT_PROFILE: &str = "/nix/var/nix/profiles/system";

fn main() -> color_eyre::Result<()> {
  use build_runner::{BuildRunner, Runnable};
  use clap::Parser;

  initialize_panic_handler::initialize_panic_handler()?;

  #[cfg(debug_assertions)]
  pretty_env_logger::init();
  let args = cli::Cli::parse();

  let build_args = BuildRunner::new(&args);
  build_args.run()
}
