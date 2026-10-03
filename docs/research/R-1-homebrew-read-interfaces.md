# R-1 — Homebrew read interfaces

- **Status:** answered (one sub-item UNRESOLVED, one blocked by this
  repository's own hook — see *Cannot determine*)
- **Answered on:** 2026-09-02 (side-effect finding, §E-A / Finding 1);
  extended and completed 2026-09-02 (full interface survey, §E-0 … §E-11)
- **Machine:** macOS 26.6.2 (build 25G83), x86_64 (kabylake, 12 logical cores),
  Homebrew 6.0.21 @ `/usr/local`, MacPorts not installed. Local timezone
  `-03`; every timestamp below is stated in UTC unless marked otherwise.
- **Blocking:** `04-migration-and-safety.md` (Process execution: "provider
  invocations set the environment that keeps them quiet and side-effect-free
  during analysis"); `02-domain-model.md` (artifact availability "must not
  trigger a provider auto-update"; package classes `GUI_APP` /
  `THIRD_PARTY_SOURCED`; ownership and user-intent modelling); INV-17 (both
  directions share a planner — the Homebrew *read* side of that planner is
  specified here); the requirement that `analyze` work offline; the read-only
  allowlist in `.claude/settings.json`.

## Question

Which Homebrew commands give machine-readable, stable data for: installed
packages; user-requested (top-level) packages; dependencies; reverse
dependencies; formula and cask metadata; deprecation and disabled state;
pinned state; taps; prefix and configuration. Which produce JSON, and how
stable is that JSON across versions. Which commands trigger a network fetch or
an auto-update as a side effect, and how is that suppressed. Which of these are
answerable **offline** — `analyze` must be. Confirm cask semantics and why
casks are outside migration scope.

---

## Evidence

All commands were run on 2026-09-02 from a Claude Code session in the
repository's checked-in environment. Output is trimmed where marked `…` but
never paraphrased. Every `brew` invocation was wrapped in `timeout 120`.

### E-0 · Environment, version, prefix, configuration

```
$ env | grep -i '^HOMEBREW' | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

`HOMEBREW_NO_INSTALL_FROM_API` was **unset** for every observation in this
note (i.e. the default, API-backed mode). It was deliberately never set — see
*Cannot determine*.

```
$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-09-02T12:37:26Z

$ brew --version
Homebrew 6.0.21
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)

$ command -v brew
/usr/local/bin/brew
```

```
$ brew --prefix
/usr/local
$ brew --cellar
/usr/local/Cellar
$ brew --caskroom
/usr/local/Caskroom
$ brew --repository
/usr/local/Homebrew
$ brew --cache
/Users/pirado/Library/Caches/Homebrew
```

```
$ brew config
HOMEBREW_VERSION: 6.0.21
ORIGIN: https://github.com/Homebrew/brew
HEAD: 560147012b9678b42ef5e83b690f0895552d1366
Last commit: 25 hours ago
Branch: stable
Core tap HEAD: e045d7385fd7ad8907621300224f8a860bdd7bbb
Core tap last commit: 3 hours ago
Core cask tap: N/A
HOMEBREW_PREFIX: /usr/local
HOMEBREW_NO_ANALYTICS: set
HOMEBREW_NO_AUTO_UPDATE: set
HOMEBREW_NO_ENV_HINTS: set
Homebrew Ruby: 4.0.6 => /usr/local/Homebrew/Library/Homebrew/vendor/portable-ruby/4.0.6/bin/ruby
CPU: dodeca-core 64-bit kabylake
Clang: 21.0.0 build 2100
Git: 2.55.0 => /usr/local/bin/git
Curl: 8.7.1 => /usr/bin/curl
macOS: 26.6.2-x86_64
CLT: 26.6.0.0.1781586589
Xcode: 26.5
```

`brew config` is line-oriented `Key: value`, not JSON. Note `Core cask tap:
N/A` — the cask tap is *not* checked out on this machine, yet cask metadata is
still available (§E-5), which is itself evidence that cask data comes from the
API cache.

Prefix path computation does **not** verify installation:

```
$ command -v jq; jq --version
/usr/bin/jq
jq-1.7.1-apple
$ brew list --formula | grep -c -x jq
0
$ brew --prefix jq
/usr/local/opt/jq
```

### E-A · Side-effect finding (recorded 2026-09-02, retained verbatim)

Command run for a formula name that does not exist:

```
$ brew info mpb
==> Tapping homebrew/core
Cloning into '/usr/local/Homebrew/Library/Taps/homebrew/homebrew-core'...
Updating files: 100% (9129/9129), done.
Tapped 5 commands and 8586 formulae (9,141 files, 1.4GB).
Error: No available formula with the name "mpb". Did you mean mpv, mpd, mpc or mob?
==> Searching for a previously deleted formula (in the last month)...
```

Wall-clock: the command exceeded a 120-second timeout and finished in the
background. Disk: a 1.4 GB clone of `homebrew/core` now exists under the
Homebrew prefix that was not there before.

**All later evidence in this note was gathered on a machine that therefore has
`homebrew/core` checked out.** That is not the default state of a Homebrew 4+
installation and is called out wherever it could change a conclusion.

```
$ find /usr/local/Homebrew/Library/Taps -maxdepth 2 -mindepth 2 -type d
/usr/local/Homebrew/Library/Taps/homebrew/homebrew-core

$ git -C /usr/local/Homebrew/Library/Taps/homebrew/homebrew-core log -1 --format='%H %cI'
e045d7385fd7ad8907621300224f8a860bdd7bbb 2026-09-02T09:54:08Z
```

### E-1 · Installed packages

```
$ brew list --formula
actionlint ada-url aom apr apr-util argon2 autoconf brotli bvi c-ares
ca-certificates cairo cask certifi cffi cloc cmake codeburn coreutils …
zeromq zstd
(180 lines)

$ brew list --cask
adoptopenjdk copilot-cli gcloud-cli gimp meld oracle-jdk visualdiffer wailbrew
wireshark-chmodbpf
(9 lines)
```

`brew list --versions` lists formulae **and** casks (189 lines = 180 + 9), one
space-separated record per line, with *all* installed kegs on the line:

```
$ brew list --versions | head -12
actionlint 1.7.12
ada-url 4.0.0
aom 3.14.1
apr 1.7.6
apr-util 1.6.5
argon2 20190702_1
autoconf 2.73
brotli 1.2.0
bvi 1.5.0
c-ares 1.34.8
ca-certificates 2026-08-13
cairo 1.18.4
…
(189 lines)

$ brew list --versions htop node openjdk@21
htop 3.5.3
node 26.7.0
openjdk@21 21.0.12.1

$ brew list --cask --versions
adoptopenjdk 16.0.1,9
copilot-cli 1.0.70
…
wireshark-chmodbpf 4.6.8
```

Multiple kegs per formula are normal on this machine:

```
$ brew list --multiple
cryptography 50.0.1 50.0.0
harfbuzz 14.3.1 14.4.0
libffi 3.7.1 3.8.0
mise 2026.9.0 2026.8.12
openssl@3 3.6.4 3.6.3
openssl@4 4.0.1 4.0.2
pcre2 10.47_1 10.48
php@8.4 8.4.25 8.4.24
…
(12 lines)
```

A cheap JSON inventory exists that is *not* `brew info`:

```
$ brew list --versions --json
{"casks":[…],"formulae":[…]}

