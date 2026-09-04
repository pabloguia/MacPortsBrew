# R-12 — Can a Homebrew cask be migrated to MacPorts, and can a cask's formula dependency be satisfied by MacPorts instead?

- **Status:** partially answered — the Homebrew half is observed on this
  machine; the MacPorts half is **cited only** (MacPorts is not installed
  here); five items marked `UNRESOLVED`
- **Answered on:** 2026-09-04
- **Machine:** macOS 26.6.2 (Tahoe, build 25G83), x86_64, Homebrew 6.0.21 @
  `/usr/local` (`homebrew/core` git revision `e045d7385fd`, last commit
  2026-09-02), MacPorts **not installed** (`/opt/local` absent, `port` not on
  `PATH`)
- **Blocking:** DP-012 / SE-046 (do casks belong in the default top-level
  view, and is their ownership `UNKNOWN`); `PackageClass::GuiApp` and
  `PackageClass::ThirdPartySourced` in `crates/domain/src/class.rs`;
  `PackageKind::GuiApp` in `crates/domain/src/package.rs`; INV-01's
  verification step; INV-07 and INV-08.

## Question

Two questions, in the owner's order.

**Q1 — Can a Homebrew cask be migrated to MacPorts at all?** Not "should it",
but "is there a destination". What does a cask install; does MacPorts ship GUI
applications and how do they differ in origin, location and recorded metadata;
how many of the casks *actually installed here* have a plausible MacPorts
equivalent; and would such a "migration" even be the operation the planner
performs for a formula, given that the source artifact is a vendor binary that
may self-update.

**Q2 — Can a Homebrew cask consume MacPorts components?** Where a cask declares
`depends_on formula:`, could that dependency be satisfied by the MacPorts
equivalent, leaving the cask installed but its dependency provided by the other
side? By what mechanism does the cask's payload find that dependency at run
time — `PATH`, an absolute path baked into a Mach-O, a linked dylib, a bundled
copy — and what would break.

## Evidence

### E-0 — Environment, and what could not be observed

Every `brew` invocation below ran with exactly these three variables in the
environment, on 2026-09-04 between 22:30 and 23:10 UTC (19:30–20:10 local,
`-03`):

```
$ env | grep -i homebrew | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

`HOMEBREW_NO_INSTALL_FROM_API` was unset throughout (default API mode). No
mutating subcommand was run; nothing was installed, updated, tapped or
downloaded. Per R-1, this variable set does **not** suppress the
`homebrew/core` clone that `brew info` on an *unknown* name performs, so every
`brew info` here was given only names produced by `brew list`, or no names at
all (`--installed`). The API cache mtime stayed at its 2026-09-02 09:20 value
throughout, as R-2 finding 11 predicts.

The machine and the two providers:

```
$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83
$ uname -m
x86_64
$ brew --version
Homebrew 6.0.21
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)
$ brew --prefix
/usr/local
$ command -v port || echo "port NOT on PATH"
port NOT on PATH
$ ls -d /opt/local
ls: /opt/local: No such file or directory
```

**MacPorts is not installed on this machine.** Every MacPorts claim in this
note is therefore *cited*, from `guide.macports.org`, `ports.macports.org`,
`github.com/macports/macports-base` or `github.com/macports/macports-ports`,
with the date read (2026-09-04 in every case), and is marked **unobserved**.
No `port` command was run, because none could be.

Network reads used `WebFetch` and `gh api`; `curl` is denied by this
repository's permission policy, which is recorded here because it constrains
how a future attempt reproduces the MacPorts half.

### E-1 — The nine casks installed here

```
$ brew list --cask
adoptopenjdk
copilot-cli
gcloud-cli
gimp
meld
oracle-jdk
visualdiffer
wailbrew
wireshark-chmodbpf

$ brew list --cask | wc -l
       9
```

```
$ brew info --json=v2 --installed > installed.json          # 3.6 s
$ jq -r '.casks[] | "\(.token)\t\(.tap)\t\(.name[0] // "-")"' installed.json
adoptopenjdk	null	-
copilot-cli	homebrew/cask	GitHub Copilot CLI
gcloud-cli	homebrew/cask	Google Cloud CLI
gimp	homebrew/cask	GIMP
meld	homebrew/cask	Meld for macOS
oracle-jdk	homebrew/cask	Oracle Java Standard Edition Development Kit
visualdiffer	homebrew/cask	VisualDiffer
wailbrew	homebrew/cask	WailBrew
wireshark-chmodbpf	homebrew/cask	Wireshark-ChmodBPF
```

```
$ jq -r '.casks[] | "\(.token)\tinstalled=\(.installed)\tlatest=\(.version)\tauto_updates=\(.auto_updates)\toutdated=\(.outdated)"' installed.json
adoptopenjdk	installed=16.0.1,9	latest=16.0.1,9	auto_updates=null	outdated=false
copilot-cli	installed=1.0.70	latest=1.0.82	auto_updates=true	outdated=true
gcloud-cli	installed=581.0.0	latest=583.0.0	auto_updates=true	outdated=true
gimp	installed=3.2.4	latest=3.2.4	auto_updates=null	outdated=false
meld	installed=3.22.3+105	latest=3.22.3+105	auto_updates=null	outdated=false
oracle-jdk	installed=26.0.2.1	latest=26.0.2.1	auto_updates=null	outdated=false
visualdiffer	installed=2.1.21	latest=2.1.22	auto_updates=true	outdated=true
wailbrew	installed=0.10.4	latest=0.11.0	auto_updates=null	outdated=true
wireshark-chmodbpf	installed=4.6.8	latest=4.6.8	auto_updates=null	outdated=false
```

Download URLs, verbatim:

```
$ jq -r '.casks[] | "\(.token)\t\(.url)"' installed.json
adoptopenjdk
copilot-cli	https://github.com/github/copilot-cli/releases/download/v1.0.82/copilot-darwin-x64.tar.gz
gcloud-cli	https://dl.google.com/dl/cloudsdk/channels/rapid/downloads/google-cloud-cli-583.0.0-darwin-x86_64.tar.gz
gimp	https://download.gimp.org/gimp/v3.2/macos/gimp-3.2.4-x86_64.dmg
meld	https://gitlab.com/api/v4/projects/51547804/packages/generic/meld_macos/3.22.3+105/Meld-3.22.3+105_x86_64.dmg
oracle-jdk	https://download.oracle.com/java/26/archive/jdk-26.0.2.1_macos-x64_bin.dmg
visualdiffer	https://github.com/visualdiffer/visualdiffer/releases/download/v2.1.22/VisualDiffer-2.1.22.zip
wailbrew	https://github.com/wickenico/WailBrew/releases/download/v0.11.0/wailbrew-v0.11.0.zip
wireshark-chmodbpf	https://www.wireshark.org/download/osx/all-versions/Wireshark%204.6.8.dmg
```

(`adoptopenjdk`'s `url` field is empty — see E-7.) Trailing whitespace has been
stripped from every quoted block in this note; where a line shows only a label,
the command produced an empty field or no output for it.

Every one is a vendor host: `github.com/<vendor>`, `dl.google.com`,
`download.gimp.org`, `gitlab.com`, `download.oracle.com`, `wireshark.org`. None
is a Homebrew-controlled build.

### E-2 — The cask DSL's artifact vocabulary, from Homebrew 6.0.21's own source

```
$ ls /usr/local/Homebrew/Library/Homebrew/cask/artifact/
abstract_artifact.rb  abstract_flight_block.rb  abstract_uninstall.rb  app.rb
appimage.rb  artifact.rb  audio_unit_plugin.rb  bashcompletion.rb  binary.rb
colorpicker.rb  command_wrapper.rb  dictionary.rb  fishcompletion.rb  font.rb
generated_completion.rb  generated_script.rb  input_method.rb  install_steps.rb
installer.rb  internet_plugin.rb  keyboard_layout.rb  manpage.rb  mdimporter.rb
moved.rb  pkg.rb  postflight_block.rb  preflight_block.rb  prefpane.rb
qlplugin.rb  relocated.rb  screen_saver.rb  service.rb  shellcompletion.rb
stage_only.rb  suite.rb  symlinked.rb  uninstall.rb  vst3_plugin.rb
vst_plugin.rb  zap.rb  zshcompletion.rb
```

`/usr/local/Homebrew/Library/Homebrew/cask/dsl.rb` lines 39–95, verbatim:

```ruby
    ORDINARY_ARTIFACT_CLASSES = [
      Artifact::Installer,
      Artifact::App,
      Artifact::AppImage,
      Artifact::Artifact,
      Artifact::AudioUnitPlugin,
      Artifact::Binary,
      Artifact::CommandWrapper,
      Artifact::Colorpicker,
      Artifact::Dictionary,
      Artifact::Font,
      Artifact::GeneratedScript,
      Artifact::InputMethod,
      Artifact::InternetPlugin,
      Artifact::KeyboardLayout,
      Artifact::Manpage,
      Artifact::Pkg,
      Artifact::Prefpane,
      Artifact::Qlplugin,
      Artifact::Mdimporter,
      Artifact::ScreenSaver,
      Artifact::Service,
      Artifact::StageOnly,
      Artifact::Suite,
      Artifact::VstPlugin,
      Artifact::Vst3Plugin,
      Artifact::ZshCompletion,
      Artifact::FishCompletion,
      Artifact::BashCompletion,
      Artifact::GeneratedCompletion,
      Artifact::Uninstall,
      Artifact::Zap,
    ].freeze
    ...
    ARTIFACT_BLOCK_CLASSES = [
      Artifact::PreflightBlock,
      Artifact::PostflightBlock,
    ].freeze

    INSTALL_STEP_ARTIFACT_CLASSES = [
      Artifact::PreflightSteps,
      Artifact::PostflightSteps,
      Artifact::UninstallPreflightSteps,
      Artifact::UninstallPostflightSteps,
    ].freeze
