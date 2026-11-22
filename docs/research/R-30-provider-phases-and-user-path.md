# R-30 — Which provider phases run with the user's `PATH`, and can a shim directory first on it come ahead of the provider's own directories?

- **Status:** partially answered. Homebrew is answered from its source at a recorded commit, plus one observation of the superenv build `PATH`. MacPorts is answered from its installed Tcl sources (2.12.6) and its installed `macports.conf`. The elevated route is cited from `man sudoers`; this machine's `/etc/sudoers` could not be read (E-15), so its actual `Defaults` are UNRESOLVED. No phase was observed running, because every phase in question is a mutation (Cannot determine).
- **Answered on:** 2026-11-22
- **Machine:** macOS 26.7.3 (25G342), x86_64. Homebrew `7.0.15-44-gee32747` @ `/usr/local`, repository `/usr/local/Homebrew` at commit `ee3274791db39cb67cf5bd75622c80c3ef05a2f6` (committed 2026-11-20). Local `homebrew/core` tap at `e045d7385fd` (2026-09-02, stale). Portable Ruby `4.0.10` present. MacPorts 2.12.6 @ `/opt/local`.
- **Blocking:** work order 16 part I, decision D11 (`docs/passes/16-plans/I.md`, "D11 — INV-07: the shim directory and the providers' phases"). It decides whether layer 4 of `docs/spec/05-environment-and-primary-provider.md` (lines 61-99) can be offered as planned, or needs a decision-pending entry under INV-07.

## Question

Layer 4 puts a MacPortsBrew-owned shim directory (`~/.local/share/macportsbrew/bin`) of symlinks to one provider's stable path first on the user's `PATH`. INV-07: MacPortsBrew never configures a Homebrew package as depending on a MacPorts-managed artifact, or the reverse. If any phase of a provider runs with the user's `PATH` and resolves a bare name through it, a shim toward the *other* provider would be reached by that phase. That would be a cross-provider configuration MacPortsBrew created.

For both providers, the question is which phases run with any part of the user's `PATH`, and how that `PATH` is composed. Can the shim directory come ahead of the provider's own directories and the system directories?

MacPortsBrew's own provider invocations get a constructed `PATH` (`crates/adapters/src/process/environment.rs:34-46`). That is not the question here.

## Evidence

All commands ran on 2026-11-22 as the unelevated user, from zsh. Every `brew` invocation had `HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1` exported. That is the quieting set in `.claude/rules/adapters.md`. `brew --version`, `--repository`, `--prefix` and `--env` do not auto-update in any case. MacPorts was invoked only as `port version`. Sources were read with `grep`, `awk 'NR>=a&&NR<=b'` and the Read tool. Line numbers are as of the commits and versions above. Nothing was written outside this note and a scratch copy of the `sudoers(5)` man page.

The project's hooks refused three reads. None was retried in another form:

- `brew ruby -e '…'`, which would have printed `ORIGINAL_PATHS` live, was refused as a mutating `brew` command.
- A `grep` whose pattern contained the elevation program's name was refused (INV-06). It was re-run without that word.
- A `grep` for the macOS service launcher's name was refused (INV-19).

### E-1 · Versions

```
$ brew --version
Homebrew 7.0.15-44-gee32747
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)
$ brew --repository; brew --prefix
/usr/local/Homebrew
/usr/local
$ git -C /usr/local/Homebrew log -1 --format='%H %cd'
ee3274791db39cb67cf5bd75622c80c3ef05a2f6 Fri Nov 20 02:12:14 2026 +0000
$ /opt/local/bin/port version
Warning: port definitions are more than two weeks old, consider updating them by running 'port selfupdate'.
Version: 2.12.6
$ ls -la /usr/local/Homebrew/Library/Homebrew/vendor/portable-ruby/
… 4.0.10 … 4.0.9 … current -> 4.0.10
```

### E-2 · Homebrew: `bin/brew` saves the user's `PATH`, then resets it

`/usr/local/Homebrew/bin/brew`:

- `PATH` is in `USED_BY_HOMEBREW_VARS` (line 234).
- Lines 251-259 run `export "HOMEBREW_${VAR}"="${!VAR}"` for each of them, with "We unconditionally override `HOMEBREW_*` here". So `HOMEBREW_PATH` is the caller's `PATH`.
- Line 301: `PATH="/usr/bin:/bin:/usr/sbin:/sbin"`.
- Line 345: `exec /usr/bin/env -i "${FILTERED_ENV[@]}" /bin/bash -p ".../brew.sh"`. `FILTERED_ENV` is built after line 301, so it carries the reset `PATH` and `HOMEBREW_PATH`.
- Lines 73-76: when brew runs inside brew, `PATH="${HOMEBREW_PATH}"` is restored first, "otherwise we would lose the original path".

`Library/Homebrew/global.rb:151-157`:

