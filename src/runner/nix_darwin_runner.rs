use std::{
  env,
  ffi::OsStr,
  fmt::{Debug, Display},
  path::Path,
};

use color_eyre::{
  eyre::{bail, eyre},
  owo_colors::OwoColorize,
};
use log::{debug, info};
use regex::Regex;

use crate::{
  cli::{Action, Cli},
  nix_commands, print_bool, DEFAULT_PROFILE,
};

pub struct NixDarwinRunner {
  pub(super) action: Option<Action>,
  pub(super) rollback: bool,
  pub(super) list_generations: bool,
  pub(super) profile: String,
  pub(super) extra_build_flags: Vec<String>,
  pub(super) flake: Option<String>,
  pub(super) flake_flags: Vec<String>,
  pub(super) flake_attr: String,
}

impl NixDarwinRunner {
  pub fn new(args: &Cli) -> color_eyre::Result<Self> {
    let extra_metadata_flags = vec![];
    let extra_build_flags = vec![];
    let profile = Self::parse_profile(&args.profile_name)?;
    info!("Current profile: {}", profile.yellow());

    let flake_flags = vec!["--extra-experimental-features".to_string(), "nix-command flakes".to_string()];

    let (flake, flake_attr) = Self::parse_flake(args, &flake_flags, &extra_metadata_flags)?;
    Ok(Self {
      action: args.action,
      rollback: args.rollback,
      list_generations: args.list_generations,
      profile,
      extra_build_flags,
      flake_flags,
      flake,
      flake_attr,
    })
  }

  fn parse_profile(profile_name: &Option<String>) -> color_eyre::Result<String> {
    fn default_value() -> String { env::var("profile").unwrap_or(DEFAULT_PROFILE.to_string()) }
    if let Some(profile_name) = &profile_name {
      if profile_name != "system" {
        let profile = format!("/nix/var/nix/profiles/system-profiles/{}", profile_name);
        let path =
          Path::new(&profile).parent().ok_or(eyre!("unable to get parent directory of {}", profile.yellow()))?;
        std::fs::create_dir_all(path)?;
        Ok(profile)
      } else {
        Ok(default_value())
      }
    } else {
      Ok(default_value())
    }
  }

  fn parse_flake(
    args: &Cli, flake_flags: &[String], extra_metadata_flags: &[String],
  ) -> color_eyre::Result<(Option<String>, String)> {
    if let Some(flake_value) = &args.flake {
      debug!("Looking for flake metadata... {flake_value}");
      let re = Regex::new(r"^(([^:/?#]+):)?(//([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?")?;

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
            Err(err) => bail!("Failed to get local hostname: {:?}", err),
          }
        };
        let flake_value = format!("{}{}{}{}", scheme, authority, path, query_with_question);
        let cmd = if nix_commands::nix_command_supports_flake_metadata(flake_flags) { "metadata" } else { "info" };

        let metadata = match nix_commands::get_flake_metadata(flake_value, cmd, flake_flags, extra_metadata_flags) {
          Ok(e) => e,
          Err(err) => bail!("Failed to get flake metadata: {:?}", err),
        };
        let url = &metadata["url"];
        debug!("Url {:?}", url.blue());
        let flake_value = match url {
          serde_json::Value::String(e) => {
            if e.is_empty() {
              bail!("flake url is empty");
            }
            e
          },
          _ => bail!("flake url is not a string"),
        }
        .to_owned();
        debug!("flake_value: {:?}", flake_value.blue());
        let flake = match metadata["resolved"]["submodules"] {
          serde_json::Value::Bool(true) => {
            if flake_value.contains('?') {
              format!("{}&submodules=1", flake_value)
            } else {
              format!("{}?submodules=1", flake_value)
            }
          },
          serde_json::Value::Bool(false) => flake_value,
          _ => bail!("submodules is not a boolean"),
        };
        debug!("flake: {:?}", flake.blue());

        (Some(flake), flake_attr)
      } else {
        (None, "".to_string())
      };

      Ok((flake, format!("darwinConfigurations.{}", flake_attr)))
    } else {
      Ok((None, "".to_string()))
    }
  }

  pub(super) fn build_configuration<OutDir: AsRef<str> + Into<String> + Display>(
    &self, out_dir: OutDir,
  ) -> color_eyre::Result<String> {
    if let Some(flake) = &self.flake {
      info!("building the system configuration from {}...", flake.yellow());
      nix_commands::nix_flake_build(flake, &self.flake_attr, &self.flake_flags, out_dir, &self.extra_build_flags)
    } else {
      info!("building the system configuration from <darwin>...");
      nix_commands::nix_build("<darwin>", "system", out_dir, &self.extra_build_flags)
    }
  }

  pub(super) fn switch_profile<SystemConfig: AsRef<OsStr> + Display>(
    &self, system_config: SystemConfig,
  ) -> color_eyre::Result<()> {
    let is_root_user = nix_commands::is_root_user()?;
    let is_read_only = nix_commands::is_read_only(&self.profile)?;
    debug!("Is root user: {} is ro {}", print_bool!(is_root_user), print_bool!(is_read_only));
    if !is_root_user && is_read_only {
      info!("setting the profile as root...");
      nix_commands::sudo_nix_env_set_profile(&self.profile, &system_config)?;
    } else {
      info!("setting the profile...");
      nix_commands::nix_env_set_profile(&self.profile, &system_config)?;
    }
    Ok(())
  }

  pub(super) fn run_profile<ExtraProfileFlags, ExtraProfileFlagsItems>(
    &self, extra_profile_flags: ExtraProfileFlags,
  ) -> color_eyre::Result<()>
  where
    ExtraProfileFlags: IntoIterator<Item = ExtraProfileFlagsItems> + Debug,
    ExtraProfileFlagsItems: AsRef<OsStr>,
  {
    let profile = &self.profile;
    let is_root_user = nix_commands::is_root_user()?;
    let is_read_only = nix_commands::is_read_only(&profile)?;
    debug!("Is root user: {} is ro {}", print_bool!(is_root_user), print_bool!(is_read_only));
    if !is_root_user && is_read_only {
      nix_commands::sudo_nix_env_profile(profile, extra_profile_flags)?;
    } else {
      nix_commands::nix_env_profile(profile, extra_profile_flags)?;
    }
    Ok(())
  }

  pub(super) fn activate_profile<SystemConfig: Display>(system_config: &SystemConfig) -> color_eyre::Result<()> {
    info!("activating user profile...");
    nix_commands::exec_activate_user(&system_config)?;
    if !nix_commands::is_root_user()? {
      info!("activating system as root...");
      nix_commands::sudo_exec_activate(&system_config)?;
    } else {
      info!("activating system...");
      nix_commands::exec_activate(&system_config)?;
    }
    Ok(())
  }
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
  use crate::{cli::CompletionArgs, runner::runnable::Runnable};
  #[test_log::test]
  fn can_run_completions() {
    fn run_completions(shell: clap_complete::Shell) -> color_eyre::Result<()> {
      let cli = Cli { action: Some(Action::Completions(CompletionArgs { shell })), ..Default::default() };
      let runner = NixDarwinRunner::new(&cli)?;
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
    let cli = Cli { action: Some(Action::Changelog), ..Default::default() };
    let runner = NixDarwinRunner::new(&cli);
    assert!(runner.is_ok());
    let runner = runner.unwrap();
    let result = runner.run();
    assert!(result.is_ok());
  }
}