```

`/usr/local/Homebrew/docs/Cask-Cookbook.md`, lines 144–150 (the same text is at
<https://docs.brew.sh/Cask-Cookbook>, read 2026-09-04):

> Each cask must declare one or more artifacts (i.e. something to install).
> Homebrew determines a cask's supported operating systems from its `depends_on`
> declarations, not from its artifact types.
> Artifact stanzas can be OS-specific or portable: `app_image` is Linux-only,
> `app` and `pkg` are macOS-only, and `binary` works on either operating system.

and lines 183–192:

> Homebrew treats cask installation artifacts as trusted vendor installation
> actions once the cask has been accepted. Artifact stanzas such as `app`, `pkg`
> and `installer script` are expected to install software and may write outside
> the Caskroom through Homebrew-managed moves, macOS installer services or
> vendor installer code.
> …
> `installer script:` is not sandboxed. Many installer scripts are vendor
> installers that require broad filesystem writes, macOS services or `sudo` …
> `pkg` artifacts are not run in the cask sandbox either. They are installed by
> macOS `/usr/sbin/installer`, which applies package payloads, scripts and
> receipts according to the package metadata.

### E-3 — Artifact census: this machine, and the whole cask catalogue

On this machine, one line per cask, the stanza keys as `brew info --json=v2`
reports them:

```
$ jq -r '.casks[] | .token as $t
         | (.artifacts // [] | map(if type=="object" then (keys|join("+")) else "STRING:"+tostring end) | join(", ")) as $a
         | "\($t)\t[\($a)]"' installed.json
adoptopenjdk	[uninstall, zap]
copilot-cli	[binary+target, generate_completions_from_executable, zap]
gcloud-cli	[preflight_steps, uninstall, installer, binary+target, binary+target, binary+target, binary+target, binary+target, bash_completion+target, target+zsh_completion, postflight_steps, zap]
gimp	[app+target, command_wrapper+target, zap]
meld	[app+target, command_wrapper+target, zap]
oracle-jdk	[uninstall, pkg]
visualdiffer	[app+target, zap]
wailbrew	[app+target, zap]
wireshark-chmodbpf	[uninstall, pkg]
```

**Payload kinds among the nine: `app` 4 (gimp, meld, visualdiffer, wailbrew),
`pkg` 2 (oracle-jdk, wireshark-chmodbpf), `binary` 2 (copilot-cli, gcloud-cli),
`installer script` 1 (gcloud-cli), and 1 with no payload stanza at all
(adoptopenjdk — see E-7).**

Across the whole catalogue, from the offline API cache (7720 cask records,
cache written 2026-09-02 09:20). Counting *casks*, not stanza occurrences:

```
$ P=~/Library/Caches/Homebrew/api/internal/packages.tahoe.jws.json.payload
$ jq -r 'select(.casks) | [.casks[] | [.raw_artifacts // [] | .[]
         | if type=="array" then .[0] else tostring end] | unique | .[]]
         | group_by(.) | map({k:.[0],n:length}) | sort_by(-.n) | .[] | "\(.n)\t\(.k)"' "$P"
4667	:zap
4139	:app
2582	:font
1669	:uninstall
658	:pkg
360	:binary
97	:installer
66	:command_wrapper
64	:postflight_steps
38	:artifact
36	:suite
34	:qlplugin
32	:preflight_steps
29	:uninstall_preflight_steps
19	:manpage
18	:uninstall_postflight_steps
14	:screen_saver
13	:bash_completion
12	:zsh_completion
11	:generate_completions_from_executable
10	:keyboard_layout
9	:fish_completion
8	:prefpane
4	:dictionary
4	:generated_script
3	:input_method
3	:service
2	:colorpicker
1	:app_image
1	:audio_unit_plugin
1	:mdimporter
1	:vst3_plugin
1	:vst_plugin