```ruby
ENV["HOMEBREW_PATH"] ||= ENV.fetch("PATH")
original_paths = PATH.new(ENV.fetch("HOMEBREW_PATH")).filter_map do |p|
  Pathname.new(p).expand_path
…
ORIGINAL_PATHS = T.let(original_paths.freeze, T::Array[Pathname])
```

`Library/Homebrew/brew.rb:71-78` handles internal commands:

```ruby
path = PATH.new(ENV.fetch("PATH"))
homebrew_path = PATH.new(ENV.fetch("HOMEBREW_PATH"))
path.prepend(HOMEBREW_SHIMS_PATH/"shared")
homebrew_path.prepend(HOMEBREW_SHIMS_PATH/"shared")
ENV["PATH"] = path.to_s
```

Lines 94-99 handle a command that is not internal: "homebrew_path.append(Commands.tap_cmd_directories)" … "# External commands expect a normal PATH" / `ENV["PATH"] = homebrew_path.to_s`.

`shims/shared/` contains `curl git rustc_wrapper svn`.

### E-3 · Homebrew: the superenv build `PATH`, observed

```
$ PATH="/nonexistent-r30-shim:$PATH" brew --env 2>&1 | head -30
…
export HOMEBREW_GIT=/usr/local/opt/git/bin/git
…
export PATH=/usr/local/Homebrew/Library/Homebrew/shims/mac/super/bin:/usr/bin:/bin:/usr/sbin:/sbin
```

The source of that `PATH` is `extend/ENV/super.rb:101-105,114`:

```ruby
path = PATH.new(Superenv.bin)
path.append(deps.map(&:opt_bin))
path.append(homebrew_extra_paths)
path.append("/usr/bin", "/bin", "/usr/sbin", "/sbin")
…
self["PATH"] = path.existing&.to_s
```

The superenv tool wrappers do not fall back to the user's `PATH`:

- `shims/mac/super/bin/pkg-config` execs `${HOMEBREW_OPT}/pkg-config/bin/pkg-config`.
- `shims/mac/super/bin/make` calls `try_exec_non_system`, which walks `type -aP` over the current `PATH` (`shims/utils.sh:80-98`). In a superenv build that is the rebuilt `PATH`.

### E-4 · Homebrew: build-time hooks that read `ORIGINAL_PATHS` even under superenv

`requirement.rb:208`: `def which(cmd, path = PATH.new(ORIGINAL_PATHS))`.

`requirement.rb:135-151`, `modify_build_environment`, is called for every requirement in every build (`build.rb:111-115`). Its comment and code:

```ruby
# XXX If the satisfy block returns a Pathname, then make sure that it
# remains available on the PATH. This makes requirements like
#   satisfy { which("executable") }
# work, even under superenv where "executable" wouldn't normally be on the
# PATH.
parent = satisfied_result_parent
…
ENV.prepend_path("PATH", parent)
```

`satisfied_result_parent` (`requirement.rb:116-124`) takes `Utils::Path.resolved_path(@satisfied_result).parent`.

In the local core tap (E-1 revision), 8 formula files contain `satisfy`. The `satisfy.*which` matches are `glibc@2.13.rb` and `glibc@2.17.rb`, with `satisfy(build_env: false) { which(TOOL).present? }`. That block returns a boolean, not a `Pathname`.

`extend/ENV/shared.rb:291-298`, `fortran`:

```ruby
if (gfortran = which("gfortran", (HOMEBREW_PREFIX/"bin").to_s))
  ohai "Using Homebrew-provided Fortran compiler"
elsif (gfortran = which("gfortran", PATH.new(ORIGINAL_PATHS)))
  ohai "Using a Fortran compiler found at #{gfortran}"
end
if gfortran … self["FC"] = self["F77"] = gfortran.to_s
```

Two formula files in the local core tap call `ENV.fortran`.

### E-5 · Homebrew: `env :std` builds

`formula_installer.rb:1204-1208` is the one place that chooses it:

```ruby
if @env.present?
  args << "--env=#{@env}"
elsif formula.env.std? || formula.deps.select(&:build?).any? { |d| d.name == "scons" }
  args << "--env=std"
```

`extend/ENV/super.rb:67-69`: `return false if env == "std"`. `build.rb:96` reads `superenv_enabled = Superenv.enabled_for?(args.env)`.

`extend/ENV/std.rb:25-30`:

```ruby
super
self["HOMEBREW_ENV"] = "std"
ORIGINAL_PATHS.reverse_each { |p| prepend_path "PATH", p }
prepend_path "PATH", HOMEBREW_SHIMS_PATH/"shared"
```

`build.rb:117-127`. This runs after the environment is set up:

```ruby
unless superenv_enabled
  keg_only_deps.each do |dep|
    ENV.prepend_path "PATH", dep.opt_bin.to_s
    …
```

