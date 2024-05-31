//! Copyright (c) Microsoft Corporation. All rights reserved.
//! Highly Confidential Material
use clap::Parser;

fn main() {
    let config = helm_to_oci::Cli::parse();
    if let Err(e) = helm_to_oci::run(config) {
        eprintln!("Error: {}", e);
    }
}
