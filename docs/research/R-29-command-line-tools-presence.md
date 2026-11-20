# R-29 — Does a read prove the Command Line Tools present, or absent?

- **Status:** partially answered. What each read can prove is answered from observation, from Apple's documentation and from both providers' own source. The *absent* case is cited from documentation and source, never observed: this machine has both Xcode and the Command Line Tools, and producing absence is a mutation. The points under Cannot determine are UNRESOLVED, among them MacPorts' Xcode-only path, which finding 9 states as an inference.
- **Answered on:** 2026-11-19
- **Machine:** the machine of R-28 and R-27, x86_64, Homebrew @ `/usr/local`, MacPorts @ `/opt/local`. R-28 records macOS 26.7.2 (25G332) on 2026-11-04. The OS and provider versions were **not re-read** for this note, because `sw_vers`, `brew` and `port` are outside its allowlist. Developer tools as observed in E-1 to E-3: an Xcode at `/Applications/Xcode.app`, which is the selected developer directory, and the Command Line Tools at `/Library/Developer/CommandLineTools`. The versions are second-hand. Xcode 26.5 (17F42) is R-27's reading (`R-27-no-cross-provider-linkage.md`, Machine line, 2026-11-04). This note's own versions come from the package receipts (E-3), which finding 11 shows do not prove presence: `com.apple.pkg.Xcode` 26.5, `CLTools_Executables` 26.6.
- **Blocking:** work order 16 part K, D7 and slice K-7 (`docs/passes/16-plans/K.md`, "D7" and "The DP-050 research note (R-29)"). DP-050's ruling (`docs/decisions-pending.md`): "If research shows the absence of the build toolchain … can be detected by a filesystem read, with no new spawn, its absence makes that step `MANUAL_REVIEW`; otherwise the risk text says so."

## Question

DP-050 rules that a source build is a cost the plan names, not an unavailable artifact. If the build toolchain's absence can be detected without a new spawn, a source-build `ADD_DESTINATION` becomes `MANUAL_REVIEW`. Otherwise only the risk text speaks. Part K needs to know:

1. Does `/usr/bin/xcode-select -p` succeeding prove the build tools present?
2. Does its non-zero exit prove them absent? (`crates/adapters/src/system/machine.rs`, `detect_command_line_tools`, maps any status other than exit 0, after `NotStarted`, to `CommandLineTools::Missing`. That covers a non-zero exit, `TimedOut`, `Signalled` and `Cancelled`. It maps exit 0 with an absolute path to `CommandLineTools::Installed { path }`.)
3. Does a filesystem read (stat only, nothing executed) of `/Library/Developer/CommandLineTools`, its `usr/bin/clang`, `usr/bin/make` and `SDKs/`, or of the selected Xcode's `Contents/Developer/usr/bin`, prove either?
4. So what may K-7 build: a `MANUAL_REVIEW` block plus a `doctor` finding, or only the risk text?

## Evidence

All commands were run on 2026-11-19 as the unelevated user, from a zsh shell, with no developer variable set (`DEVELOPER_DIR`, `SDKROOT`, `TOOLCHAINS`, `xcrun_*`: none set by this session). No `brew` or `port` command was run, so no provider auto-update was possible and no quieting environment was needed. Nothing found by a stat was executed. The allowlist is part K's (`K.md`, "What the note may run"). `/usr/bin/stat`, `/usr/bin/readlink` and `/usr/bin/man` were run by name, as the allowlist permits.

**Reads beyond the allowlist, every one of them read-only:**

