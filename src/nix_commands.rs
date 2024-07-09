use std::{env, ffi::OsStr, fs, path::Path};

use color_eyre::{
  eyre::{bail, eyre},
  owo_colors::OwoColorize,
};
use log::{debug, info};
use serde_json::Value;
use subprocess::{Exec, Redirection};

use crate::{print_bool, DEFAULT_PROFILE};

type Result<T> = color_eyre::Result<T>;

/// Get the current hostname
pub fn get_local_hostname() -> Result<String> {
  let hostname = gethostname::gethostname()
    .into_string()
    .map_err(|e| eyre!("unable to get hostname: {e:?}"))
    .inspect(|hostname| info!("Getting local hostname {}", hostname.purple().bold()));

  debug!("Local hostname: {hostname:?}");
  hostname
}

/// Check if the nix command supports flake metadata
pub fn nix_command_supports_flake_metadata<S>(flake_flags: &[S]) -> bool
where
  S: AsRef<OsStr>,
{
  debug!("checking if the nix command supports flakes");
  Exec::cmd("nix").args(flake_flags).arg("flake").arg("metadata").arg("--version").join().is_ok_and(|s| s.success())
}

#[deprecated]
pub fn get_flake_metadata<Flake, Cmd, FlakeFlags, MetadataFlags>(
  flake: Flake, cmd: Cmd, flake_flags: &[FlakeFlags], extra_metadata_flags: &[MetadataFlags],
) -> Result<Value>
where
  Flake: AsRef<OsStr> + std::fmt::Debug,
  Cmd: AsRef<OsStr> + std::fmt::Debug,
  FlakeFlags: AsRef<OsStr> + std::fmt::Debug,
  MetadataFlags: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Getting flake metadata {flake:?} {cmd:?} {extra_metadata_flags:?}");
  let output = Exec::cmd("nix")
    .args(flake_flags)
    .arg("flake")
    .arg(cmd)
    .arg("--json")
    .args(extra_metadata_flags)
    .arg("--")
    .arg(flake)
    .capture()?;

  serde_json::from_slice(&output.stdout).map_err(|e| e.into())
}

pub fn nix_instantiate_find_file<File>(file: File) -> Result<String>
where
  File: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Finding file {file:?}");
  let output = Exec::cmd("nix-instantiate").arg("--find-file").arg(file).capture()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn exec_editor<File>(file: File) -> Result<()>
where
  File: AsRef<OsStr> + std::fmt::Debug,
{
  let editor = env::var("EDITOR").unwrap_or("vi".to_string());
  Exec::cmd(editor).arg(file).join()?;
  Ok(())
}

#[deprecated]
pub fn nix_edit<Flake, FlakeAttr, FlakeFlagsItems>(
  flake: Flake, flake_attr: FlakeAttr, flake_flags: &[FlakeFlagsItems],
) -> Result<()>
where
  Flake: AsRef<OsStr> + std::fmt::Display,
  FlakeAttr: AsRef<OsStr> + std::fmt::Display,
  FlakeFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("editing flake {flake} {flake_attr} {flake_flags:?}");
  Exec::cmd("nix").args(flake_flags).arg("edit").arg("--").arg(format!("{}#{}", flake, flake_attr)).join()?;

  Ok(())
}

#[deprecated]
pub fn nix_build<Exp, Attr, OutDir, BuildFlagsItems>(
  expression: Exp, attr: Attr, out_dir: OutDir, extra_build_flags: &[BuildFlagsItems],
) -> Result<String>
where
  Exp: AsRef<OsStr> + std::fmt::Display,
  Attr: AsRef<OsStr> + std::fmt::Display,
  OutDir: AsRef<str> + std::fmt::Display,
  BuildFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Building the system configuration {expression} {attr} {extra_build_flags:?}");

  let args = vec!["--out-link", out_dir.as_ref()];
  let output =
    Exec::cmd("nix-build").arg(expression).args(extra_build_flags).args(&args).arg("-A").arg(attr).capture()?;
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
    if result.success() {
      Exec::cmd("ls").args(&["-l", out_dir.as_ref()]).join()?;
      debug!("nvd diff {DEFAULT_PROFILE} {out_dir}");
      Exec::cmd("nvd").args(&["diff", DEFAULT_PROFILE, out_dir.as_ref()]).join()?;

      Ok(out_dir.into())
    } else {
      bail!("Failed to build the system configuration")
    }
  } else {
    let output = Exec::cmd("nix")
      .args(flake_flags)
      .arg("build")
      .arg("--json")
      .args(extra_build_flags)
      .arg("--")
      .arg(format!("{}#{}.system", flake, flake_attr))
      .stdout(Redirection::None)
      .capture()?;

    if output.exit_status.success() {
      let json_output: Value = serde_json::from_slice(&output.stdout)?;

      json_output[0]["outputs"]["out"].as_str().map(|a| a.to_string()).ok_or(eyre!("unable to get output"))
    } else {
      bail!("Failed to run nix build: {}", String::from_utf8_lossy(&output.stderr).trim())
    }
  }
}

pub fn is_root_user() -> Result<bool> {
  const USERNAME: &str = "root";
  debug!("Checking if the user is {}", USERNAME.bold().yellow());
  Ok(env::var("USER").map_err(|e| eyre!("Unable to get user from env variables {}", e.red()))? == USERNAME)
}

pub fn is_read_only<P: AsRef<Path> + std::fmt::Display>(path: &P) -> Result<bool> {
  debug!("Checking if {} is read-only", path.yellow());
  let metadata = fs::metadata(path)?;
  let is_read_only = metadata.permissions().readonly();
  debug!("Is {} read-only: {}", path.yellow(), print_bool!(is_read_only, "readonly", "write allowed"));
  Ok(is_read_only)
}

pub fn sudo_nix_env_profile<Profile, ExtraProfileFlagsItems>(
  profile: Profile, extra_profile_flags: &[ExtraProfileFlagsItems],
) -> Result<()>
where
  Profile: AsRef<OsStr> + std::fmt::Display,
  ExtraProfileFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  info!("Running {}", format!("sudo nix-env -p {} {:?}", profile.yellow(), extra_profile_flags.blue()));
  let status = Exec::cmd("sudo").arg("nix-env").arg("-p").arg(profile).args(extra_profile_flags).join()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env");
  }
}

pub fn nix_env_profile<Profile, ExtraProfileFlagsItems>(
  profile: Profile, extra_profile_flags: &[ExtraProfileFlagsItems],
) -> Result<()>
where
  Profile: AsRef<OsStr> + std::fmt::Display,
  ExtraProfileFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  info!("Running {}", format!("nix-env -p {} {:?}", profile.yellow(), extra_profile_flags.blue()));
  let status = Exec::cmd("nix-env").arg("-p").arg(profile).args(extra_profile_flags).join()?;
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
  let status = Exec::cmd("sudo").arg("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).join()?;

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
  info!("Running {}", format!("nix-env -p {} --set {}", profile.yellow(), system_config.blue()));
  let status = Exec::cmd("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).join()?;

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
  let command = format!("{}/activate-user", system_config);
  info!("Running {}", command.yellow());
  let status = Exec::cmd(command).join()?;
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
  let command = format!("{}/activate", system_config);
  info!("Running {}", format!("sudo {}", command.yellow()));
  let status = Exec::cmd("sudo").arg(command).join()?;

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
  let command = format!("{}/activate", system_config);
  info!("Running {}", command.yellow());
  let status = Exec::cmd(command).join()?;

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