$ jq -r 'select(.casks)|.casks|length' "$P"
7720
```

And how many payload kinds a single cask carries:

```
$ jq -r 'select(.casks) |
   def kinds: [.raw_artifacts // [] | .[] | if type=="array" then .[0] else tostring end] | unique;
   def payload: kinds | map(select(. as $k | [":app",":font",":pkg",":binary",":installer",
     ":suite",":app_image",":artifact",":qlplugin",":prefpane",":screen_saver",":keyboard_layout",
     ":dictionary",":input_method",":colorpicker",":vst_plugin",":vst3_plugin",":audio_unit_plugin",
     ":mdimporter",":manpage",":generated_script",":command_wrapper",":stage_only"] | index($k)));
   [ (.casks|length),
     ([.casks[]|select((payload|length)==0)]|length),
     ([.casks[]|select((payload|length)==1)]|length),
     ([.casks[]|select((payload|length)>1)]|length) ]
   | "total=\(.[0]) no_payload_stanza=\(.[1]) exactly_one=\(.[2]) more_than_one=\(.[3])"' "$P"
total=7720 no_payload_stanza=5 exactly_one=7374 more_than_one=341
```

Cask-Cookbook on where `app` and `suite` land:

> | `app` | yes | Relative path to an `.app` that should be moved into the
> `/Applications` folder on installation. |
> | `suite` | yes | Relative path to a containing directory that should be moved
> into the `/Applications` folder on installation. |

and the per-install target directories Homebrew actually recorded here:

```
$ cat /usr/local/Caskroom/gimp/.metadata/config.json
{"default":{"languages":["en-US","pt-BR"],"appdir":"/Applications",
"keyboard_layoutdir":"/Library/Keyboard Layouts",
"colorpickerdir":"/Users/pirado/Library/ColorPickers",
"prefpanedir":"/Users/pirado/Library/PreferencePanes",
"qlplugindir":"/Users/pirado/Library/QuickLook",
"mdimporterdir":"/Users/pirado/Library/Spotlight",
"dictionarydir":"/Users/pirado/Library/Dictionaries",
"fontdir":"/Users/pirado/Library/Fonts",
"servicedir":"/Users/pirado/Library/Services",
"input_methoddir":"/Users/pirado/Library/Input Methods",
"internet_plugindir":"/Users/pirado/Library/Internet Plug-Ins",
"audio_unit_plugindir":"/Users/pirado/Library/Audio/Plug-Ins/Components",
"vst_plugindir":"/Users/pirado/Library/Audio/Plug-Ins/VST",
"vst3_plugindir":"/Users/pirado/Library/Audio/Plug-Ins/VST3",
"screen_saverdir":"/Users/pirado/Library/Screen Savers"},"env":{},"explicit":{}}
```

Not one of those defaults is inside `HOMEBREW_PREFIX`.

### E-4 — Where the nine payloads actually landed

```
$ ls -la /usr/local/Caskroom/gimp/*/
lrwxr-xr-x  GIMP.app -> /Applications/GIMP.app
-rwxr-xr-x  gimp.wrapper.sh
$ ls -la /usr/local/Caskroom/oracle-jdk/*/
-rw-r--r--  216150527  JDK 26.0.2.1.pkg
$ ls -la /usr/local/Caskroom/copilot-cli/*/
-rwxr-xr-x  149461856  copilot
$ ls -la /usr/local/Caskroom/gcloud-cli/581.0.0/
lrwxr-xr-x  google-cloud-sdk -> /usr/local/share/google-cloud-sdk
```

The pkg casks:

```
$ ls -la /Library/Java/JavaVirtualMachines/
drwxr-xr-x  adoptopenjdk-16.jdk
drwxr-xr-x  jdk-26.jdk
drwxr-xr-x  microsoft-11.jdk
drwxr-xr-x  temurin-8.jdk

$ pkgutil --pkgs | grep -iE "jdk|java|wireshark"
net.adoptopenjdk.16.jdk
com.oracle.jdk-26.0.2.1
org.wireshark.ChmodBPF.pkg
com.microsoft.11.jdk
net.temurin.8.jdk

$ defaults read /Library/LaunchDaemons/org.wireshark.ChmodBPF.plist
{
    AssociatedBundleIdentifiers = "org.wireshark.Wireshark";
    Label = "org.wireshark.ChmodBPF";
    Program = "/Library/Application Support/Wireshark/ChmodBPF/ChmodBPF";
    RunAtLoad = 1;
}

$ dscl . -read /Groups/access_bpf GroupMembership
GroupMembership: pirado
```

The uninstall stanzas that are the only record Homebrew keeps of those writes:

```
$ jq -r '.casks[]|select(.token|IN("oracle-jdk","adoptopenjdk","wireshark-chmodbpf"))
         |"\(.token): \(.artifacts|tostring)"' installed.json
adoptopenjdk: [{"uninstall":[{"pkgutil":"net.adoptopenjdk.16.jdk"}]},{"zap":[…]}]
oracle-jdk: [{"uninstall":[{"pkgutil":"com.oracle.jdk-26.0.2.1"}]},{"pkg":["JDK 26.0.2.1.pkg"]}]
wireshark-chmodbpf: [{"uninstall":[{"early_script":{"executable":"/usr/sbin/installer",
  "args":["-pkg","/usr/local/Caskroom/wireshark-chmodbpf/4.6.8/Uninstall ChmodBPF.pkg",
  "-target","/"],"sudo":true,"must_succeed":false},
  "pkgutil":"org.wireshark.ChmodBPF.pkg"}]},{"pkg":["Install ChmodBPF.pkg"]}]
```

Cask-Cookbook, on why the uninstall stanza has to exist:

> For most casks, uninstall actions are determined automatically, and an
> explicit `uninstall` stanza is not needed. However, a cask which uses the
> `pkg` or `installer` stanzas will **not** know how to uninstall correctly
> unless an `uninstall` stanza is given.

### E-5 — What Homebrew records about a cask, including an intent field the project's notes say does not exist

The keys of a cask record in `brew info --json=v2`:

```
$ jq -r '.casks[0]|keys|join("\n")' installed.json
artifacts  auto_updates  autobump  bundle_short_version  bundle_version
caveats  caveats_rosetta  conflicts_with  container  depends_on
deprecate_args  deprecated  deprecation_date  deprecation_reason
deprecation_replacement_cask  deprecation_replacement_formula  desc
disable_args  disable_date  disable_reason  disable_replacement_cask
disable_replacement_formula  disabled  full_token  homepage  installed
installed_time  languages  name  no_autobump_message  old_tokens  outdated
pinned  pinned_version  rename  ruby_source_checksum  ruby_source_path  sha256
skip_livecheck  tap  tap_git_head  token  url  url_specs  version

$ jq -r '[.casks[]|keys]|add|unique|map(select(test("request|intent";"i")))|join(", ")' installed.json
   (empty)
```

**No intent field in the JSON.** But Homebrew 6.0.21 writes one to disk.
`/usr/local/Homebrew/Library/Homebrew/cask/tab.rb` lines 105–120, verbatim:

```ruby
    def to_json(*_args)
      attributes = {
        "homebrew_version"         => homebrew_version,
        "loaded_from_api"          => loaded_from_api,
        "loaded_from_internal_api" => loaded_from_internal_api,
        "uninstall_flight_blocks"  => uninstall_flight_blocks,
        "installed_on_request"     => installed_on_request,
        "time"                     => time,
        "runtime_dependencies"     => runtime_dependencies,
        "source"                   => source,
        "arch"                     => arch,
        "uninstall_artifacts"      => uninstall_artifacts,
        "built_on"                 => built_on,
      }
```

`/usr/local/Homebrew/Library/Homebrew/cask/installer.rb` lines 198–201:

```ruby
      tab = Tab.create(@cask)
      tab.installed_on_request = installed_on_request?
      tab.write
```

`/usr/local/Homebrew/Library/Homebrew/tab.rb:18`:
`FILENAME = "INSTALL_RECEIPT.json"`. `cask/metadata.rb:18-20`:
`metadata_main_container_path` is `caskroom_path.join(METADATA_SUBDIR)`, i.e.
`/usr/local/Caskroom/<token>/.metadata`.

On disk, on this machine:

```
$ for c in adoptopenjdk copilot-cli gcloud-cli gimp meld oracle-jdk visualdiffer wailbrew wireshark-chmodbpf; do
    printf '%-22s ' "$c"
    find /usr/local/Caskroom/$c/.metadata -maxdepth 3 -name 'INSTALL_RECEIPT.json'
    echo
  done
adoptopenjdk
copilot-cli            /usr/local/Caskroom/copilot-cli/.metadata/INSTALL_RECEIPT.json
gcloud-cli             /usr/local/Caskroom/gcloud-cli/.metadata/INSTALL_RECEIPT.json
gimp
meld                   /usr/local/Caskroom/meld/.metadata/INSTALL_RECEIPT.json
oracle-jdk             /usr/local/Caskroom/oracle-jdk/.metadata/INSTALL_RECEIPT.json
visualdiffer           /usr/local/Caskroom/visualdiffer/.metadata/INSTALL_RECEIPT.json
wailbrew               /usr/local/Caskroom/wailbrew/.metadata/INSTALL_RECEIPT.json
wireshark-chmodbpf     /usr/local/Caskroom/wireshark-chmodbpf/.metadata/INSTALL_RECEIPT.json

$ for c in copilot-cli gcloud-cli meld oracle-jdk visualdiffer wailbrew wireshark-chmodbpf; do
    printf '%-20s ' "$c"
    jq -c '{installed_on_request, homebrew_version, time}' \
       /usr/local/Caskroom/$c/.metadata/INSTALL_RECEIPT.json
  done
copilot-cli          {"installed_on_request":true,"homebrew_version":"6.0.9","time":1783725207}
gcloud-cli           {"installed_on_request":true,"homebrew_version":"6.0.18","time":1787506948}
meld                 {"installed_on_request":true,"homebrew_version":"4.5.13","time":1754584536}
oracle-jdk           {"installed_on_request":true,"homebrew_version":"6.0.18","time":1787105598}
visualdiffer         {"installed_on_request":true,"homebrew_version":"6.0.18","time":1787505111}
wailbrew             {"installed_on_request":true,"homebrew_version":"6.0.18","time":1786963056}
wireshark-chmodbpf   {"installed_on_request":true,"homebrew_version":"6.0.18","time":1786963694}
```

**Seven of nine casks carry `installed_on_request` on disk; two do not.** The
oldest receipt observed was written by Homebrew 4.5.13 on 2025-08-07. `gimp`
(installed 2026-04-24) and `adoptopenjdk` (2024-03-10) have a `.metadata`
directory with `config.json` and a cached `Casks/<token>.json`, but no
`INSTALL_RECEIPT.json`:

```
$ find /usr/local/Caskroom/gimp/.metadata -maxdepth 5
/usr/local/Caskroom/gimp/.metadata
/usr/local/Caskroom/gimp/.metadata/config.json
/usr/local/Caskroom/gimp/.metadata/3.2.4
/usr/local/Caskroom/gimp/.metadata/3.2.4/20260424191842.083
/usr/local/Caskroom/gimp/.metadata/3.2.4/20260424191842.083/Casks
/usr/local/Caskroom/gimp/.metadata/3.2.4/20260424191842.083/Casks/gimp.json
```

The CLI does not expose the field for casks:

```
$ brew list --cask --installed-on-request
Usage: brew list, ls [options] [installed_formula|installed_cask ...]
   … (usage text; the combination is rejected)

$ brew list --installed-on-request | grep -cE "gimp|meld|wailbrew|copilot-cli"
0
```

### E-6 — Payload identity: what Homebrew can and cannot assert about a cask's bytes

`/usr/local/Homebrew/docs/Homebrew-Security-and-Supply-Chain.md`, lines 189–199,
verbatim (same text at
<https://docs.brew.sh/Homebrew-Security-and-Supply-Chain>, read 2026-09-04):

> ### Casks have a different trust model
>
> Formulae and casks share Homebrew's reviewed package metadata and signed JSON
> API, but they do not share the same artefact security model.
> `homebrew/core` formulae are open source build recipes based on checksummed
> source. Homebrew builds bottles for formulae in its own controlled, ephemeral
> CI and checksums the result, so users normally trust Homebrew's reviewed
> source-to-bottle pipeline rather than a binary supplied and signed by the
> upstream developer.
>
> Casks instead install prebuilt applications and installers supplied directly
> by the upstream developer. This is necessary for native macOS applications and
> proprietary software, but Homebrew usually cannot reproduce those binaries
> from source or compare them with an independently produced build.
> A cask's `sha256` proves that the downloaded bytes match the package metadata,
> but it does not prove who produced those bytes or whether the program inside
> is trustworthy.
> **Some casks must use `sha256 :no_check` because their download URL changes
> contents in place, while self-updating applications can replace themselves
> outside Homebrew entirely.**
> Cask installation artefacts are treated as trusted vendor installation actions
> once a cask is accepted, so users must still trust the vendor.

Measured over the 7720 cached cask records:

```
$ jq -r 'select(.casks)|[.casks[]|.sha256]|group_by(.)|map({k:.[0],n:length})
         |sort_by(-.n)|.[0:4][]|"\(.n)\t\(.k|tostring[0:20])"' "$P"
2765	:no_check
6	e67a68ee3386db63f48b
3	57468a4adc55d954ead4
3	7fc800d2ac6b92844895

$ jq -r 'select(.casks)|[.casks[]|.version|tostring]|group_by(.)
         |map({k:.[0],n:length})|sort_by(-.n)|.[0:3][]|"\(.n)\t\(.k)"' "$P"
2139	latest
77	3.5.1
32	2.0.0
```

**2765 of 7720 casks (35.8%) declare `sha256 :no_check`; 2139 of 7720 (27.7%)
declare `version :latest`.**

Self-update machinery in the four `.app` payloads here:

```
$ for a in /Applications/GIMP.app /Applications/Meld.app /Applications/VisualDiffer.app /Applications/WailBrew.app; do
    printf '%-32s ' "$a"
    find "$a" -maxdepth 4 -name 'Sparkle.framework' -o -maxdepth 4 -name 'Autoupdate.app'
    echo
  done
/Applications/GIMP.app
/Applications/Meld.app
/Applications/VisualDiffer.app   /Applications/VisualDiffer.app/Contents/Frameworks/Sparkle.framework
/Applications/WailBrew.app

$ defaults read /Applications/VisualDiffer.app/Contents/Info.plist SUFeedURL
https://app.visualdiffer.com/appcast.xml
```

Whether Homebrew's record had already drifted from disk, today:

```
$ for a in VisualDiffer WailBrew GIMP Meld; do printf '%-14s ' "$a"
    defaults read "/Applications/$a.app/Contents/Info.plist" CFBundleShortVersionString; done
VisualDiffer   2.1.21
WailBrew       0.10.4
GIMP           3.2.4
Meld           3.22.3

$ jq -r '.casks[]|select(.token|IN("visualdiffer","wailbrew","gimp","meld"))
         |"\(.token)\tinstalled=\(.installed)\tbundle_short_version=\(.bundle_short_version)"' installed.json
gimp	installed=3.2.4	bundle_short_version=3.2.4
meld	installed=3.22.3+105	bundle_short_version=3.22.3
visualdiffer	installed=2.1.21	bundle_short_version=2.1.21
wailbrew	installed=0.10.4	bundle_short_version=0.10.4
```

No drift observed on 2026-09-04.

### E-7 — A cask whose definition no longer exists

`adoptopenjdk` reports `tap: null` (E-1) and no `url`. The only definition
Homebrew still has is the copy cached at install time in 2024, and it is
degenerate:

```
$ jq -c '{token,tap,url,version,artifacts}' \
     "/usr/local/Caskroom/adoptopenjdk/.metadata/16.0.1,9/20240310183744.949/Casks/adoptopenjdk.json"
{"token":null,"tap":null,"url":null,"version":null,
 "artifacts":[{"uninstall":[{"pkgutil":"net.adoptopenjdk.16.jdk"}]},
              {"zap":[{"trash":["~/Library/Saved Application State/net.java.openjdk.*.java.savedState", …]}]}]}
```

The `.pkg` is still in the Caskroom (`OpenJDK16U-jdk_x64_mac_hotspot_16.0.1_9.pkg`,
206 MB, E-4) and `net.adoptopenjdk.16.jdk` is still in the system receipt
database, but Homebrew can no longer say what artifact stanza produced it.

### E-8 — MacPorts does ship GUI applications, and they land somewhere else — cited, unobserved

`ports.macports.org` category listing, read 2026-09-04:

> <https://ports.macports.org/category/aqua/> — "1,332 port(s) found"

and the site-wide total, from the unfiltered search page read the same day:

> <https://ports.macports.org/search/?q=> — "41,655 port(s) found"

Where an `.app` goes. `macports-base`, `doc/macports.conf.in`
(<https://raw.githubusercontent.com/macports/macports-base/master/doc/macports.conf.in>,
read 2026-09-04):

```
prefix              	@prefix_expanded@

# Directory for application bundles installed by ports.
applications_dir    	@MPAPPLICATIONSDIR@

# Directory for frameworks installed by ports.
frameworks_dir      	@MPFRAMEWORKSDIR@
```

and the default, from `macports-base/aclocal.m4` lines 690–712
(fetched via `gh api repos/macports/macports-base/contents/aclocal.m4`,
2026-09-04), verbatim:

```m4
AC_DEFUN([MP_PATH_APPLICATIONS],[
	AC_REQUIRE([MP_CHECK_INSTALLUSER])

	AC_ARG_WITH(applications-dir,[AS_HELP_STRING([--with-applications-dir],[applications installation directory])], MPAPPLICATIONSDIR=${withval})

	oldprefix=$prefix
	if test "x$prefix" = "xNONE" ; then
		prefix=$ac_default_prefix
	fi
	AC_MSG_CHECKING([for Applications installation directory])

	if test "x$MPAPPLICATIONSDIR" = "x" ; then
		if test "$DSTUSR" = "root" ; then
			MPAPPLICATIONSDIR="/Applications/MacPorts"
		else
			MPAPPLICATIONSDIR="$(eval echo ~$DSTUSR)/Applications/MacPorts"
		fi
	fi

	AC_MSG_RESULT([$MPAPPLICATIONSDIR])
	AC_SUBST(MPAPPLICATIONSDIR)
	prefix=$oldprefix
])
```

The mechanism a port uses to produce one, `_resources/port1.0/group/app-1.0.tcl`
(read 2026-09-04):

> "This PortGroup helps create an application bundle the user can open from the
> Finder or the Dock."

with the bundle written to `${destroot}${applications_dir}/${app.name}.app/Contents/`,
`app.executable` defaulting to `${name}` resolved against `${prefix}/bin`.

A worked example, `aqua/gimp2-launcher/Portfile` (read 2026-09-04), line 60:

```tcl
    copy ${worksrcpath}/GIMP.app ${destroot}${applications_dir}
```

and `textproc/meld/Portfile` (read 2026-09-04), lines 4–7 and 67–68:

```tcl
PortGroup               python 1.0
PortGroup               app 1.0
PortGroup               active_variants 1.1
PortGroup               yelp 1.0
…
app.icon                data/icons/hicolor/scalable/apps/org.gnome.meld.svg
app.use_launch_script   yes
```

**So a MacPorts GUI application lands at `/Applications/MacPorts/<Name>.app` by
default, not `/Applications/<Name>.app`, and its executable is normally a thin
launcher over `${prefix}/bin/<name>`.** This is unobserved — no MacPorts
installation exists here to check it against.

What MacPorts records about it: the registry, per R-3 (§E-9 and the interface
table at line 1039), tracks every installed file (`port -q contents <name>`),
the `requested` flag as the provider's own record of user intent, and the
active variant set. R-4 §E-9 covers what the registry does *not* record. Both
are cross-references to notes in this directory, not new claims.

### E-9 — Origin under MacPorts is per-port, not per-provider — cited, unobserved

Source-built. `java/openjdk26/Portfile`
(<https://raw.githubusercontent.com/macports/macports-ports/master/java/openjdk26/Portfile>,
read 2026-09-04): name `openjdk26`, version `26.0.2.1`, categories `java devel`,
`supported_archs x86_64 arm64`, fetches the `openjdk/jdk26u` source archive from
GitHub, runs a `configure`/`make images` build, and in `destroot` symlinks the
result into `/Library/Java/JavaVirtualMachines/jdk-26-macports.jdk`.

Vendor binary. `java/openjdk26-temurin/Portfile` (read 2026-09-04): categories
`java devel`, `supported_archs x86_64 arm64`, `use_configure no`, `build {}`,
`master_sites https://github.com/adoptium/temurin26-binaries/releases/download/`,
`distname OpenJDK26U-jdk_${arch_classifier}_mac_hotspot_${version}_${build}`,
destroot copies `Contents` into place and symlinks into
`/Library/Java/JavaVirtualMachines`.

The sharpest case is a port that fetches **the same bytes as the cask**.
`llm/copilot-cli/Portfile` (read 2026-09-04), verbatim extract:

```tcl
name                copilot-cli      # via github.setup github copilot-cli 1.0.82 v
categories          llm
license             Restrictive/Distributable
supported_archs     arm64 x86_64
platforms           {darwin >= 20}
use_configure       no
build               {}

# Pre-built binary -- architecture-specific distfiles
if {${configure.build_arch} eq "arm64"} {
    set arch_string     darwin-arm64
    checksums           rmd160  5f6ee50c3d5307d488b361aed208a64e7afa22d4 \
                        sha256  c03bfc58c4402ec31d09ca11c379e1c17e95c3441f48009d721bf556fcc2c42e \
                        size    84019157
} else {
    set arch_string     darwin-x64
    checksums           rmd160  b10fa36f7584d9765985882f543ed1554be7c08d \
                        sha256  512cbd527e510664be8854ddfee3f3215f890a2f548b0477e9dd544b54abbd8c \
                        size    94489922
}

master_sites        https://github.com/github/copilot-cli/releases/download/v${version}/
distname            copilot-${arch_string}

destroot {
    xinstall -m 0755 ${worksrcpath}/copilot ${destroot}${prefix}/bin/copilot
}
```

The cask, observed on this machine:

```
$ jq -r '.casks[]|select(.token=="copilot-cli")|{url, sha256, version}' installed.json
{
  "url": "https://github.com/github/copilot-cli/releases/download/v1.0.82/copilot-darwin-x64.tar.gz",
  "sha256": "512cbd527e510664be8854ddfee3f3215f890a2f548b0477e9dd544b54abbd8c",
  "version": "1.0.82"
}
```

**Same version, same release URL, same `sha256`
(`512cbd527e510664be8854ddfee3f3215f890a2f548b0477e9dd544b54abbd8c`).** One
side symlinks it to `/usr/local/bin/copilot`, the other installs it to
`${prefix}/bin/copilot`.

R-4 establishes the rest of the MacPorts artifact story (binary archives from
`packages.macports.org`, `buildfromsource ifneeded`, licence-gated
distributability) and is not repeated here.

### E-10 — Name-match census: cask tokens against MacPorts port directories

Method: the set of directory names `<category>/<name>/Portfile` in the
`macports-ports` repository at `master`, intersected with the set of cask
tokens in the Homebrew API cache.

```
$ gh api "repos/macports/macports-ports/git/trees/master?recursive=1" > mptree.json   # 2.9 s
$ jq -r '.truncated' mptree.json
false
$ jq -r '.tree[]|select(.type=="blob")|.path|select(endswith("/Portfile"))
         |split("/")|select(length==3)|.[1]' mptree.json | sort -u > portnames.txt
$ wc -l < portnames.txt
   20069
$ jq -r 'select(.casks)|.casks|keys[]' "$P" | sort -u > casktokens.txt
$ wc -l < casktokens.txt
    7720
$ comm -12 casktokens.txt portnames.txt | wc -l
     197
$ tr 'A-Z' 'a-z' < portnames.txt | sort -u > portnames_lc.txt
$ tr 'A-Z' 'a-z' < casktokens.txt | sort -u > casktokens_lc.txt
$ comm -12 casktokens_lc.txt portnames_lc.txt | wc -l
     246
$ grep -c '^font-' casktokens.txt
    2590
$ comm -12 casktokens.txt portnames.txt | head -40 | tr '\n' ' '
1password-cli acorn alacritty android-ndk android-platform-tools ape arc audacity
avidemux avogadro barrier bean beekeeper-studio birdfont blender bluefish boop
bowtie bzflag cabal chai chipmunk chromedriver claude-code codex confluent-cli
consul cool-retro-term copilot-cli corelocationcli cutter darktable dash
dbeaver-community deco deluge denemo digikam diskspace djview
```

Spot-check of the extraction:

```
$ for n in meld gimp wireshark-chmodbpf copilot-cli google-cloud-sdk openjdk26-temurin visualdiffer wailbrew; do
    printf '%-22s ' "$n"; grep -qx "$n" portnames.txt && echo PRESENT || echo absent; done
meld                   PRESENT
gimp                   PRESENT
wireshark-chmodbpf     PRESENT
copilot-cli            PRESENT
google-cloud-sdk       PRESENT
openjdk26-temurin      PRESENT
visualdiffer           absent
wailbrew               absent
```

**197 of 7720 cask tokens (2.6%) are also a MacPorts port directory name; 246
(3.2%) when case is folded.** Error bars, in both directions:

- **Undercount.** Only 20 069 directories were counted, against the 41 655
  ports `ports.macports.org` reports. The gap is *subports*, which have no
  directory of their own (`py313-google-cloud-core` lives in
  `python/py-google-cloud-core/Portfile`). Every subport is invisible to this
  method.
- **Undercount.** Different naming conventions defeat exact matching:
  `gcloud-cli` ↔ `google-cloud-sdk` is a real equivalence this method misses.
- **Overcount.** A name match is evidence, not proof. `copilot` exists in both
  and means AWS Copilot in MacPorts' `devel` category and GitHub Copilot in
  its `llm` category.
- **Neither.** 2590 of the 7720 casks (33.5%) are `font-*` tokens. MacPorts has
  font ports, but under wholly different names, so they neither match nor are
  absent for a meaningful reason.

### E-11 — The nine installed casks, one by one, against MacPorts

Each MacPorts fact below was read from `ports.macports.org` or the
`macports-ports` repository on **2026-09-04**, and none was observed on a
running MacPorts installation.

| Cask (version here) | MacPorts search result | Verdict |
|---|---|---|
| `copilot-cli` 1.0.70 | port `copilot-cli` 1.0.82, category `llm`, same distfile URL and `sha256` (E-9) | **Same software, same bytes** |
| `meld` 3.22.3+105 | port `meld` 3.22.2, `textproc devel`, `PortGroup app 1.0`, `app.use_launch_script yes` | **Same software, built from source, `.app` under `applications_dir`** |
| `wireshark-chmodbpf` 4.6.8 | port `wireshark-chmodbpf` 1.1, `net`, `supported_archs noarch`, `startupitem.create yes`, `startupitem.name wireshark.ChmodBPF`, `addgroup access_bpf` | **Same function, different implementation** |
| `gimp` 3.2.4 | port `gimp` 2.10.38 (a `PortGroup stub` meta-port), plus `gimp3-devel` 2.99.rc1; no stable `gimp3` port | **Same software, older major version** |
| `gcloud-cli` 581.0.0 | port `google-cloud-sdk`, `devel python`; name differs | **Same software, different name** |
| `adoptopenjdk` 16.0.1,9 | `0 port(s) found` for `adoptopenjdk`; the family is `openjdk{8,11,17,21,22,23,24,25,26}-temurin`; **no 16** | **Family equivalent, no version equivalent** |
| `oracle-jdk` 26.0.2.1 | no `oracle-jdk`; `openjdk21-oracle`, `openjdk22-oracle` (Oracle's GPL OpenJDK builds) and `openjdk26` (source-built OpenJDK) exist | **Different vendor/licence, no version equivalent** |
| `visualdiffer` 2.1.21 | `0 port(s) found`, "No results found." | **None** |
| `wailbrew` 0.10.4 | `0 port(s) found`, "No results found." | **None** |

Verbatim search results backing the two zeros and the two families:

> `https://ports.macports.org/search/?q=visualdiffer&name=on` → "0 port(s) found" / "No results found."
> `https://ports.macports.org/search/?q=wailbrew&name=on` → "0 port(s) found" / "No results found."
> `https://ports.macports.org/search/?q=adoptopenjdk&name=on` → "0 port(s) found" / "No results found."
> `https://ports.macports.org/search/?q=temurin&name=on` → 9 ports: `openjdk8-temurin`, `openjdk11-temurin`, `openjdk17-temurin`, `openjdk21-temurin`, `openjdk22-temurin`, `openjdk23-temurin`, `openjdk24-temurin`, `openjdk25-temurin`, `openjdk26-temurin`
> `https://ports.macports.org/search/?q=oracle&name=on` → 22 ports; the only JDKs are `openjdk21-oracle` and `openjdk22-oracle`
> `https://ports.macports.org/search/?q=google-cloud&name=on` → 11 ports, first is `google-cloud-sdk` — "Command-line interface for Google Cloud Platform products and services"
> `https://ports.macports.org/port/copilot-cli/` → "copilot-cli", version 1.0.82, "Installations: 7 / Requested Installations: 7"
> `https://ports.macports.org/port/meld/` → "meld", version 3.22.2, "Visual diff and merge tool"
> `https://ports.macports.org/search/?q=gimp&name=on` → 27 ports; `gimp` "GIMP - Batteries Included", `gimp2`, `gimp2-devel`, `gimp3-devel`, `gimp2-launcher` "Application bundle launcher"; **no `gimp3`**
> `https://ports.macports.org/search/?q=wireshark&name=on` → 6 ports: `wireshark2`, `wireshark3`, `wireshark4`, `wireshark22`, `wireshark30`, `wireshark-chmodbpf`

`net/wireshark-chmodbpf/Portfile` (read 2026-09-04), the relevant lines:

```tcl
name                wireshark-chmodbpf
version             1.1
categories          net
license             {GPL-2 GPL-3}
supported_archs     noarch
use_configure       no
build               {}

# name of the group used on /dev/bpf*
set bpf_group       access_bpf

pre-destroot {
    addgroup ${bpf_group}
}

destroot {
    reinplace s|@BPF_GROUP@|${bpf_group}|g ${worksrcpath}/sbin/${name}
    xinstall -m 0755 -o root -g wheel ${worksrcpath}/sbin/${name} ${destroot}${prefix}/sbin/${name}
}

startupitem.create      yes
startupitem.name        wireshark.ChmodBPF
startupitem.autostart   yes
startupitem.executable  ${prefix}/sbin/${name}
startupitem.pidfile     none
```

### E-12 — Casks can depend on formulae: the DSL, the census, the enforcement point

`/usr/local/Homebrew/docs/Cask-Cookbook.md` lines 405–436, verbatim:

> ### Stanza: `depends_on`
>
> `depends_on` is used to declare dependencies and requirements for a cask.
> **`depends_on` is not consulted until `install` is attempted.**
>
> | key | description |
> | `cask:` | required Homebrew cask tokens as string or array |
> | `formula:` | required Homebrew formula names as string or array |
> | `macos:` | minimum macOS release as a symbol, or an array of exact acceptable releases … |
> | `maximum_macos:` | maximum macOS release requirement using a `<=` comparison |
> | `linux:` | Linux requirement, expressed as `depends_on :linux` |
> | `arch:` | hardware requirements as symbol or array |
>
> #### `depends_on` *formula*
>
> The value should be one or more names of formulae needed by the current cask,
> as a string or array of strings.
>
> Example: some distributions are contained in archive formats such as `7z`
> which are not supported by stock Apple tools. For these cases, a more capable
> archive reader may be pulled in at install time by declaring a dependency on
> the `unar` formula:
>
> ```ruby
> depends_on formula: "unar"
> ```

Census over the 7720 cached cask records:

```
$ jq -r 'select(.casks)|[.casks[]|.depends_on_args//empty
         |if type=="array" then (.[]|if type=="object" then (keys|join("+")) else tostring end)
          else (keys|join("+")) end]
         |group_by(.)|map({k:.[0],n:length})|sort_by(-.n)|.[]|"\(.n)\t\(.k)"' "$P"
4850	:macos
122	:arch+:macos
44	:cask+:macos
22	:formula+:macos
3	:arch
2	:formula
2	:maximum_macos
1	:arch+:cask+:macos
1	:cask+:formula+:macos
1	:linux
1	:macos+:maximum_macos

$ jq -r 'select(.casks)|[.casks|to_entries[]|select(.value.depends_on_args[":formula"]?)]|length' "$P"
25

$ jq -r 'select(.casks)|.casks|to_entries[]|select(.value.depends_on_args[":formula"]?)
         |"\(.key)\t\(.value.depends_on_args[":formula"]|join(","))"' "$P"
aptible	libfido2
beutl	ffmpeg@6
displaycal	argyll-cms
dmidiplayer	fluid-synth
doteditor	graphviz
droid	ripgrep
gcloud-cli	python@3.14
geotag	exiftool
goneovim	neovim
gpgfrontend	gnupg
ibabel	open-babel
kiibohd-configurator	dfu-util
mactex-no-gui	ghostscript
mactex	ghostscript
markdown-service-tools	multimarkdown
metasploit	nmap
n1ghtshade	libimobiledevice,libirecovery,libplist,libusb,libusbmuxd,libzip,openssl
ncar-ncl	gcc
neovide-app	neovim
panwriter	pandoc
restic-browser	restic
streamlink-twitch-gui	streamlink
superslicer	zstd
vmpk	fluid-synth
vv	neovim
```

**25 of 7720 casks (0.32%) declare a formula dependency**; 46 declare a cask
dependency (44 + 1 + 1 from the shape census above). And how many bake a
Homebrew prefix into their artifact stanzas:

```
$ jq -r 'select(.casks)|[.casks|to_entries[]
         |select((.value.raw_artifacts|tostring)|test("HOMEBREW_PREFIX|homebrew_prefix"))|.key]|length' "$P"
75
$ jq -r 'select(.casks)|[.casks|to_entries[]
         |select((.value.raw_artifacts|tostring)|test("/opt/homebrew|/usr/local"))|.key]|length' "$P"
77
$ jq -r 'select(.casks)|.casks|to_entries[]|select(.value.depends_on_args[":formula"]?)
         |select((.value.raw_artifacts|tostring)|test("HOMEBREW_PREFIX|homebrew_prefix"))|.key' "$P"
gcloud-cli
ncar-ncl
```

Homebrew's own dependency graph crosses the formula/cask boundary:

```
$ brew uses --installed python@3.14
gcloud-cli
jupyterlab
nmap
pipx
semgrep
```

and `brew autoremove` honours it —
`/usr/local/Homebrew/Library/Homebrew/utils/autoremove.rb` lines 15–29, verbatim:

```ruby
      def removable_formulae(formulae, casks)
        unused_formulae = unused_formulae_with_no_formula_dependents(formulae)
        cask_dep_names = cask_dependent_formula_names(casks, formulae)
        unused_formulae.reject { |f| cask_dep_names.intersect?(f.possible_names) }
      end

      # A set of names for all installed {Formula} objects that are {Cask} formula
      # dependencies (direct or transitive).
      # @private
      sig { params(casks: T::Array[Cask::Cask], formulae: T::Array[Formula]).returns(T::Set[String]) }
      def cask_dependent_formula_names(casks, formulae)
        formulae_by_name = formulae.to_h { |f| [f.name, f] }
        names = casks.flat_map { |cask| cask.depends_on.formula }.flat_map do |name|
```

The cask install receipt also records the resolved dependency closure:

```
$ jq -c '.runtime_dependencies' /usr/local/Caskroom/gcloud-cli/.metadata/INSTALL_RECEIPT.json
{"formula":[{"full_name":"python@3.14","version":"3.14.7","revision":0,"bottle_rebuild":0,
  "pkg_version":"3.14.7","declared_directly":true},
 {"full_name":"mpdecimal","version":"4.0.1",…,"declared_directly":false},
 {"full_name":"openssl@3","version":"3.6.3",…,"declared_directly":false},
 {"full_name":"sqlite","version":"3.53.4",…,"declared_directly":false},
 {"full_name":"xz","version":"5.8.3",…,"declared_directly":false},
 {"full_name":"zstd","version":"1.5.7",…,"declared_directly":false},
 {"full_name":"ca-certificates","version":"2026-08-13",…,"declared_directly":false},
 {"full_name":"readline","version":"8.3.3",…,"declared_directly":false},
 {"full_name":"lz4","version":"1.10.0",…,"declared_directly":false}]}
```

### E-13 — How the one formula-dependent cask here resolves its dependency at run time

The cask's own install-time wiring bakes an absolute Homebrew path into
`postflight_steps`:

```
$ jq -r '.casks[]|select(.token=="gcloud-cli")|.artifacts' installed.json
… (trimmed to the relevant step) …
{ "command": {"base":"homebrew_prefix","path":"share/google-cloud-sdk/bin/gcloud"},
  "network_access": true,
  "guards": [{"condition":"on","value":"macos","id":"2"}],
  "type": "run",
  "args": ["config","virtualenv","create","--python-to-use",
           "{{HOMEBREW_PREFIX}}/opt/python@3.14/libexec/bin/python"],
  "env": {"CLOUDSDK_PYTHON":"{{HOMEBREW_PREFIX}}/opt/python@3.14/libexec/bin/python"} }
```

But that virtualenv does not exist here, so nothing absolute survived into the
installed state:

```
$ ls -la ~/.config/gcloud/virtenv
ls: /Users/pirado/.config/gcloud/virtenv: No such file or directory
```

The installed launcher resolves Python by `PATH`.
`/usr/local/share/google-cloud-sdk/bin/gcloud`, verbatim:

```sh
# Wrapper around 'which' and 'command -v', tries which first, then falls back
# to command -v
_cloudsdk_which() {
  which "$1" 2>/dev/null || command -v "$1" 2>/dev/null
}
…
order_python() {
  …
      if "$python_version" -c "import sys; sys.exit(0 if ((3,9) <= (sys.version_info.major, sys.version_info.minor) <= (3,14)) else 1)" > /dev/null 2>&1; then
…
setup_cloudsdk_python() {
  # if $CLOUDSDK_PYTHON is not set, look for bundled python else
  # prefer python3 over python
  if [ -z "$CLOUDSDK_PYTHON" ]; then
    # Is bundled python present and working?
    if [ -x "$CLOUDSDK_ROOT_DIR/platform/bundledpythonunix/bin/python3" ] &&  \
       "$CLOUDSDK_ROOT_DIR/platform/bundledpythonunix/bin/python3" --version > /dev/null 2>&1;
    then
      CLOUDSDK_PYTHON="$CLOUDSDK_ROOT_DIR/platform/bundledpythonunix/bin/python3"
      CLOUDSDK_PYTHON_SITEPACKAGES=1
    …
      primary_python=python3.14
      CLOUDSDK_PYTHON=$(order_python "$primary_python" python3.13 python3.12 python3 python3.11 python3.10 python)
```

There is no bundled interpreter:

```
$ ls /usr/local/share/google-cloud-sdk/platform/
bq
ext-runtime
gcloud-crc32c_licenses
gsutil
```

so the resolution on this machine is:

```
$ echo "[${CLOUDSDK_PYTHON:-<unset>}]"
[<unset>]
$ which python3.14
/usr/local/bin/python3.14
$ ls -la /usr/local/bin/python3.14
lrwxr-xr-x@ 1 pirado admin 43 Aug 17 07:35 /usr/local/bin/python3.14 -> ../Cellar/python@3.14/3.14.7/bin/python3.14
```

Nothing in the installed payload names the Homebrew prefix:

```
$ grep -rl "/usr/local/opt\|/usr/local/Cellar" /usr/local/share/google-cloud-sdk
   (no output)
$ ./scan.sh /usr/local/share/google-cloud-sdk
root=/usr/local/share/google-cloud-sdk executable_files=36 mach_o=1 with_usr_local_link=0
```

### E-14 — Do any cask payloads link against Homebrew dylibs? Measured

`scan.sh` is read-only: it walks a root with `find -type f -perm -u+x`,
classifies each file with `file`, and for each Mach-O runs `otool -L`, reporting
any load command beginning `/usr/local/`.

```
$ for r in /Applications/GIMP.app /Applications/Meld.app /Applications/VisualDiffer.app /Applications/WailBrew.app; do ./scan.sh "$r" | tail -1; done
root=/Applications/GIMP.app         executable_files=688 mach_o=608 with_usr_local_link=0
root=/Applications/Meld.app         executable_files=181 mach_o=140 with_usr_local_link=0
root=/Applications/VisualDiffer.app executable_files=7   mach_o=7   with_usr_local_link=0
root=/Applications/WailBrew.app     executable_files=1   mach_o=1   with_usr_local_link=0
```

The shape of what they link instead:

```
$ otool -L /Applications/GIMP.app/Contents/MacOS/gimp | tail -n +2 | awk '{print $1}'
@rpath/lib/libgimpbase-3.0.0.dylib
…
@rpath/lib/libgtk-3.0.dylib
@rpath/lib/libglib-2.0.0.dylib
@rpath/lib/libcairo.2.dylib
/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit
/usr/lib/libSystem.B.dylib

$ otool -L /Applications/Meld.app/Contents/MacOS/Meld | tail -n +2 | awk '{print $1}'
/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation
@executable_path/../Frameworks/Python.framework/Versions/3.10/Python
/usr/lib/libc++.1.dylib
/usr/lib/libSystem.B.dylib

$ otool -L /usr/local/Caskroom/copilot-cli/1.0.70/copilot | tail -n +2 | awk '{print $1}'
/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation
/System/Library/Frameworks/Security.framework/Versions/A/Security
/usr/lib/libc++.1.dylib
/usr/lib/libSystem.B.dylib
```

**Zero of the 756 Mach-O files inside the four `.app` payloads, and zero of the
one Mach-O in the gcloud SDK, carry a `/usr/local/` load command.** GIMP bundles
its whole GTK stack under `@rpath`; Meld bundles Python 3.10 as a framework
under `@executable_path`; the other two are system-frameworks-only.

## Findings

### On Q1 — is there a destination?

1. **A cask installs a vendor-produced artifact, and 95.5% of casks install
   exactly one kind of it.** The DSL supports 31 ordinary artifact classes plus
   flight blocks and install steps (E-2). Across 7720 casks the payload kinds
   are `app` 4139 (53.6%), `font` 2582 (33.4%), `pkg` 658 (8.5%), `binary` 360
   (4.7%), `installer` 97 (1.3%), with 7374 casks carrying exactly one payload
   kind and only 5 carrying none (E-3). On this machine the nine casks are 4
   `app`, 2 `pkg`, 2 `binary`, 1 `installer script`, and 1 with no payload
   stanza left (E-3, E-7). Every download URL here is a vendor host (E-1).

2. **Nothing a cask installs lives inside the Homebrew prefix by default.**
   `appdir` is `/Applications`, and every other artifact directory is a system
   or per-user macOS location (E-3). A `pkg` cask's files are placed by
   `/usr/sbin/installer` and inventoried in Apple's receipt database, not by
   Homebrew: `pkgutil --pkgs` lists `com.oracle.jdk-26.0.2.1`,
   `net.adoptopenjdk.16.jdk` and `org.wireshark.ChmodBPF.pkg`, and Homebrew's
   only record of those writes is a hand-written `uninstall pkgutil:` string
   (E-4). One of them installs a root LaunchDaemon at
   `/Library/LaunchDaemons/org.wireshark.ChmodBPF.plist` running a program under
   `/Library/Application Support/`, and creates the `access_bpf` group (E-4).

3. **MacPorts does ship GUI applications, and they land somewhere else.** 1332
   ports are in the `aqua` category out of 41 655 (E-8). An `.app` goes to
   `${applications_dir}`, whose default is `/Applications/MacPorts` when the
   install user is root (`aclocal.m4` `MP_PATH_APPLICATIONS`, E-8) — *not*
   `/Applications`. The `app-1.0` PortGroup builds the bundle into
   `${destroot}${applications_dir}/${app.name}.app/Contents/` with the
   executable resolved against `${prefix}/bin` (E-8). **Cited, unobserved.**

4. **Origin is a per-port property under MacPorts, not a per-provider one.**
   `openjdk26` builds OpenJDK from source and symlinks the result into
   `/Library/Java/JavaVirtualMachines/`; `openjdk26-temurin` has
   `use_configure no`, `build {}` and fetches Eclipse Adoptium's prebuilt
   binary; `copilot-cli` fetches **the identical GitHub release tarball, with
   the identical `sha256`, that the cask fetches** (E-9). So "casks are vendor
   binaries, ports are source builds" is false as a general rule. What *is*
   true and asymmetric: MacPorts records every installed file in its registry
   (R-3), while a cask's `pkg` payload is recorded only by macOS.

5. **On this machine, 5 of 9 casks have a MacPorts equivalent that is
   recognisably the same software; 2 have a family equivalent at a different
   version or from a different vendor; 2 have none** (E-11).
   - Same software: `copilot-cli` (same bytes), `meld` (3.22.2 vs 3.22.3+105),
     `wireshark-chmodbpf` (same function, MacPorts' own implementation),
     `gimp` (2.10.38 vs 3.2.4 — one major version behind), `gcloud-cli` →
     `google-cloud-sdk` (name differs).
   - Family only: `adoptopenjdk` 16 (MacPorts has Temurin 8/11/17/21/22/23/24/25/26,
     **not 16**); `oracle-jdk` 26.0.2.1 (MacPorts has `openjdk26` source-built and
     `openjdk21-oracle`/`openjdk22-oracle`, none of which is Oracle's JDK 26).
   - None: `visualdiffer`, `wailbrew` — both "0 port(s) found".

   **Method and its error bars.** Four of the five were found by exact
   token↔port-directory match; `gcloud-cli`→`google-cloud-sdk` required a
   human-judged search, which is exactly the failure mode of name matching. A
   name match is evidence, not proof: `copilot` means two different programs
   under the two providers (E-10). A name miss is not proof of absence: subports
   have no directory and were invisible to the census.

6. **Catalogue-wide, name matching finds a MacPorts port for 2.6% of casks
   (197 of 7720; 246, 3.2%, case-folded)** (E-10). Read this as a *floor*
   inflated by false positives and depressed by subports and naming conventions
   — not as a coverage figure. A third of the cask catalogue is fonts, for which
   the method is meaningless.

7. **A cask "migration" is not the operation the planner performs for a
   formula, and INV-01's verification step is where that breaks.** For a formula
   the two sides are the same upstream source built to comparable configuration,
   and `VERIFY_DESTINATION` can meaningfully assert the replacement works before
   `REMOVE_SOURCE` runs. For a cask:
   - The version usually differs and can differ by a major release
     (`gimp` 3.2.4 → 2.10.38), which is not the same software in the sense that
     matters to a user's documents and plug-ins.
   - The **vendor** can differ (`oracle-jdk` → `openjdk26`), which changes the
     licence, not just the build.
   - The install location changes (`/Applications/GIMP.app` →
     `/Applications/MacPorts/GIMP.app`), so Dock items, Login Items, Launch
     Services registrations and any absolute path a user or a script holds all
     move.
   - Homebrew itself says the source artifact may not be pinnable: 2765 of 7720
     casks (35.8%) use `sha256 :no_check` and 2139 (27.7%) use `version
     :latest`, and "self-updating applications can replace themselves outside
     Homebrew entirely" (E-6). One of the four apps here carries Sparkle with a
     live appcast URL, and three casks declare `auto_updates: true`.
   - For the two `pkg` casks the *source* side cannot even be enumerated by
     Homebrew (finding 2), so "verify the source was removed" would be a claim
     about Apple's receipt database, not about anything MacPortsBrew captured.

   **Conclusion for Q1: yes, a destination sometimes exists — for 5 of the 9
   casks here — but "migrating a cask" is a different operation from migrating
   a formula, and the current step machine cannot honestly verify it.** Keeping
   `GuiApp` in `forces_manual_review()` is correct on the evidence, and the
   reason is not "GUI apps are scary" but "the two artifacts are frequently not
   the same software, and the source side is not fully enumerable".

### On Q2 — can a cask consume MacPorts components?

8. **Yes, a cask can depend on a formula, by `depends_on formula:`, and it is
   rare.** 25 of 7720 casks (0.32%) declare one; the full list is in E-12. On
   this machine exactly one does: `gcloud-cli` → `python@3.14`.

9. **The dependency is enforced only at install time.** The Cask Cookbook is
   explicit: "`depends_on` is not consulted until `install` is attempted"
   (E-12). Nothing re-checks it afterwards, and Homebrew imposes no run-time
   linkage of its own.

10. **How the payload finds the dependency is a per-cask property, and for the
    one measurable case here it is `PATH`.** The gcloud launcher searches
    `python3.14 → python3.13 → python3.12 → python3 → python3.11 → python3.10 →
    python` via `which`/`command -v`, accepting the first that reports a version
    in `[3.9, 3.14]`; there is no bundled interpreter in this distribution; and
    on this machine it lands on `/usr/local/bin/python3.14`, a Homebrew symlink,
    only because Homebrew's `bin` is the first entry on `PATH` that has one
    (E-13). **A MacPorts `python314` earlier on `PATH` would satisfy the
    launcher's own test.**

11. **But the same cask also bakes an absolute Homebrew path at install time.**
    `postflight_steps` runs `gcloud config virtualenv create --python-to-use
    {{HOMEBREW_PREFIX}}/opt/python@3.14/libexec/bin/python` with
    `CLOUDSDK_PYTHON` set to the same absolute path (E-13). Had that virtualenv
    been created, the recorded interpreter would be an absolute
    `/usr/local/opt/...` path and substitution would silently break. It was not
    created here (`~/.config/gcloud/virtenv` does not exist), so on *this*
    machine the resolution is purely `PATH`. Catalogue-wide, 75 of 7720 casks
    (1.0%) mention `HOMEBREW_PREFIX` in their artifact stanzas and 77 mention a
    literal `/opt/homebrew` or `/usr/local`; two of those are among the 25
    formula-dependent casks — `gcloud-cli` and `ncar-ncl` (E-12). **So both
    resolution modes the question anticipated are real, in the same cask.**

12. **No cask payload examined here links a Homebrew dylib.** Zero of 756
    Mach-O files across the four `.app` bundles, and zero in the gcloud SDK,
    carry a `/usr/local/` load command (E-14). Vendor apps bundle their
    dependencies under `@rpath` or `@executable_path` — GIMP its entire GTK
    stack, Meld a Python 3.10 framework — or use system frameworks. The
    "linked dylib" substitution hazard did not occur on this machine; that is a
    measurement over nine casks, not a general property.

13. **What would break if the formula were removed and the port used instead:**
    - `brew uses --installed python@3.14` lists `gcloud-cli` (E-12), and
      `Utils::Autoremove.removable_formulae` explicitly protects formulae that
      casks depend on (E-12). Removing `python@3.14` puts Homebrew's own graph
      into a state where a cask's declared dependency is unsatisfied.
    - Any `brew reinstall --cask gcloud-cli` or `brew upgrade --cask` would
      re-install `python@3.14`, because `depends_on` is consulted at install
      (finding 9). The configuration is therefore **not stable across the next
      cask upgrade** — it un-does itself.
    - The install receipt's `runtime_dependencies` closure would stop describing
      reality (E-12).
    - Run-time behaviour would depend entirely on `PATH` order, which is user
      configuration MacPortsBrew does not own and (SE-036) touches only through
      one managed block.

14. **This is the same boundary INV-07 and INV-08 draw, seen from the other
    side, and it is already forbidden.** INV-07: "MacPortsBrew never configures,
    links or records a Homebrew package as depending on a MacPorts-managed
    artifact, or vice versa." Deliberately arranging for a cask's
    `depends_on formula:` to be met by a port is precisely "configuring a
    Homebrew package to depend on a MacPorts-managed artifact". INV-08: "The
    presence of `openssl3` in MacPorts does not satisfy Homebrew's `openssl@3`
    requirement, in any calculation, ever" — a MacPorts `python314` does not
    satisfy the cask's `python@3.14` requirement in any calculation
    MacPortsBrew performs. Nothing new is needed; the invariants already cover
    the cask direction. What is honest to *say* is narrower and worth saying:
    *"the vendor's launcher resolves this by `PATH`, so another provider's
    interpreter can end up serving it; MacPortsBrew reports that as a
    resolution fact (the `Resolution` type), never arranges it, and Homebrew
    re-installs its own copy at the next cask upgrade."*

### On the ownership question that motivated the item

15. **The premise that "a cask carries no record of user intent" is false as
    stated, and true as stated about the JSON.** `brew info --json=v2` has no
    intent field for casks (E-5) — that part of R-1 finding 4 is confirmed. But
    Homebrew 6.0.21 writes `installed_on_request` into
    `/usr/local/Caskroom/<token>/.metadata/INSTALL_RECEIPT.json` on every cask
    install (`cask/installer.rb:198-200`, `cask/tab.rb:105-120`), and **seven of
    the nine casks here carry it, all `true`**, the oldest written by Homebrew
    4.5.13 in August 2025 (E-5). `gimp` and `adoptopenjdk` have no receipt.
    `brew list --installed-on-request` is formula-only and rejects `--cask`
    (E-5), so the file is the only interface.

16. **Therefore the correct model for cask ownership is three-valued, and the
    provider supplies two of the three.** `TOP_LEVEL` when the receipt says
    `installed_on_request: true`; `DEPENDENCY` when it says `false`; **`UNKNOWN`
    when there is no receipt** — a real, observed state (2 of 9 here), not a
    theoretical one. This is a *provider record of intent*, exactly the
    mechanism AGENTS.md prefers, and not a graph inference. A cask genuinely can
    be a dependency: 46 casks in the catalogue declare `depends_on cask:`
    (E-12), and Homebrew installs those with `installed_on_request: false`
    (`cask/installer.rb:541,553`).

## Cannot determine

- **Why `gimp` and `adoptopenjdk` have no `INSTALL_RECEIPT.json`.**
  `Cask::Installer#install` writes the tab unconditionally (E-5), so the absence
  must come from another path — an older Homebrew that did not write it for
  casks, an upgrade path, `--adopt`, or manual removal. `adoptopenjdk` (2024) is
  plausibly pre-feature; `gimp` (2026-04-24, i.e. *after* meld's 4.5.13 receipt
  of 2025-08-07) is not explained. A future attempt needs `git log` on
  `Library/Homebrew/cask/tab.rb` and `cask/upgrade.rb` in the `Homebrew/brew`
  repository to date the feature and identify the paths that skip it, plus a
  machine with a cask installed via `brew upgrade --cask` and one via
  `brew install --cask --adopt`. **UNRESOLVED**, and load-bearing for DP-012:
  the fraction of casks that carry intent decides how much option (a) costs.
- **Everything about MacPorts is unobserved.** The `applications_dir` default,
  the `aqua` count, the Portfile behaviours, the registry contents and the
  `requested` flag are all citations read on 2026-09-04 from official sources.
  No `port` command was run because MacPorts is not installed. A future attempt
  on a dual-provider machine should run `port -q installed requested`,
  `port -q contents <a GUI port>`, and `port info --index gimp meld
  wireshark-chmodbpf copilot-cli google-cloud-sdk` and compare. **UNRESOLVED**
  for the whole MacPorts half.
- **Whether a MacPorts `wireshark-chmodbpf` and the cask can coexist.** Both
  install a root LaunchDaemon doing the same job (`org.wireshark.ChmodBPF` vs.
  MacPorts' `startupitem.name wireshark.ChmodBPF`, which R-8 §E-7 says becomes
  `org.macports.*`), and both create the `access_bpf` group (E-4, E-11).
  Whether installing both is merely redundant or actively conflicting was not
  determined, and cannot be without a MacPorts installation. **UNRESOLVED.**
- **How many of the 197 name matches are genuine equivalences.** Establishing
  that needs per-pair comparison of homepage, upstream project and version — a
  mapping-discovery task, not this note's. **UNRESOLVED.**
- **Whether the internal API cache's `raw_artifacts` shape matches
  `brew info --json=v2`'s `artifacts` shape in every case.** The two were used
  interchangeably for counting here (E-3 uses the cache for the catalogue and
  the JSON for the nine). They agreed for the nine installed casks, but no
  general guarantee was found, and R-2 already warns that the internal API is
  explicitly internal and undocumented. **UNRESOLVED.**
- **Fonts.** 2582 casks (33.4%) are font casks. Whether MacPorts has equivalents
  for any meaningful fraction of them was not investigated; name matching is
  useless there.

## Impact on the specification

- **Contradicts `R-1` finding 4's last sentence**, and by extension
  `AGENTS.md` ("casks carry no intent field, so ownership is `UNKNOWN`"),
  `.claude/rules/adapters.md`, SE-042, and the comments at
  `crates/adapters/src/homebrew/mod.rs:603` ("A cask carries no record of user
  intent at all (R-1 finding 4)") and `crates/adapters/src/homebrew/json.rs:412`.
  Casks **do** carry `installed_on_request`, in
  `<caskroom>/<token>/.metadata/INSTALL_RECEIPT.json`, on 7 of 9 casks here
  (finding 15). The JSON-only statement is correct; the absolute statement is
  not. This should become a `docs/spec-evolution.md` entry; it is reported, not
  edited, from here.
- **Bears directly on SE-046 / DP-012** — see the final section.
- **Confirms `PackageClass::GuiApp ∈ forces_manual_review()`**
  (`crates/domain/src/class.rs:36-44`) on new evidence: not because GUI apps are
  special, but because source and destination are frequently not the same
  software and the source side of a `pkg` cask is not enumerable by the provider
  (findings 2, 7).
- **Suggests `PackageKind::GuiApp`'s doc comment is now inaccurate.**
  `crates/domain/src/package.rs:12-13` says "`GuiApp` is the Homebrew cask
  concept; the other provider has no equivalent (R-1, SE-012)". MacPorts has
  1332 `aqua` ports and a first-class `applications_dir` (finding 3). The
  *concept* exists on both sides; what differs is the install location and, per
  port, the origin. A provider-neutral wording would be better, and the domain
  rule forbids naming a provider there in any case.
- **Suggests `ThirdPartySourced` needs a sibling state or a wider definition.**
  `adoptopenjdk` is installed from a tap that no longer exists: `tap: null`,
  `url: null`, and a cached cask definition whose `token`, `version` and `url`
  are all `null` (E-7). That is not "from a non-core source" so much as "from a
  source that is gone", and it makes `ArtifactAvailability` genuinely `Unknown`
  rather than absent — the R-2 distinction between "nothing published" and "not
  published for this platform" has a third neighbour: "the record of what was
  published is gone".
- **Adds nothing new to INV-07/INV-08 but confirms they already cover the cask
  direction** (finding 14). No invariant change is warranted.
- **Offline/network split, for `analyze`.** Everything in findings 1, 2, 7
  (Homebrew half), 8, 10, 11, 12, 15 and 16 was obtained **offline**:
  `brew list --cask`, `brew info --json=v2 --installed`, the Caskroom
  `.metadata` files, the cached API payload, `otool`, `file`, `pkgutil --pkgs`,
  `defaults read`. Only the MacPorts half (findings 3, 4, 5, 6) needed the
  network, and on a machine with MacPorts installed it would be answerable from
  the local ports tree instead. **The cask-side analysis is offline-safe.**
- **`docs/spec/` was not edited from this note.**

## Re-check when

- **Homebrew's major or minor version changes.** The cask `Tab` /
  `INSTALL_RECEIPT.json` behaviour (finding 15), the `ORDINARY_ARTIFACT_CLASSES`
  list (E-2) and the internal cache's `raw_artifacts` shape (E-3) are all
  version-coupled to **Homebrew 6.0.21**.
- **Before any code reads `<caskroom>/<token>/.metadata/INSTALL_RECEIPT.json`.**
  Resolve the first `UNRESOLVED` item — the two casks without a receipt — so the
  adapter's `None` branch is written against a known cause, and confirm with
  `security-reviewer` that the file is an acceptable surface to read rather than
  an internal one, as R-2 asks for the `.payload` sidecar.
- **When MacPorts is installed on a development machine.** The entire MacPorts
  half becomes observable, and finding 5's per-cask verdicts should be
  re-derived with `port info --index` rather than a website search.
- **When the cask counts matter for a release.** Re-run the three censuses; they
  need only the cache and `jq`:

  ```
  P=~/Library/Caches/Homebrew/api/internal/packages.<tag>.jws.json.payload
  jq -r 'select(.casks)|[.casks[]|[.raw_artifacts//[]|.[]|if type=="array" then .[0] else tostring end]|unique|.[]]|group_by(.)|map({k:.[0],n:length})|sort_by(-.n)|.[]|"\(.n)\t\(.k)"' "$P"
  jq -r 'select(.casks)|[.casks|to_entries[]|select(.value.depends_on_args[":formula"]?)]|length' "$P"
  jq -r 'select(.casks)|[.casks[]|select(.sha256==":no_check")]|length' "$P"
  ```

  The name-match census needs
  `gh api "repos/macports/macports-ports/git/trees/master?recursive=1"` and the
  `split("/")|select(length==3)|.[1]` extraction in E-10; check `.truncated` is
  `false` before trusting it.
- **If Homebrew changes `depends_on`'s enforcement point.** Finding 9 —
  "not consulted until `install` is attempted" — is the entire reason a
  cross-provider substitution un-does itself at the next upgrade.

## What this means for DP-012

DP-012 asks whether casks belong in the default top-level view, given that
`crates/domain/src/ownership.rs:72-82` folds "no intent record" into
`TOP_LEVEL` while three documents promise `UNKNOWN`. Its options are **(a)**
`None ⇒ Ownership::Unknown`, and **(b)** relax the written rule to "never an
*unrecorded* graph inference" and keep the current behaviour. The
recommendation on file is (b) for the rule and (a) for the reachability of
`Unknown`.

**This research changes the premise both options rest on.** The entry, SE-046,
AGENTS.md and the adapter all assume casks have *no* intent record, so the only
question is what to do with the resulting `None`. That assumption is false:
Homebrew writes `installed_on_request` to the cask's install receipt, and 7 of
the 9 casks on the reviewer's own machine — the same nine the DP-012 entry
counts — carry it, all `true` (finding 15).

My reading, item by item:

- **It strengthens option (a), and removes most of its cost.** (a) was
  unattractive because it would move nine rows out of a 56-row top-level view
  on evidence of absence. With the receipt read, seven of those nine are
  `TOP_LEVEL` on the provider's *own* record — the strongest evidence class
  MacPortsBrew recognises — and only two become `UNKNOWN`. The headline count
  falls by two, not nine, and the two that fall are genuinely unknown.
- **It weakens option (b)'s justification.** (b) exists to defend a graph
  inference that would no longer be needed for 78% of the casks here. Relaxing
  the written rule to permit a *recorded* inference would be licensing a
  fallback that a better mechanism has just displaced.
- **It does not settle DP-012 as posed, and it is not mine to settle.** Two
  things remain the owner's:
  1. What to do with the residual `UNKNOWN` rows — show them in normal mode or
     not. That is still a scope decision under INV-13, and this note says
     nothing about it beyond making the population small and real.
  2. Whether reading `INSTALL_RECEIPT.json` is acceptable at all. It is an
     on-disk file in the provider's prefix, not a documented CLI surface, and
     the two casks without one are unexplained (first `UNRESOLVED` item).
     Reading an undocumented file to answer an ownership question is the same
     category of decision as R-2's open question about the `.payload` sidecar,
     and it should pass `security-reviewer` first.
- **A third option now exists that was not on the list, and I would put it to
  the owner:** **(c)** read cask intent from the install receipt —
  `true ⇒ TOP_LEVEL`, `false ⇒ DEPENDENCY`, missing ⇒ `UNKNOWN` — making
  `Ownership::Unknown` reachable without inventing an inference, and keeping the
  written rule exactly as it stands in AGENTS.md ("use the provider's own record
  of intent, never a graph inference"). Under (c) no document has to be relaxed
  and no code has to guess; what has to change is the *claim* that casks have no
  intent field, which is a correction rather than a concession.

So: **the answer strengthens (a), weakens (b), and mainly argues that the
question should be re-posed against the corrected fact.** It does not settle
whether `UNKNOWN` rows appear in normal mode, and it should not be read as
settling it.
