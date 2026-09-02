// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew command-line entry point. `macportsbrew` and `mpb` are the
//! same program (`docs/spec/06-cli-and-ux.md`).

use clap::Parser;
use macportsbrew::app::{run, Cli};

fn main() {
    let cli = Cli::parse();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let code = run(&cli, &mut stdout.lock(), &mut stderr.lock());
    std::process::exit(code);
}
