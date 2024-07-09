use std::{env, ffi::OsStr, fs, path::Path};

use color_eyre::{
  eyre::{bail, eyre},
  owo_colors::OwoColorize,
  Section, SectionExt,
};
use log::{debug, info, trace};
use serde_json::Value;
use subprocess::{Exec, Redirection};
use tracing::debug_span;

use crate::{print_bool, DEFAULT_PROFILE};

type Result<T> = color_eyre::Result<T>;

pub(crate) trait ExecTrace {
  fn trace(self) -> Self;
}
impl ExecTrace for Exec {
  fn trace(self) -> Self {
    let cmd = self.to_cmdline_lossy();
    let split = cmd.split(' ').collect::<Vec<_>>();
    let cmd = format!("{} {}", split[0].cyan(), split[1..].join(" ").yellow());
    debug_span!("Running command {cmd}");
    debug!("Running command {cmd}");

    self
  }
}

/// Get the current hostname
pub fn get_local_hostname() -> Result<String> {
  let hostname = gethostname::gethostname()
    .into_string()
    .map_err(|e| eyre!("unable to get hostname").with_section(|| format!("{:?}", e)))
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
  Exec::cmd("nix")
    .args(flake_flags)
    .arg("flake")
    .arg("metadata")
    .arg("--version")
    .trace()
    .join()
    .is_ok_and(|s| s.success())
}

pub fn get_flake_metadata<FlakeFlags, MetadataFlags>(
  flake: &(impl AsRef<OsStr> + std::fmt::Display + ?Sized), cmd: &(impl AsRef<OsStr> + std::fmt::Display + ?Sized),
  flake_flags: &[FlakeFlags], extra_metadata_flags: &[MetadataFlags],
) -> Result<Value>
where
  FlakeFlags: AsRef<OsStr> + std::fmt::Debug,
  MetadataFlags: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Getting flake metadata {} {} {:?}", flake.cyan(), cmd.yellow(), extra_metadata_flags.yellow());
  let output = Exec::cmd("nix")
    .args(flake_flags)
    .arg("flake")
    .arg(cmd)
    .arg("--json")
    .args(extra_metadata_flags)
    .arg("--")
    .arg(flake)
    .trace()
    .capture()?;

  serde_json::from_slice(&output.stdout).map_err(|e| eyre!("unable to parse flake metadata").with_error(|| e))
}

pub fn nix_instantiate_find_file(file: &(impl AsRef<OsStr> + std::fmt::Debug + ?Sized)) -> Result<String> {
  debug!("Finding file {file:?}");
  let output = Exec::cmd("nix-instantiate").arg("--find-file").arg(file).trace().capture()?;
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn exec_editor(file: &impl AsRef<OsStr>) -> Result<()> {
  #[cfg(test)]
  {
    Exec::cmd("nvim")
      .arg("-v")
      .arg(file)
      .trace()
      .stdout(subprocess::NullFile)
      .stderr(subprocess::NullFile)
      .join()
      .map(|_| ())
      .map_err(|e| eyre!("unable to open editor").with_error(|| e))
  }
  #[cfg(not(test))]
  {
    let editor = env::var("EDITOR").unwrap_or("vi".to_string());
    Exec::cmd(editor).arg(file).trace().join().map(|_| ()).map_err(|e| eyre!("unable to open editor").with_error(|| e))
  }
}

pub fn nix_edit<FlakeFlagsItems>(
  flake: &(impl AsRef<OsStr> + std::fmt::Display), flake_attr: &(impl AsRef<OsStr> + std::fmt::Display),
  flake_flags: &[FlakeFlagsItems],
) -> Result<()>
where
  FlakeFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("editing flake {flake} {flake_attr} {flake_flags:?}");
  Exec::cmd("nix").args(flake_flags).arg("edit").arg("--").arg(format!("{}#{}", flake, flake_attr)).trace().join()?;

  Ok(())
}

pub fn nix_build<BuildFlagsItems>(
  expression: &(impl AsRef<OsStr> + std::fmt::Display + ?Sized),
  attr: &(impl AsRef<OsStr> + std::fmt::Display + ?Sized), out_dir: &(impl AsRef<str> + std::fmt::Display),
  extra_build_flags: &[BuildFlagsItems],
) -> Result<String>
where
  BuildFlagsItems: AsRef<OsStr> + std::fmt::Debug,
{
  debug!("Building the system configuration {} {} {:?}", expression.blue(), attr.yellow(), extra_build_flags.blue());

  let args = vec!["--out-link", out_dir.as_ref()];
  let output =
    Exec::cmd("nix-build").arg(expression).args(extra_build_flags).args(&args).arg("-A").arg(attr).capture()?;
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if output.exit_status.success() {
    Ok(stdout)
  } else {
    Err(eyre!("Failed to build the system configuration").with_section(|| stdout))
  }
}

