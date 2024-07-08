use std::{
  env,
  ffi::OsStr,
  fs,
  os::unix::process::CommandExt,
  path::Path,
  process::{Command, ExitStatus, Output},
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
  fn status_command(&mut self) -> color_eyre::Result<ExitStatus>;
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

  fn status_command(&mut self) -> color_eyre::Result<ExitStatus> {
    let command_call = self.get_command();
    debug!("Running {command_call}");
    self.status().map_err(|e| e.into())
  }
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

pub fn get_flake_metadata<FlakeFlags, Cmd, MetadataFlags, Flake>(
  flake_flags: &[FlakeFlags], cmd: Cmd, extra_metadata_flags: &[MetadataFlags], flake: Flake,
) -> Result<Value>
where
  FlakeFlags: AsRef<OsStr> + std::fmt::Debug,
  Cmd: AsRef<OsStr> + std::fmt::Debug,
  MetadataFlags: AsRef<OsStr> + std::fmt::Debug,
  Flake: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Getting flake metadata {flake:?} {cmd:?} {extra_metadata_flags:?}");
  let output = Command::new("nix")
    .args(flake_flags)
    .arg("flake")
    .arg(cmd)
    .arg("--json")
    .args(extra_metadata_flags)
    .arg("--")
    .arg(flake)
    .run_command_with_output()?;

  serde_json::from_slice(&output.stdout).map_err(|e| e.into())
}

pub fn nix_instantiate_find_file<File>(file: File) -> Result<String>
where
  File: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Finding file {file:?}");
  let output = Command::new("nix-instantiate").arg("--find-file").arg(file).run_command_with_output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn exec_editor<File>(file: File)
where
  File: AsRef<OsStr> + std::fmt::Debug,
{
  let editor = env::var("EDITOR").unwrap_or("vi".to_string());
  Command::new(editor).arg(file).exec_command();
}

pub fn exec_nix_edit<Flake, FlakeAttr, FlakeFlags, FlakeFlagsItems>(
  flake: Flake, flake_attr: FlakeAttr, flake_flags: FlakeFlags,
) -> Result<()>
where
  Flake: AsRef<OsStr> + std::fmt::Display,
  FlakeAttr: AsRef<OsStr> + std::fmt::Display,
  FlakeFlags: IntoIterator<Item = FlakeFlagsItems> + std::fmt::Debug,
  FlakeFlagsItems: AsRef<OsStr>,
{
  debug!("editing flake {flake} {flake_attr} {flake_flags:?}");
  Command::new("nix").args(flake_flags).arg("edit").arg("--").arg(format!("{}#{}", flake, flake_attr)).run_command()?;

  Ok(())
}

