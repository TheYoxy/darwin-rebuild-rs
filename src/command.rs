use std::process::{Command, Output};

use color_eyre::{eyre::eyre, owo_colors::OwoColorize};
use log::{error, trace};

use crate::print_bool;


trait GetCommand {
  fn get_command(&self) -> String;
}

impl GetCommand for Command {
  fn get_command(&self) -> String {
    let args = self.get_args().filter_map(|a| a.to_str()).collect::<Vec<_>>().join(" ");
    format!("{} {}", self.get_program().to_str().unwrap().yellow(), args.bright_yellow())
  }
}

pub trait RunCommand {
  fn run_command(&mut self) -> color_eyre::Result<()>;
  fn run_command_with_output(&mut self) -> color_eyre::Result<Output>;
}

impl RunCommand for Command {
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
