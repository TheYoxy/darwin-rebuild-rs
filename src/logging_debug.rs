pub(crate) fn setup_logging(_verbose: bool) -> color_eyre::Result<()> {
  pretty_env_logger::try_init().map_err(|e| e.into())
}
