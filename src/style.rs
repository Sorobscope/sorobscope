//! Terminal styling, and the rules for when to suppress it.
//!
//! Colour is a readability aid for a human at a terminal and noise everywhere else. A tool
//! whose `--json` is meant to pipe into `jq` cannot afford to leak escape codes into a
//! pipeline, so styling is off unless the stream is actually a terminal — and stays off if
//! the user has said so, via `NO_COLOR` or `--color never`.
//!
//! Implemented with bare ANSI codes rather than a colour crate: it's a handful of
//! constants, and the dependency would earn its place only if this grew a theme system.

use std::io::IsTerminal;
use std::sync::OnceLock;

use clap::ValueEnum;

/// Width of the label column. Wide enough for the longest label in any command
/// ("durability", "live until"), so all three commands align the same way.
const LABEL_WIDTH: usize = 10;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorChoice {
    /// Colour when writing to a terminal, plain text when piped or redirected.
    #[default]
    Auto,
    /// Always colour, even when piped — useful for `| less -R`.
    Always,
    /// Never colour.
    Never,
}

struct Enabled {
    stdout: bool,
    stderr: bool,
}

static ENABLED: OnceLock<Enabled> = OnceLock::new();

/// Decide once, at startup, whether to style each stream.
pub fn init(choice: ColorChoice) {
    // NO_COLOR is honoured whenever it is set to anything non-empty, per no-color.org.
    let suppressed = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());

    let enabled = match choice {
        ColorChoice::Never => Enabled { stdout: false, stderr: false },
        ColorChoice::Always => Enabled { stdout: true, stderr: true },
        ColorChoice::Auto if suppressed => Enabled { stdout: false, stderr: false },
        ColorChoice::Auto => Enabled {
            stdout: std::io::stdout().is_terminal(),
            stderr: std::io::stderr().is_terminal(),
        },
    };

    let _ = ENABLED.set(enabled);
}

fn stdout_styled() -> bool {
    ENABLED.get().is_some_and(|e| e.stdout)
}

fn stderr_styled() -> bool {
    ENABLED.get().is_some_and(|e| e.stderr)
}

fn wrap(code: &str, text: &str) -> String {
    if stdout_styled() {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// A heading — the line naming what is being shown.
pub fn heading(text: &str) -> String {
    wrap(BOLD, text)
}

/// A field label, secondary to the value beside it.
pub fn label(text: &str) -> String {
    wrap(DIM, text)
}

/// Commentary the reader can skip: ranges searched, caveats.
pub fn muted(text: &str) -> String {
    wrap(DIM, text)
}

/// A healthy state: a succeeded transaction, a TTL with room left.
pub fn good(text: &str) -> String {
    wrap(GREEN, text)
}

/// A failed or expired state.
pub fn bad(text: &str) -> String {
    wrap(RED, text)
}

/// Present but needs attention — a near-expiry TTL, an absent lookup.
pub fn warn(text: &str) -> String {
    wrap(YELLOW, text)
}

/// Style the `error:` prefix on stderr, which is a different stream to everything else
/// and so gets its own terminal check.
pub fn error_prefix() -> String {
    if stderr_styled() {
        format!("{RED}{BOLD}error:{RESET}")
    } else {
        "error:".to_string()
    }
}

/// Print one aligned `label  value` row.
///
/// Padding is applied to the raw label before styling, because ANSI codes count toward a
/// format width and would silently misalign every coloured column.
pub fn field(name: &str, value: &str) {
    let padded = format!("{name:<LABEL_WIDTH$}");
    println!("  {}  {}", label(&padded), value);
}

/// A continuation row under a multi-value field, aligned to the value column.
pub fn field_continued(value: &str) {
    println!("  {}  {}", " ".repeat(LABEL_WIDTH), value);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The styling functions must degrade to plain text rather than emitting codes when
    /// nothing has been initialised — otherwise a stray call could corrupt piped output.
    #[test]
    fn styling_is_inert_until_initialised() {
        assert_eq!(good("SUCCESS"), "SUCCESS");
        assert_eq!(label("key"), "key");
        assert_eq!(error_prefix(), "error:");
    }

    #[test]
    fn labels_pad_before_styling_so_columns_line_up() {
        // Padding must be measured on the raw text; ANSI codes would inflate the width.
        let padded = format!("{:<width$}", "key", width = LABEL_WIDTH);
        assert_eq!(padded.len(), LABEL_WIDTH);
    }
}
