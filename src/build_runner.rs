use std::{
  env::{self, args},
  ffi::OsStr,
  fs,
  path::Path,
};

use color_eyre::owo_colors::OwoColorize;
use completion::generate_completion;
use log::{debug, info};
use regex::Regex;

use crate::{
  cli::{Action, Cli},
  nix_commands, print_bool, DEFAULT_PROFILE,
};

pub struct BuildRunner {
  action: Action,
  rollback: bool,
  list_generations: bool,
  profile: String,
  extra_build_flags: Vec<String>,
  flake: Option<String>,
  flake_flags: Vec<String>,
  flake_attr: String,
}
impl BuildRunner {
  pub fn new(args: &Cli) -> Self {
    let mut extra_metadata_flags = vec![];
    let mut extra_build_flags = vec![];
    let profile = Self::parse_profile(&args.profile_name);
    info!("Current profile: {}", profile.yellow());

    for value in [&args.max_jobs, &args.cores, &args.update_input, &args.substituters].into_iter().flatten() {
      extra_build_flags.push(value.clone());
      extra_metadata_flags.push(value.clone());
    }

    for values in [&args.option, &args.arg, &args.argstr, &args.override_input].into_iter().flatten() {
      extra_build_flags.extend(values.iter().map(|s| s.to_string()));
      extra_metadata_flags.extend(values.iter().map(|s| s.to_string()));
    }

    let flake_flags = vec!["--extra-experimental-features".to_string(), "nix-command flakes".to_string()];

    let (flake, flake_attr) = Self::parse_flake(args, flake_flags.clone(), extra_metadata_flags.clone());
    Self {
      action: args.action,
      rollback: args.rollback,
      list_generations: args.list_generations,
      profile,
      extra_build_flags,
      flake_flags,
      flake,
      flake_attr,
    }
  }

  fn parse_profile(profile_name: &Option<String>) -> String {
    if let Some(profile_name) = &profile_name {
      if profile_name != "system" {
        let profile = format!("/nix/var/nix/profiles/system-profiles/{}", profile_name);
        std::fs::create_dir_all(Path::new(&profile).parent().unwrap()).unwrap();
        profile
      } else {
        std::env::var("profile").unwrap_or(DEFAULT_PROFILE.to_string())
      }
    } else {
      std::env::var("profile").unwrap_or(DEFAULT_PROFILE.to_string())
    }
  }

  fn parse_flake(args: &Cli, flake_flags: Vec<String>, extra_metadata_flags: Vec<String>) -> (Option<String>, String) {
    if let Some(flake_value) = &args.flake {
      debug!("Looking for flake metadata... {flake_value}");
      let re = Regex::new(r"^(([^:/?#]+):)?(//([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?").unwrap();

      let (flake, flake_attr) = if let Some(caps) = re.captures(flake_value) {
        let scheme = if let Some(r) = caps.get(1) { r.as_str() } else { "" };
        let authority = if let Some(e) = caps.get(3) { e.as_str() } else { "" };
        let path = if let Some(e) = caps.get(5) { e.as_str() } else { "" };
        let query_with_question = if let Some(e) = caps.get(6) { e.as_str() } else { "" };
        let flake_attr = if let Some(e) = caps.get(9) {
          e.as_str().to_string()
        } else {
          match nix_commands::get_local_hostname() {
            Ok(e) => e,
            Err(err) => panic!("Failed to get local hostname: {:?}", err),
          }
        };
        let flake_value = format!("{}{}{}{}", scheme, authority, path, query_with_question);
        let cmd =
          if nix_commands::nix_command_supports_flake_metadata(flake_flags.clone()) { "metadata" } else { "info" };

        let metadata = match nix_commands::get_flake_metadata(&flake_flags, cmd, &extra_metadata_flags, flake_value) {
          Ok(e) => e,
          Err(err) => panic!("Failed to get flake metadata: {:?}", err),
        };
        debug!("url: {:?}", metadata["url"].blue());
        let flake_value = metadata["url"].as_str().unwrap().to_string();
        debug!("flake_value: {:?}", flake_value.blue());
        let flake = if metadata["resolved"]["submodules"].as_bool().unwrap_or(false) {
          if flake_value.contains('?') {
            Some(format!("{}&submodules=1", flake_value))
          } else {
            Some(format!("{}?submodules=1", flake_value))
          }
        } else {
          Some(flake_value)
        };
        debug!("flake: {:?}", flake.blue());

        (flake, flake_attr)
      } else {
        (None, "".to_string())
      };

      (flake, format!("darwinConfigurations.{}", flake_attr))
    } else {
      (None, "".to_string())
    }
  }
}