$ brew list --versions --json | jq -c '.formulae[0:2]'
[{"name":"actionlint","versions":["1.7.12"],"linked_version":"1.7.12","optlinked_version":"1.7.12","pinned_version":null},
 {"name":"ada-url","versions":["4.0.0"],"linked_version":"4.0.0","optlinked_version":"4.0.0","pinned_version":null}]
```

`brew list --help` (Homebrew 6.0.21) describes it as:

```
      --json                       Output installed formulae and casks with
                                   versions, linked and opt-linked formula
                                   versions and pinned versions as JSON using
                                   the fast Bash command path. Requires
                                   --versions, no named arguments and jq.
```

It ran successfully here using Apple's `/usr/bin/jq` (jq is not a Homebrew
formula on this machine).

### E-2 · User intent vs inferred top-level position

Two *different* mechanisms, and they disagree on this machine.

```
$ brew leaves
actionlint bvi cask cloc cmake codeburn findutils firebase-cli gemini-cli gh
ghidra git git-lfs gitleaks go-task graphviz htop httpd jupyterlab just links
llvm maturin mise moreutils nmap nvm nvtop opentofu osv-scanner p7zip phpbrew
pipx pkgconf python@3.12 python@3.13 sem-cli semgrep sonar-scanner swiftformat
swiftlint tmux tree unzip uv watch wget wireshark xcodegen
(49 lines)

$ brew leaves --installed-on-request
… (44 lines)

$ brew leaves --installed-as-dependency
cmake httpd llvm maturin pkgconf
(5 lines)
```

```
$ brew list --installed-on-request
(47 lines)
$ brew list --no-installed-on-request
(133 lines)
```

```
$ brew info --json=v2 --installed | jq -r '[.formulae[]|.installed[0].installed_on_request]|group_by(.)|map({v:.[0],n:length})' -c
[{"v":false,"n":133},{"v":true,"n":47}]
```

The three ways of asking "did the user ask for this?" and the one way of asking
"is this top-level?" reconciled:

```
$ jq -r '.formulae[]|select(.installed[0].installed_on_request==true)|.name' brew-info-installed.json | sort > json-ior.txt
$ sort leaves-ior.txt > leaves-ior-sorted.txt
$ comm -23 json-ior.txt leaves-ior-sorted.txt
coreutils
curl
node
$ comm -13 json-ior.txt leaves-ior-sorted.txt
(empty)
```

The three are exactly the requested formulae that something else also depends
on:

```
$ brew uses --installed coreutils
cask
$ brew uses --installed curl
php@8.4 phpbrew
$ brew uses --installed node
codeburn firebase-cli gemini-cli jupyterlab
```

`brew leaves --help` (Homebrew 6.0.21):

```
Usage: brew leaves [--installed-on-request] [--installed-as-dependency]

List installed formulae that are not dependencies of another installed formula
or cask.

  -r, --installed-on-request       Only list leaves that were manually
                                   installed.
  -p, --installed-as-dependency    Only list leaves that were installed as
                                   dependencies.
```

**The `installed_as_dependency` field asked about in the work order does not
exist in this Homebrew's JSON, and is not reliably present on disk either.**

```
$ jq -r '[.. | objects | keys[]] | map(select(test("installed_as_dependency"))) | length' brew-info-installed.json
0

$ jq -r '[.formulae[].installed[]?|keys[]]|unique|join(", ")' brew-info-installed.json
built_as_bottle, installed_on_request, poured_from_bottle, runtime_dependencies, time, used_options, version
```

On disk, in the install receipts (192 kegs):

```
$ for f in /usr/local/Cellar/*/*/INSTALL_RECEIPT.json; do jq -e 'has("installed_on_request")' "$f" >/dev/null && c=$((c+1)); done
192 of 192

$ (same loop for has("installed_as_dependency"))
70 of 192

$ for f in …; do jq -r 'if has("installed_as_dependency") then .homebrew_version else empty end' "$f"; done | awk -F. '{print $1}' | sort | uniq -c
  33 4
  37 5
$ for f in …; do jq -r 'if has("installed_as_dependency") then empty else .homebrew_version end' "$f"; done | awk -F. '{print $1}' | sort | uniq -c
   2 4
  14 5
 106 6

$ for f in …; do jq -r 'if has("installed_as_dependency") then (.installed_as_dependency|tostring) else empty end' "$f"; done | sort | uniq -c
  11 false
  59 true
```

Receipt shape (one keg), and the JSON view of the same keg:

```
$ jq -r 'keys[]' /usr/local/Cellar/htop/*/INSTALL_RECEIPT.json | tr '\n' ' '
aliases arch built_as_bottle built_on changed_files compiler homebrew_version
installed_on_request loaded_from_api loaded_from_internal_api
poured_from_bottle runtime_dependencies source source_modified_time time
unused_options used_options

$ jq -c '{installed_as_dependency, installed_on_request, poured_from_bottle, built_as_bottle, time, arch, built_on}' /usr/local/Cellar/htop/*/INSTALL_RECEIPT.json
{"installed_as_dependency":null,"installed_on_request":true,"poured_from_bottle":true,"built_as_bottle":true,"time":1786962997,"arch":"x86_64","built_on":{"os":"Macintosh","os_version":"macOS 14.8","cpu_family":"penryn","xcode":"16.2","clt":"16.2.0.0.1.1733547573","preferred_perl":"5.34"}}
```

(`installed_as_dependency: null` here is jq reporting an **absent** key, not a
stored null — confirmed by the `has()` counts above.)

Receipt provenance flags, all 192 kegs:

```
$ for f in /usr/local/Cellar/*/*/INSTALL_RECEIPT.json; do jq -r '"api=\(.loaded_from_api) internal=\(.loaded_from_internal_api)"' "$f"; done | sort | uniq -c
   7 api=false internal=false
  14 api=true  internal=false
  61 api=true  internal=null
 110 api=true  internal=true
```

### E-3 · Dependencies

```
$ brew deps --installed | head -6
actionlint: gmp shellcheck
ada-url: fmt
adoptopenjdk:
aom: libvmaf
apr:
apr-util: apr ca-certificates openssl@3
…
(189 lines — formulae and casks; `adoptopenjdk` is a cask)

$ brew deps --installed --tree | head -20
actionlint
└── shellcheck
    └── gmp

ada-url
└── fmt

adoptopenjdk

aom
└── libvmaf

apr

apr-util
├── apr
└── openssl@3
    └── ca-certificates

argon2
…
(650 lines)

$ brew deps --formula htop
ncurses
```

There is **no JSON output for `brew deps`**:

```
$ brew deps --installed --json
Usage: brew deps [options] [formula|cask ...]

Show dependencies for formula. When given multiple formula arguments, show the
intersection of dependencies for each formula. By default, deps shows all
required and recommended dependencies.
…
```

(An unrecognised option makes `brew deps` print usage.)

Per-keg runtime dependencies with resolved versions *are* available in JSON,
inside `installed[].runtime_dependencies` (§E-11).

### E-4 · Reverse dependencies

```
$ brew uses --installed ncurses
htop tmux watch
$ brew uses --installed --recursive ncurses
htop tmux watch
$ brew uses ncurses
aarch64-elf-gdb
apachetop
arm-none-eabi-gdb
asciiquarium
atop
bash
…
(the whole tap — many hundreds of lines)

$ brew uses --installed --json ncurses
Usage: brew uses [options] formula [...]