Local core tap (E-1 revision):

```
$ grep -rl '^\s*env :std' …/homebrew-core/Formula | wc -l
       0
$ grep -rlE 'depends_on "scons" => :build' …/homebrew-core/Formula | wc -l
       7
rmlint.rb gpsd.rb apache-serf.rb ffmpeg2theora.rb openvino.rb huexpress.rb makensis.rb
```

### E-6 · Homebrew: `post_install`, including after a bottle is poured

`formula.rb:1778-1808`, `run_post_install`:

```ruby
new_env = {
  HOMEBREW_PATH: nil,
  PATH:          PATH.new(ORIGINAL_PATHS),
}
… with_env(new_env) do
  ENV.clear_sensitive_environment!
  …
  run_post_install_steps if post_install_steps_defined?
  if post_install_defined?
    odeprecated "`post_install`", "`post_install_steps`"
    post_install
```

The F-2 E-9 citation `formula.rb:1729-1738` has moved. At this commit the code is lines 1785-1787, and `post_install` is deprecated in favour of `post_install_steps`. Both run inside this environment.

`formula_installer.rb:1071-1100`, `finish`, runs after a build and after a pour. It ends with `post_install if formula.post_install_steps_defined? || formula.post_install_defined?`, unless `build_bottle?` or `skip_post_install?`.

`formula_installer.rb:1538-1543` runs `postinstall.rb` in `Sandbox.run_or_fork`, and `postinstall.rb:36` calls `formula.run_post_install`.

In the local core tap, 163 formula files match `def post_install|post_install_steps|post_install do`.

### E-7 · Homebrew: casks

`cask_artifact.rb:81-82` is the sandboxed child for cask `install_steps` and `generated_completions` (`cask_artifact.rb:94,119`; `cask/artifact/install_steps.rb:75,93`; `cask/artifact/generated_completion.rb:127`):

```ruby
ENV.delete("HOMEBREW_PATH")
ENV["PATH"] = PATH.new(ORIGINAL_PATHS).to_s
```

`cask/artifact/installer.rb:40-45` runs an installer `script` with `PATH.new(HOMEBREW_PREFIX/"bin", HOMEBREW_PREFIX/"sbin", ENV.fetch("PATH"))`.

`cask/artifact/pkg.rb:84-97` runs `/usr/sbin/installer` elevated with an `env` of `LOGNAME`/`USER`/`USERNAME` only.

### E-8 · Homebrew: `brew`'s own non-build subprocesses (F-2 row 501)

**git and curl** (`utils/git.sh:13-28`, `utils/curl.sh:67-71`, `utils/tool.sh:28-107`). `select_tool` is called with these candidates, in order:

1. the brewed tool (`${HOMEBREW_PREFIX}/opt/git/bin/git`, `${HOMEBREW_PREFIX}/opt/curl/bin/curl`);
2. the system tool (`${developer_dir}/usr/bin/git`, `/usr/bin/curl`);
3. `${HOMEBREW_GIT_PATH}` / `${HOMEBREW_CURL_PATH}`.

Two details matter:

- `tool.sh:40-43`: only a bare-name third candidate is looked up on the user's `PATH`, as `suggested="$(PATH="${HOMEBREW_PATH:-${PATH}}" type -P "${suggested}")"`.
- `tool.sh:83`: `[[ ${i} -eq 2 && -n "${HOMEBREW_SELECTED_TOOL}" ]] && break`, with the comment "Compare both installed defaults before considering the final suggestion".

`cmd/update.sh:73-79`: `git()` resolves `GIT_EXECUTABLE` through `shims/shared/git --homebrew=print-path`. That shim (`shims/shared/git:23-29`) execs `$(type -P "${HOMEBREW_GIT}")` when `HOMEBREW_GIT` is set. Its fallbacks are `${HOMEBREW_PREFIX}/bin/<tool>`, `try_exec_non_system` on the current `PATH`, the Xcode path and `/usr/bin` (lines 43-67). E-3 observed `HOMEBREW_GIT=/usr/local/opt/git/bin/git`.

`cmd/update.sh` and `brew.sh` contain no `PATH=` assignment. A grep of `cmd/*.sh` found `PATH=` only in `exec.sh` and `shellenv.sh`.

**The Ruby that runs brew** (`utils/ruby.sh`). It uses the portable Ruby when `vendor/portable-ruby/current/bin/ruby` is executable (lines 132-134).

The user's `PATH` is consulted only in two places:

- in `find_ruby` (lines 53-73), where `which -a ruby` runs over `HOMEBREW_PATH` at line 69;
- when `system_ruby_supported` is true, which on macOS requires `can_use_ruby_from_path` (lines 27, 30-37). That in turn needs `HOMEBREW_DEVELOPER` or `HOMEBREW_TESTS`, plus `HOMEBREW_USE_RUBY_FROM_PATH`.

