pub(crate) fn setup_logging(_verbose: bool) -> color_eyre::Result<()> {
  use color_eyre::Section;
  pretty_env_logger::try_init().map_err(|e| color_eyre::eyre::eyre!("unable to setup logging").with_error(|| e))
}
