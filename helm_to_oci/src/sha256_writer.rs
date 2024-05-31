//! Copyright (c) Microsoft Corporation. All rights reserved.
//! Highly Confidential Material
use openssl::sha::Sha256;
use std::fmt::Write as _;
use std::io::{Result, Write};

/// Wraps a writer and calculates the sha256 digest of data written to the inner writer
pub(crate) struct Sha256Writer<W> {
    writer: W,
    sha: Sha256,
}

impl<W> Sha256Writer<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            writer,
            sha: Sha256::new(),
        }
    }

    /// Return the hex encoded sha256 digest of the written data, and the underlying writer
    pub(crate) fn finish(self) -> (String, W) {
        let mut digest = String::new();
        for byte in self.sha.finish().iter() {
            write!(digest, "{:02x}", byte).unwrap();
        }
        (digest, self.writer)
    }
}

impl<W> Write for Sha256Writer<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.writer.write(buf)?;
        self.sha.update(&buf[..len]);
        Ok(len)
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()
    }
}
