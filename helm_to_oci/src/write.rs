//! Functions for writing messages.
//!
//! Copyright (c) Microsoft Corporation. All rights reserved.
//! Highly Confidential Material
use std::fmt::Display;
use std::io::{self, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

fn msg(label: &str, message: impl Display, color: &ColorSpec) -> io::Result<()> {
    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    stderr.set_color(color)?;
    write!(&mut stderr, "{:>20} ", label)?;
    stderr.set_color(ColorSpec::new().set_fg(None))?;
    writeln!(&mut stderr, "{}", message)?;
    Ok(())
}

/// Write an ok message to stderr
///
/// # Errors
///
/// Will return `Err` if a problem is encountered writing to stderr
pub fn ok(label: &str, message: impl Display) -> io::Result<()> {
    msg(label, message, ColorSpec::new().set_fg(Some(Color::Green)))
}

/// Write an error message to stderr
///
/// # Errors
///
/// Will return `Err` if a problem is encountered writing to stderr
pub fn error(label: &str, message: impl Display) -> io::Result<()> {
    msg(
        label,
        message,
        ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true),
    )
}