#[derive(Debug)]
enum BuildArgsAction {
  Rollback,
  List,
  Edit,
  Switch,
  Activate,
  Build,
  Check,
  Changelog,
  Completions(clap_complete::Shell),
}

impl From<Action> for BuildArgsAction {
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

pub trait Runnable {
  fn run(&self) -> color_eyre::Result<()>;
}
impl Runnable for BuildRunner {
  fn run(&self) -> color_eyre::Result<()> {
    let out_dir = tempfile::Builder::new().prefix("nix-darwin-").tempdir()?;
    let out_link = out_dir.path().join("result");
    let out_link_str = out_link.clone().to_str().unwrap().to_string();
    debug!("out_dir: {:?}", out_dir.black().on_yellow());
    debug!("out_link: {:?}", out_link.yellow());

    #[cfg(debug_assertions)]
    {
      let exists = std::fs::exists(&out_link_str)?;
      debug_assert!(!exists, "the system configuration should not exist");
    }

    let action = if self.rollback {
      BuildArgsAction::Rollback
    } else if self.list_generations {
      BuildArgsAction::List
    } else {
      self.action.into()
    };

    info!("Running action: {:?}", action.bold().purple());
    let result = match action {
      BuildArgsAction::Rollback => {
        let extra_profile_flags = vec!["--rollback"];
        run_profile(&self.profile, &extra_profile_flags)?;
        let system_config = fs::read_to_string(format!("{}/systemConfig", self.profile)).unwrap();
        activate_profile(&system_config)
      },
      BuildArgsAction::List => {
        let extra_profile_flags = vec!["--list-generations"];
        run_profile(&self.profile, &extra_profile_flags)
      },
      BuildArgsAction::Edit => {
        let darwin_config = nix_commands::nix_instantiate_find_file("darwin-config")?;
        if let Some(flake) = &self.flake {
          nix_commands::exec_nix_edit(flake, &self.flake_attr, &self.flake_flags)
        } else {
          nix_commands::exec_editor(&darwin_config);
          Ok(())
        }
      },
      BuildArgsAction::Activate => {
        let system_config = nix_commands::get_real_path(args().next().unwrap().replace("/sw/bin/darwin-rebuild", ""))?;
        activate_profile(&system_config)
      },
      BuildArgsAction::Build => {
        build_configuration(&self.flake, &self.flake_attr, &self.flake_flags, &out_link_str, &self.extra_build_flags)?;
        Ok(())
      },
      BuildArgsAction::Check => {
        let system_config = build_configuration(
          &self.flake,
          &self.flake_attr,
          &self.flake_flags,
          &out_link_str,
          &self.extra_build_flags,
        )?;
        unsafe {
          env::set_var("checkActivation", "1");
        }
        nix_commands::exec_activate_user(&system_config)
      },
      BuildArgsAction::Switch => {
        let system_config = build_configuration(
          &self.flake,
          &self.flake_attr,
          &self.flake_flags,
          &out_link_str,
          &self.extra_build_flags,
        )?;
        #[cfg(debug_assertions)]
        {
          let exists = std::fs::exists(&system_config)?;
          debug_assert!(exists, "the system configuration does not exist");

          // let metadata = std::fs::metadata(&system_config)?;
          // debug_assert!(metadata.is_symlink(), "the store must be a symlink");
        }

        switch_profile(&system_config, &self.profile)?;
        activate_profile(&system_config)
      },
      BuildArgsAction::Changelog => {
        info!("\nCHANGELOG\n");
        nix_commands::print_changelog(DEFAULT_PROFILE)
      },
      BuildArgsAction::Completions(shell) => generate_completion(shell),
    };
    drop(out_dir);
    result
  }
}

fn switch_profile<SystemConfig, Profile>(system_config: SystemConfig, profile: Profile) -> color_eyre::Result<()>
where
  Profile: AsRef<Path> + std::fmt::Display + std::convert::AsRef<std::ffi::OsStr>,
  SystemConfig: std::convert::AsRef<std::ffi::OsStr> + std::fmt::Display,
{
  let is_root_user = nix_commands::is_root_user();
  let is_read_only = nix_commands::is_read_only(&profile)?;
  debug!("Is root user: {} is ro {}", print_bool!(is_root_user), print_bool!(is_read_only));
  if !is_root_user && is_read_only {
    info!("setting the profile as root...");
    nix_commands::sudo_nix_env_set_profile(profile, system_config)?;
  } else {
    info!("setting the profile...");
    nix_commands::nix_env_set_profile(profile, system_config)?;
  }
  Ok(())
}

fn build_configuration<Flake, FlakeAttr, FlakeFlagsItems, OutDir, BuildFlagsItems>(
  flake: &Option<Flake>, flake_attr: FlakeAttr, flake_flags: &[FlakeFlagsItems], out_dir: OutDir,
  extra_build_flags: &[BuildFlagsItems],
) -> color_eyre::Result<String>
where
  Flake: AsRef<OsStr> + std::fmt::Display,
  FlakeAttr: AsRef<OsStr> + std::fmt::Display,
  OutDir: AsRef<str> + Into<String> + std::fmt::Display,
  FlakeFlagsItems: AsRef<OsStr> + std::fmt::Debug,
  BuildFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  if let Some(flake) = &flake {
    info!("building the system configuration from {}...", flake.yellow());
    nix_commands::nix_flake_build(flake, flake_attr, flake_flags, out_dir, extra_build_flags)
  } else {
    info!("building the system configuration from <darwin>...");
    nix_commands::nix_build("<darwin>", "system", out_dir, extra_build_flags)
  }
}