Show formulae and casks that specify formula as a dependency; that is, show
dependents of formula. …
```

No JSON. `--installed` is essential: without it the answer is tap-wide.

### E-5 · Metadata (`brew info --json=v2`)

```
$ brew info --json=v2 --installed > docs/research/.scratch/brew-info-installed.json
exit=0 ; real 5.28 user 1.94 sys 1.08 ; 876713 bytes

$ jq -r 'keys[]' brew-info-installed.json
casks
formulae
$ jq -r '"formulae: \(.formulae|length)  casks: \(.casks|length)"' brew-info-installed.json
formulae: 180  casks: 9
```

Union of formula-object keys across all 180 entries:

```
aliases, autobump, bottle, build_dependencies, caveats, compatibility_version,
conflicts_with, conflicts_with_reasons, dependencies, deprecate_args,
deprecated, deprecation_date, deprecation_reason,
deprecation_replacement_cask, deprecation_replacement_formula, desc,
disable_args, disable_date, disable_reason, disable_replacement_cask,
disable_replacement_formula, disabled, full_name, head_dependencies, homepage,
installed, keg_only, keg_only_reason, license, link_overwrite, linked_keg,
name, no_autobump_message, oldnames, optional_dependencies, options, outdated,
patches, pinned, post_install_defined, post_install_steps, pour_bottle_only_if,
recommended_dependencies, requirements, revision, ruby_source_checksum,
ruby_source_path, service, skip_livecheck, tap, tap_git_head,
test_dependencies, urls, uses_from_macos, uses_from_macos_bounds,
version_scheme, versioned_formulae, versions
```

Union of cask-object keys across all 9 entries:

```
artifacts, auto_updates, autobump, bundle_short_version, bundle_version,
caveats, caveats_rosetta, conflicts_with, container, depends_on,
deprecate_args, deprecated, deprecation_date, deprecation_reason,
deprecation_replacement_cask, deprecation_replacement_formula, desc,
disable_args, disable_date, disable_reason, disable_replacement_cask,
disable_replacement_formula, disabled, full_token, homepage, installed,
installed_time, languages, name, no_autobump_message, old_tokens, outdated,
pinned, pinned_version, rename, ruby_source_checksum, ruby_source_path, sha256,
skip_livecheck, tap, tap_git_head, token, url, url_specs, version
```

Field values on this machine:

```
$ jq -r '"pinned true: \([.formulae[]|select(.pinned==true)]|length)"' …
pinned true: 0
$ … outdated true …
outdated true: 12 -> codeburn, emacs, firebase-cli, gh, git-lfs, libgcrypt, node, pandoc, sem-cli, semgrep, swiftformat, uv
$ … keg_only true …
keg_only true: 19 -> apr, apr-util, curl, icu4c@78, krb5, libffi, libpq, llvm, m4, ncurses, net-snmp, openjdk, openjdk@21, openldap, openssl@4, php@8.4, readline, sqlite, unzip
$ … deprecated true …
deprecated true: 1 -> gemini-cli
$ … disabled true …
disabled true: 0
$ jq -r '"tap values: \([.formulae[]|.tap]|unique|join(", "))"' …
tap values: homebrew/core
$ jq -r '"formulae with non-null service: \([.formulae[]|select(.service!=null)|.name]|join(", "))"' …
emacs, httpd, jupyterlab, php@8.4, unbound
```

Deprecation, full field set, for the one deprecated formula:

```
$ jq -c '.formulae[]|select(.name=="gemini-cli")|{deprecated, deprecation_date, deprecation_reason, deprecation_replacement_formula, deprecation_replacement_cask, disabled, disable_date, disable_reason}' …
{"deprecated":true,"deprecation_date":"2026-06-18","deprecation_reason":"unsupported","deprecation_replacement_formula":null,"deprecation_replacement_cask":"antigravity-cli","disabled":false,"disable_date":"2026-12-18","disable_reason":null}
```

**`installed[]` is ordered oldest keg first, and reading `installed[0]` gives
the wrong provenance.** Compare:

```
$ jq -r '"poured_from_bottle true: \([.formulae[]|select(.installed[0].poured_from_bottle==true)]|length)"' …
poured_from_bottle true: 180 / false: 0

$ brew list --poured-from-bottle | wc -l
173
$ brew list --built-from-source
mise openssl@3 pcre2 php@8.4 pydantic simdjson tree-sitter
(7)

$ jq -r '[.formulae[]|select([.installed[].poured_from_bottle]|any(.==false))|.name]|join(", ")' …
mise, openssl@3, pcre2, php@8.4, pydantic, simdjson, tree-sitter
```

All kegs of those seven:

```
{"name":"mise","linked_keg":"2026.9.0","kegs":[{"version":"2026.8.12","poured_from_bottle":true,"built_as_bottle":true},{"version":"2026.9.0","poured_from_bottle":false,"built_as_bottle":false}]}
{"name":"openssl@3","linked_keg":"3.6.4","kegs":[{"version":"3.6.3","poured_from_bottle":true,…},{"version":"3.6.4","poured_from_bottle":false,…}]}
{"name":"pcre2","linked_keg":"10.48","kegs":[{"version":"10.47_1","poured_from_bottle":true,…},{"version":"10.48","poured_from_bottle":false,…}]}
{"name":"php@8.4","linked_keg":null,"kegs":[{"version":"8.4.24","poured_from_bottle":true,…},{"version":"8.4.25","poured_from_bottle":false,…}]}
{"name":"pydantic","linked_keg":null,"kegs":[{"version":"2.13.4","poured_from_bottle":true,…},{"version":"2.13.5","poured_from_bottle":false,…}]}
{"name":"simdjson","linked_keg":"4.6.9","kegs":[{"version":"4.6.8","poured_from_bottle":true,…},{"version":"4.6.9","poured_from_bottle":false,…}]}
{"name":"tree-sitter","linked_keg":"0.27.0","kegs":[{"version":"0.26.13","poured_from_bottle":true,…},{"version":"0.27.0","poured_from_bottle":false,…}]}