Portable Ruby is present on this machine (E-1).

**Git downloads** (`download_strategy/git_download_strategy.rb:106-114,133`): `"PATH" => PATH.new(ENV.fetch("PATH"), ORIGINAL_PATHS).to_s`.

**Unpacking:**

- `unpack_strategy/bzip2.rb:32` uses `Utils::Path.formula_opt_bin_env("bzip2", ORIGINAL_PATHS)`.
- That is defined at `utils/path.rb:256-266` as `PATH.new(formula_opt_bin(formula_name), *paths, ENV.fetch("PATH"))`.

**Helper executables found through `ORIGINAL_PATHS`:**

- `utils/executable.rb:14-19`, `ensure!`, checks these in order: `Utils::Shell.which(name)` (current `PATH`), `Utils::Shell.which(name, ORIGINAL_PATHS)`, `HOMEBREW_PREFIX/"opt/#{formula_name}/bin/#{name}"`, `HOMEBREW_PREFIX/"bin/#{name}"`.
- Callers of `ensure!`:
  - `attestation.rb:91-99`: `gh`, "verifying attestations", reached from `sort_formulae_for_install` (`attestation.rb:111-117`);
  - `github_packages.rb:116`: `skopeo`;
  - `dev-cmd/pr-pull.rb:86`: `unzip`.
- `formula.rb:389-423`, `ensure_installed!`: "a matching system executable in `ORIGINAL_PATHS` can satisfy the request without installing the formula".
- `utils/shell.rb:53-55`: `with_homebrew_path` is `with_env(PATH: PATH.new(ORIGINAL_PATHS).to_s, &block)`. It is used by `utils/editor.rb:36` and `language/python.rb:105-107`. The second runs the target Python with `-c "import site; …"`.
- `cmd/log.rb:42`, and `dev-cmd/bump-*.rb`, set `ENV["PATH"] = PATH.new(ORIGINAL_PATHS).to_s`.
- `caveats.rb:104,120,215,242` and `formula_cellar_checks.rb:36,344` call `which(…, ORIGINAL_PATHS)`. `formula_cellar_checks.rb:343-344` tries `which("objdump")` and then `which("objdump", ORIGINAL_PATHS)` to pick the `objdump` it then runs.

**Sandbox:** `sandbox.rb:332-336` searches `ORIGINAL_PATHS` only for a relative executable name. On macOS the executable is the absolute `SANDBOX_EXEC = "/usr/bin/sandbox-exec"` (`extend/os/mac/sandbox.rb:15,95,133`).

### E-9 · Homebrew: `brew bundle`, `brew exec`, `brew sh`

- `bundle/extensions/extension.rb:70-72`: `which(package_manager_name, ORIGINAL_PATHS)`.
- `bundle/extensions/npm.rb:39-44` and `krew.rb:35-40`: `{ "PATH" => "#{executable.dirname}:#{ORIGINAL_PATHS.join(":")}" }`.
- `cargo.rb:359`: `"#{cargo.dirname}:#{ENV.fetch("PATH")}"`.
- `vscode_extension.rb:41-44`: `which("code"|"codium"|"cursor"|"code-insiders", ORIGINAL_PATHS)`.
- `bundle/subcommand/exec.rb:150`: `ENV.setup_build_environment`, a superenv `PATH`. Then lines 173-174 prepend the command's directory if `which(command)` finds it on that `PATH`. Then lines 206-209 run `ENV.append_path "PATH", homebrew_path`, which is the user's `PATH` appended after Homebrew's.
- `cmd/exec.sh:280-289`: `exec_path="${HOMEBREW_PATH:-${PATH}}"`, with the formulae's directories prepended, then `export PATH`.
- `dev-cmd/sh.rb:32-33,60-67`: `--env=std` "Use the standard `$PATH` instead of superenv's".

### E-10 · Homebrew: `brew services`

- `service.rb:563-565`: `std_service_path_env` returns `"#{HOMEBREW_PREFIX}/bin:#{HOMEBREW_PREFIX}/sbin:/usr/bin:/bin:/usr/sbin:/sbin"`.
- `service.rb:486-498`: `effective_environment_variables` is the formula's declared `environment_variables`, merged with `${HOMEBREW_USER_CONFIG_HOME}/services/<name>.env` when that file exists. The merge is skipped for root.

The generated service definition takes no input from the caller's shell `PATH`, as far as this code shows. The service manager's own default environment was not read (Cannot determine).

### E-11 · MacPorts: `binpath`, configuration and default

`/opt/local/etc/macports/macports.conf`, lines 14-18 (byte-identical to `macports.conf.default`, `cmp` returned 0):

```
# Colon-delimited list of directories to search for external tools
# (make(1), pkg-config(1), etc.). While installing ports, MacPorts uses
# this list for PATH. Changing this setting is intended for advanced
# users only and is unsupported.
#binpath             	/opt/local/bin:/opt/local/sbin:/bin:/sbin:/usr/bin:/usr/sbin
```