fn activate_profile<SystemConfig>(system_config: &SystemConfig) -> color_eyre::Result<()>
where
  SystemConfig: std::fmt::Display,
{
  info!("activating user profile...");
  nix_commands::exec_activate_user(&system_config)?;
  if !nix_commands::is_root_user() {
    info!("activating system as root...");
    nix_commands::sudo_exec_activate(&system_config)?;
  } else {
    info!("activating system...");
    nix_commands::exec_activate(&system_config)?;
  }
  Ok(())
}

fn run_profile<Profile, ExtraProfileFlags, ExtraProfileFlagsItems>(
  profile: &Profile, extra_profile_flags: ExtraProfileFlags,
) -> color_eyre::Result<()>
where
  Profile: std::convert::AsRef<OsStr> + std::fmt::Display + std::convert::AsRef<std::path::Path>,
  ExtraProfileFlags: IntoIterator<Item = ExtraProfileFlagsItems> + std::fmt::Debug,
  ExtraProfileFlagsItems: AsRef<OsStr>,
{
  if !nix_commands::is_root_user() && !nix_commands::is_read_only(profile)? {
    nix_commands::sudo_nix_env_profile(profile, extra_profile_flags)?;
  } else {
    nix_commands::nix_env_profile(profile, extra_profile_flags)?;
  }
  Ok(())
}

pub(crate) mod completion {
  use clap::CommandFactory;
  use clap_complete::Shell;
  use log::debug;

  use crate::cli::Cli;

  fn print_completions<G: clap_complete::Generator>(gen: G, cmd: &mut clap::Command) {
    use clap_complete::generate;
    debug!("Generating completions for command: {:?}", cmd.get_name());
    #[cfg(not(test))]
    let mut buf = std::io::stdout();
    #[cfg(test)]
    let mut buf = std::io::sink();
    generate(gen, cmd, cmd.get_name().to_string(), &mut buf);
  }

  pub(crate) fn generate_completion(shell: Shell) -> color_eyre::Result<()> {
    let mut cmd = Cli::command();
    debug!("Generating completions for shell: {}", shell);
    print_completions(shell, &mut cmd);
    Ok(())
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::cli::CompletionArgs;

  #[test_log::test]
  fn can_run_completions() {
    fn run_completions(shell: clap_complete::Shell) -> color_eyre::Result<()> {
      let cli = Cli { action: Action::Completions(CompletionArgs { shell }), ..Default::default() };
      let runner = BuildRunner::new(&cli);
      runner.run()
    }

    assert!(run_completions(clap_complete::Shell::Zsh).is_ok());
    assert!(run_completions(clap_complete::Shell::Bash).is_ok());
    assert!(run_completions(clap_complete::Shell::PowerShell).is_ok());
    assert!(run_completions(clap_complete::Shell::Fish).is_ok());
    assert!(run_completions(clap_complete::Shell::Elvish).is_ok());
  }

  #[test_log::test]
  fn can_run_changelog() {
    let cli = Cli { action: Action::Changelog, ..Default::default() };
    let runner = BuildRunner::new(&cli);
    let result = runner.run();
    assert!(result.is_ok());
  }
}