pub fn nix_build<Exp, Attr, BuildFlags, OutDir, BuildFlagsItems>(
  expression: Exp, attr: Attr, out_dir: OutDir, extra_build_flags: BuildFlags,
) -> Result<String>
where
  Exp: AsRef<OsStr> + std::fmt::Display,
  Attr: AsRef<OsStr> + std::fmt::Display,
  OutDir: AsRef<str> + std::fmt::Display,
  BuildFlags: IntoIterator<Item = BuildFlagsItems> + std::fmt::Debug,
  BuildFlagsItems: AsRef<OsStr>,
{
  debug!("Building the system configuration {expression} {attr} {extra_build_flags:?}");

  let args = vec!["--out-link", out_dir.as_ref()];
  let output = Command::new("nix-build")
    .arg(expression)
    .args(extra_build_flags)
    .args(&args)
    .arg("-A")
    .arg(attr)
    .run_command_with_output()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn nix_flake_build<Flake, FlakeAttr, FlakeFlagsItems, OutDir, BuildFlagsItems>(
  flake: Flake, flake_attr: FlakeAttr, flake_flags: &[FlakeFlagsItems], out_dir: OutDir,
  extra_build_flags: &[BuildFlagsItems],
) -> Result<String>
where
  Flake: AsRef<OsStr> + std::fmt::Display,
  FlakeAttr: AsRef<OsStr> + std::fmt::Display,
  OutDir: AsRef<str> + std::fmt::Display + Into<String>,
  FlakeFlagsItems: AsRef<OsStr> + std::fmt::Debug,
  BuildFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  debug!(
    "Building the system configuration {} {} {:?} {:?}",
    flake.blue(),
    flake_attr.yellow(),
    flake_flags.cyan(),
    extra_build_flags.blue()
  );
  let nom = true;
  if nom {
    let args = vec!["--out-link", out_dir.as_ref()];
    let cmd = {
      Exec::cmd("nix")
        .args(flake_flags)
        .arg("build")
        .args(&["--log-format", "internal-json", "-v"])
        .args(&args)
        .args(extra_build_flags)
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

    Exec::cmd("ls").args(&["-l", out_dir.as_ref()]).join()?;
    debug!("nvd diff {DEFAULT_PROFILE} {out_dir}");
    Exec::cmd("nvd").args(&["diff", DEFAULT_PROFILE, out_dir.as_ref()]).join()?;

    Ok(out_dir.into())
  } else {
    let output = Command::new("nix")
      .args(flake_flags)
      .arg("build")
      .arg("--json")
      .args(extra_build_flags)
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

pub fn is_read_only<P: AsRef<Path> + std::fmt::Display>(path: &P) -> Result<bool> {
  debug!("Checking if {} is read-only", path.yellow());
  let metadata = fs::metadata(path)?;
  let is_read_only = metadata.permissions().readonly();
  debug!("Is {} read-only: {}", path.yellow(), print_bool!(is_read_only, "readonly", "write allowed"));
  Ok(is_read_only)
}

pub fn sudo_nix_env_profile<Profile, ExtraProfileFlags, ExtraProfileFlagsItems>(
  profile: Profile, extra_profile_flags: ExtraProfileFlags,
) -> Result<()>
where
  Profile: AsRef<OsStr> + std::fmt::Display,
  ExtraProfileFlags: IntoIterator<Item = ExtraProfileFlagsItems> + std::fmt::Debug,
  ExtraProfileFlagsItems: AsRef<OsStr>,
{
  info!("Running sudo nix-env -p {profile} {extra_profile_flags:?}");
  let status = Command::new("sudo").arg("nix-env").arg("-p").arg(profile).args(extra_profile_flags).status_command()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env");
  }
}

pub fn nix_env_profile<Profile, ExtraProfileFlags, ExtraProfileFlagsItems>(
  profile: Profile, extra_profile_flags: ExtraProfileFlags,
) -> Result<()>
where
  Profile: AsRef<OsStr> + std::fmt::Display,
  ExtraProfileFlags: IntoIterator<Item = ExtraProfileFlagsItems> + std::fmt::Debug,
  ExtraProfileFlagsItems: AsRef<OsStr>,
{
  info!("Running nix-env -p {profile} {extra_profile_flags:?}");
  let status = Command::new("nix-env").arg("-p").arg(profile).args(extra_profile_flags).status_command()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run nix-env");
  }
}

pub fn get_real_path<S: AsRef<Path> + std::fmt::Debug>(path: S) -> Result<String> {
  let canonical_path = std::fs::canonicalize(&path)?;
  canonical_path.to_str().ok_or(eyre!("unable to get the real path of {path:?}")).map(|e| e.to_string())
}

pub fn sudo_nix_env_set_profile<Profile, SystemConfig>(profile: Profile, system_config: SystemConfig) -> Result<()>
where
  Profile: AsRef<OsStr> + std::fmt::Display,
  SystemConfig: AsRef<OsStr> + std::fmt::Display,
{
  info!("Running {}", format!("sudo nix-env -p {} --set {}", profile.yellow(), system_config.blue()));
  let status =
    Command::new("sudo").arg("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).status_command()?;

  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env --set");
  }
}

pub fn nix_env_set_profile<Profile, SystemConfig>(profile: Profile, system_config: SystemConfig) -> Result<()>
where
  Profile: AsRef<OsStr> + std::fmt::Display,
  SystemConfig: AsRef<OsStr> + std::fmt::Display,
{
  let status = Command::new("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).status_command()?;

  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run nix-env --set");
  }
}

pub fn exec_activate_user<SystemConfig>(system_config: &SystemConfig) -> Result<()>
where
  SystemConfig: std::fmt::Display,
{
  let status = Command::new(format!("{}/activate-user", system_config)).status_command()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run activate-user");
  }
}

pub fn sudo_exec_activate<SystemConfig>(system_config: &SystemConfig) -> Result<()>
where
  SystemConfig: std::fmt::Display,
{
  let status = Command::new("sudo").arg(format!("{}/activate", system_config)).status_command()?;

  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo activate");
  }
}

pub fn exec_activate<SystemConfig>(system_config: &SystemConfig) -> Result<()>
where
  SystemConfig: std::fmt::Display,
{
  let status = Command::new(format!("{}/activate", system_config)).status_command()?;

  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run activate");
  }
}

pub fn print_changelog<SystemConfig>(system_config: SystemConfig) -> Result<()>
where
  SystemConfig: std::fmt::Display,
{
  let file = format!("{}/darwin-changes", system_config);
  debug!("Printing changelog for {}", file.yellow());
  let changelog = fs::read_to_string(file)?;
  let lines: Vec<&str> = changelog.lines().take(32).collect();
  for line in lines {
    println!("{}", line);
  }
  Ok(())
}
