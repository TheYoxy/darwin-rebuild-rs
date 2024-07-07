use crate::completion::generate_completion;

pub mod cli;
pub mod initialize_panic_handler;
pub mod macros;
pub mod nix_commands;

pub(crate) mod completion {
  use clap::CommandFactory;
  use log::debug;

  use crate::cli::{Cli, CompletionArgs};

  fn print_completions<G: clap_complete::Generator>(gen: G, cmd: &mut clap::Command) {
    use clap_complete::generate;
    debug!("Generating completions for command: {:?}", cmd.get_name());
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
  }

  pub(crate) fn generate_completion(completion: &CompletionArgs) -> color_eyre::Result<()> {
    let mut cmd = Cli::command();
    debug!("Generating completions for shell: {:?}", completion);
    print_completions(completion.shell, &mut cmd);
    Ok(())
  }
}

fn main() -> color_eyre::Result<()> {
  use std::{env, fs, path::Path, process::exit};

  use clap::Parser;
  use log::info;
  use regex::Regex;

  use crate::cli::Action;

  initialize_panic_handler::initialize_panic_handler()?;

  #[cfg(debug_assertions)]
  pretty_env_logger::init();
  let args = cli::Cli::parse();

  if let Action::Completions(shell) = args.action {
    return generate_completion(&shell);
  }

  let mut extra_metadata_flags = vec![];
  let mut extra_build_flags = vec![];
  let extra_lock_flags = vec![];
  let mut extra_profile_flags = vec![];
  let profile = if let Some(profile_name) = args.profile_name {
    if profile_name != "system" {
      let profile = format!("/nix/var/nix/profiles/system-profiles/{}", profile_name);
      fs::create_dir_all(Path::new(&profile).parent().unwrap()).unwrap();
      profile
    } else {
      env::var("profile").unwrap_or("@profile@".to_string())
    }
  } else {
    env::var("profile").unwrap_or("@profile@".to_string())
  };
  let action = if args.rollback {
    extra_profile_flags.push("--rollback".to_string());
    Action::Rollback
  } else if args.list_generations {
    extra_profile_flags.push("--list-generations".to_string());
    Action::List
  } else {
    args.action
  };
  let mut flake = args.flake;

  for value in [args.max_jobs, args.cores, args.update_input, args.substituters].into_iter().flatten() {
    extra_build_flags.push(value.clone());
    extra_metadata_flags.push(value);
  }

  for values in [args.option, args.arg, args.argstr, args.override_input].into_iter().flatten() {
    extra_build_flags.extend(values.iter().map(|s| s.to_string()));
    extra_metadata_flags.extend(values.iter().map(|s| s.to_string()));
  }

  let flake_flags = vec!["--extra-experimental-features", "nix-command flakes"];
  let mut flake_attr = "".to_string();

  if let Some(flake_value) = &flake {
    let re = Regex::new(r"^(([^:/?#]+):)?(//([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?").unwrap();
    if let Some(caps) = re.captures(flake_value) {
      let scheme = if let Some(r) = caps.get(1) { r.as_str() } else { "" };
      let authority = if let Some(e) = caps.get(3) { e.as_str() } else { "" };
      let path = if let Some(e) = caps.get(5) { e.as_str() } else { "" };
      let query_with_question = if let Some(e) = caps.get(6) { e.as_str() } else { "" };
      flake_attr = if let Some(e) = caps.get(9) { e.as_str().to_string() } else { "".to_string() };

      flake = Some(format!("{}{}{}{}", scheme, authority, path, query_with_question));
    }

    if flake_attr.is_empty() {
      flake_attr = nix_commands::get_local_hostname()?;
    }

    flake_attr = format!("darwinConfigurations.{}", flake_attr);
  }

  if let Some(flake_value) = &flake {
    let cmd = if nix_commands::nix_command_supports_flake_metadata(&flake_flags) { "metadata" } else { "info" };

    let metadata =
      nix_commands::get_flake_metadata(&flake_flags, cmd, &extra_metadata_flags, &extra_lock_flags, flake_value)?;
    let flake_value = metadata["url"].as_str().unwrap().to_string();

    if metadata["resolved"]["submodules"].as_bool().unwrap_or(false) {
      if flake_value.contains('?') {
        flake = Some(format!("{}&submodules=1", flake_value));
      } else {
        flake = Some(format!("{}?submodules=1", flake_value));
      }
    }
  }

  if action != Action::Build {
    if flake.is_some() {
      extra_build_flags.push("--no-link".to_string());
    } else {
      extra_build_flags.push("--no-out-link".to_string());
    }
  }

  if action == Action::Edit {
    let darwin_config = nix_commands::nix_instantiate_find_file("darwin-config")?;
    if let Some(flake) = &flake {
      nix_commands::exec_nix_edit(&flake_flags, &extra_lock_flags, flake, &flake_attr)?;
    } else {
      nix_commands::exec_editor(&darwin_config);
    }
  }

  let mut system_config = "".to_string();
  if action == Action::Switch || action == Action::Build || action == Action::Check {
    info!("building the system configuration...");
    if let Some(flake) = &flake {
      system_config =
        nix_commands::nix_flake_build(&flake_flags, &extra_build_flags, &extra_lock_flags, flake, &flake_attr)?;
    } else {
      system_config = nix_commands::nix_build("<darwin>", &extra_build_flags, "system")?;
    }
  }

  if action == Action::List || action == Action::Rollback {
    if !nix_commands::is_root_user() && !nix_commands::is_read_only(&profile)? {
      nix_commands::sudo_nix_env_profile(&profile, &extra_profile_flags)?;
    } else {
      nix_commands::nix_env_profile(&profile, &extra_profile_flags)?;
    }
  }

  if action == Action::Rollback {
    system_config = fs::read_to_string(format!("{}/systemConfig", profile)).unwrap();
  }

  if action == Action::Activate {
    system_config = nix_commands::get_real_path(env::args().next().unwrap().replace("/sw/bin/darwin-rebuild", ""))?;
  }

  if system_config.is_empty() {
    exit(0);
  }

  if action == Action::Switch {
    if !nix_commands::is_root_user() && !nix_commands::is_read_only(&profile)? {
      nix_commands::sudo_nix_env_set_profile(&profile, &system_config)?;
    } else {
      nix_commands::nix_env_set_profile(&profile, &system_config)?;
    }
  }

  if action == Action::Switch || action == Action::Activate || action == Action::Rollback {
    nix_commands::exec_activate_user(&system_config)?;
    if !nix_commands::is_root_user() {
      nix_commands::sudo_exec_activate(&system_config)?;
    } else {
      nix_commands::exec_activate(&system_config)?;
    }
  }

  if action == Action::Changelog {
    info!("\nCHANGELOG\n");
    nix_commands::print_changelog(&system_config)?;
  }

  if action == Action::Check {
    unsafe {
      env::set_var("checkActivation", "1");
    }
    nix_commands::exec_activate_user(&system_config)?;
  }

  Ok(())
}
