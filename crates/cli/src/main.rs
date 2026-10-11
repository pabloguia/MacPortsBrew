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
        .try_get_matches()
        .unwrap_or_else(|e| usage_error(&e));
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| usage_error(&e));
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut out = stdout.lock();
    let code = run(&cli, &mut out, &mut stderr.lock());
    // `process::exit` runs no destructor, so whatever is still buffered is
    // flushed here; a write that fails now (a closed pipe, a full disk) must
    // not leave a success code behind a truncated result.
    let code = match std::io::Write::flush(&mut out) {
        Ok(()) => code,
        Err(_) if code == 0 => 1,
        Err(_) => code,
    };
    std::process::exit(code);
}

/// Help and version go out as clap writes them. A usage error quotes the
/// command line (`--to <value>`), so a control or bidirectional character in
/// it is escaped line by line, as everywhere else quoted text is shown.
fn usage_error(e: &clap::Error) -> ! {
    use clap::error::ErrorKind;
    if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        e.exit();
    }
    let text = e.render().to_string();
    let shown: Vec<String> = text
        .lines()
        .map(macportsbrew_domain::escape_hostile)
        .collect();
    eprintln!("{}", shown.join("\n"));
    std::process::exit(e.exit_code());
}