`/opt/local/libexec/macports/lib/macports1.0/macports.tcl` (2.12.6), lines 1527-1531, in `mportinit` (which starts at line 960):

```tcl
if {![info exists binpath]} {
    set env(PATH) ${prefix}/bin:${prefix}/sbin:/bin:/sbin:/usr/bin:/usr/sbin
} else {
    set env(PATH) $binpath
}
```

Line 1144: `set user_path $env(PATH)` ("Save the path for future processing").

The only reader of `macports::user_path` is `diagnose.tcl:725-729`. It checks whether `$port_loc/bin` is on it and executes nothing found there. A grep of the local ports tree (`…/release/tarballs/ports`, Portfiles and `*.tcl`) for `user_path` found no matches.

### E-12 · MacPorts: environment sanitising and phase execution

`macports.tcl:1720-1762`: `keepenvkeys` = `DISPLAY`, the `DYLD_*` five, `HOME JAVA_HOME MASTER_SITE_LOCAL ARCHIVE_SITE_LOCAL PATCH_SITE_LOCAL PATH PORTSRC RSYNC_PROXY USER GROUP LANG http_proxy COLUMNS LINES`, the proxy variables in both cases, `extra_env`, and usable `TMP`/`TMPDIR`. Every other variable is `unset`. `PATH` is kept, but it was already replaced at line 1528.

`port1.0/portutil.tcl:360-482`, `command_exec`:

- It merges the phase's `${varprefix}.env` (via `parse_environment`), plus `MACOSX_DEPLOYMENT_TARGET`, `CPATH`/`LIBRARY_PATH` from `compiler.*`, `DEVELOPER_DIR` and `SDKROOT`, over the current `env` (lines 416-464).
- It runs `system …` (line 468), then restores `env` (lines 476-478).

`port1.0/portdestroot.tcl:132-139` prepends `clonebin_path` to `env(PATH)` and later restores it. `portutil.tcl:3272` searches `$env(PATH)` for binaries.

### E-13 · MacPorts: external commands run before the `PATH` reset

`macports.tcl:1125-1141`. When `HOME` is set, `user_home` comes from `$env(HOME)` and no command runs. When `HOME` is unset, these run with the inherited `PATH`, by bare name:

- line 1132, with `SUDO_USER` set: `exec -ignorestderr dscl … | cut -d ' ' -f 2`;
- line 1134: `exec id -u`;
- line 1135: `exec … dscl … [exec … id -un …] … | cut …`.

Line 1360 uses the absolute `/usr/bin/sw_vers`.

The `port` script's shebang is `#!/opt/local/libexec/macports/bin/tclsh8.6` (`file`: "a /opt/local/libexec/macports/bin/tclsh8.6 script text executable").

### E-14 · MacPorts: commands that restore the user's whole environment

`/opt/local/bin/port`:

- Line 5588: `set boot_env [array get env]`, "Save off a copy of the environment before mportinit monkeys with it".
- `port help` (lines 1594-1609) restores `boot_env`, then runs `system -nodup [list ${macports::autoconf::man_path} $pagepath]`.
- `port edit` (lines 3793-3819) restores `boot_env`, then `exec … {*}$editor $portfile`. The editor comes from `$opts`, `VISUAL` or `EDITOR` as a name, defaulting to `/usr/bin/vi`.

Neither is a fetch, build, destroot or activate phase.

### E-15 · The elevated route: `sudo`

```
$ ls -l /etc/sudoers
-r--r-----  1 root  wheel  1709 Nov  6 02:12 /etc/sudoers
$ grep -n 'env_reset\|secure_path\|env_keep\|Defaults' /etc/sudoers
ugrep: warning: cannot read /etc/sudoers: Permission denied
$ ls -la /etc/sudoers.d
total 0
drwxr-xr-x   2 root  wheel    64 Nov  6 02:12 .
drwxr-xr-x  84 root  wheel  2688 Nov 19 21:01 ..
```

`/etc/sudoers.d` is empty, so no drop-in adds a `secure_path`. Whether the main file includes that directory, or sets one itself, is not readable.

`man sudoers` on this machine ("Sudo 1.9.17p2, June 7, 2025"), rendered with `col -b`.

Lines 153-163:

> By default, the env_reset flag is enabled. This causes commands to be executed with a new, minimal environment. … Additional variables, such as DISPLAY, PATH and TERM, are preserved from the invoking user's environment if permitted by the env_check, or env_keep options. … If the PATH and TERM variables are not preserved from the user's environment, they will be set to default values.

Lines 1286-1298, `env_reset`:

> If set, sudo will run the command in a minimal environment containing the TERM, PATH, HOME, MAIL, SHELL, LOGNAME, USER and SUDO_* variables. … If the secure_path setting is enabled, its value will be used for the PATH environment variable. This flag is on by default.

Lines 233-236: with `-i`, "The DISPLAY, PATH and TERM variables remain unchanged".

The installed man page mentions `secure_path` only at lines 206 and 1297. It has no paragraph defining the option.

The sudo project's manual (<https://www.sudo.ws/docs/man/sudoers.man/>, read 2026-11-22) does define it. It is the upstream of the installed program, but not one of the README's named official sources, so treat it as corroboration only:

> "If set, `sudo` will use this value in place of the user's `PATH` environment variable."
> "Users in the group specified by the _exempt\_group_ option are not affected by _secure\_path_."

It also states the option "is disabled by default". That last phrase is relayed by the fetch tool, not quoted from the page.

## Findings

**Homebrew** (7.0.15-44-gee32747, commit `ee32747`)

1. **Every `brew` process carries the caller's `PATH`, in the caller's order.** It is held as `HOMEBREW_PATH` and as `ORIGINAL_PATHS`. The working `PATH` is reset to the system directories (E-2). Whether the user's `PATH` reaches a subprocess is decided per call site, by which of those two values the site uses.

2. **Superenv builds: no.** The build `PATH` is rebuilt from Homebrew's shims, dependencies' `opt_bin`, extra paths and the system directories. A directory first on the caller's `PATH` was absent from the observed build `PATH` (E-3). The tool wrappers resolve through that rebuilt `PATH` or through absolute Homebrew paths (E-3).

   There are two hooks inside a superenv build where `ORIGINAL_PATHS` can still decide something (E-4):
   - A requirement whose `satisfy` returns a `Pathname` found by its default `which` gets that file's *resolved* parent directory prepended to the build `PATH`. A symlink shim resolves to the other provider's `bin`.
   - `ENV.fortran` picks `gfortran` from `ORIGINAL_PATHS` when `${HOMEBREW_PREFIX}/bin` has none.

   No core formula at the tap revision read uses the first. Two call `ENV.fortran`.

3. **`env :std` builds: yes, ahead of Homebrew's prefix and the system directories.** The composed order is (E-5):
   1. keg-only dependencies' `opt_bin`;
   2. `shims/shared` (`curl git rustc_wrapper svn`);
   3. `ORIGINAL_PATHS` in the user's order;
   4. the inherited `brew` `PATH` (system directories).

   A shim directory first on the user's `PATH` therefore comes ahead of `/usr/local/bin` and the system directories. It comes behind keg-only dependencies and the four shared shims. `std` is selected by `env :std` in the formula (0 core formulae at the revision read) or by a `scons` build dependency (7 core formulae), as well as by internal `--env=`.

4. **`post_install` and `post_install_steps`: yes, and nothing comes ahead.** `PATH` is exactly `ORIGINAL_PATHS` (E-6). It runs after a bottle pour as well as after a build, unless skipped. 163 core formula files define one. `shims/shared` is not on that `PATH`, so every bare name a post-install step runs, `git` included, is resolved in the user's order.

5. **Cask `install_steps` and generated completions: yes, and nothing comes ahead** (`PATH = ORIGINAL_PATHS`, E-7). A cask installer `script` puts `${HOMEBREW_PREFIX}/bin:sbin` first, then the running `PATH`. The source read does not establish whether that running `PATH` is the reset one or `ORIGINAL_PATHS` (Cannot determine).

6. **`brew update`, git and curl: no, by default.** Both are selected from absolute candidates: brewed, then system. The user's `PATH` is consulted only for a bare-name `HOMEBREW_GIT_PATH`/`HOMEBREW_CURL_PATH`, and only if neither default qualifies (E-8). That is a configuration-dependent exception, so it is deferred to runtime detection of those two variables. `brew update`'s `git` resolves through `HOMEBREW_GIT`. A shimmed `git` first on the user's `PATH` does not reach `brew update` unless one of those variables names it. This answers F-2 row 501 for git and curl.

7. **The Ruby that runs `brew`: no, on macOS, unless developer variables are set** (`HOMEBREW_DEVELOPER`/`HOMEBREW_TESTS` plus `HOMEBREW_USE_RUBY_FROM_PATH`). Portable Ruby is used and is present here (E-8).

8. **Other non-build sites resolve a bare name in the user's order, ahead of Homebrew's own directories** (E-8):
   - `Utils::Executable.ensure!` checks `ORIGINAL_PATHS` before `${HOMEBREW_PREFIX}/opt/<f>/bin` and `${HOMEBREW_PREFIX}/bin`. This covers attestation verification's `gh`, `skopeo` and `unzip`.
   - `Formula#ensure_installed!(executable:)`.
   - `with_homebrew_path`: the editor, and the Python `site` probe.
   - `brew log` and the `bump-*` commands.
   - The `objdump` used by a cellar check.

   A shim toward MacPorts named `gh` would be used to verify a Homebrew bottle's attestation, wherever that path is taken (Cannot determine: when).

