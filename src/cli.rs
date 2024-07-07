use std::{env, fs, os::unix::process::CommandExt, process::Command};

use clap::{command, Parser, Subcommand};
use color_eyre::eyre::{bail, eyre};
use serde_json::Value;

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
}

type Result<T> = color_eyre::Result<T>;
pub fn get_local_hostname() -> Option<String> {
  let output = Command::new("scutil").arg("--get").arg("LocalHostName").output().ok()?;
  Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn nix_command_supports_flake_metadata(flake_flags: &[&str]) -> bool {
  Command::new("nix").args(flake_flags).arg("flake").arg("metadata").arg("--version").output().is_ok()
}

pub fn get_flake_metadata(
  flake_flags: &[&str], cmd: &str, extra_metadata_flags: &[String], extra_lock_flags: &[String], flake: &str,
) -> Result<Value> {
  let output = Command::new("nix")
    .args(flake_flags)
    .arg("flake")
    .arg(cmd)
    .arg("--json")
    .args(extra_metadata_flags)
    .args(extra_lock_flags)
    .arg("--")
    .arg(flake)
    .output()?;

  serde_json::from_slice(&output.stdout).map_err(|e| e.into())
}

pub fn nix_instantiate_find_file(file: &str) -> Result<String> {
  let output = Command::new("nix-instantiate").arg("--find-file").arg(file).output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn exec_editor(file: &str) {
  let editor = env::var("EDITOR").unwrap_or("vi".to_string());
  Command::new(editor).arg(file).exec();
}

pub fn exec_nix_edit(
  flake_flags: &[&str], extra_lock_flags: &[String], flake: &String, flake_attr: &str,
) -> Result<()> {
  Command::new("nix")
    .args(flake_flags)
    .arg("edit")
    .args(extra_lock_flags)
    .arg("--")
    .arg(format!("{}#{}", flake, flake_attr))
    .output()?;

  Ok(())
}

pub fn nix_build(expression: &str, extra_build_flags: &[String], attr: &str) -> Result<String> {
  let output = Command::new("nix-build").arg(expression).args(extra_build_flags).arg("-A").arg(attr).output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn nix_flake_build(
  flake_flags: &[&str], extra_build_flags: &[String], extra_lock_flags: &[String], flake: &str, flake_attr: &str,
) -> Result<String> {
  let output = Command::new("nix")
    .args(flake_flags)
    .arg("build")
    .arg("--json")
    .args(extra_build_flags)
    .args(extra_lock_flags)
    .arg("--")
    .arg(format!("{}#{}.system", flake, flake_attr))
    .output()?;

  if output.status.success() {
    let json_output: Value = serde_json::from_slice(&output.stdout)?;

    json_output[0]["outputs"]["out"].as_str().map(|a| a.to_string()).ok_or(eyre!("unable to get output"))
  } else {
    bail!("Failed to run nix build: {}", String::from_utf8_lossy(&output.stderr).trim())
  }
}

pub fn is_root_user() -> bool { env::var("USER").unwrap() == "root" }

pub fn is_read_only(path: &str) -> Result<bool> {
  let metadata = fs::metadata(path)?;
  Ok(metadata.permissions().readonly())
}

pub fn sudo_nix_env_profile(profile: &str, extra_profile_flags: &[String]) -> Result<()> {
  let status = Command::new("sudo").arg("nix-env").arg("-p").arg(profile).args(extra_profile_flags).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env");
  }
}

pub fn nix_env_profile(profile: &str, extra_profile_flags: &[String]) -> Result<()> {
  let status = Command::new("nix-env").arg("-p").arg(profile).args(extra_profile_flags).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run nix-env");
  }
}

pub fn get_real_path(path: String) -> Result<String> {
  let output = Command::new("readlink").arg("-f").arg(path).output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn sudo_nix_env_set_profile(profile: &str, system_config: &str) -> Result<()> {
  let status = Command::new("sudo").arg("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).status()?;

  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env --set");
  }
}

pub fn nix_env_set_profile(profile: &str, system_config: &str) -> Result<()> {
  let status = Command::new("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run nix-env --set");
  }
}

pub fn exec_activate_user(system_config: &str) -> Result<()> {
  let status = Command::new(format!("{}/activate-user", system_config)).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run activate-user");
  }
}

pub fn sudo_exec_activate(system_config: &str) -> Result<()> {
  let status = Command::new("sudo").arg(format!("{}/activate", system_config)).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo activate");
  }
}

pub fn exec_activate(system_config: &str) -> Result<()> {
  let status = Command::new(format!("{}/activate", system_config)).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run activate");
  }
}

pub fn print_changelog(system_config: &str) -> Result<()> {
  let changelog = fs::read_to_string(format!("{}/darwin-changes", system_config))?;
  let lines: Vec<&str> = changelog.lines().take(32).collect();
  for line in lines {
    println!("{}", line);
  }
  Ok(())
}