pub fn nix_flake_build<Attr, BuildFlagsItems>(
  flake: &(impl AsRef<OsStr> + std::fmt::Display), flake_attr: &(impl AsRef<OsStr> + std::fmt::Display),
  flake_flags: &[Attr], out_dir: &(impl AsRef<str> + std::fmt::Display), extra_build_flags: &[BuildFlagsItems],
) -> Result<String>
where
  Attr: AsRef<OsStr> + std::fmt::Debug,
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
        .args(&extra_build_flags)
        .arg("--")
        .arg(format!("{}#{}.system", flake, flake_attr))
        .trace()
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Merge)
        | Exec::cmd("nom").args(&["--json"])
    }
    .stdout(Redirection::None);

    let result = cmd.join()?;
    trace!("Result: {:?}", result.yellow());
    if result.success() {
      debug!("build succedded, printing diff");
      Exec::cmd("nvd").args(&["diff", DEFAULT_PROFILE, out_dir.as_ref()]).trace().join()?;

      Ok(out_dir.as_ref().to_string())
    } else {
      Err(eyre!("Failed to build the system configuration"))
    }
  } else {
    let output = Exec::cmd("nix")
      .args(flake_flags)
      .arg("build")
      .arg("--json")
      .args(&extra_build_flags)
      .arg("--")
      .arg(format!("{}#{}.system", flake, flake_attr))
      .stdout(Redirection::None)
      .capture()?;

    if output.exit_status.success() {
      let json_output: Value = serde_json::from_slice(&output.stdout)?;

      json_output[0]["outputs"]["out"].as_str().map(|a| a.to_string()).ok_or(
        eyre!("unable to get output").with_section(|| {
          let stdout = String::from_utf8_lossy(&output.stdout);
          stdout.to_string().header("stdout: ")
        }),
      )
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr);
      Err(eyre!("Failed to run nix build")).with_section(|| stderr.to_string().header("stderr: "))
    }
  }
}
pub fn is_root_user() -> Result<bool> {
  const USERNAME: &str = "root";
  debug!("Checking if the user is {}", USERNAME.bold().yellow());
  let user = env::var("USER").map_err(|e| eyre!("Unable to get user from env variables").with_error(|| e))?;
  Ok(user == USERNAME)
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
  Profile: AsRef<OsStr>,
  ExtraProfileFlagsItems: AsRef<OsStr>,
{
  let status = Exec::cmd("sudo").arg("nix-env").arg("-p").arg(profile).args(extra_profile_flags).trace().join()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run sudo nix-env");
  }
}

pub fn nix_env_profile(profile: &impl AsRef<OsStr>, extra_profile_flags: &[&impl AsRef<OsStr>]) -> Result<()> {
  let status = Exec::cmd("nix-env").arg("-p").arg(profile).args(extra_profile_flags).trace().join()?;
  if status.success() {
    Ok(())
  } else {
    bail!("Failed to run nix-env");
  }
}

pub fn get_real_path(path: &(impl AsRef<Path> + std::fmt::Debug)) -> Result<String> {
  let canonical_path = std::fs::canonicalize(&path)?;
  canonical_path.to_str().ok_or(eyre!("unable to get the real path of {path:?}")).map(|e| e.to_string())
}

pub trait SetProfile {
  fn sudo_nix_env_set_profile(profile: &impl AsRef<OsStr>, system_config: &impl AsRef<OsStr>) -> Result<()>;

  fn nix_env_set_profile(profile: &impl AsRef<OsStr>, system_config: &impl AsRef<OsStr>) -> Result<()>;
}

impl SetProfile for () {
  fn sudo_nix_env_set_profile(profile: &impl AsRef<OsStr>, system_config: &impl AsRef<OsStr>) -> Result<()> {
    let status =
      Exec::cmd("sudo").arg("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).trace().join()?;

    if status.success() {
      Ok(())
    } else {
      bail!("Failed to run sudo nix-env --set");
    }
  }

  fn nix_env_set_profile(profile: &impl AsRef<OsStr>, system_config: &impl AsRef<OsStr>) -> Result<()> {
    let status = Exec::cmd("nix-env").arg("-p").arg(profile).arg("--set").arg(system_config).trace().join()?;
    if status.success() {
      Ok(())
    } else {
      bail!("Failed to run nix-env --set");
    }
  }
}

pub fn exec_activate_user<SystemConfig>(system_config: &SystemConfig) -> Result<()>
where
  SystemConfig: std::fmt::Display,
{
  let command = format!("{}/activate-user", system_config);
  let status = Exec::cmd(command).trace().join()?;
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
  let status = Exec::cmd("sudo").arg(command).trace().join()?;

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
  let status = Exec::cmd(command).trace().join()?;

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