$ for f in /usr/local/Cellar/mise/*/INSTALL_RECEIPT.json; do echo -n "$f: "; jq -c '{poured_from_bottle, built_as_bottle, installed_on_request, homebrew_version}' $f; done
/usr/local/Cellar/mise/2026.8.12/INSTALL_RECEIPT.json: {"poured_from_bottle":true,"built_as_bottle":true,"installed_on_request":true,"homebrew_version":"6.0.19-9-g533246d"}
/usr/local/Cellar/mise/2026.9.0/INSTALL_RECEIPT.json:  {"poured_from_bottle":false,"built_as_bottle":false,"installed_on_request":true,"homebrew_version":"6.0.21"}
```

### E-6 · Bottle availability — the three states, separated

`bottle` in `brew info --json=v2` describes the **latest** version, not the
installed one, and on this machine it is **platform-filtered**:

```
$ jq -r '[.formulae[]|(.bottle.stable.files? // {})|keys[]]|group_by(.)|map("\(.[0])=\(length)")|join("  ")' …
all=7  sonoma=137  tahoe=18

$ jq -r '[.formulae[]|select((.bottle|length)==0)|.name]|join(", ")' …
codeburn, emacs, firebase-cli, gh, git-lfs, mise, node, openssl@3, pandoc,
pcre2, php@8.4, pydantic, sem-cli, semgrep, simdjson, swiftformat,
tree-sitter, uv

$ jq -r '.formulae[]|"\(if (.bottle|length)==0 then "nobottle" else "bottle" end)\t\(.outdated)"' … | sort | uniq -c
 161 bottle    false
   1 bottle    true
   7 nobottle  false
  11 nobottle  true

$ jq -r '[.formulae[]|select((.bottle|length)==0 and .outdated==false)|.name]|join(", ")' …
mise, openssl@3, pcre2, php@8.4, pydantic, simdjson, tree-sitter
```

That last set is **identical** to `brew list --built-from-source`.

```
$ jq -c '.formulae[]|select(.name=="node")|{name, versions, outdated, bottle, installed:[.installed[]|{version,poured_from_bottle}]}' …
{"name":"node","versions":{"stable":"26.8.1","head":"HEAD","bottle":false},"outdated":true,"bottle":{},"installed":[{"version":"26.7.0","poured_from_bottle":true}]}
```

The tap on disk (read offline, no network) shows the artifact *does* exist —
for other platforms:

```
$ grep -A 9 '^  bottle do' /usr/local/Homebrew/Library/Taps/homebrew/homebrew-core/Formula/n/node.rb
  bottle do
    sha256 arm64_tahoe:   "489838f2…"
    sha256 arm64_sequoia: "57b4cb8b…"
    sha256 arm64_sonoma:  "dd9950f4…"
    sha256 arm64_linux:   "f5616d4a…"
    sha256 x86_64_linux:  "b675cb33…"
  end

$ … Formula/s/simdjson.rb
  bottle do
    sha256 cellar: :any, arm64_tahoe:   "bbfac78d…"
    sha256 cellar: :any, arm64_sequoia: "e09e5abf…"
    sha256 cellar: :any, arm64_sonoma:  "b48ba21e…"
    sha256 cellar: :any, arm64_linux:   "47103d06…"
    sha256 cellar: :any, x86_64_linux:  "0ae85ef5…"
  end

$ … Formula/p/pcre2.rb
  bottle do
    sha256 cellar: :any, arm64_tahoe:   "46681bd5…"
    sha256 cellar: :any, arm64_sequoia: "a24eb46d…"
    sha256 cellar: :any, arm64_sonoma:  "eb8e1a4b…"
    sha256 cellar: :any, arm64_linux:   "e3802b43…"
    sha256 cellar: :any, x86_64_linux:  "73de8eb8…"
  end
```

Contrast with a formula that *does* have an Intel macOS bottle:

```
$ grep -A 9 '^  bottle do' … Formula/h/htop.rb
  bottle do
    sha256 cellar: :any, arm64_tahoe:   "d57bf20d…"
    sha256 cellar: :any, arm64_sequoia: "c9f94a86…"
    sha256 cellar: :any, arm64_sonoma:  "76274649…"
    sha256 cellar: :any, sonoma:        "598e42a8…"
    sha256 cellar: :any, arm64_linux:   "5421cf57…"
    sha256 cellar: :any, x86_64_linux:  "781f9ced…"
  end
```

and what `brew info --json=v2` reduced that to on this machine:

```
$ jq -c '.formulae[]|select(.name=="htop")|.bottle' …
{"stable":{"rebuild":0,"root_url":"https://ghcr.io/v2/homebrew/core","files":{"sonoma":{"cellar":":any","url":"https://ghcr.io/v2/homebrew/core/htop/blobs/sha256:598e42a8…","sha256":"598e42a8…"}}}}
```

The cached API index explains the reduction — it stores exactly one bottle for
the running platform:

```
$ jq -s -c '.[1].metadata' ~/Library/Caches/Homebrew/api/internal/packages.tahoe.jws.json.payload
{"homebrew_version":"6.0.21-28-gfd18738","bottle_tag":"tahoe","generated_at":1788350020}
   # generated_at = 2026-09-02T11:53:40Z

$ jq -s -c '.[1].formulae.htop|{bottle_tag, bottle_cellar, bottle_checksum, stable_version}' …payload
{"bottle_tag":":sonoma","bottle_cellar":":any","bottle_checksum":"598e42a8…","stable_version":"3.5.3"}

$ jq -s -r '[.[1].formulae.node|keys[]|select(startswith("bottle"))]|join(", ")' …payload
(none)

$ jq -s -r '[.[1].formulae|to_entries[]|select(.value|has("bottle_tag")|not)|.key]|length' …payload
857
   # of 8586 formulae in the index
```

### E-7 · Pinned state

```
$ brew list --pinned > pinned.txt 2>&1
exit=0
bytes=0
(no output — nothing is pinned)

$ ls -la /usr/local/var/homebrew/pinned
ls: /usr/local/var/homebrew/pinned: No such file or directory
```

Also exposed as `pinned` (formulae and casks) and `pinned_version` (casks, and
in `brew list --versions --json`) in JSON, and in `brew outdated --json=v2`.

### E-8 · Outdated

```
$ brew outdated
codeburn
emacs
firebase-cli
gh
git-lfs
libgcrypt
node
pandoc
sem-cli
semgrep
swiftformat
uv
visualdiffer
wailbrew
(14 lines — 12 formulae + 2 casks)

$ brew outdated --json=v2 | jq -r 'keys|join(", ")'
casks, formulae
$ … | jq -c '.formulae[0]'
{"name":"codeburn","installed_versions":["0.9.20"],"current_version":"0.9.23","pinned":false,"pinned_version":null}
$ … | jq -c '.casks[0]'
{"name":"visualdiffer","installed_versions":["2.1.21"],"current_version":"2.1.22","pinned":false,"pinned_version":null}
```

### E-9 · Taps — NOT OBSERVED (blocked)

```
$ brew tap
$ brew tap-info --installed --json
Blocked by MacPortsBrew INV-05 and by docs/spec/07-quality-and-testing.md.
This command mutates the developer's real Homebrew installation. …
```

This repository's own `PreToolUse` hook refuses any `brew tap*` invocation,
including the argument-less read-only listing form and `brew tap-info`. Per
`CLAUDE.md` ("If a hook blocks something you believe is legitimate, that is a
question for the human, not an obstacle to route around") the command was **not
rephrased**. Tap state was instead read from the filesystem (§E-A) and from
`brew config` (`Core tap HEAD`, `Core cask tap: N/A`) and from the JSON `tap`
and `tap_git_head` fields (§E-5, §E-11).

### E-10 · Network behaviour and the API cache

The cache directory does **not** contain `formula.jws.json` or `cask.jws.json`.
Homebrew 6.0.21 uses a single merged, platform-tagged internal index:

```
$ ls -la ~/Library/Caches/Homebrew/api/
drwxr-xr-x@ 4 pirado  staff     128 Sep  2 09:20 .
-rw-r--r--@ 1 pirado  staff  106523 Sep  2 09:20 cask_names.txt
drwxr-xr-x@ 5 pirado  staff     160 Sep  2 09:20 internal

$ for f in …; do TZ=UTC stat -f '%N  size=%z  mtime=%Sm UTC' -t '%Y-%m-%dT%H:%M:%S' "$f"; done
…/api/cask_names.txt                                     size=106523    mtime=2026-09-02T12:20:29 UTC
…/api/internal/packages.tahoe.jws.json                   size=15567297  mtime=2026-09-02T12:20:29 UTC
…/api/internal/packages.tahoe.jws.json.payload           size=14306521  mtime=2026-09-02T12:20:29 UTC
…/api/internal/packages.tahoe.jws.json.payload.index     size=450765    mtime=2026-09-02T12:38:08 UTC
```

```
$ jq -s -r '"docs=\(length)"' …payload
docs=2
$ jq -s -r '"formulae: \(.[1].formulae|type) \(.[1].formulae|length)  casks: \(.[1].casks|type) \(.[1].casks|length)"' …payload
formulae: object 8586  casks: object 7720
$ jq -s -r '.[1]|keys|join(", ")' …payload
cask_renames, cask_tap_git_head, cask_tap_migrations, casks, formula_aliases,
formula_renames, formula_tap_git_head, formula_tap_migrations, formulae, metadata
```

Controlled test, run at 12:45:30 UTC — 25 minutes after the last cache refresh
and far past the documented 450-second API auto-update interval:

```
T0 12:45:30
  …/packages.tahoe.jws.json               mtime=12:20:29
  …/packages.tahoe.jws.json.payload       mtime=12:20:29
  …/packages.tahoe.jws.json.payload.index mtime=12:38:08
  …/cask_names.txt                        mtime=12:20:29
--- run: brew list --formula ---              exit=0 at 12:45:30   (all four mtimes unchanged)
--- run: brew info --json=v2 --installed ---  exit=0 at 12:45:35   (all four mtimes unchanged)
--- run: brew leaves ---                      exit=0 at 12:45:39   (all four mtimes unchanged)
--- run: brew outdated ---                    exit=0 at 12:45:42   (all four mtimes unchanged)
```

An earlier, separate before/after measurement around the first
`brew info --json=v2 --installed` run (12:37:26 → 12:37:32 UTC) likewise showed
`packages.tahoe.jws.json` and `cask_names.txt` unchanged at 12:20:29 UTC.

Official documentation, read from the Homebrew repository checked out at
`/usr/local/Homebrew` (HEAD `560147012b9678b42ef5e83b690f0895552d1366`,
committed 2026-09-01), file `manpages/brew.1`, read 2026-09-02. Canonical URL:
<https://docs.brew.sh/Manpage>.

> **HOMEBREW_API_AUTO_UPDATE_SECS**
> Check Homebrew's API for new formulae or cask data every
> `$HOMEBREW_API_AUTO_UPDATE_SECS` seconds. Alternatively, disable API
> auto-update checks entirely with `$HOMEBREW_NO_AUTO_UPDATE`.
> *Default:* `450`.

> **HOMEBREW_AUTO_UPDATE_SECS**
> Run `brew update` once every `$HOMEBREW_AUTO_UPDATE_SECS` seconds before some
> commands, e.g. `brew install`, `brew upgrade` or `brew tap`. Alternatively,
> disable auto-update entirely with `$HOMEBREW_NO_AUTO_UPDATE`.
> *Default:* `86400` (24 hours), `3600` (1 hour) if a developer command has
> been run or `300` (5 minutes) if `$HOMEBREW_NO_INSTALL_FROM_API` is set.

> **HOMEBREW_NO_INSTALL_FROM_API**
> If set, do not install formulae and casks in homebrew/core and homebrew/cask
> taps using Homebrew's API and instead use (large, slow) local checkouts of
> these repositories.

> **HOMEBREW_NO_ANALYTICS**
> If set, do not send analytics. Google Analytics were destroyed. …

> **HOMEBREW_NO_ENV_HINTS**
> If set, do not print any hints about changing Homebrew's behaviour with
> environment variables.

### E-11 · JSON schema stability, and the `--json=v2` citation

`brew info --help` (Homebrew 6.0.21, run 2026-09-02):

```
      --json                       Print a JSON representation. Currently the
                                   default value for version is v1.v1 is
                                   valid for formula only. v2 is valid for
                                   both formula and cask.See the docs for
                                   examples of using the JSON output:
                                   https://docs.brew.sh/Querying-Brew
      --installed                  Output a human-readable inventory of
                                   installed formulae and casks. If --json is
                                   passed, print JSON for installed formulae
                                   and, with --json=v2, installed casks.
      --variations                 Include the variations hash in each formula's
                                   JSON output.
```

<https://docs.brew.sh/Manpage>, read 2026-09-02, states for `brew info
--json`: "Print a JSON representation. Currently the default value for version
is v1. v1 is valid for formula only. v2 is valid for both formula and cask."
For `brew outdated --json`: "Print output in JSON format. There are two
versions: v1 and v2. v1 is deprecated and is currently the default if no
version is specified. v2 prints outdated formulae and casks." For `brew
tap-info --json`: "Print a JSON representation of tap. Currently the default
and only accepted value for version is v1."

`docs/Querying-Brew.md` in the same checkout (front matter
`last_review_date: "2025-02-08"`), canonical URL
<https://docs.brew.sh/Querying-Brew>, read 2026-09-02 — **this is the
stability statement**:

> The default schema version is `v1`, which returns info about formulae;
> specify `--json=v2` to include both formulae and casks. Note that fields may
> be added to the schema as needed without incrementing the schema. Any
> significant breaking changes will cause a change to the schema version.
>
> The schema itself is not currently documented outside of the code in
> [`formula.rb`](https://github.com/Homebrew/brew/blob/2e6b6ab3a20da503ba2a22a37fdd6bd936d818ed/Library/Homebrew/formula.rb#L1922-L2017)
> that generates it.

Observed shapes:

```
$ brew info --json htop   | jq -r 'type'   ->  array   (len=1)
$ brew info --json=v1 htop| jq -r 'type'   ->  array   (len=1)
$ brew info --json=v2 htop| jq -r 'keys'   ->  ["casks","formulae"]  (formulae=1, casks=0)
$ diff <(v1 element keys) <(v2 .formulae[0] keys)  -> identical
$ brew info --json=v2 --installed --formula | jq -r '"formulae=\(.formulae|length) casks=\(.casks|length)"'
formulae=180 casks=0
```

### E-12 · Fixture reference — one installed formula, `brew info --json=v2`

Raw untrimmed output of `brew info --json=v2 --installed` (876 713 bytes,
180 formulae + 9 casks) saved to
`docs/research/.scratch/brew-info-installed.json` (git-ignored) for later
fixture capture.

The `htop` entry, trimmed to the load-bearing keys (empty arrays, `patches`,
`options`, `head_dependencies`, `urls.head`, `requirements`, `link_overwrite`
and `conflicts_with*` elided):

```json
{
  "name": "htop",
  "full_name": "htop",
  "tap": "homebrew/core",
  "oldnames": ["htop-osx"],
  "aliases": [],
  "versioned_formulae": [],
  "desc": "Improved top (interactive process viewer)",
  "license": "GPL-2.0-or-later",
  "homepage": "https://htop.dev/",
  "versions": { "stable": "3.5.3", "head": "HEAD", "bottle": true },
  "urls": {
    "stable": {
      "url": "https://github.com/htop-dev/htop/releases/download/3.5.3/htop-3.5.3.tar.xz",
      "tag": null, "revision": null, "using": null,
      "checksum": "a8b164386494cb85bb255a415a3f5f80afe7a0c4491da5d113b3a0f951087e65"
    }
  },
  "revision": 0,
  "version_scheme": 0,
  "compatibility_version": null,
  "autobump": true,
  "bottle": {
    "stable": {
      "rebuild": 0,
      "root_url": "https://ghcr.io/v2/homebrew/core",
      "files": {
        "sonoma": {
          "cellar": ":any",
          "url": "https://ghcr.io/v2/homebrew/core/htop/blobs/sha256:598e42a8354e720c30f70d5713d977cf88386379710e7876f9e733ea132c8f98",
          "sha256": "598e42a8354e720c30f70d5713d977cf88386379710e7876f9e733ea132c8f98"
        }
      }
    }
  },
  "pour_bottle_only_if": null,
  "keg_only": false,
  "keg_only_reason": null,
  "build_dependencies": ["pkgconf"],
  "dependencies": ["ncurses"],
  "test_dependencies": [],
  "recommended_dependencies": [],
  "optional_dependencies": [],
  "uses_from_macos": [],
  "caveats": "htop requires root privileges to correctly display all running processes,\nso you will need to run `sudo htop`.\n…",
  "installed": [
    {
      "version": "3.5.3",
      "used_options": [],
      "built_as_bottle": true,
      "poured_from_bottle": true,
      "time": 1786962997,
      "runtime_dependencies": [
        {
          "full_name": "ncurses",
          "version": "6.6",
          "revision": 0,
          "bottle_rebuild": 0,
          "pkg_version": "6.6",
          "declared_directly": true
        }
      ],
      "installed_on_request": true
    }
  ],
  "linked_keg": "3.5.3",
  "pinned": false,
  "outdated": false,
  "deprecated": false,
  "deprecation_date": null,
  "deprecation_reason": null,
  "deprecation_replacement_formula": null,
  "deprecation_replacement_cask": null,
  "disabled": false,
  "disable_date": null,
  "disable_reason": null,
  "post_install_steps": [],
  "post_install_defined": false,
  "service": null,
  "tap_git_head": "e045d7385fd7ad8907621300224f8a860bdd7bbb",
  "ruby_source_path": "Formula/h/htop.rb",
  "ruby_source_checksum": { "sha256": "0b162472e8a39c14fee92c25654388c264b86c16fcbc7cab34266fe1b12d72ad" }
}
```

One installed cask, complete (`visualdiffer`), for contrast:

```json
{
  "token": "visualdiffer",
  "full_token": "visualdiffer",
  "old_tokens": [],
  "tap": "homebrew/cask",
  "name": ["VisualDiffer"],
  "desc": "Visually compare folders and files",
  "homepage": "https://visualdiffer.com/",
  "url": "https://github.com/visualdiffer/visualdiffer/releases/download/v2.1.22/VisualDiffer-2.1.22.zip",
  "url_specs": {},
  "version": "2.1.22",
  "installed": "2.1.21",
  "installed_time": 1787515911,
  "bundle_version": "2.1.21",
  "bundle_short_version": "2.1.21",
  "pinned": false,
  "pinned_version": null,
  "outdated": true,
  "sha256": "16cd75d5dcb902266e4bf18b62db671c83ffb82a9d3399a2a97cc5aa46aeb60e",
  "artifacts": [
    { "app": ["VisualDiffer.app"], "target": "/Applications/VisualDiffer.app" },
    { "zap": [ { "trash": [ "~/Library/Application Support/VisualDiffer", "~/Library/Preferences/com.visualdiffer.plist", "~/Library/Saved Application State/com.visualdiffer.savedState" ] } ] }
  ],
  "depends_on": { "macos": { ">=": ["13"] } },
  "auto_updates": true,
  "deprecated": false,
  "disabled": false,
  "tap_git_head": "6cb45133ec1037466c45bfe7864d5f3bf000dcbf",
  "languages": [],
  "ruby_source_path": "Casks/v/visualdiffer.rb",
  "ruby_source_checksum": { "sha256": "df0bb31c504876faf445128699778b18e40050e114cf524eaef7743b56f7cb22" }
}
```

### E-13 · Cask semantics (official terminology)

`/usr/local/Homebrew/manpages/brew.1`, TERMINOLOGY section, read 2026-09-02
(canonical URL <https://docs.brew.sh/Manpage>):

> **formula** — Homebrew package definition that builds from upstream sources
>
> **cask** — Homebrew package definition that installs pre-compiled binaries
> built and signed by upstream
>
> **keg** — installation destination directory of a given **formula** version,
> e.g. `/opt/homebrew/Cellar/foo/0.1`
>
> **keg-only** — a **formula** is *keg-only* if it is not symlinked into
> Homebrew's prefix
>
> **Caskroom** — directory containing one or more named **casks**, e.g.
> `/opt/homebrew/Caskroom`
>
> **tap** — directory (and usually Git repository) of **formulae**, **casks**
> and/or **external commands**
>
> **bottle** — pre-built **keg** poured into a **rack** of the **Cellar**
> instead of building from upstream sources

---

## Findings

1. **`brew info <name>` for a name the API does not know taps
   `homebrew/core`** — a network clone of the full formula repository into the
   provider's prefix — before reporting the miss, in order to search for a
   recently deleted formula (§E-A). A command classified as read-only wrote
   1.4 GB into the developer's Homebrew installation and took minutes. The
   project's quieting variables did not suppress it. "Package not found" is a
   legitimate answer the adapter must return without side effects; an adapter
   that reaches it the way `brew info` does violates the analysis-must-not-
   mutate rule. *(Original 2026-09-02 finding, unchanged.)*

2. **The quiet environment that matters for network suppression is
   `HOMEBREW_NO_AUTO_UPDATE=1`, and the documentation says so explicitly.**
   `HOMEBREW_API_AUTO_UPDATE_SECS` defaults to 450 seconds and its manual entry
   says API auto-update checks are disabled entirely by
   `$HOMEBREW_NO_AUTO_UPDATE` (§E-10). Observation matches: with
   `HOMEBREW_NO_AUTO_UPDATE=1` set, `brew list --formula`, `brew info
   --json=v2 --installed`, `brew leaves` and `brew outdated` all left every
   file under `~/Library/Caches/Homebrew/api/` byte-for-byte untouched 25
   minutes after the cache was last refreshed (§E-10) — i.e. well past the
   450-second window in which an unquieted Homebrew would have gone to the
   network. `HOMEBREW_NO_ANALYTICS=1` suppresses telemetry and
   `HOMEBREW_NO_ENV_HINTS=1` suppresses advisory stderr noise; neither affects
   the network path. **Record for the adapter: the three variables the project
   already sets are necessary, and `HOMEBREW_NO_AUTO_UPDATE` is the one doing
   the offline work.** *(Narrowed 2026-10-02 by
   [R-15](R-15-homebrew-brew-env.md): this holds for a present cache — with
   none, a Ruby read downloads it first — and a `brew.env` can outrank or
   delete the variable.)*

3. **Metadata is answerable offline, from a signed local cache — but the cache
   is platform-filtered.** Homebrew 6.0.21 does **not** use
   `formula.jws.json` / `cask.jws.json`. It uses a single merged index,
   `~/Library/Caches/Homebrew/api/internal/packages.<macos-codename>.jws.json`
   (here `packages.tahoe.jws.json`, 15.6 MB, plus a 14.3 MB detached
   `.payload` and a 450 KB local `.payload.index`), holding 8 586 formulae and
   7 720 casks, with `metadata.bottle_tag: "tahoe"` (§E-10). `brew info
   --json=v2 --installed` is served from it in ~5 s with no network access.
   Casks resolve from it too, even though the cask tap is not checked out
   (`Core cask tap: N/A`, §E-0). The `.payload.index` is a *locally derived*
   index; a rebuild of it writes into the cache directory but is not a network
   fetch and does not touch the prefix.

4. **The authoritative record of user intent is `installed_on_request`, and
   nothing else.** `installed_as_dependency` does **not** exist anywhere in
   Homebrew 6.0.21's `--json=v2` output (§E-2), and on disk it is present in
   only 70 of 192 install receipts — 33 written by Homebrew 4.x, 37 by 5.x,
   **none** by 6.x — with both `true` (59) and `false` (11) values. It is a
   legacy field and must not be read. `installed_on_request` is present in all
   192 receipts and in every JSON `installed[]` entry. `brew list
   --installed-on-request` (47) agrees exactly with the JSON field (47), so
   either is usable; the JSON is preferred because it is one call for the whole
   inventory. **The cask JSON carries no intent field** — a cask's `installed`
   is a bare version string, with `installed_time` but no
   `installed_on_request` (§E-11, §E-12).

   > **Corrected 2026-09-04 (R-12).** This finding originally read "Casks carry
   > no intent field **at all**", generalising from the JSON to the provider.
   > That is false: Homebrew writes `installed_on_request` into each cask's
   > `<Caskroom>/<token>/.metadata/.../INSTALL_RECEIPT.json`, and 7 of the 9 casks
   > on this machine carry one, all `true`. What §E-11 and §E-12 established —
   > and all they established — is the JSON's shape, which is what the adapter
   > reads and is re-verified on 2026-09-04 at 41 keys with no such field. The
   > overgeneralisation propagated to `AGENTS.md`, `.claude/rules/adapters.md`,
   > SE-042, SE-046 and two source comments before anyone looked on disk.

5. **`brew leaves` is an inferred top-level position and is not a substitute
   for user intent.** On this machine `brew leaves` returns 49 formulae, of
   which 44 were installed on request and 5 (`cmake httpd llvm maturin
   pkgconf`) were installed as dependencies and merely have no remaining
   dependents. Conversely `coreutils`, `curl` and `node` were explicitly
   requested by the user but are hidden from `brew leaves` because something
   else depends on them (§E-2, confirmed by `brew uses --installed`). 47
   requested ≠ 49 leaves ≠ 44 requested-leaves; three different questions.
   **Use `installed_on_request` for ownership/intent; use `brew leaves` only
   when the question really is "has no dependents".**

6. **The three artifact states are distinguishable, but not from
   `brew info --json=v2` alone on this machine.** Seven formulae (`mise`,
   `openssl@3`, `pcre2`, `php@8.4`, `pydantic`, `simdjson`, `tree-sitter`) are
   reported by `brew list --built-from-source`, have a keg with
   `poured_from_bottle: false`, and are exactly the seven formulae whose
   `bottle` object is `{}` while `outdated` is `false` (§E-5, §E-6). Reading
   the tap on disk shows all three of them publish `arm64_tahoe`,
   `arm64_sequoia`, `arm64_sonoma`, `arm64_linux` and `x86_64_linux` bottles
   and **no x86_64 macOS bottle**. So:
   - *No prebuilt artifact published at all* — not observed on this machine;
     cannot be distinguished from the next state using `brew info` alone,
     because the cached index is generated for one `bottle_tag`.
   - *An artifact exists but not for this platform* — the actual state of these
     seven, and of 857 of the 8 586 formulae in the index, which carry no
     `bottle_tag` key for `tahoe`/x86_64. In JSON this appears as
     `bottle: {}` and `versions.bottle: false`, which is **the same
     representation as "no bottle at all"**. Separating them requires either
     the tap's `.rb` (offline, if tapped) or a non-platform-filtered API query
     (network).
   - *A source build was forced* — these seven again, visible as
     `poured_from_bottle: false` on the newest keg, or via `brew list
     --built-from-source`. Here the cause is the platform gap, not a
     non-default variant: no `used_options` are set on any keg (§E-5).
   This is the Homebrew Intel Tier-3 policy (F-1) showing up as observable
   local state, and it is the strongest argument the product has.

7. **`installed[]` is ordered oldest keg first; reading `installed[0]` gives
   the wrong answer.** Taking `installed[0].poured_from_bottle` reports 180/180
   poured, contradicting `brew list --built-from-source` (7). The correct read
   is the keg whose `version` equals `linked_keg`, or an `any()` over all kegs
   (§E-5). `linked_keg` is `null` for keg-only/unlinked formulae (`php@8.4`,
   `pydantic`), so the fixture and the adapter need both rules. 12 formulae
   have more than one keg installed (§E-1).

8. **Which read commands emit JSON, and in what shape:**

   | Command | JSON | Shape / version field |
   |---|---|---|
   | `brew info --json` / `--json=v1` | yes | **array** of formula objects; formulae only |
   | `brew info --json=v2` | yes | **object** `{"casks":[…],"formulae":[…]}`; formula objects have identical keys to v1 |
   | `brew info --json=v2 --installed` | yes | same, whole inventory (180+9 here, 876 KB, ~5 s) |
   | `brew outdated --json=v2` | yes | object `{"casks":[…],"formulae":[…]}`; `{name, installed_versions[], current_version, pinned, pinned_version}` |
   | `brew list --versions --json` | yes | object `{"casks":[…],"formulae":[…]}`; `{name, versions[], linked_version, optlinked_version, pinned_version}`. Requires `--versions`, no named args, and an external `jq` |
   | `brew tap-info --json` | documented as v1-only | **not observed** — blocked (§E-9) |
   | `brew deps` (any flags) | **no** | prints usage on `--json` |
   | `brew uses` (any flags) | **no** | prints usage on `--json` |
   | `brew leaves` | **no** | newline-separated names |
   | `brew list` (without `--json`) | no | newline- or space-separated names/versions |
   | `brew config` | no | `Key: value` lines |
   | `brew --prefix/--cellar/--caskroom/--repository/--cache` | no | one path |

   There is **no schema/version field inside the JSON payload** — the version is
   selected by the `--json=vN` argument only, and the consumer must record which
   it asked for.

9. **JSON stability is explicitly weak-additive, per Homebrew's own doc.**
   "fields may be added to the schema as needed without incrementing the
   schema. Any significant breaking changes will cause a change to the schema
   version," and "The schema itself is not currently documented outside of the
   code in `formula.rb`" (§E-11, `docs/Querying-Brew.md`, `last_review_date:
   2025-02-08`). Consequence for the adapter: **parse permissively** — ignore
   unknown keys, treat every optional key as absent-able (`installed_as_
   dependency` is the live proof, Finding 4), and never rely on key order or
   on the absence of a key meaning `false`. Pin the requested version
   (`--json=v2`) explicitly rather than relying on the default, which is `v1`
   for `brew info` and (deprecated) `v1` for `brew outdated`.

10. **Cask semantics, from Homebrew's own terminology (§E-13):** a formula is a
    "package definition that builds from upstream sources"; a cask is a
    "package definition that installs pre-compiled binaries built and signed by
    upstream". A cask is not built by Homebrew, has no Cellar keg, no
    `keg_only`, no `bottle`, no `installed_on_request`, and its payload is a
    signed vendor artifact placed at a `target` path (here
    `/Applications/VisualDiffer.app`) with `zap` cleanup hints and an
    `auto_updates` flag meaning the app updates *itself* outside Homebrew.
    This **confirms SE-012 and `02-domain-model.md`**: MacPorts has no
    equivalent concept — it builds ports from source into `/opt/local` and has
    no mechanism that installs a vendor-signed `.app` bundle and then steps
    aside while the vendor self-updates it. Any cask→port mapping would be
    invented. Casks belong in inventory and `doctor` (`GUI_APP`), not in
    migration planning.

11. **Offline vs network, for `analyze`:**
    - **Offline and safe** (observed, with `HOMEBREW_NO_AUTO_UPDATE=1`):
      `brew --prefix`, `--cellar`, `--caskroom`, `--repository`, `--cache`,
      `brew config`, `brew --version`, `brew list` (all observed flags,
      including `--json`), `brew leaves` (all flags), `brew deps --installed
      [--tree]`, `brew uses --installed`, `brew outdated [--json=v2]`,
      `brew info --json=v2 --installed`, and `brew info --json=v2 <name>` **for
      a name already seen in `brew list` output**. Everything `analyze` needs
      is in this set.
    - **Network-dependent or unsafe**: `brew info <name>` for any name not
      already known to be installed (Finding 1 — clones 1.4 GB); any
      `brew info` at all if `HOMEBREW_NO_AUTO_UPDATE` is not set and the API
      cache is older than 450 s; `brew uses <name>` without `--installed`
      (tap-wide, and needs the tap); anything that answers "does a bottle exist
      for a *different* platform" (Finding 6).
    - **Machine-specific caveat:** this machine has `homebrew/core` checked out
      (§E-A), which is not the default for Homebrew 4+. A machine without it
      may resolve some of the above differently. The offline conclusion rests
      on the cached API index, which is present independently of the tap, and
      on the manual's statement about `HOMEBREW_NO_AUTO_UPDATE` — not on the
      tap being present. Treat "is the API cache present and how old is it" as
      **runtime capability detection**, not a static assumption.

---

## Cannot determine

- **`brew tap` and `brew tap-info --json` were not observed.** This
  repository's own `PreToolUse` hook blocks every `brew tap*` invocation,
  including the argument-less read-only listing form. Per `CLAUDE.md` the
  command was not rephrased to evade the guard. Tap state was obtained from
  the filesystem, `brew config` and the JSON `tap` / `tap_git_head` fields
  instead, which is sufficient for inventory but leaves `tap-info`'s JSON
  schema unverified (documented as v1-only). **UNRESOLVED** — a future attempt
  needs a human decision on whether the hook's `brew tap` pattern should
  exempt the zero-argument and `tap-info` forms, or a captured fixture.
- **Whether `HOMEBREW_NO_INSTALL_FROM_API=1` suppresses the Finding 1 tap
  clone.** Deliberately not tested: with that variable set, any cask-touching
  query would require the `homebrew/cask` tap, which is *not* checked out here
  (`Core cask tap: N/A`), so the test could itself trigger a second large
  clone into the developer's prefix — the exact side effect this note exists to
  avoid. **UNRESOLVED.** A future attempt needs a disposable Homebrew prefix
  (`HOMEBREW_PREFIX` on scratch storage) or a container, not this machine.
  The safe adapter rule in the meantime is Finding 1's: never pass an unknown
  name to `brew info`.
- **Whether `brew deps` / `brew uses` also clone the tap for unknown names.**
  Not tested, for the same reason.
- **Whether the state "no bottle published for any platform" is distinguishable
  from "no bottle for this platform" without the tap or a network call.** Not
  on this machine: the cached index is generated for a single `bottle_tag`
  (Finding 6). A future attempt would need `HOMEBREW_API_DOMAIN`'s full
  `formula.json` or the tap.
- **What rebuilt `packages.tahoe.jws.json.payload.index` at 12:38:08 UTC.**
  Not attributable to a specific command from the timeline; the controlled test
  at 12:45 showed no read command rebuilding it. It is a local derived file, so
  the offline conclusion is unaffected.
- **Behaviour on a machine without `homebrew/core` checked out**, and on
  Homebrew versions other than 6.0.21. Both are runtime-detection concerns.
- **No formula on this machine was installed with a non-default variant**
  (`used_options` is empty everywhere), so the "source build forced by a
  non-default variant" state was not observed — only the "forced by missing
  platform artifact" state.

---

## Impact on the specification

- **Confirms `02-domain-model.md`** (`GUI_APP` out of migration scope) and
  **SE-012**: Finding 10 supplies the cask semantics that entry cites R-1 for.
  SE-012's "R-1 confirms cask semantics" is now discharged with an official
  quotation.
- **Confirms `04-migration-and-safety.md`** (Process execution) and
  **`02-domain-model.md`** (artifact availability must not trigger auto-update):
  Finding 2 names the variable that does the work and cites the manual for it.
  The adapter's environment construction should set `HOMEBREW_NO_AUTO_UPDATE=1`
  and treat it as load-bearing, not cosmetic, with `HOMEBREW_NO_ANALYTICS=1`
  and `HOMEBREW_NO_ENV_HINTS=1` alongside.
- **Confirms F-1 / INV-20** with local evidence: 857 of 8 586 formulae in the
  current index have no bottle for x86_64 macOS, and seven installed formulae
  were compiled from source on this machine as a direct consequence
  (Finding 6).
- **Nothing here contradicts a current statement in `docs/spec/`.** Three items
  are *underspecified* rather than wrong, and are candidates for the main
  agent to route into `docs/spec-evolution.md`:
  1. `02-domain-model.md` should say **which** mechanism establishes ownership.
     Finding 4 and Finding 5 show `installed_on_request` and `brew leaves`
     give different answers (47 vs 49 vs 44 on one machine), and that
     `installed_as_dependency` is a trap. If the model says "top-level" without
     naming the mechanism, two implementations will disagree.
  2. The domain model needs the **three artifact states as distinct values**,
     not a boolean "bottle available". Finding 6 shows Homebrew's own JSON
     collapses two of them into `bottle: {}` on the very platform this product
     targets, so the adapter must either mark the third state "indeterminate
     offline" or be given a documented way to separate it.
  3. The `analyze`-must-work-offline requirement should point at the concrete
     command set in Finding 11, and should treat API-cache presence and age as
     **runtime capability detection** — including the degraded case where the
     cache is absent, in which case metadata is not offline-answerable at all.
- **`.claude/settings.json` / hooks:** the read-only allowlist is correct in
  intent, but (a) `brew info` on an unknown name remains slow and
  disk-consuming until Finding 1's suppressor is known, and (b) the `brew tap*`
  block currently also refuses the read-only zero-argument listing and
  `tap-info` (§E-9). That is a question for the human, not something to work
  around.

---

## Re-check when

- Homebrew's **major version** changes (this note is 6.0.21-specific in three
  places: the API cache layout `api/internal/packages.<codename>.jws.json`,
  the absence of `installed_as_dependency`, and the `--json=v2` field set).
- **Homebrew's Intel removal deadline moves.** Finding 6's numbers (857/8586,
  seven local source builds) are a snapshot of a *deteriorating* situation and
  will get worse; they are time-sensitive by nature and must be re-measured,
  not quoted, after 2027-09 (F-1 / INV-20).
- `docs/Querying-Brew.md` gains an incremented schema version, or the manual's
  `--json` wording changes.
- The two UNRESOLVED items are settled: a disposable prefix becomes available
  for the `HOMEBREW_NO_INSTALL_FROM_API` test, or the human rules on the
  `brew tap` hook pattern.