9. **Sites where the user's `PATH` is inherited but placed behind Homebrew's** (E-8, E-9):
   - git downloads (`ENV["PATH"]`, then `ORIGINAL_PATHS`);
   - `bzip2` unpacking (brewed `bzip2` first, then `ORIGINAL_PATHS`);
   - `brew bundle exec` (superenv `PATH`, then the user's appended).

   On these, a shim comes after Homebrew's prefix and the system directories. It still wins for any name Homebrew's directories do not hold.

10. **`brew bundle` extensions find their package manager in the user's order** (`npm`, `cargo`, `krew`, VS Code-family editors) and run it with its directory first, then `ORIGINAL_PATHS` (E-9). **`brew exec`** runs a command with `HOMEBREW_PATH`, plus the requested formulae's directories prepended (E-9). **External (non-internal) `brew` commands** run with `HOMEBREW_PATH`, `shims/shared` prepended (E-2).

11. **`brew services`: no evidence of inheritance.** The service definition's `PATH` is the formula-declared value, conventionally `std_service_path_env`, plus an optional per-user `.env` file, not the caller's shell `PATH` (E-10).

**MacPorts** (2.12.6)

12. **`binpath`: default `${prefix}/bin:${prefix}/sbin:/bin:/sbin:/usr/bin:/usr/sbin`, commented out in the installed configuration** (E-11). `mportinit` overwrites `env(PATH)` with it unconditionally, before any port is opened (E-11). The caller's `PATH` is saved as `macports::user_path` and is used only by `port diagnose` as a membership check (E-11).

13. **Fetch, configure, build, destroot, activate: no.** Every phase command runs through `command_exec` over the `binpath` environment, plus the Portfile's own phase environment (E-12). The Portfile is the port's content, not the user's `PATH`. `destroot` may prepend MacPorts' own `clonebin_path` (E-12). A shim directory first on the user's `PATH` cannot come ahead, whatever the elevation route, because the reset happens inside `port` itself.

14. **Edge case: before the reset.** With `HOME` unset, `mportinit` runs `id`, `dscl` and `cut` by bare name with the inherited `PATH` (E-13). With `HOME` set, as under an ordinary shell and under `sudo`'s `env_reset` (E-15: `HOME` is set for the target user), nothing runs. None of the three is a build or link step.

15. **`port help` and `port edit` restore the caller's entire environment**, `PATH` included, to run `man` and the user's editor (E-14). These are not install phases, and nothing they run links or builds against another prefix. Whether running a user-chosen editor through the user's `PATH` counts as a "phase" under INV-07 is a reading for the main agent, not a fact.

16. **The elevated route**, per the installed `sudoers(5)` (E-15). `env_reset` is on by default. `PATH` is kept from the caller unless `secure_path` is set, in which case `secure_path` replaces it. This machine's `Defaults` are unreadable unprivileged, so whether `secure_path` is set here is UNRESOLVED. **It does not change finding 13**: `mportinit` replaces `PATH` either way. What `sudo`'s `PATH` does decide is how `sudo` resolves the name `port` itself. That is a question about the shim directory containing a `port` entry, not about a MacPorts phase.

17. **The unprivileged route** (`port` run without elevation, which works for read verbs and for a user-owned prefix) passes through the same `mportinit` (E-11, E-13). Finding 13 holds for it unchanged.

## Cannot determine

- **Whether the phases behave as read, at run time.** Not observed. Running a `post_install`, an `env :std` build, a cask `install_steps` or any `port` phase is a mutation. A disposable machine or VM, outside this repository's boundaries, would settle it by putting a marker directory first on `PATH` and logging `PATH` from a formula's `post_install_steps` and from a Portfile's `pre-configure`. `brew ruby -e 'puts ORIGINAL_PATHS'` would observe finding 1 read-only, but the project's hook refuses it as a mutating `brew` command.
- **When Homebrew takes the attestation path** that resolves `gh` (finding 8): which `HOMEBREW_*` settings, and whether this machine's defaults reach it. Tracing the callers of `sort_formulae_for_install` and `check_attestation` at this commit would settle it.
- **The running `PATH` in a cask installer `script`** (finding 5): whether it runs in the parent (reset `PATH`) or the sandboxed child (`ORIGINAL_PATHS`). Tracing where `Installer#install_phase` is dispatched would settle it.
- **Third-party taps**: any tap formula with `env :std`, a `Pathname`-returning `satisfy { which(…) }` or `ENV.fortran` is reached by findings 2-3. That depends on the user's taps and has to be detected at run time.
- **This machine's `/etc/sudoers` `Defaults`** (`secure_path`, `env_keep`): unreadable unprivileged (mode `0440 root:wheel`). Reading it needs elevation, which this project never uses. Not needed for the MacPorts answer.
- **The service manager's default environment** for a Homebrew service definition that declares no `PATH`. Not read, because the project hook refuses the launcher's name. Not needed to show that the caller's shell `PATH` is not an input.
- **Counts of core formulae using `std` or `post_install`** come from a tap checkout last updated 2026-09-02. With the API in use, the current set is whatever the API serves.

## Impact on the specification

- **`docs/passes/16-plans/I.md` D11**: the branch "A phase does inherit it" applies, for Homebrew only (Answer for D11 below). D11 already anticipates this branch from F-2. This note adds that `post_install`/`post_install_steps` and cask `install_steps` put nothing ahead of the user's `PATH`, and that several non-build helpers (`gh` for attestations, and others) prefer the user's order over Homebrew's own prefix.
- **D11's bullet "A shimmed `git` reaches `brew update`"** is contradicted by default (finding 6). `brew update`'s `git` is the selected absolute `HOMEBREW_GIT`. Only a bare-name `HOMEBREW_GIT_PATH` makes the user's `PATH` relevant. A shimmed `git` *is* reached when a `post_install` step runs `git` by name, because `shims/shared` is not on that `PATH` (finding 4). In an `env :std` build, `shims/shared/git` comes first and routes to `HOMEBREW_GIT` (finding 3).
- **D11 option (d)**: MacPorts shows no install phase that inherits the user's `PATH` (findings 12-17). Homebrew shows several. This is the per-provider fact (d) would be decided from, as dated data (INV-20).
- **`docs/research/F-2-usr-local-interference.md`**, E-9 and the table row at line 501:
  - The `formula.rb:1729-1738` citation is stale at this commit (now lines 1785-1787).
  - `post_install` is now deprecated in favour of `post_install_steps`, with the same environment.
  - Row 501 ("not investigated") is now answered here: git, curl and Ruby are not reached; the helpers in finding 8 are.
  - Do not edit F-2 from here; this is reported.
- **`docs/spec/09-research-checklist.md`** has no R-30 entry. A grep for `R-30` matched nothing. The main agent may want to add one.
- **INV-07 and `docs/spec/05-environment-and-primary-provider.md`**: nothing here contradicts their text. The note supplies the fact D11 conditions on.

## Re-check when

- Homebrew changes any of the following (record the new commit):
  - `bin/brew`'s `PATH`/`HOMEBREW_PATH` handling;
  - `global.rb`'s `ORIGINAL_PATHS`;
  - `Formula#run_post_install`;
  - `extend/ENV/std.rb` or `super.rb#setup_build_environment`;
  - `formula_installer.rb`'s `--env=std` selection;
  - `cask_artifact.rb`;
  - `utils/tool.sh`'s `select_tool`;
  - `Utils::Executable.ensure!`.
- Homebrew removes `post_install` in favour of `post_install_steps` and changes that environment, or removes `env :std`.
- A MacPorts release changes `mportinit`'s `binpath` assignment, `keepenvkeys`, `command_exec`, or the `boot_env` restores in `port`.
- The project's hooks allow a read-only `brew ruby -e`, or a disposable test machine becomes available. Then the run-time observations under Cannot determine become possible.

## Answer for D11

- **Homebrew: a phase does inherit a `PATH` in which the shim directory could come first.**
  - In `post_install`/`post_install_steps`, which also runs after a bottle pour, and in cask `install_steps`, the `PATH` *is* the user's `PATH`, so a shim directory first on it is first there (findings 4-5).
  - In `env :std` builds it comes ahead of `${HOMEBREW_PREFIX}/bin` and the system directories, behind only keg-only dependencies and `shims/shared` (finding 3).
  - Several non-build helpers (attestation `gh`, `ensure_installed!`, `with_homebrew_path`) resolve in the user's order ahead of Homebrew's own prefix (finding 8).
  - Superenv builds and `brew update`'s git and curl do not inherit, by default (findings 2, 6).
  - D11's second branch applies to Homebrew: "A phase does inherit it … A DP (`/decision-pending`) records it."
- **MacPorts: no phase that fetches, configures, builds, destroots or activates inherits the user's `PATH`.**
  - `mportinit` replaces it with `binpath` unconditionally, on both the elevated and the unprivileged route (findings 12-13, 16-17).
  - The exceptions found are not install phases: bare-name `id`/`dscl`/`cut` only when `HOME` is unset (finding 14), and `port help`/`port edit` (finding 15).
  - For MacPorts, D11's first branch holds: "No phase of the other provider inherits a `PATH` in which the shim directory could come first."
- The asymmetry is a dated per-provider fact (INV-20) about Homebrew 7.0.15-44-gee32747 and MacPorts 2.12.6, not a property of either migration direction (INV-17).
