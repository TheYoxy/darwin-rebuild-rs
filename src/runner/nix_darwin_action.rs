use crate::cli::Action;

#[derive(Debug)]
pub(super) enum NixDarwinAction {
  Rollback,
  ListGenerations,
  Edit,
  Switch,
  Activate,
  Build,
  Check,
  Changelog,
  Completions(clap_complete::Shell),
}

impl From<Action> for NixDarwinAction {
  fn from(value: Action) -> Self {
    match value {
      Action::Edit => Self::Edit,
      Action::Switch => Self::Switch,
      Action::Activate => Self::Activate,
      Action::Build => Self::Build,
      Action::Check => Self::Check,
      Action::Changelog => Self::Changelog,
      Action::Completions(args) => Self::Completions(args.shell),
    }
  }
}
