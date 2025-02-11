use color_eyre::owo_colors::OwoColorize;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
  fmt,
  fmt::{FormatEvent, FormatFields},
  registry::LookupSpan,
};
struct InfoFormatter;

impl<S, N> FormatEvent<S, N> for InfoFormatter
where
  S: Subscriber + for<'a> LookupSpan<'a>,
  N: for<'a> FormatFields<'a> + 'static,
{
  fn format_event(
    &self, ctx: &fmt::FmtContext<'_, S, N>, mut writer: fmt::format::Writer, event: &Event,
  ) -> std::fmt::Result {
    // Based on https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/trait.FormatEvent.html#examples
    // Without the unused parts
    let metadata = event.metadata();
    let level = metadata.level();

    if *level == Level::ERROR {
      write!(writer, "{} ", "!".red())?;
    } else if *level == Level::WARN {
      write!(writer, "{} ", "!".yellow())?;
    } else {
      write!(writer, "{} ", ">".green())?;
    }

    ctx.field_format().format_fields(writer.by_ref(), event)?;

    if *level != Level::INFO {
      if let (Some(file), Some(line)) = (metadata.file(), metadata.line()) {
        write!(writer, " @ {}:{}", file, line)?;
      }
    }

    writeln!(writer)?;
    Ok(())
  }
}

pub(crate) fn setup_logging(verbose: bool) -> color_eyre::Result<()> {
  use tracing_subscriber::{
    filter::{filter_fn, FilterExt},
    prelude::*,
    EnvFilter,
  };

  let layer_debug = fmt::layer()
    .with_writer(std::io::stderr)
    .without_time()
    .compact()
    .with_line_number(true)
    .with_filter(EnvFilter::from_default_env().or(filter_fn(move |_| verbose)))
    .with_filter(filter_fn(|meta| *meta.level() > Level::INFO));

  let layer_info = fmt::layer()
    .with_writer(std::io::stderr)
    .without_time()
    .with_target(false)
    .with_level(false)
    .event_format(InfoFormatter)
    .with_filter(filter_fn(|meta| {
      let level = *meta.level();
      (level == Level::INFO) || (level == Level::WARN)
    }));

  tracing_subscriber::registry().with(layer_debug).with(layer_info).init();

  tracing::trace!("Logging OK");

  Ok(())
}