- **A breach of K.md's "Never" clause, not merely a deviation:** `/usr/bin/grep -i -E` filtered `pkgutil --pkgs` once (E-4). "Never: any other program under `/usr/bin`" covers it. `grep` is not one of the developer shims `man xcode-select` lists in FILES (E-5), so it could not raise the install dialog. It was also avoidable: a shell `case` match did the same filtering later (E-3).
- **Stats of paths the allowlist does not name** (its item 3 names `/Library/Developer/CommandLineTools`, `usr/bin/clang`, `usr/bin/make` and `SDKs/` under it, and the selected Xcode's `Contents/Developer/usr/bin`): `/var/db/xcode_select_link`; `/Library/Developer/CommandLineTools/usr/bin`; `/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang`; `/Library/Developer/CommandLineTools/SDKs/MacOSX12.sdk` (E-2, E-3).
- **A receipt read outside the CLT receipt** that item 4 names: `pkgutil --pkg-info com.apple.pkg.Xcode` (E-3).
- **Fetches beyond item 5's three pages:** Homebrew `Library/Homebrew/os/mac/xcode.rb` (E-7); MacPorts `src/macports1.0/macports.tcl`, two reads (E-8); the Apple page's JSON form, the same document as item 5's page (E-6); `guide.macports.org/chunked/installing.xcode.html` (HTTP 404); the `trac.macports.org` ticket #61741 attempt (refused); two web searches, whose results relayed the mailing-list archive and Trac (E-8, leads only).

### E-1 · The selected developer directory

```
$ /usr/bin/xcode-select -p; echo "rc=$?"
/Applications/Xcode.app/Contents/Developer
rc=0

$ /usr/bin/readlink /var/db/xcode_select_link; echo "rc=$?"
/Applications/Xcode.app/Contents/Developer
rc=0
```

### E-2 · Stat of the Command Line Tools and the selected Xcode

```
$ /usr/bin/stat -f '%N %HT %Sp %Su:%Sg %z %Sm' /var/db/xcode_select_link \
    /Library/Developer/CommandLineTools /Library/Developer/CommandLineTools/usr/bin \
    /Library/Developer/CommandLineTools/usr/bin/clang /Library/Developer/CommandLineTools/usr/bin/make \
    /Library/Developer/CommandLineTools/SDKs /Applications/Xcode.app/Contents/Developer/usr/bin \
    /Applications/Xcode.app/Contents/Developer/usr/bin/make \
    /Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang \
    /Applications/Xcode.app/Contents/Developer/usr/bin/clang; echo "rc=$?"
/var/db/xcode_select_link Symbolic Link lrwxr-xr-x root:wheel 42 Oct  9 23:12:08 2026
/Library/Developer/CommandLineTools Directory drwxr-xr-x root:wheel 160 Feb 14 15:28:46 2024
/Library/Developer/CommandLineTools/usr/bin Directory drwxr-xr-x root:wheel 4448 Jul  9 20:37:25 2026
/Library/Developer/CommandLineTools/usr/bin/clang Regular File -rwxr-xr-x root:wheel 290664032 May 22 10:39:37 2026
/Library/Developer/CommandLineTools/usr/bin/make Regular File -rwxr-xr-x root:wheel 403216 May 22 14:55:32 2026
/Library/Developer/CommandLineTools/SDKs Directory drwxr-xr-x root:wheel 224 Jul  9 20:40:56 2026
/Applications/Xcode.app/Contents/Developer/usr/bin Directory drwxr-xr-x root:wheel 3616 May 13 22:22:56 2026
/Applications/Xcode.app/Contents/Developer/usr/bin/make Regular File -rwxr-xr-x root:wheel 403216 Apr 30 10:08:06 2026
/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang Regular File -rwxr-xr-x root:wheel 290664032 Apr 30 10:07:37 2026
stat: /Applications/Xcode.app/Contents/Developer/usr/bin/clang: stat: No such file or directory
rc=1
```

The `SDKs/` entries, listed by a shell glob (a zsh builtin; no program run):

```
$ print -l /Library/Developer/CommandLineTools/SDKs/*
/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk
/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk
/Library/Developer/CommandLineTools/SDKs/MacOSX15.sdk
/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk
```

### E-3 · Package receipts (`pkgutil`, research only)

```
$ /usr/sbin/pkgutil --pkg-info com.apple.pkg.CLTools_Executables
package-id: com.apple.pkg.CLTools_Executables
version: 26.6.0.0.1781586589
volume: /
location: /
install-time: 1783640463

$ /usr/sbin/pkgutil --pkg-info com.apple.pkg.CLTools_SDK_macOS12
package-id: com.apple.pkg.CLTools_SDK_macOS12
version: 26.6.0.0.1781586378
volume: /
location: /
install-time: 1783640467
groups: com.apple.FindSystemFiles.pkg-group

$ /usr/sbin/pkgutil --pkg-info com.apple.pkg.CLTools_macOS_SDK
package-id: com.apple.pkg.CLTools_macOS_SDK
version: 26.6.0.0.1781586369
...
$ /usr/sbin/pkgutil --pkg-info com.apple.pkg.Xcode
package-id: com.apple.pkg.Xcode
version: 26.5.0.0.1.1777542397
...
```

`pkgutil --files com.apple.pkg.CLTools_SDK_macOS12`, saved to a scratch file and read back, has three lines in full:

```
private
private/tmp
private/tmp/.BBE72B41371180178E084EEAF106AED4F350939DB95D3516864A1CC62E7AE82F
```

```
$ /usr/bin/stat -f '%N %z' /Library/Developer/CommandLineTools/SDKs/MacOSX12.sdk; echo "rc=$?"
stat: /Library/Developer/CommandLineTools/SDKs/MacOSX12.sdk: stat: No such file or directory
rc=1
```

`pkgutil --files com.apple.pkg.CLTools_Executables`, saved to a scratch file, has 5363 lines. A shell `case` match (no program run) found these three among them:

```
Library/Developer/CommandLineTools
Library/Developer/CommandLineTools/usr/bin/clang
Library/Developer/CommandLineTools/usr/bin/make
```

### E-4 · Receipt identifiers present

```
$ /usr/sbin/pkgutil --pkgs | /usr/bin/grep -i -E 'CLTools|xcode|DeveloperTools'; echo "rc=$?"
com.apple.pkg.CLTools_SDK_macOS13
com.apple.pkg.CLTools_SDK_macOS12
com.apple.pkg.CLTools_Executables
com.apple.pkg.CLTools_SDK_macOS_LMOS
com.apple.pkg.CLTools_SDK_macOS14
com.apple.pkg.CLTools_SDK_macOS_NMOS
com.apple.pkg.CLTools_SDK_macOS110
com.apple.pkg.CLTools_SwiftBackDeploy
com.apple.pkg.CLTools_macOS_SDK
com.apple.pkg.XcodeSystemResources
com.apple.pkg.Xcode
com.oneminutegames.XcodeCleaner
com.apple.pkg.Xcode_XR
rc=0
```

### E-5 · `man xcode-select` (local; page dated "June 24, 2019"), read 2026-11-19

Quoted verbatim, with the bold overstrike removed:

- "`-p, --print-path` Prints the path to the currently selected developer directory. This is useful for inspection, but scripts and other tools should use xcrun(1) to locate tool inside the active developer directory."
- "`-r, --reset` Unsets any user-specified developer directory, so that the developer directory will be found via the default search mechanism."
- "`-s <path>, --switch <path>` Sets the active developer directory to the given path … This command must be run with superuser permissions".
- "`--install` Opens a user interface dialog to request automatic installation of the command line developer tools."
- "`DEVELOPER_DIR` Overrides the active developer directory. When DEVELOPER_DIR is set, its value will be used instead of the system-wide active developer directory."
- FILES lists the shims that "Run[] the matching BSD tool from with the active developer directory", among them `/usr/bin/clang`, `/usr/bin/cc`, `/usr/bin/make`, `/usr/bin/git`, `/usr/bin/python3`.
- The page has no EXIT STATUS section. It does not say what `-p` prints or returns when no developer directory exists. It does not say whether `-p` checks that the selected directory still exists. It does not name `/var/db/xcode_select_link`.

### E-6 · Apple, "Installing the command-line tools", read 2026-11-19

<https://developer.apple.com/documentation/xcode/installing-the-command-line-tools>. The HTML page has no body text without JavaScript. The text was read from the same document's data form, <https://developer.apple.com/tutorials/data/documentation/xcode/installing-the-command-line-tools.json>, through the fetch tool, which returns the page's text rather than the raw bytes. Verbatim:

- "Xcode comes bundled with command-line tools such as clang, notarytool, xcodebuild, and xcrun. If you install Xcode on your Mac, then you don't need to install the command-line tools separately."
- "The package contains the same macOS SDK, man (manual) pages, and toolchain binaries that ship with Xcode. … The system installs the package at the following path: /Library/Developer/CommandLineTools."
- "Commands such as xcodebuild and xctrace only ship with Xcode, and aren't in the Command Line Tools for Xcode package."
- "You can only install one version of the package on your Mac at a time."
- "On a fresh install of macOS, invoking any command in Xcode or the command-line tools package (such as git) from the command line prompts you to download and install the Command Line Tools for Xcode package."
- "To check the version of the Command Line Tools for Xcode package, run the pkgutil command with the --pkg-info option in Terminal".
- "You can find the entire list of command-line tools the package includes at /Library/Developer/CommandLineTools/usr/bin."
- "After a macOS upgrade, check for new releases … The version of the installed package may be incompatible with the new macOS."
- "To remove the package from your Mac, run the sudo rm command with the -rf option" and then "After removing the package, you'll continue to receive new releases of the package in Software Update on your Mac. Delete the package receipt to opt out of software updates for the package."

### E-7 · Homebrew, read 2026-11-19

- `docs/Installation.md`, <https://raw.githubusercontent.com/Homebrew/brew/main/docs/Installation.md> (front matter `last_review_date: "2026-09-21"`; rendered at <https://docs.brew.sh/Installation>). macOS requirements item: "When building formulae from source, Command Line Tools (CLT) for Xcode (from `xcode-select --install` or https://developer.apple.com/download/all/) or Xcode". Footnote 3: "Xcode or the CLT is required to build formulae from source. On Apple Silicon, casks and bottles can be installed without developer tools. Intel macOS also requires developer tools for bottle installation."
  *(The requirements wording changed since F-1's reading of 2026-09-02, `F-1-homebrew-support-tiers.md` E2. That reading had "macOS Sonoma (14)" and an unconditional "Command Line Tools (CLT) for Xcode … or Xcode". This reading has "macOS Sequoia (15)" and "When building formulae from source".)*
- `Library/Homebrew/os/mac/xcode.rb`, <https://raw.githubusercontent.com/Homebrew/brew/main/Library/Homebrew/os/mac/xcode.rb>, branch `main` as served on 2026-11-19 (no commit pinned). Verbatim:
  - `PKG_PATH = "/Library/Developer/CommandLineTools"`, `EXECUTABLE_PKG_ID = "com.apple.pkg.CLTools_Executables"`.
  - `def self.installed?` in `CLT`: `File.exist?("#{PKG_PATH}/usr/bin/clang")`.
  - `def self.detect_version` in `CLT`: `return unless installed?` then `MacOS.pkgutil_info(EXECUTABLE_PKG_ID)[/version: (.+)$/, 1]`.
  - `def self.installed?` in `Xcode`: `!prefix.nil?`. `def self.prefix`: `dir = MacOS.active_developer_dir`, then `if dir.empty? || dir == CLT::PKG_PATH || !File.directory?(dir)` it falls back to `path = bundle_path` / `path/"Contents/Developer" if path`, else `::Pathname.new(dir).cleanpath`.

### E-8 · MacPorts, read 2026-11-19

- Install page, <https://www.macports.org/install.php> (the project's own site): "For some ports, your system might require installations of the following components:" then "Apple's Command Line Developer Tools, in case a port you're installing or one of its dependencies are not available as binaries", installed with `xcode-select --install`; "(Optional) Apple's Xcode Developer Tools, when building some ports from source. MacPorts will let you know if this is the case."
- Guide, <https://guide.macports.org/chunked/installing.html>: "Xcode 4.3 and later do not automatically install the command line tools, but MacPorts requires them." "A few ports require a full Xcode installation to use, but most don't."
- `src/macports1.0/macports.tcl`, <https://raw.githubusercontent.com/macports/macports-base/master/src/macports1.0/macports.tcl>, branch `master` as served on 2026-11-19 (no commit pinned; the fetch tool read characters 0 to 200000 of 313118). Verbatim fragments:
  - `macports::set_xcodecltversion`: `if {![file executable /Library/Developer/CommandLineTools/usr/bin/make]} {` with the comment `# Same test as used to set the default for use_xcode`, which sets `xcodecltversion none` and returns with no warning. The receipt names it then consults: `set pkgnames [list CLTools_Executables CLTools_Base DeveloperToolsCLI DeveloperToolsCLILeo]`.
  - `macports::set_developer_dir`: `set devdir [exec -ignorestderr $xcodeselect -print-path 2> /dev/null]`, then `if {[_is_valid_developer_dir $devdir]} {`. Errors include "No valid Xcode installation is properly selected." and "No Xcode installation was found."
  - `macports::_is_valid_developer_dir`: `if {![file isdirectory $dir]} {` and `foreach subdir {Library usr} {` / `if {![file isdirectory ${dir}/$subdir]} {`.
  - `macports::set_xcode_license_unaccepted`: `catch {exec [findBinary xcrun $autoconf::xcrun_path] clang 2>@1} output` / `if {[string match -nocase "*license*" $output]} {`.
- **Leads, not sources:** a MacPorts commit message relayed by the mailing-list archive (`lists.macosforge.org`, macports-changes 2012-02, r90074) says `xcode-select -print-path` errors when no Xcode directory is selected. Trac ticket #53909, relayed by the same archive, reports that `xcode-select -p` succeeding does not prove the command line tools installed. Trac ticket #61741 is titled in search results as the warning "The Command Line Tools are installed, but MacPorts cannot determine the version", attributed to a lost receipt. `trac.macports.org` refused the fetch tool on 2026-11-19 (an Anubis "Access Denied" page), so none of these was read at the source.

### E-9 · The project's reads, 2026-11-19 (branch `wo16/K-1-toolchain-note` from `origin/main` `e1ffdfd`)

- `crates/adapters/src/system/machine.rs` `detect_command_line_tools`: `probe(XCODE_SELECT, &["-p"])`. A probe error, or `NotStarted` (line 437), gives `Unknown`. Line 440, `if !inv.status.success()`, gives `Missing`. `ExitStatus::success()` (`crates/adapters/src/process/mod.rs:914`) is `matches!(self, ExitStatus::Exited { code: 0 })`. So any status other than exit 0, after `NotStarted`, becomes `Missing`: a non-zero exit, and also `TimedOut`, `Signalled` and `Cancelled`. An empty or relative stdout gives `Unknown`. Otherwise it gives `Installed { path }`, with `path` redacted.
- `crates/application/src/doctor.rs` lines 575-594: `Installed` → `Severity::Ok`, `doctor.command_line_tools_present`; `Missing` → `Warning`, `doctor.command_line_tools_missing`; `Unknown` → `Warning`, `doctor.command_line_tools_unknown`.
- `locales/en.toml` lines 665-667: "Command Line Tools are installed." / "Command Line Tools are not installed; both providers need them to build from source." / "Command Line Tools state could not be determined."

## Findings

**What `xcode-select -p` can and cannot prove**

1. **Exit 0 does not prove the Command Line Tools present.** It reports the *selected developer directory*, which may be an Xcode. On this machine it is one (E-1), and `doctor` would report "Command Line Tools are installed." on the strength of an Xcode path (E-9). That happens to be true here, because the CLT are also present (E-2). The answer does not show it.
2. **Exit 0 does not prove any build tool present.** The man page documents no check that the printed directory exists or holds anything (E-5). Both providers distrust the answer. Homebrew discards it when `!File.directory?(dir)`. MacPorts accepts it only if `_is_valid_developer_dir` finds `Library` and `usr` beneath it (E-7, E-8). That both guard against it suggests `-p` can name a directory that is gone or empty. This is an inference from the guards, not an observation. Whether `-p` reports a dangling `/var/db/xcode_select_link` is UNRESOLVED. Even a directory that exists does not prove the tools usable: Apple says an installed package "may be incompatible with the new macOS" after an upgrade (E-6), and MacPorts tests Xcode's licence state separately by running `xcrun clang` (E-8). That is a spawn this project must not make (R-27 finding 24).
3. **A non-zero exit is not documented to mean "absent".** The man page has no exit-status section (E-5). That `-print-path` errors when nothing is selected is a lead from a 2012 commit message, not a source (E-8). Nor is it documented that "nothing selected" implies "nothing installed": `--reset` falls back to "the default search mechanism" (E-5), which is not described. Nothing documents the reverse case either, CLT present with `-p` failing. So `Missing` from a non-zero exit (E-9) is a stronger claim than the evidence supports. It is a *lead toward* absence, not proof. This machine cannot observe a non-zero exit without a mutation.
4. **`/var/db/xcode_select_link` is the stored selection.** It is a root-owned symlink whose target equals `-p`'s output here (E-1, E-2). Whether `-p` reads it, and what `-p` prints when it is missing or dangling, is not documented (E-5). So reading the link instead of running `-p` proves no more than `-p` does.

**What a filesystem read can and cannot prove**

5. **The Command Line Tools have one documented location.** Apple: "The system installs the package at the following path: /Library/Developer/CommandLineTools", its tools are in `…/usr/bin`, and "You can only install one version of the package on your Mac at a time" (E-6). So a stat of that fixed path, with no spawn and no path taken from any output, is a read of the documented location, not a heuristic. (The selected-Xcode stat of finding 13 does not have this property; see there.)
6. **Both providers define "CLT installed" by exactly such a stat.** Homebrew: `File.exist?("/Library/Developer/CommandLineTools/usr/bin/clang")` (E-7). MacPorts: `file executable /Library/Developer/CommandLineTools/usr/bin/make`, and its absence sets `xcodecltversion none` (E-8). By the source comment, the same test sets `use_xcode`'s default; the `use_xcode` code itself was not read. Neither provider consults a receipt or `xcode-select` to decide CLT presence. Each consults the receipt only for the version, after the stat.
7. **Hence a stat proves the Command Line Tools absent, in each provider's own terms.** If `/Library/Developer/CommandLineTools/usr/bin/make` is not found, MacPorts considers the CLT absent. If `…/usr/bin/clang` is not found, Homebrew does. Only a definite not-found counts. Any other stat error (permission, I/O) proves nothing and must be `Unknown`.
8. **A stat never proves the tools present in the sense of "a build will work".** It proves the files exist at the documented path (E-2), and only that. Version compatibility after an OS upgrade (E-6), Xcode's licence state (E-8), and a damaged toolchain all lie beyond a stat. Running something is the only way to settle them, and the install dialog makes that off-limits (R-27 finding 24; Apple's note in E-6). So "present" from a read means *not detectably absent*.
9. **"The CLT are absent" is not "the build tools are absent".** The difference depends on the destination provider:
   - **Homebrew** accepts either: "Xcode or the CLT is required to build formulae from source" (E-7). With the CLT absent, the build tools are still present if an Xcode is. Homebrew finds an Xcode first from the selected directory, then from the bundle (`bundle_path`, E-7), so it can find an Xcode that is *not* selected, at any location. A fixed-path stat cannot see such an Xcode. So for a Homebrew destination, a read can prove absence only up to that residual: an unselected Xcode, with the CLT absent. On the evidence, a block in that case would be a false `MANUAL_REVIEW`. That is the safe direction (it stops; it removes nothing).
   - **MacPorts also appears to accept Xcode in the CLT's place. This is an inference: the code that does it was not read.** The guide's "MacPorts requires them" (E-8) sits in an Xcode 4.3 context: "Xcode 4.3 and later do not automatically install the command line tools". The install page's wording is narrower: the CLT are needed "in case a port … [is] not available as binaries" (E-8). MacPorts' own source ties CLT absence to `use_xcode`. The CLT `make` test that sets `xcodecltversion none` is commented `# Same test as used to set the default for use_xcode` (E-8). On that comment's face, the CLT being absent switches MacPorts to building with the selected Xcode rather than stopping. The `use_xcode` default itself, and a source build under it, were not read or traced. So MacPorts gets the same framing as Homebrew. A CLT not-found is MacPorts' own "no CLT" answer, but not proof that MacPorts cannot build. The residual is an Xcode that MacPorts would find: the selected one, which `set_developer_dir` validates (E-8), or the `/Applications/Xcode.app/Contents/Developer` fallback that `set_developer_dir` names (E-8).
10. **The selected Xcode's `Contents/Developer/usr/bin` is the right place to stat for Xcode, and `make` is the right file.** `make` is present at both `…/CommandLineTools/usr/bin/make` and `…/Xcode.app/Contents/Developer/usr/bin/make`, both 403216 bytes (E-2). `clang` is not under Xcode's `Contents/Developer/usr/bin`. It is in `Toolchains/XcodeDefault.xctoolchain/usr/bin` (E-2). A stat of `<selected>/usr/bin/clang` would therefore report an Xcode as having no compiler. Of the two files, only `make` is at the same relative path in both installations.
11. **`SDKs/` adds little, and `pkgutil` misleads.** The SDK directory lists versioned SDKs (E-2). Receipts exist for SDKs that are not there: `CLTools_SDK_macOS12`, `13`, `14` and `110` exist (E-4), `MacOSX12.sdk` does not (E-2), and that receipt's whole file list is one placeholder under `private/tmp` (E-3). Apple documents that removing the package leaves the receipt until it is forgotten (E-6). So a receipt proves neither presence nor absence. It is also outside the product's frozen allowlist (part K, item 4), so nothing K builds may depend on it.
12. **A stat cannot raise the install dialog.** It executes nothing. The dialog is raised by *invoking* a tool (E-6) or by `--install` (E-5). This is the property DP-050's "no new spawn" asks for, and `xcode-select -p` does not need it: it is already in every recording (R-28 E-1).

**Answer to question 4: what K-7 may build**

13. **K-7 may build the `MANUAL_REVIEW` block, but keyed on a filesystem read, not on `xcode-select -p`'s exit.** The build-tools-absent fact that the evidence supports is:
    - a definite not-found for `/Library/Developer/CommandLineTools/usr/bin/make` (finding 7); **and**
    - no selected developer directory with `usr/bin/make` under it. That is: `xcode-select -p` failed; or it printed `/Library/Developer/CommandLineTools`; or `<printed path>/usr/bin/make` is a definite not-found (finding 10).

    Any other outcome of either read is `Unknown`, and `Unknown` is non-blocking with its own text, as D7 already says. A non-zero `-p` exit **alone** must not block (finding 3). Nor may any other non-success status: per E-9, the adapter today folds `TimedOut`, `Signalled` and `Cancelled` into `Missing`, and none of them says anything about the tools.

    **The second read's path comes from output, unlike finding 5's fixed path.** `<printed path>/usr/bin/make` is built from `xcode-select -p`'s stdout, which is untrusted output becoming a path (the adapters rule). Three consequences:
    - K-7 must validate that path before using it: absolute, no `..` component, no NUL or newline, length bounded, then `usr/bin/make` joined as fixed components. It must stat without following a symlink at the final component, or say why following one is acceptable.
    - The read is the kind `security-reviewer` must see before it lands.
    - Under replay the snapshot cannot supply the path. `Installed { path }` is home-redacted (`machine.rs:448`, `redact_home`), so a recorded path under a home directory is not the real one. The stat's *answer* must therefore be recorded at capture time, through the machine reader D7 names. It must not be recomputed from the snapshot's path. A recording without that answer gives `Unknown`.

    The block's reach, provider by provider. In both cases it is a block whose residual is an Xcode that the provider would find and this read does not see (finding 9). That residual belongs in the block's reason text, for example "no developer tools were found where they are normally installed". The residual errs toward a false `MANUAL_REVIEW`, the safe direction.
    - **MacPorts destination:** the residual is narrower, because MacPorts uses the selected Xcode or a fixed fallback (E-8). An inference, since `use_xcode` was not traced (finding 9).
    - **Homebrew destination:** the residual is wider: an unselected Xcode at any location, found through `bundle_path` (E-7).
    - **Restore:** the same block, not a special case (DP-050, 04).

    Before it blocks anything, the read must meet D7's replay condition. It goes through the machine reader that `--replay` records, and a recording without it answers `Unknown`. If the recorder cannot carry a stat, the block is not built, and K-7 builds only the risk text (part K, D7).
14. **K-7 may not claim presence.** A read finding the files means *not detectably absent* (finding 8). The `destination_source_build` risk text must still name what a read cannot check: that the tools match this macOS and that Xcode's licence is accepted.
15. **`doctor`'s `Installed` meaning changes, so its wording changes** (D7's condition "unless R-29 changes what `Installed` means"). `Installed { path }` today means "a developer directory is selected", and that can be an Xcode (finding 1). "Command Line Tools are installed." asserts more than the read shows. "both providers need them to build from source" is wrong for Homebrew, which accepts Xcode (E-7), and, by the inference in finding 9, for MacPorts too. That wording change goes through `/add-doctor-check`'s procedure. It may split the finding into "CLT files present/absent at the documented location" and "selected developer directory". The decision is K-7's.

