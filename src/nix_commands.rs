use std::{
  env,
  ffi::OsStr,
  fs,
  os::unix::process::CommandExt,
  process::{Command, Output},
};

use color_eyre::{
  eyre::{bail, eyre},
  owo_colors::OwoColorize,
};
use log::{debug, error, info, trace};
use serde_json::Value;
use subprocess::{Exec, Redirection};

use crate::{print_bool, DEFAULT_PROFILE};

type Result<T> = color_eyre::Result<T>;

trait GetCommand {
  fn get_command(&self) -> String;
}

impl GetCommand for Command {
  fn get_command(&self) -> String {
    let args = self.get_args().filter_map(|a| a.to_str()).collect::<Vec<_>>().join(" ");
    format!("{} {}", self.get_program().to_str().unwrap().yellow(), args.bright_yellow())
  }
}

#[cfg_attr(test, mockall::automock)]
pub trait RunCommand {
  fn exec_command(&mut self) -> std::io::Error;
  fn run_command(&mut self) -> color_eyre::Result<()>;
  fn run_command_with_output(&mut self) -> color_eyre::Result<Output>;
}

impl RunCommand for Command {
  fn exec_command(&mut self) -> std::io::Error {
    let command_call = self.get_command();
    trace!("Executing {command_call}");
    self.exec()
  }

  fn run_command(&mut self) -> color_eyre::Result<()> {
    handle_output_result(self)?;
    Ok(())
  }

  fn run_command_with_output(&mut self) -> color_eyre::Result<Output> { handle_output_result(self) }
}

fn handle_output_result(command: &mut Command) -> color_eyre::Result<Output> {
  let command_call = command.get_command();
  let output = command.output();

  match output {
    Ok(output) => {
      let code = output.status.code().ok_or(eyre!("unable to get status code for command output"))?;
      let status = print_bool!(output.status.success(), code, code);
      if !output.status.success() {
        error!("{command_call} -> {status}");
        error!("stdout: {stdout}", stdout = String::from_utf8_lossy(&output.stdout));
        error!("stderr: {stderr}", stderr = String::from_utf8_lossy(&output.stderr));
      } else {
        trace!("{command_call} -> {status}");
      }

      Ok(output)
    },
    Err(e) => {
      error!("an error occurred while calling {command_call}");
      Err(e.into())
    },
  }
}

/// Get the current hostname
pub fn get_local_hostname() -> Result<String> {
  info!("Getting local hostname");
  let hostname = gethostname::gethostname().into_string().map_err(|e| eyre!("unable to get hostname: {e:?}"));

  debug!("Local hostname: {hostname:?}");
  hostname
}

/// Check if the nix command supports flake metadata
pub fn nix_command_supports_flake_metadata<I, S>(flake_flags: I) -> bool
where
  I: IntoIterator<Item = S>,
  S: AsRef<OsStr>,
{
  debug!("checking if the nix command supports flakes");
  Command::new("nix").args(flake_flags).arg("flake").arg("metadata").arg("--version").run_command().is_ok()
}

pub fn get_flake_metadata<FlakeFlags, Cmd, MetadataFlags, ExtraLockFlags, Flake>(
  flake_flags: &[FlakeFlags], cmd: Cmd, extra_metadata_flags: &[MetadataFlags], extra_lock_flags: &[ExtraLockFlags],
  flake: Flake,
) -> Result<Value>
where
  FlakeFlags: AsRef<OsStr> + std::fmt::Debug,
  Cmd: AsRef<OsStr> + std::fmt::Debug,
  MetadataFlags: AsRef<OsStr> + std::fmt::Debug,
  ExtraLockFlags: AsRef<OsStr> + std::fmt::Debug,
  Flake: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Getting flake metadata {flake:?} {cmd:?} {extra_metadata_flags:?} {extra_lock_flags:?}");
  let output = Command::new("nix")
    .args(flake_flags)
    .arg("flake")
    .arg(cmd)
    .arg("--json")
    .args(extra_metadata_flags)
    .args(extra_lock_flags)
    .arg("--")
    .arg(flake)
    .run_command_with_output()?;

  serde_json::from_slice(&output.stdout).map_err(|e| e.into())
}

