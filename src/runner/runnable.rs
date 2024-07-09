use std::{env, env::args};

use color_eyre::{eyre::bail, owo_colors::OwoColorize};
use log::{debug, info};

use crate::{
  nix_commands,
  runner::{
    nix_darwin_action::NixDarwinAction,
    nix_darwin_runner::{completion::generate_completion, NixDarwinRunner},
  },
  DEFAULT_PROFILE,
};

pub trait Runnable {
  fn run(&self) -> color_eyre::Result<()>;
}

impl Runnable for NixDarwinRunner {
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

    let action = if let Some(action) = self.action {
      action.into()
    } else if self.rollback {
      NixDarwinAction::Rollback
    } else if self.list_generations {
      NixDarwinAction::ListGenerations
    } else {
      bail!("No action specified")
    };

    info!("Starting action: {:?}", action.bold().purple());
    let result = match action {
      NixDarwinAction::Rollback => {
        let extra_profile_flags = vec!["--rollback"];
        self.run_profile(&extra_profile_flags)?;
        let system_config = std::fs::read_to_string(format!("{}/systemConfig", self.profile)).unwrap();
        Self::activate_profile(&system_config)
      },
      NixDarwinAction::ListGenerations => {
        let extra_profile_flags = vec!["--list-generations"];
        self.run_profile(&extra_profile_flags)
      },
      NixDarwinAction::Edit => {
        let darwin_config = nix_commands::nix_instantiate_find_file("darwin-config")?;
        if let Some(flake) = &self.flake {
          nix_commands::nix_edit(flake, &self.flake_attr, &self.flake_flags)
        } else {
          nix_commands::exec_editor(&darwin_config)
        }
      },
      NixDarwinAction::Activate => {
        let path = args().next().unwrap().replace("/sw/bin/darwin-rebuild", "");
        let system_config = nix_commands::get_real_path(&path)?;
        Self::activate_profile(&system_config)
      },
      NixDarwinAction::Build => self.build_configuration(&out_link_str).map(|_| ()),
      NixDarwinAction::Check => {
        let system_config = self.build_configuration(&out_link_str)?;
        unsafe {
          env::set_var("checkActivation", "1");
        }
        nix_commands::exec_activate_user(&system_config)
      },
      NixDarwinAction::Switch => {
        let system_config = self.build_configuration(&out_link_str)?;
        #[cfg(debug_assertions)]
        {
          let exists = std::fs::exists(&system_config)?;
          debug_assert!(exists, "the system configuration does not exist");
        }

        self.switch_profile(&system_config)?;
        Self::activate_profile(&system_config)
      },
      NixDarwinAction::Changelog => {
        info!("\nCHANGELOG\n");
        nix_commands::print_changelog(DEFAULT_PROFILE)
      },
      NixDarwinAction::Completions(shell) => generate_completion(shell),
    };
    drop(out_dir);
    result
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  // mod containers {
  //   use testcontainers::{runners::AsyncRunner, GenericImage};

  //   #[test_log::test]
  //   fn test() { let _image = GenericImage::new("sickcodes/docker-osx", "latest").start(); }
  // }
  mod without_flakes {
    use clap::Parser;

    use super::*;
    use crate::{cli::Cli, runner::runnable::NixDarwinRunner};

    const APP_NAME: &str = env!("CARGO_BIN_NAME");
    fn get_runner(args: Vec<&str>) -> NixDarwinRunner {
      let mut cli_args = vec![APP_NAME];
      cli_args.append(&mut args.clone());
      let cli = Cli::parse_from(cli_args);
      NixDarwinRunner::new(&cli).unwrap()
    }

    #[test_log::test]
    fn should_run_changelog() {
      let runner = get_runner(["changelog"].into());
      let result = runner.run();
      assert!(result.is_ok());
    }

    #[test_log::test]
    fn should_run_list_generations() {
      let runner = get_runner(["--list-generations"].into());
      let result = runner.run();
      assert!(result.is_ok());
    }

    #[test_log::test]
    fn should_run_edit() {
      let runner = get_runner(["edit"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    fn should_run_build() {
      let runner = get_runner(["build"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    #[ignore]
    fn should_run_check() {
      let runner = get_runner(["check"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    #[ignore]
    fn should_run_switch() {
      let runner = get_runner(["switch"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    #[ignore]
    fn should_run_activate() {
      let runner = get_runner(["activate"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }
  }

  mod with_flakes {
    use clap::Parser;

    use super::*;
    use crate::{cli::Cli, runner::runnable::NixDarwinRunner};
    const APP_NAME: &str = env!("CARGO_BIN_NAME");
    fn get_runner(args: Vec<&str>) -> NixDarwinRunner {
      let mut cli_args = vec![APP_NAME, "--flake", "./assets#darwin-rebuild-rs"];
      cli_args.append(&mut args.clone());
      let cli = Cli::parse_from(cli_args);
      NixDarwinRunner::new(&cli).unwrap()
    }

    #[test_log::test]
    fn should_run_changelog() {
      let runner = get_runner(["changelog"].into());
      let result = runner.run();
      assert!(result.is_ok());
    }

    #[test_log::test]
    fn should_run_list_generations() {
      let runner = get_runner(["--list-generations"].into());
      let result = runner.run();
      assert!(result.is_ok());
    }

    #[test_log::test]
    fn should_run_edit() {
      let runner = get_runner(["edit"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    fn should_run_build() {
      let runner = get_runner(["build"].into());
      let result = runner.run();
      assert!(result.is_ok(), "Result: {:?}", result.red());
    }

    #[test_log::test]
    #[ignore]
    fn should_run_check() {
      let runner = get_runner(["check"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    #[ignore]
    fn should_run_switch() {
      let runner = get_runner(["switch"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }

    #[test_log::test]
    #[ignore]
    fn should_run_activate() {
      let runner = get_runner(["activate"].into());
      let result = runner.run();
      assert!(result.is_ok(), "{:?}", result);
    }
  }
}
