use clap::{command, Args, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(version, about, author, long_about = None)]
pub struct Cli {
  /// The command to execute
  #[command(subcommand)]
  pub action: Action,
  /// List generations
  #[arg(long)]
  pub list_generations: bool,
  /// Profile name
  #[arg(short, long)]
  pub profile_name: Option<String>,
  /// Rollback
  #[arg(long)]
  pub rollback: bool,
  /// Switch generation
  #[arg(short = 'G', long)]
  pub switch_generation: Option<String>,
  /// Max jobs
  #[arg(short, long)]
  pub max_jobs: Option<String>,
  /// Cores
  #[arg(long)]
  pub cores: Option<String>,
  /// Dry run
  #[arg(long)]
  pub dry_run: bool,
  /// Keep going
  #[arg(short, long)]
  pub keep_going: bool,
  /// Keep failed
  #[arg(short = 'K', long)]
  pub keep_failed: bool,
  /// Fallback
  #[arg(long)]
  pub fallback: bool,
  /// Show trace
  #[arg(long)]
  pub show_trace: bool,
  /// Option
  #[arg(long, number_of_values = 2)]
  pub option: Option<Vec<String>>,
  /// Arg
  #[arg(long, number_of_values = 2)]
  pub arg: Option<Vec<String>>,
  #[arg(long, number_of_values = 2)]
  pub argstr: Option<Vec<String>>,
  /// Flake
  #[arg(long)]
  pub flake: Option<String>,
  /// Update input
  #[arg(long)]
  pub update_input: Option<String>,
  /// Override input
  #[arg(long, number_of_values = 2)]
  pub override_input: Option<Vec<String>>,
  /// Offline
  #[arg(long)]
  pub offline: bool,
  /// Substituters
  #[arg(long)]
  pub substituters: Option<String>,
}

#[derive(Subcommand, Debug, Eq, PartialEq)]
pub enum Action {
  List,
  Rollback,
  Edit,
  Switch,
  Activate,
  Build,
  Check,
  Changelog,
  #[clap(value_enum)]
  Completions(CompletionArgs),
}

#[derive(Args, Debug, Eq, PartialEq)]
pub struct CompletionArgs {
  /// The shell to generate the completion script for
  pub shell: Shell,
}