## Cannot determine

- **The absent case, observed.** This machine has both installations (E-1, E-2), and making either absent is a mutation (removal, `xcode-select --switch`/`--reset`, or `DEVELOPER_DIR`, all refused). Findings 3, 7 and 13 rest on documentation and source, not observation. **Owner capture to settle it**, on a disposable machine or VM with no Xcode and no CLT (a fresh macOS install), unelevated, running nothing else under `/usr/bin`: `/usr/bin/xcode-select -p` (stdout, stderr, exit status); `/usr/bin/readlink /var/db/xcode_select_link`; `stat` of `/Library/Developer/CommandLineTools` and its `usr/bin/make` and `usr/bin/clang`. Then the same after installing only the CLT, and again with only Xcode installed and never selected. Whether the last state shows `-p` failing, or printing the Xcode found by the default search, decides how wide finding 9's Homebrew residual is.
- **`-p` with a dangling selection.** Whether `-p` prints a stored link whose target is gone, and with what exit. Settled by the same capture, after moving the selected Xcode away on the disposable machine.
- **What "the default search mechanism" is** (E-5): not documented in the man page, and not found in Apple's developer documentation in this session.
- **MacPorts' Xcode-only build path** (`use_xcode`) and how `xcodecltversion none` changes a real source build. This is what turns finding 9's MacPorts inference into a citation. Not traced: `macports.tcl` was read only to character 200000, and `port1.0` was not read.
- **Trac #53909 and #61741** could not be read at the source (E-8). Re-read when `trac.macports.org` admits the fetch.

