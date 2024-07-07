#[macro_export]
macro_rules! print_bool {
  ($message:expr) => {{
    use color_eyre::owo_colors::{AnsiColors, OwoColorize};

    if $message {
      $message.color(AnsiColors::Green)
    } else {
      $message.color(AnsiColors::Red)
    }
  }};
  ($message:expr, $t:expr, $f:expr) => {{
    use color_eyre::owo_colors::{AnsiColors, OwoColorize};

    if $message {
      $t.color(AnsiColors::Green)
    } else {
      $f.color(AnsiColors::Red)
    }
  }};
}
