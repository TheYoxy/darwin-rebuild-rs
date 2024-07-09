use anstyle::Style;
use clap::{builder::Styles, command, Args, Parser, Subcommand};
use clap_complete::Shell;

fn make_style() -> Styles {
  Styles::plain()
    .header(Style::new().bold())
    .literal(Style::new().bold().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))))
}

#[derive(Default, Debug, Parser)]
#[command(version, about, author, long_about = None, styles=make_style())]
pub struct Cli {
  /// The command to execute
  #[command(subcommand)]
  pub action: Option<Action>,
  /// List generations
  #[arg(long, global = true, conflicts_with("rollback"))]
  pub list_generations: bool,
  /// Rollback
  #[arg(long, global = true, conflicts_with("list_generations"))]
  pub rollback: bool,
  /// Profile name
  #[arg(short, long, global = true)]
  pub profile_name: Option<String>,
  /// Flake
  #[arg(short, long, env = "FLAKE", global = true, value_hint = clap::ValueHint::DirPath)]
  pub flake: Option<String>,
  /// Show debug logs
  #[arg(long, short, global = true)]
  pub verbose: bool,
}

#[derive(Args, Debug, Eq, PartialEq, Clone, Copy)]
pub struct BuildArgs {}

#[derive(Subcommand, Default, Debug, Eq, PartialEq, Clone, Copy)]
pub enum Action {
  #[default]
  Build,
  Check,
  Switch,
  Edit,
  Activate,
  Changelog,
  #[clap(value_enum)]
  Completions(CompletionArgs),
}

#[derive(Args, Debug, Eq, PartialEq, Clone, Copy)]
pub struct CompletionArgs {
  /// The shell to generate the completion script for
  pub shell: Shell,
}

#[cfg(test)]
mod tests {
  use rstest::rstest;

  use super::*;

  const APP_NAME: &str = env!("CARGO_BIN_NAME");
  #[rstest]
  #[case::build("build", Action::Build)]
  #[case::check("check", Action::Check)]
  #[case::switch("switch", Action::Switch)]
  #[case::edit("edit", Action::Edit)]
  #[case::activate("activate", Action::Activate)]
  fn should_parse_cli_build(#[case] cmd: &str, #[case] action: Action) {
    use clap::Parser;
    let cli = Cli::parse_from([APP_NAME, cmd, "--verbose"]);
    assert!(cli.verbose);
    assert_eq!(cli.action, Some(action));
  }

  #[test]
  fn should_parse_cli_list_generations() {
    use clap::Parser;
    let cli = Cli::parse_from([APP_NAME, "--list-generations"]);
    assert_eq!(cli.action, None);
    assert!(cli.list_generations);
  }

  #[test]
  fn should_parse_cli_rollback() {
    use clap::Parser;
    let cli = Cli::parse_from([APP_NAME, "--rollback"]);
    assert_eq!(cli.action, None);
    assert!(cli.rollback);
  }
}