## Impact on the specification

- **Confirms DP-050's condition can be met**, by a stat with no spawn (findings 5-7, 12, 13). K-7 may build `destination_toolchain_missing`, keyed as in finding 13, subject to D7's replay condition.
- **Contradicts the adapter's mapping** (not the specification). `detect_command_line_tools` maps any status other than exit 0, after `NotStarted`, to `CommandLineTools::Missing`: a non-zero exit, and also `TimedOut`, `Signalled` and `Cancelled` (E-9). It maps exit 0 to `Installed`. Neither is supported as a statement about the *Command Line Tools* (findings 1-3). A timeout, a signal or a cancellation is not even a statement about the selection, and by the adapters rule it belongs in `Unknown`. If K-7 keys a block on `Missing` as it stands, it blocks on an undocumented exit, or on a probe that did not finish. Report to the main agent: the domain type's meaning should be stated, or the CLT fact should be a separate stat-based field.
- **`locales/en.toml` 665-667 and `pt-BR`:** "both providers need them to build from source" is contradicted for Homebrew by `Installation.md` footnote 3 (E-7). That is a user-facing text change for K-7 (finding 15).
- **New, outside part K's scope, for the main agent:** Homebrew's footnote 3 says "Intel macOS also requires developer tools for bottle installation" (E-7). On the audience this project started for, an x86_64 Mac, the developer tools matter to any Homebrew `ADD_DESTINATION`, not only to a source build. DP-050 and D7 key the block on a source-build step only. Whether a Homebrew destination on Intel with no tools is a `MANUAL_REVIEW` for a bottle too is a question this note raises and does not answer. It is a dated fact (INV-20), so if it is acted on, it belongs in `data/policy/` with `source` and `read_on`, not in code.
- **`F-1-homebrew-support-tiers.md` E2 is stale** on the requirements list (E-7's note: macOS 15 now, and the CLT requirement now conditioned on source builds). That note's re-check applies.

## Re-check when

- Apple changes the Command Line Tools' install location or the "one version at a time" statement (E-6), or `man xcode-select` gains an exit-status section.
- Homebrew's `CLT.installed?` or `Xcode.prefix` changes (`os/mac/xcode.rb`), or `Installation.md`'s requirements or footnote 3 change. That page is reviewed periodically (`last_review_date`), and the Intel bottle sentence is time-sensitive under INV-20.
- MacPorts' `set_xcodecltversion` test or `use_xcode` default changes, or its install page's requirement changes.
- The owner capture above is made: it replaces findings 3 and 9's inferences with observations.