pub fn nix_instantiate_find_file(file: &str) -> Result<String> {
  let output = Command::new("nix-instantiate").arg("--find-file").arg(file).run_command_with_output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn exec_editor(file: &str) {
  let editor = env::var("EDITOR").unwrap_or("vi".to_string());
  Command::new(editor).arg(file).exec_command();
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
    .run_command()?;

  Ok(())
}

pub fn nix_build(expression: &str, extra_build_flags: &[String], attr: &str) -> Result<String> {
  debug!("Building the system configuration {expression} {extra_build_flags:?} {attr}");
  let output =
    Command::new("nix-build").arg(expression).args(extra_build_flags).arg("-A").arg(attr).run_command_with_output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn nix_flake_build(
  flake_flags: &[&str],
  extra_build_flags: &[String],
  extra_lock_flags: &[String],
  flake: &str,
  flake_attr: &str,
) -> Result<String> {
  debug!("Building the system configuration {flake} {flake_attr} {extra_build_flags:?} {extra_lock_flags:?}");
  let nom = true;
  if nom {
    let out_dir = tempfile::Builder::new().prefix("nix-darwin-").tempdir()?;
    let out_link = out_dir.path().join("result");
    let out_link_str = out_link.to_str().unwrap();
    debug!("out_dir: {:?}", out_dir);
    debug!("out_link: {:?}", out_link);

    let cmd = {
      Exec::cmd("nix")
        .args(flake_flags)
        .arg("build")
        .args(&["--log-format", "internal-json", "--verbose"])
        .args(&["--out-link", out_link_str])
        .args(extra_build_flags)
        .args(extra_lock_flags)
        .arg("--")
        .arg(format!("{}#{}.system", flake, flake_attr))
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Merge)
        | Exec::cmd("nom").args(&["--json"])
    }
    .stdout(Redirection::None);

    debug!("Cmd: {:?}", cmd);
    let result = cmd.join()?;
    debug!("result: {:?}", result);

    debug!("nvd diff {DEFAULT_PROFILE} {out_link_str}");
    Exec::cmd("nvd").args(&["diff", DEFAULT_PROFILE, out_link_str]).join()?;

    Ok(out_link_str.to_string())
  } else {
    let output = Command::new("nix")
      .args(flake_flags)
      .arg("build")
      .arg("--json")
      .args(extra_build_flags)
      .args(extra_lock_flags)
      .arg("--")
      .arg(format!("{}#{}.system", flake, flake_attr))
      .run_command_with_output()?;

    if output.status.success() {
      let json_output: Value = serde_json::from_slice(&output.stdout)?;

      json_output[0]["outputs"]["out"].as_str().map(|a| a.to_string()).ok_or(eyre!("unable to get output"))
    } else {
      bail!("Failed to run nix build: {}", String::from_utf8_lossy(&output.stderr).trim())
    }
  }
}

pub fn is_root_user() -> bool {
  debug!("Checking if the user is root");
  env::var("USER").unwrap() == "root"
}

pub fn is_read_only(path: &str) -> Result<bool> {
  debug!("Checking if {path} is read-only");
  let metadata = fs::metadata(path)?;
  Ok(metadata.permissions().readonly())
}

pub fn sudo_nix_env_profile(profile: &str, extra_profile_flags: &[String]) -> Result<()> {
  info!("Running sudo nix-env -p {profile} {extra_profile_flags:?}");
  let status = Command::new("sudo").arg("nix-env").arg("-p").arg(profile).args(extra_profile_flags).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env");
  }
}

pub fn nix_env_profile(profile: &str, extra_profile_flags: &[String]) -> Result<()> {
  info!("Running nix-env -p {profile} {extra_profile_flags:?}");
  let status = Command::new("nix-env").arg("-p").arg(profile).args(extra_profile_flags).status()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run nix-env");
  }
}

pub fn get_real_path(path: String) -> Result<String> {
  use std::{fs, path::Path};
  let canonical_path = fs::canonicalize(Path::new(&path))?;
  canonical_path.to_str().ok_or(eyre!("unable to get the real path of {path}")).map(|e| e.to_string())
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
