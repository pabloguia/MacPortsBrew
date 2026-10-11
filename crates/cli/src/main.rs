// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew command-line entry point. `macportsbrew` and `mpb` are the
//! same program (`docs/spec/06-cli-and-ux.md`).

use clap::{CommandFactory, FromArgMatches};
use macportsbrew::app::{run, version, Cli};

fn main() {
    // `--version` prints the policy data, target and commit as well (pass 09,
    // D-V1); `-V` stays `macportsbrew <version>`.
    let matches = Cli::command()
        .long_version(version::long_version_of_this_build())
        .get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let code = run(&cli, &mut stdout.lock(), &mut stderr.lock());
    std::process::exit(code);
}
