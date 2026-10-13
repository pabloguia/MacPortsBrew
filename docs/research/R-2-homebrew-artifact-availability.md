# R-2 — How to determine, without installing, whether a Homebrew formula will pour a bottle or build from source on this machine

- **Status:** answered
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @
  `/usr/local`, MacPorts not installed
- **Blocking:** `docs/spec/02-domain-model.md` (artifact availability, and the
  requirement that determining it must not trigger a provider auto-update);
  `docs/spec/03-mapping-and-analysis.md` (the recommendation engine's "which
  provider is better here" input); `analyze` must work offline
  (`docs/spec/06-cli-and-ux.md`); the Homebrew adapter's
  `artifactAvailability(pkg)`.

## Question

For a named formula, before installing anything, decide which of these holds on
*this* machine:

- **(a)** no prebuilt artifact is published at all;
- **(b)** an artifact exists but not for this platform;
- **(c)** a source build is forced by configuration or by a non-default choice,
  even though a usable artifact exists.

Record the exact bottle-tag names present for installed formulae on this Intel
macOS 26 machine; establish how Homebrew names the tag for the current
platform; establish the effect of a non-default prefix; and separate what is
answerable offline from what needs the network.

## Evidence

Every `brew` invocation below ran with
`HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1`
in the environment, on 2026-09-02 between 12:37 and 12:52 UTC, under
`timeout 120`. No mutating subcommand was run. Nothing was installed, updated,
tapped or downloaded.

### E1 — the machine and its Homebrew, as Homebrew reports it

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

`brew config` prints `macOS: 26.6.2-x86_64` — the OS version and the arch, **not
the bottle tag**. There is no `bottle tag:` line. `brew --prefix` prints
`/usr/local`.

The local `homebrew/core` checkout exists on this machine because of the
incident recorded in `R-1-homebrew-read-interfaces.md` (a `brew info` on an
unknown name tapped it). That is what makes the `HOMEBREW_NO_INSTALL_FROM_API`
comparisons in E3 and E5 possible here without a network fetch. **A clean
machine will not have it.**

### E2 — the API cache is per-bottle-tag in Homebrew 6, and it is named `tahoe`

```
$ ls -la ~/Library/Caches/Homebrew/api/
cask_names.txt
formula_aliases.txt
formula_names.txt
internal/

$ ls -la ~/Library/Caches/Homebrew/api/internal/
   343700  executables.txt
 15567297  packages.tahoe.jws.json
 14306521  packages.tahoe.jws.json.payload
   450765  packages.tahoe.jws.json.payload.index
```

There is **no `~/Library/Caches/Homebrew/api/formula.jws.json`** on this
machine. Homebrew's own source says why:

```
$ grep -n 'internal/packages' /usr/local/Homebrew/Library/Homebrew/api/internal.rb
31:        "internal/packages.#{effective_tag}.jws.json"

$ grep -n -B 18 'internal/packages' /usr/local/Homebrew/Library/Homebrew/api/internal.rb
20-      private_class_method def self.effective_tag
21-        @effective_tag ||= T.let(SimulateSystem.current_tag, T.nilable(Utils::Bottles::Tag))
22-      end
...
30-      private_class_method def self.packages_endpoint
31:        "internal/packages.#{effective_tag}.jws.json"
```

```
$ grep -n -A 6 'HOMEBREW_USE_INTERNAL_API:' /usr/local/Homebrew/Library/Homebrew/env_config.rb
763:      HOMEBREW_USE_INTERNAL_API:                 {
764-        description: "If set, fetch formula and cask data from Homebrew's internal API. This is now the default.",
765-        boolean:     :set,
766-        replacement: "the default behaviour",
767-        odeprecated: true,
768-      },

$ grep -n 'internal API is now always used' /usr/local/Homebrew/Library/Homebrew/api.rb
221:      # The internal API is now always used; read this only to surface its deprecation.
```

### E3 — `bottle.stable.files` in the default API mode carries **one** tag, not the published set

```
$ brew info --json=v2 --installed > installed.json      # 180 formulae, 3.3 s

$ jq -r '[.formulae[] | .bottle.stable.files // {} | keys[]]
         | group_by(.) | map({t:.[0],n:length}) | sort_by(-.n)
         | .[] | "\(.n)\t\(.t)"' installed.json
137	sonoma
18	tahoe
7	all

$ jq -r '[.formulae[] | select((.bottle|length)==0) | .name] | join(" ")' installed.json
codeburn emacs firebase-cli gh git-lfs mise node openssl@3 pandoc pcre2 php@8.4
pydantic sem-cli semgrep simdjson swiftformat tree-sitter uv

$ jq -r '[.formulae[] | select(.bottle.stable.files.tahoe) | .name] | join(" ")' installed.json
autoconf cffi cmake coreutils gmp libssh2 libtool lz4 m4 mpdecimal pkgconf
python@3.12 python@3.13 python@3.14 readline sqlite xz zstd

$ jq -r '[.formulae[] | select(.bottle.stable.files.all) | .name] | join(" ")' installed.json
ca-certificates cask certifi nvm pipx pycparser sonar-scanner
```

**The exact and complete set of tag names appearing in `bottle.stable.files`
across all 180 installed formulae on this Intel macOS 26 machine is:
`sonoma`, `tahoe`, `all`.** No `arm64_*` tag appears anywhere, and no Intel tag
other than `sonoma` and `tahoe` appears.

A single entry, verbatim:

```
$ jq -c '.formulae[] | select(.name=="wget")
         | {name, tap, versions, bottle,
            installed: [.installed[] | {version, poured_from_bottle, built_as_bottle,
                                        used_options, installed_as_dependency,
                                        installed_on_request}]}' installed.json
{"name":"wget","tap":"homebrew/core","versions":{"stable":"1.25.0","head":"HEAD","bottle":true},
 "bottle":{"stable":{"rebuild":2,"root_url":"https://ghcr.io/v2/homebrew/core",
   "files":{"sonoma":{"cellar":"/usr/local/Cellar",
     "url":"https://ghcr.io/v2/homebrew/core/wget/blobs/sha256:32e76ca0ef4b26d7022dd1cad7621e8cd9bccca7677b7d4646d30d2fa009f413",
     "sha256":"32e76ca0ef4b26d7022dd1cad7621e8cd9bccca7677b7d4646d30d2fa009f413"}}}},
 "installed":[{"version":"1.25.0","poured_from_bottle":true,"built_as_bottle":true,
               "used_options":[],"installed_as_dependency":null,"installed_on_request":true}]}
```

The same five formulae read from the local `homebrew/core` checkout instead of
the API, which carries the *published* set:

```
$ HOMEBREW_NO_INSTALL_FROM_API=1 brew info --json=v2 wget gh node emacs autoconf   # 2.6 s
$ jq -c '.formulae[] | {name, tap,
                        bottle_files: ((.bottle.stable.files // {}) | keys),
                        rebuild: .bottle.stable.rebuild,
                        versions_bottle: .versions.bottle}' noapi.json
{"name":"wget",    "tap":"homebrew/core","bottle_files":["arm64_linux","arm64_sequoia","arm64_sonoma","arm64_tahoe","sonoma","x86_64_linux"],"rebuild":2,"versions_bottle":true}
{"name":"gh",      "tap":"homebrew/core","bottle_files":["arm64_linux","arm64_sequoia","arm64_sonoma","arm64_tahoe","x86_64_linux"],"rebuild":0,"versions_bottle":true}
{"name":"node",    "tap":"homebrew/core","bottle_files":["arm64_linux","arm64_sequoia","arm64_sonoma","arm64_tahoe","x86_64_linux"],"rebuild":0,"versions_bottle":true}
{"name":"emacs",   "tap":"homebrew/core","bottle_files":["arm64_linux","arm64_sequoia","arm64_sonoma","arm64_tahoe","x86_64_linux"],"rebuild":0,"versions_bottle":true}
{"name":"autoconf","tap":"homebrew/core","bottle_files":["arm64_linux","arm64_sequoia","arm64_sonoma","arm64_tahoe","sequoia","sonoma","tahoe","x86_64_linux"],"rebuild":0,"versions_bottle":true}
```

And `versions.bottle` for the same formulae **in the default API mode**:

```
$ jq -r '.formulae[] | select((.bottle|length)==0)
         | "\(.name)\ttap=\(.tap)\tversions.bottle=\(.versions.bottle)"' installed.json
codeburn	tap=homebrew/core	versions.bottle=false
emacs	tap=homebrew/core	versions.bottle=false
gh	tap=homebrew/core	versions.bottle=false
node	tap=homebrew/core	versions.bottle=false
openssl@3	tap=homebrew/core	versions.bottle=false
php@8.4	tap=homebrew/core	versions.bottle=false
…  (all 18 report versions.bottle=false)
```

`gh` reports `versions.bottle=false` under the default API mode and
`versions.bottle=true` from the tap. Same Homebrew, same formula, same minute.

The third route to the published set, over the network, is the public
per-formula API (<https://formulae.brew.sh/api/formula/gh.json>, read
2026-09-02):

```
bottle.stable.rebuild: 0
versions.bottle:       true
bottle.stable.files keys: arm64_tahoe, arm64_sequoia, arm64_sonoma,
                          arm64_linux, x86_64_linux
```

### E4 — how Homebrew names the tag for the current platform, and the older-macOS fallback

From Homebrew 6.0.21's own source on this machine.

`/usr/local/Homebrew/Library/Homebrew/extend/os/mac/utils/bottles.rb`, verbatim
(trimmed to the two relevant methods):

```ruby
def tag(tag = nil)
  if tag.nil?
    Utils::Bottles::Tag.new(system: MacOS.version.to_sym, arch: ::Hardware::CPU.arch)
  else
    super
  end
end

def find_matching_tag(tag, no_older_versions: false)
  # Used primarily by developers testing beta macOS releases.
  if no_older_versions ||
     (OS::Mac.version.prerelease? &&
       Homebrew::EnvConfig.developer? &&
       Homebrew::EnvConfig.skip_or_later_bottles?)
    super(tag)
  else
    super(tag) || find_older_compatible_tag(tag)
  end
end

# Find a bottle built for a previous version of macOS.
def find_older_compatible_tag(tag)
  tag_version = begin
    tag.to_macos_version
  rescue MacOSVersion::Error
    nil
  end

  return if tag_version.blank?

  tags.find do |candidate|
    next if candidate.standardized_arch != tag.standardized_arch

    candidate.to_macos_version <= tag_version
  rescue MacOSVersion::Error
    false
  end
end
```

`/usr/local/Homebrew/Library/Homebrew/utils/bottles.rb`, verbatim:

```ruby
def standardized_arch
  return :x86_64 if [:x86_64, :intel].include? arch
  return :arm64 if [:arm64, :arm, :aarch64].include? arch

  arch
end

def to_sym
  arch_to_symbol(standardized_arch)
end

def arch_to_symbol(arch)
  if system == :all && arch == :all
    :all
  elsif macos? && standardized_arch == :x86_64
    system
  else
    :"#{arch}_#{system}"
  end
end
```

and the generic (non-macOS-overridden) matcher:

```ruby
def find_matching_tag(tag, no_older_versions: false)
  if @tag_specs.key?(tag)
    tag
  else
    all = Tag.from_symbol(:all)
    all if @tag_specs.key?(all)
  end
end
```

### E5 — the full decision procedure, from Homebrew's source

`/usr/local/Homebrew/Library/Homebrew/formula_installer.rb` lines 258–302,
verbatim:

```ruby
sig { params(output_warning: T::Boolean).returns(T::Boolean) }
def pour_bottle?(output_warning: false)
  return false if !formula.bottle_tag? && !formula.local_bottle_path
  return true  if force_bottle?
  return false if build_from_source? || build_bottle? || interactive?
  return false if @cc
  return false unless options.empty?

  unless formula.pour_bottle?
    if output_warning && formula.pour_bottle_check_unsatisfied_reason
      opoo <<~EOS
        Building #{formula.full_name} from source:
          #{formula.pour_bottle_check_unsatisfied_reason}
      EOS
    end
    return false
  end

  return true if formula.local_bottle_path

  bottle = selected_bottle
  return false if bottle.nil?

  unless bottle.compatible_locations?
    if output_warning
      cellar = bottle.built_cellar.to_s
      prefix = Pathname(cellar).parent
      cause = if Homebrew::EnvConfig.no_relocate_build_prefix?
        "`HOMEBREW_NO_RELOCATE_BUILD_PREFIX` disables bottle build-prefix relocation."
      else
        "Your prefix `#{HOMEBREW_PREFIX}` is #{HOMEBREW_PREFIX.to_s.length} characters long, but this bottle " \
          "can only be relocated to a prefix with a maximum length of #{prefix.to_s.length} characters."
      end
      opoo <<~EOS
        Building #{formula.full_name} from source as the bottle needs:
        - `HOMEBREW_CELLAR=#{cellar}` (yours is #{HOMEBREW_CELLAR})
        - `HOMEBREW_PREFIX=#{prefix}` (yours is #{HOMEBREW_PREFIX})
        #{cause}
      EOS
    end
    return false
  end

  true
end
```

`HOMEBREW_BUILD_FROM_SOURCE` does not exist in Homebrew 6.0.21:

```
$ grep -rln 'HOMEBREW_BUILD_FROM_SOURCE' /usr/local/Homebrew/Library/Homebrew/
   (no output)
$ grep -n 'HOMEBREW_BUILD_FROM_SOURCE' /usr/local/Homebrew/manpages/brew.1
   (no output)
```

and setting it has no effect on the resolution:

```
$ HOMEBREW_BUILD_FROM_SOURCE=1 brew --cache wget autoconf
/Users/<user>/Library/Caches/Homebrew/downloads/d2d3f876…--wget--1.25.0.sonoma.bottle.2.tar.gz
/Users/<user>/Library/Caches/Homebrew/downloads/5ed6ef9a…--autoconf--2.73.tahoe.bottle.tar.gz
```

`pour_bottle?` DSL usage across the whole core tap:

```
$ cd /usr/local/Homebrew/Library/Taps/homebrew/homebrew-core
$ grep -rho 'pour_bottle? *only_if: *:[a-z_]*' Formula/ | sort | uniq -c | sort -rn
  19 pour_bottle? only_if: :clt_installed
  17 pour_bottle? only_if: :default_prefix
$ grep -rl 'pour_bottle?' Formula/ | wc -l
      36
```

Manpage (`/usr/local/Homebrew/manpages/brew.1`, Homebrew 6.0.21, read
2026-09-02; same text at <https://docs.brew.sh/Manpage>, read 2026-09-02):

```
-s, --build-from-source
    Compile formula from source even if a bottle is available.
    Dependencies will still be installed from bottles if they are available.
--build-bottle
    Prepare the formula for eventual bottling during installation,
    skipping any post-install steps.
--force-bottle
    Install from a bottle if it exists for the current or newest version of
    macOS, even if it would not normally be used for installation.
```

`docs/Bottles.md` (<https://docs.brew.sh/Bottles>, read 2026-09-02;
`last_review_date: "2026-07-18"`) lists when bottles are not used:

> - Users explicitly request source compilation via `--build-from-source`
> - The formula specifies non-use through `pour_bottle?`
> - Installation includes any specified options (bottles use default options only)
> - The bottle lacks current validity (missing or mismatched checksum)
> - The bottle's `cellar` setting is neither relocatable nor matches the current
>   `HOMEBREW_CELLAR`

and on the `all` tag:

> Use the `all:` tag for a platform-independent bottle whose contents are
> identical and usable on every supported operating system and architecture.

`docs/Installation.md` (<https://docs.brew.sh/Installation>, read 2026-09-02;
`last_review_date: "2026-08-27"`) on the prefix:

> The script installs Homebrew to its default prefix (`/opt/homebrew` for Apple
> Silicon, `/usr/local` for macOS Intel and `/home/linuxbrew/.linuxbrew` for
> Linux) … This prefix is required for most bottles (binary packages) to be
> used.

### E6 — `brew --cache` is a zero-network, zero-write probe that yields the answer directly

```
$ for f in wget gh node emacs autoconf cmake sqlite ca-certificates openssl@3 php@8.4 llvm; do
    printf '%-16s ' "$f"; brew --cache "$f" | sed 's|.*/||'
  done
wget             d2d3f876…--wget--1.25.0.sonoma.bottle.2.tar.gz
gh               2c704b49…--cli-2.99.0.tar.gz
node             1e69ba6b…--node-v26.8.1.tar.xz
emacs            9da488f9…--emacs-31.1.tar.xz
autoconf         5ed6ef9a…--autoconf--2.73.tahoe.bottle.tar.gz
cmake            fce1eaae…--cmake--4.4.3.tahoe.bottle.tar.gz
sqlite           385bbe86…--sqlite--3.53.4.tahoe.bottle.1.tar.gz
ca-certificates  d826f8f6…--ca-certificates--2026-08-13.all.bottle.1.tar.gz
openssl@3        821ebc26…--openssl-3.6.4.tar.gz
php@8.4          fa799b68…--php-8.4.25.tar.xz
llvm             480329f6…--llvm--23.1.0.sonoma.bottle.tar.gz
```

(The `…` above elides the middle of each 64-character hash; the leading and
trailing forms are verbatim.)

It downloads nothing:

```
$ ls -1 /Users/<user>/Library/Caches/Homebrew/downloads | wc -l
       0
$ ls -la /Users/<user>/Library/Caches/Homebrew/downloads/d2d3f876…--wget--1.25.0.sonoma.bottle.2.tar.gz
ls: …: No such file or directory
```

Flags:

```
$ brew --cache --build-from-source wget
/Users/<user>/Library/Caches/Homebrew/downloads/d06ba293…--wget-1.25.0.tar.gz

$ brew --cache --force-bottle gh
/Users/<user>/Library/Caches/Homebrew/downloads/2c704b49…--cli-2.99.0.tar.gz

$ brew --cache --bottle-tag=arm64_tahoe wget
Warning: Bottle for tag :arm64_tahoe is unavailable.

$ brew --cache --os=tahoe --arch=arm64 wget gh
Error: New arch must be :arm or :intel

$ brew --cache --os=tahoe --arch=arm wget gh
Warning: Bottle for tag :arm64_tahoe is unavailable.
Warning: Bottle for tag :arm64_tahoe is unavailable.
```

Manpage for it (Homebrew 6.0.21, read 2026-09-02):

```
--cache [options] [formula|cask ...]
    Display Homebrew's download cache. See also $HOMEBREW_CACHE.
    If a formula or cask is provided, display the file or directory used to cache it.
    --os                    Show cache file for the given operating system.
                            (Pass all to show cache files for all operating systems.)
    --arch                  Show cache file for the given CPU architecture.
                            (Pass all to show cache files for all architectures.)
    -s, --build-from-source Show the cache file used when building from source.
    --force-bottle          Show the cache file used when pouring a bottle.
```

### E7 — what actually happened at install time on this machine

```
$ jq -r '[.formulae[].installed[] | .poured_from_bottle]
         | group_by(.) | map({v:.[0],n:length}) | .[] | "\(.n)\t\(.v)"' installed.json
7	false
185	true

$ jq -r '.formulae[] | select(any(.installed[]; .poured_from_bottle==false))
         | "\(.name)\tbottle_files=\((.bottle.stable.files//{})|keys|join(","))\t"
           + ([.installed[] | "v\(.version) poured=\(.poured_from_bottle) built_as_bottle=\(.built_as_bottle) opts=\(.used_options|join(" ")) on_request=\(.installed_on_request)"] | join(" | "))' installed.json
mise         bottle_files=  v2026.8.12 poured=true  built_as_bottle=true  opts= on_request=true  | v2026.9.0  poured=false built_as_bottle=false opts= on_request=true
openssl@3    bottle_files=  v3.6.3     poured=true  built_as_bottle=true  opts= on_request=false | v3.6.4     poured=false built_as_bottle=false opts= on_request=false
pcre2        bottle_files=  v10.47_1   poured=true  built_as_bottle=true  opts= on_request=false | v10.48     poured=false built_as_bottle=false opts= on_request=false
php@8.4      bottle_files=  v8.4.24    poured=true  built_as_bottle=true  opts= on_request=false | v8.4.25    poured=false built_as_bottle=false opts= on_request=false
pydantic     bottle_files=  v2.13.4    poured=true  built_as_bottle=true  opts= on_request=false | v2.13.5    poured=false built_as_bottle=false opts= on_request=false
simdjson     bottle_files=  v4.6.8     poured=true  built_as_bottle=true  opts= on_request=false | v4.6.9     poured=false built_as_bottle=false opts= on_request=false
tree-sitter  bottle_files=  v0.26.13   poured=true  built_as_bottle=true  opts= on_request=false | v0.27.0    poured=false built_as_bottle=false opts= on_request=false
```

`used_options` is empty for every one of them, so none of these source builds
was caused by a non-default option.

### E8 — offline determination, and its agreement with `brew`

Reading the cached payload directly with `jq`, no `brew` involved:

```
$ P=~/Library/Caches/Homebrew/api/internal/packages.tahoe.jws.json.payload
$ jq -r 'select(.formulae) | .formulae | to_entries[]
         | select(.key|IN("wget","gh","node","emacs","autoconf","cmake",
                          "ca-certificates","openssl@3","php@8.4"))
         | "\(.key)\tbottle_tag=\(.value.bottle_tag // "NONE")\tcellar=\(.value.bottle_cellar // "-")\tver=\(.value.stable_version)"' "$P"
autoconf        bottle_tag=NONE     cellar=-               ver=2.73
ca-certificates bottle_tag=:all     cellar=-               ver=2026-08-13
cmake           bottle_tag=NONE     cellar=-               ver=4.4.3
emacs           bottle_tag=NONE     cellar=-               ver=31.1
gh              bottle_tag=NONE     cellar=-               ver=2.99.0
node            bottle_tag=NONE     cellar=-               ver=26.8.1
openssl@3       bottle_tag=NONE     cellar=-               ver=3.6.4
php@8.4         bottle_tag=NONE     cellar=-               ver=8.4.25
wget            bottle_tag=:sonoma  cellar=/usr/local/Cellar ver=1.25.0
```

A per-formula record in the internal payload has these keys (example `a2ps`):

```
{"desc":"Any-to-PostScript filter","executables":[…],"head_dependencies":[…],
 "homepage":"https://www.gnu.org/software/a2ps/","license":"GPL-3.0-or-later",
 "ruby_source_checksum":"dcc5da9a…","stable_dependencies":[…],
 "stable_checksum":"8d13915a…","stable_url_args":["https://ftpmirror.gnu.org/gnu/a2ps/a2ps-4.15.8.tar.gz"],
 "stable_version":"4.15.8","bottle_tag":":sonoma","bottle_cellar":"/usr/local/Cellar",
 "bottle_checksum":"1460a22f…"}
```

Census over the cached payload (this machine's tag = `tahoe`):

```
$ jq -r 'select(.formulae)
         | [ (.formulae|length),
             ([.formulae[]|select(has("bottle_checksum"))]|length),
             ([.formulae[]|select(has("bottle_checksum")|not)]|length) ]
         | "total=\(.[0])  with_bottle_for_this_tag=\(.[1])  no_bottle_for_this_tag=\(.[2])"' "$P"
total=8586  with_bottle_for_this_tag=7808  no_bottle_for_this_tag=778

$ jq -r 'select(.formulae) | [.formulae[] | select(has("bottle_checksum"))
         | (.bottle_tag // "<same-as-file-tag>")]
         | group_by(.) | map({t:.[0],n:length}) | sort_by(-.n) | .[] | "\(.n)\t\(.t)"' "$P"
6591	:sonoma
1123	:all
  79	<same-as-file-tag>
  10	:sequoia
   3	:ventura
   1	:big_sur
   1	:monterey
```

Independent cross-check, scanning the 8586 Ruby formula files in the local
`homebrew/core` checkout at HEAD `e045d738` with a `bottle do` / `sha256 …
<tag>:` regex (script kept at
`scratchpad/` for the pass; reproduced in "Re-check when" below):

```
total formula files: 8586
no bottle block / no sha256 tags: 5
with any Intel macOS tag: 6685
with any arm64 macOS tag:  7297
with Intel 'tahoe':          79
with 'arm64_tahoe':        7276
with ':all':               1123

per-tag counts:
7305	x86_64_linux
7279	arm64_linux
7277	arm64_sonoma
7277	arm64_sequoia
7276	arm64_tahoe
6680	sonoma
2004	ventura
1991	arm64_ventura
1351	monterey
1342	arm64_monterey
1123	all
1071	big_sur
1063	arm64_big_sur
  88	sequoia
  79	tahoe

poursable on Intel macOS 26 (tahoe):            7808 (90.9%)
poursable on Apple Silicon macOS 26 (arm64_tahoe): 8420 (98.1%)
would build from source on Intel but not on ARM:  612
```

The two methods agree exactly on 7808 and on 79.

Cache-freshness observations bearing on the network:

```
$ stat -f '%Sm %N' -t '%H:%M:%S' ~/Library/Caches/Homebrew/api/internal/packages.tahoe.jws.json
09:20:29 …/packages.tahoe.jws.json          # at 12:47:55Z, after ~15 brew invocations

$ HOMEBREW_API_DOMAIN=http://127.0.0.1:1 brew --cache wget gh
/Users/<user>/Library/Caches/Homebrew/downloads/d2d3f876…--wget--1.25.0.sonoma.bottle.2.tar.gz
/Users/<user>/Library/Caches/Homebrew/downloads/2c704b49…--cli-2.99.0.tar.gz
```

Manpage, Homebrew 6.0.21, read 2026-09-02:

```
HOMEBREW_API_AUTO_UPDATE_SECS
    Check Homebrew's API for new formulae or cask data every
    $HOMEBREW_API_AUTO_UPDATE_SECS seconds. Alternatively, disable API
    auto-update checks entirely with $HOMEBREW_NO_AUTO_UPDATE.
    Default: 450.

HOMEBREW_NO_INSTALL_FROM_API
    If set, do not install formulae and casks in homebrew/core and
    homebrew/cask taps using Homebrew's API and instead use (large, slow)
    local checkouts of these repositories.

HOMEBREW_API_DOMAIN
    Use this URL as the download mirror for Homebrew JSON API. If metadata
    files at that URL are temporarily unavailable, the default API domain will
    be used as a fallback mirror.  Default: https://formulae.brew.sh/api.

HOMEBREW_BOTTLE_DOMAIN
    Use this URL as the download mirror for bottles and their manifests. …
    Default: https://ghcr.io/v2/homebrew/core.
```

## Findings

1. **The bottle tag for this machine is `tahoe`** — the bare macOS codename with
   no arch prefix. The rule, from Homebrew's own source (E4), is: the tag is
   `Tag.new(system: MacOS.version.to_sym, arch: Hardware::CPU.arch)`, rendered
   by `arch_to_symbol` as **the bare `system` name when `standardized_arch ==
   :x86_64` on macOS**, and as `"#{arch}_#{system}"` otherwise. So Intel macOS
   26 is `tahoe`; Apple Silicon macOS 26 is `arm64_tahoe`. `:all` is the
   platform-independent tag.

2. **There is no `brew` subcommand that prints the tag.** `brew config` prints
   `macOS: 26.6.2-x86_64` (E1), which is the inputs to the rule, not the tag.
   The tag is observable **without running Ruby** from the API cache filename:
   `~/Library/Caches/Homebrew/api/internal/packages.<tag>.jws.json` (E2), built
   from `effective_tag` in `api/internal.rb`. Detecting the tag by parsing that
   filename is the recommended runtime capability detection; deriving it from
   `sw_vers` plus `uname -m` is the fallback and needs a macOS-version →
   codename table that must be maintained.

3. **The exact tag names present in `bottle.stable.files` for the 180 formulae
   installed on this machine are `sonoma` (137), `tahoe` (18) and `all` (7),
   with 18 formulae carrying no bottle block at all** (E3). There are **no
   `arm64_*` tags** — but that is *not* because they do not exist upstream. It
   is because Homebrew 6 fetches a per-tag API document and
   `bottle.stable.files` in that mode contains **only the one tag selected for
   this machine**. This is the single most important trap in this note:
   `bottle.stable.files` under the default configuration is a *resolution
   result*, not a *publication record*.

4. **Yes, Intel tags exist on this machine and they are `sonoma` and `tahoe`.**
   137 of 180 installed formulae resolve to the Intel `sonoma` bottle even
   though this is macOS 26, because of Homebrew's documented-in-source
   older-macOS fallback: `find_older_compatible_tag` accepts any candidate with
   the same `standardized_arch` and `to_macos_version <= tag_version` (E4).
   A `sonoma` (Intel macOS 14) bottle is therefore poured on Intel macOS 26.
   18 formulae have a genuine Intel `tahoe` bottle.

5. **The three states, and how to tell them apart:**

   | State | Definitive test | Offline? |
   |---|---|---|
   | (c) source build forced | `--build-from-source`/`-s`, `--build-bottle`, `--interactive`, `--cc=…`, any `--with-*`/`--without-*` option, or `--HEAD` is passed; or the formula's own `pour_bottle? only_if:` is unsatisfied; or `bottle.compatible_locations?` is false (prefix/Cellar) | yes |
   | (b) artifact exists, not for this platform | the published tag set (from the tap, or `formulae.brew.sh/api/formula/<n>.json`) is non-empty but contains no tag matching this machine's tag, no older same-arch macOS tag, and no `all` | **no** — needs the tap or the network |
   | (a) nothing published at all | the published tag set is empty, i.e. the formula has no `bottle do` block | **no** — needs the tap or the network |

   The default-mode API **cannot** distinguish (a) from (b). `gh` reports
   `versions.bottle=false` and an empty `bottle.stable.files` from the API, and
   `versions.bottle=true` with five published tags from the tap and from the
   public per-formula API, in the same minute (E3). Only 5 of 8586 core
   formulae are genuinely state (a) (E8); 773 of the 778 with no bottle for
   this tag are state (b).

6. **`HOMEBREW_BUILD_FROM_SOURCE` does not exist.** It is absent from the entire
   `Library/Homebrew` tree and from the 6.0.21 manpage, and setting it changes
   nothing (E5). Any specification or code that treats it as the "forced source
   build" switch is wrong. The real switches are the per-invocation flags
   `-s`/`--build-from-source`, `--build-bottle`, `--interactive`, `--cc`, and
   any non-empty `options`, exactly as enumerated by `FormulaInstaller#pour_bottle?`
   (E5) and by `docs/Bottles.md` (E5).

7. **`HOMEBREW_NO_INSTALL_FROM_API` is not a source-build switch either.** It
   changes *where metadata comes from* (API vs a local tap checkout), which is
   why it widens `bottle.stable.files` to the published set (E3). It does not
   force a source build. It does, however, require a ~1.4 GB `homebrew/core`
   checkout on the machine (`R-1`), so an adapter must not set it opportunistically.

8. **Prefix effect on bottle eligibility is two distinct mechanisms, and neither
   is "the prefix must be the default".**
   - `bottle.compatible_locations?` (E5): the bottle's built `cellar` must be
     relocatable, or the prefix must be short enough for relocation. The
     error text Homebrew itself prints is about *prefix length*, not identity.
     `:any` and `:any_skip_relocation` cellars relocate freely.
   - `pour_bottle? only_if: :default_prefix` in the formula: a hard,
     formula-authored requirement. **17 of 8586 core formulae use it**, plus 19
     using `only_if: :clt_installed` (E5).
   `docs/Installation.md` says the default prefix "is required for most bottles
   (binary packages) to be used" (E5); the source shows the mechanism is
   relocation compatibility, not prefix equality. This machine is at
   `/usr/local`, the documented default prefix for macOS Intel, so no
   prefix-driven source build occurs here.

9. **`installed[].poured_from_bottle` is the record of what actually happened**,
   per installed keg, and it is per *version*: 185 of 192 installed kegs were
   poured; 7 formulae have an older poured keg and a newer source-built keg,
   with `used_options` empty in every case (E7). That is the Intel bottle
   erosion visible inside one Cellar. `built_as_bottle` tracks alongside it.
   `installed_on_request` in the same record is Homebrew's own record of *user
   intent*; it is the field to use for that purpose rather than inferring
   top-level packages from the dependency graph, but it is an R-1/R-3 concern
   and is only reported here incidentally.

10. **`brew --cache <formula>` is the best read-only probe of the decision.** It
    downloads nothing (the downloads directory stayed empty, E6), writes
    nothing, and returns a filename that names the outcome: a bottle path is
    `<sha>--<name>--<version>.<tag>.bottle[.<rebuild>].tar.gz`, a source path is
    `<sha>--<upstream filename>`. It honours `--build-from-source`,
    `--force-bottle`, `--bottle-tag`, `--os` and `--arch`, so the same command
    answers "and what would happen on Apple Silicon?". Note `--arch` takes
    `arm`/`intel`, not `arm64`/`x86_64` (E6), and that in default API mode a
    cross-platform query returns `Warning: Bottle for tag :arm64_tahoe is
    unavailable.` because the cached document only carries this machine's tag —
    cross-platform queries need `HOMEBREW_NO_INSTALL_FROM_API=1` or the network.
    `brew --cache` reflects `--build-from-source` but **not** the formula's own
    `pour_bottle? only_if:` predicate, so it is a probe of tags and flags, not
    the full `pour_bottle?`.

11. **Offline vs network.**
    - **Offline, no network, no `brew` process:** this machine's tag (from the
      cache filename); whether a bottle will be used for a given formula and
      which tag was selected (`bottle_tag`, `bottle_checksum`, `bottle_cellar`
      in `packages.<tag>.jws.json.payload`); the version; the dependency lists.
      This is enough for `analyze` and it agrees exactly with `brew` (E8: the
      `jq` census and the tap scan both give 7808/79).
    - **Offline but needs `brew`:** `brew --cache`, `brew info --json=v2
      --installed`, `brew config` — all of which read the same cache and
      completed in ~1–3 s with `HOMEBREW_API_DOMAIN` pointed at a dead address
      (E6, E8).
    - **Needs the network (or a local tap):** the *published* tag set, hence
      distinguishing (a) from (b); and refreshing the cache once it is older
      than `HOMEBREW_API_AUTO_UPDATE_SECS` (default 450 s).
    - **`HOMEBREW_NO_AUTO_UPDATE=1` disables the API auto-update check
      entirely**, per the manpage's own cross-reference and confirmed by
      observation: the cached `packages.tahoe.jws.json` mtime stayed at
      09:20:29 across ~15 `brew` invocations up to 12:47:55Z, more than 3 hours
      and far beyond the 450 s default (E8). **This is the variable that makes
      `analyze` offline-safe**, together with `HOMEBREW_NO_ANALYTICS=1` and
      `HOMEBREW_NO_ENV_HINTS=1`. Recording it here as `R-1` asked.

12. **Semantics of `bottle_tag` in the internal payload:** present means a
    *fallback* tag was selected; **absent while `bottle_checksum` is present**
    means the bottle is for the file's own tag. On this machine: 79 formulae
    with no `bottle_tag` (native `tahoe`), 6591 `:sonoma`, 1123 `:all`,
    10 `:sequoia`, 3 `:ventura`, 1 `:monterey`, 1 `:big_sur` — 7808 total, and
    778 with no `bottle_checksum` at all (E8). An adapter must handle the
    absent-key case or it will misclassify the 79 best-supported formulae as
    source builds.

## Cannot determine

- **Whether the offline path survives a *stale* cache.** The forced-offline test
  (E8) ran against a cache that was fresh in Homebrew's own terms with
  `HOMEBREW_NO_AUTO_UPDATE=1` set, so no fetch was attempted. What happens when
  the cache is genuinely stale *and* the network is down — a hard error, a
  warning plus cached data, or a silent fallback — is not established. A future
  attempt needs either a machine with a >450 s-stale cache and
  `HOMEBREW_NO_AUTO_UPDATE` unset while the network is severed, or a reading of
  `Homebrew::API.cached_internal_packages_payload` (`api.rb:436`) with
  `stale_seconds:` traced through. **UNRESOLVED**, and load-bearing for the
  offline guarantee.
- **The signature status of the `.payload` sidecar.** `api.rb:429` mentions the
  payload being "served entirely" without the usual validation path. Whether
  MacPortsBrew may read `packages.<tag>.jws.json.payload` directly, or must
  verify the JWS in `packages.<tag>.jws.json`, is a security question this note
  does not answer. It should go to `security-reviewer` before the adapter reads
  the sidecar. **UNRESOLVED.**
- **Stability of the internal API's shape.** `api/internal.rb` and the
  `packages.<tag>.jws.json` layout are explicitly *internal*; the file name
  containing `internal/` is Homebrew telling us so. There is no documented
  compatibility promise. Nothing official was found stating a schema.
- **Whether the 79 Intel `tahoe` bottles are a maintained subset.** See
  `F-1-homebrew-support-tiers.md`, "Cannot determine".
- **Casks.** This note is entirely about formulae. `brew info --json=v2`
  reports casks in a separate array with a different artifact model; not
  examined.

## Impact on the specification

- **Confirms `docs/spec/02-domain-model.md`**: artifact availability is a real,
  computable, three-state property, and it must not trigger an auto-update. The
  quiet environment (`HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`,
  `HOMEBREW_NO_ENV_HINTS=1`) is confirmed sufficient for the read paths used
  here.
- **Confirms the offline requirement is achievable for `analyze`** — but only
  for the "will this pour?" question. It is **not** achievable for the
  "(a) versus (b)" distinction, which needs the tap or the network. If
  `docs/spec/` states or implies that the full availability model is offline,
  that statement needs qualifying. The domain model should carry a
  fourth value alongside (a)/(b)/(c) — something like
  `PublishedElsewhereUnknown` — for the offline case, rather than collapsing to
  "no artifact".
- **Contradicts nothing found in `docs/spec/` about `HOMEBREW_BUILD_FROM_SOURCE`**
  (no occurrence was found), but the *research prompt* for this item named it as
  a candidate mechanism, so recording Finding 6 loudly: it does not exist in
  Homebrew 6.0.21.
- **Refines the "non-default prefix" statement.** Any specification text saying
  "bottles require the default prefix" should be softened to Homebrew's actual
  mechanism (Finding 8): relocation compatibility plus 17 formulae with an
  explicit `only_if: :default_prefix`. `docs/Installation.md`'s own wording is
  "required for **most** bottles".
- **New input for `docs/spec/03-mapping-and-analysis.md`**: the recommendation
  engine now has a concrete, cheap, offline signal per package — poured vs
  source, and which tag, and whether it is a fallback from an older macOS. The
  headline number for an Intel macOS 26 machine is 90.9% coverage against 98.1%
  on Apple Silicon, and 612 formulae that pour on ARM and compile on Intel.
  That comparison is the product's core argument and it is computable.
- **Bears on `docs/spec/09-research-checklist.md` F-1** — see the separate note;
  the "stopped building new bottles for Intel" wording needs attribution.
- Nothing here contradicts an invariant. **`docs/spec/` not edited from this
  note.**

## Re-check when

- **Homebrew's major or minor version changes.** The internal per-tag API
  (`packages.<tag>.jws.json`) is new relative to the flat `formula.jws.json`
  that the research prompt expected, and it is explicitly internal. Everything
  in Findings 2, 3, 11 and 12 is version-coupled to **Homebrew 6.0.21**.
- **A new macOS release.** The tag changes (`tahoe` → the macOS 27 codename)
  and the fallback set grows. Re-read `extend/os/mac/utils/bottles.rb`.
- **Any time the Intel coverage numbers matter for a release.** Re-run the
  census. The offline half needs only `jq` and the cache:

  ```
  P=~/Library/Caches/Homebrew/api/internal/packages.<tag>.jws.json.payload
  jq -r 'select(.formulae) | [ (.formulae|length),
           ([.formulae[]|select(has("bottle_checksum"))]|length) ]
         | "total=\(.[0]) poursable=\(.[1])"' "$P"
  ```

  The tap-scan half needs a `homebrew/core` checkout and the regex
  `^\s*sha256\s+(?:cellar:\s*\S+,\s*)?([a-z0-9_]+):\s*"` applied inside each
  file's `bottle do … end` block; Intel macOS tags are the bare codenames,
  arm64 tags carry the `arm64_` prefix, `all` is platform-independent, and
  `x86_64_linux`/`arm64_linux` are Linux.
- **Before the adapter reads `packages.<tag>.jws.json.payload` directly** —
  resolve the two UNRESOLVED items above first.
