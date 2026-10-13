# R-8 — How does each provider manage background services, and where do common stateful packages keep their data?

- **Status:** partially answered — Homebrew side observed and cited; MacPorts
  side cited only (MacPorts is not installed on this machine); three items
  marked `UNRESOLVED`
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (Tahoe), x86_64, Homebrew 6.0.21 @ `/usr/local`
  (`homebrew/core` git revision `e045d7385fd7ad8907621300224f8a860bdd7bbb`,
  2026-09-02T09:54:08Z), MacPorts **not installed** (`/opt/local` absent,
  `port` not on `PATH`)
- **Blocking:** `02-domain-model.md` (Package classes — `STATEFUL`, `SERVICE`,
  `RUNTIME`), INV-19 (stateful packages are never migrated blind), INV-20
  (provider facts are dated data), `02-domain-model.md` (Snapshot / offline
  analysis, INV-15).

> **Machine note, added 2026-09-15.** This note states, correctly for its answer
> date, that MacPorts could not be observed here. **It can be since 2026-09-06**,
> when MacPorts 2.12.6 was installed at `/opt/local` (first ports 2026-09-07).
> Nothing below has been re-checked against it; the citations stand as citations.
> R-5 and R-5a were re-checked in place and show what that costs — R-5 gained
> fourteen evidence blocks and produced SE-077. Where this note names a "future
> attempt" that needed a MacPorts machine, that attempt is now available (F-46).

## Question

Enough to **detect** the `STATEFUL`, `SERVICE` and `RUNTIME` classes under both
providers and refuse to migrate them (INV-19) — not enough to migrate them.
Specifically: what metadata does each provider expose about background
services, where does each put its launch machinery, where do common stateful
packages keep data under each prefix, and which of these questions are
answerable offline.

## Evidence

### E-0 — Environment used for every Homebrew invocation below

Observed 2026-09-02 in this session:

```
$ env | grep -i homebrew | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

Every `brew` command in this note ran under exactly those three variables
(`HOMEBREW_NO_INSTALL_FROM_API` unset, i.e. default API mode). Per R-1, this
set does **not** suppress the `homebrew/core` tap clone that `brew info` on an
*unknown* name performs; accordingly every `brew info` here was given only
names produced by `brew list`, or no names at all (`--installed`).

Cache mtimes before and after the `brew info --json=v2 --installed` run at
09:40:

```
$ stat -f "%Sm %N" -t "%H:%M" ~/Library/Caches/Homebrew/api/internal/*
09:38 .../executables.txt
09:20 .../packages.tahoe.jws.json
09:20 .../packages.tahoe.jws.json.payload
09:38 .../packages.tahoe.jws.json.payload.index
```

(Listing taken at 09:50; the same mtimes were observed at 09:38, before the
`brew info` run.)

### E-1 — Homebrew: the `service` key in `brew info --json=v2`

```
$ time brew info --json=v2 --installed > brew-installed.json
brew info --json=v2 --installed  1.98s user 0.90s system 70% cpu 4.115 total
$ ls -l brew-installed.json
-rw-r--r--  1 <user>  wheel  876713 Sep  2 09:40 brew-installed.json
```

Derived from that file (python, read-only):

```
top-level keys: ['formulae', 'casks']
formulae: 180 casks: 9
count with 'service' key present: 180
count service==null: 175
with non-null service: ['emacs', 'httpd', 'jupyterlab', 'php@8.4', 'unbound']
union of service keys: ['environment_variables', 'error_log_path',
  'keep_alive', 'log_path', 'require_root', 'run', 'run_type', 'working_dir']
```

The five non-null blocks, verbatim from the JSON:

```json
emacs:    {"keep_alive": {"always": true},
           "run": ["/usr/local/opt/emacs/bin/emacs", "--fg-daemon"],
           "run_type": "immediate"}

httpd:    {"environment_variables": {"PATH": "/usr/local/bin:/usr/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin"},
           "run": ["/usr/local/opt/httpd/bin/httpd", "-D", "FOREGROUND"],
           "run_type": "immediate"}

jupyterlab: {"error_log_path": "/usr/local/var/log/jupyterlab.log",
           "keep_alive": {"always": true},
           "log_path": "/usr/local/var/log/jupyterlab.log",
           "run": ["/usr/local/opt/jupyterlab/bin/jupyter-lab"],
           "run_type": "immediate"}

php@8.4:  {"error_log_path": "/usr/local/var/log/php-fpm.log",
           "keep_alive": {"always": true},
           "run": ["/usr/local/opt/php@8.4/sbin/php-fpm", "--nodaemonize"],
           "run_type": "immediate",
           "working_dir": "/usr/local/var"}

unbound:  {"keep_alive": {"always": true},
           "require_root": true,
           "run": ["/usr/local/opt/unbound/sbin/unbound", "-d", "-c",
                   "/usr/local/etc/unbound/unbound.conf"],
           "run_type": "immediate"}
```

Note the paths are **already resolved to absolute prefix paths** in
`--json=v2`, and that `require_root` is present only on `unbound`.

### E-2 — Homebrew: the offline catalogue uses a *different* schema

The signed API catalogue Homebrew keeps on disk
(`~/Library/Caches/Homebrew/api/internal/packages.tahoe.jws.json.payload`, a
two-line JWS file: header line, then payload line) was parsed read-only:

```
metadata: {'homebrew_version': '6.0.21-28-gfd18738', 'bottle_tag': 'tahoe',
           'generated_at': 1788350020}
payload top keys: ['metadata', 'formulae', 'casks', 'formula_aliases',
  'formula_renames', 'cask_renames', 'formula_tap_git_head',
  'cask_tap_git_head', 'formula_tap_migrations', 'cask_tap_migrations']
formula count: 8586
records lacking a 'service' key entirely: 8586
service_run_args present: 353    service_args present: 356
```

i.e. **there is no `service` key in the offline catalogue at all.** The same
information is carried under `service_run_args` and `service_args`:

```
postgresql@17 run: [["$HOMEBREW_PREFIX/opt/postgresql@17/bin/postgres", "-D", "$HOMEBREW_PREFIX/var/postgresql@17"]]
              args: [[":environment_variables", {":LC_ALL": "en_US.UTF-8"}], [":run_type", ":immediate"],
                     [":working_dir", "$HOMEBREW_PREFIX"],
                     [":log_path", "$HOMEBREW_PREFIX/var/log/postgresql@17.log"],
                     [":error_log_path", "$HOMEBREW_PREFIX/var/log/postgresql@17.log"],
                     [":stop_timeout", 120], [":keep_alive", {":always": true}]]
mysql         run: [["$HOMEBREW_PREFIX/opt/mysql/bin/mysqld_safe", "--datadir=$HOMEBREW_PREFIX/var/mysql"]]
              args: [[":run_type", ":immediate"], [":working_dir", "$HOMEBREW_PREFIX/var/mysql"], [":keep_alive", {":always": true}]]
redis         run: [["$HOMEBREW_PREFIX/opt/redis/bin/redis-server", "$HOMEBREW_PREFIX/etc/redis.conf"]]
              args: [[":run_type", ":immediate"], [":working_dir", "$HOMEBREW_PREFIX/var"],
                     [":log_path", "$HOMEBREW_PREFIX/var/log/redis.log"],
                     [":error_log_path", "$HOMEBREW_PREFIX/var/log/redis.log"], [":keep_alive", {":always": true}]]
nginx         run: [["$HOMEBREW_PREFIX/opt/nginx/bin/nginx", "-g", "daemon off;"]]
              args: [[":run_type", ":immediate"], [":working_dir", "$HOMEBREW_PREFIX"], [":keep_alive", {":always": false}]]
unbound       args: [..., [":require_root", true], ...]
mongodb-community: NOT IN CORE CATALOG
```

Frequency of `service_args` keys across the 356 core formulae that have one:

```
{':run_type': 356, ':keep_alive': 281, ':error_log_path': 199, ':log_path': 191,
 ':working_dir': 173, ':require_root': 55, ':environment_variables': 52,
 ':interval': 8, ':process_type': 6, ':stop_timeout': 5, ':cron': 2,
 ':sockets': 2, ':input_path': 2, ':macos_legacy_timers': 1, ':nice': 1}

formulae overriding the launchd label (:name): 0
```

Paths in this schema are **unexpanded** — the literal token
`$HOMEBREW_PREFIX` appears, and must be substituted by the reader.

### E-3 — Homebrew: launchd label and file locations, from the shipped source

Homebrew's own source on this machine
(`/usr/local/Homebrew/Library/Homebrew/`, which is the checkout of
Homebrew/brew):

```
service.rb:81-87
    def default_plist_name
      "homebrew.mxcl.#{@formula.name}"
    end

    def default_service_name
      "homebrew.#{@formula.name}"
    end

formula.rb:1469-1481
  def plist_name = service.plist_name
  def service_name = service.service_name
  def launchd_service_path = (any_installed_prefix || opt_prefix)/"#{plist_name}.plist"
  def systemd_service_path = (any_installed_prefix || opt_prefix)/"#{service_name}.service"

services/system.rb:66-91
      def self.boot_path
          Pathname.new("/Library/LaunchDaemons")
      def self.user_path
          Pathname.new("#{Dir.home}/Library/LaunchAgents")
      # If root, return `boot_path`, else return `user_path`.
        root? ? boot_path : user_path

services/formula_wrapper.rb (dest_dir / dest)
      def dest_dir
        System.root? ? System.boot_path : System.user_path
      end
      def dest
        dest_dir + service_file.basename
      end

services/formula_wrapper.rb:390
        /homebrew(?>\.mxcl)?\.([\w+-.@]+)(\.plist|\.service)?\z/
```

`docs.brew.sh/Manpage`, `services` section, read 2026-09-02:

> "Manage background services with macOS' `launchctl`(1) daemon manager or
> Linux's `systemctl`(1) service manager."
>
> "If `sudo` is passed, operate on `/Library/LaunchDaemons` or
> `/usr/lib/systemd/system` (started at boot). Otherwise, operate on
> `~/Library/LaunchAgents` or `~/.config/systemd/user` (started at login)."

Subcommands listed there: `list`, `info`, `run`, `start`, `stop`, `kill`,
`restart`, `cleanup`; flags include `--all`, `--json`, `--file`,
`--sudo-service-user`.

**Correction to the brief:** the `Homebrew/homebrew-services` README is no
longer a source. Read 2026-09-02
(<https://raw.githubusercontent.com/Homebrew/homebrew-services/master/README.md>),
it says only that `brew services` has been moved into the main `Homebrew/brew`
repository and "still works as before". It contains no path, naming or
subcommand documentation. The authoritative text is `docs.brew.sh/Manpage`
plus `Library/Homebrew/services/` in `Homebrew/brew`.

### E-4 — Homebrew: the plist exists in the keg; nothing is loaded on this machine

```
$ find /usr/local/Cellar -maxdepth 3 -name "homebrew.mxcl.*.plist"
/usr/local/Cellar/emacs/31.1/homebrew.mxcl.emacs.plist
/usr/local/Cellar/php@8.4/8.4.25/homebrew.mxcl.php@8.4.plist
/usr/local/Cellar/php@8.4/8.4.24/homebrew.mxcl.php@8.4.plist
/usr/local/Cellar/httpd/2.4.68/homebrew.mxcl.httpd.plist
/usr/local/Cellar/jupyterlab/4.6.3/homebrew.mxcl.jupyterlab.plist
/usr/local/Cellar/unbound/1.26.0/homebrew.mxcl.unbound.plist

$ ls /usr/local/opt/emacs/*.plist /usr/local/opt/httpd/*.plist /usr/local/opt/unbound/*.plist
/usr/local/opt/emacs/homebrew.mxcl.emacs.plist
/usr/local/opt/httpd/homebrew.mxcl.httpd.plist
/usr/local/opt/unbound/homebrew.mxcl.unbound.plist
```

Exactly the five formulae of E-1 (php@8.4 twice, two installed versions).

```
$ ls ~/Library/LaunchAgents | grep -i homebrew
(no output)

$ ls ~/Library/LaunchAgents
ai.perplexity.CometUpdater.wake.plist
com.DigiDNA.iMazing2Mac.Mini.plist
com.adobe.GC.Invoker-1.0.plist
com.google.GoogleUpdater.wake.plist
com.jetbrains.toolbox.plist
com.macpaw.CleanMyMac5.Updater.plist
com.microsoft.EdgeUpdater.wake.plist
com.user.claude-litellm-cleanup.plist

$ ls /Library/LaunchDaemons /Library/LaunchAgents | grep -iE "homebrew|macports"
(none)
```

No `brew services` command was run — `brew services *` is denied by
`.claude/settings.json` and by INV-19's hook. The evidence above is entirely
filesystem-level.

### E-5 — A Homebrew cask can plant launch machinery under a *vendor* label

```
$ ls /Library/LaunchDaemons | grep -i wireshark
org.wireshark.ChmodBPF.plist

$ python3 -c "import plistlib; d=plistlib.load(open('/Library/LaunchDaemons/org.wireshark.ChmodBPF.plist','rb')); print(...)"
{'Label': 'org.wireshark.ChmodBPF', 'ProgramArguments': None, 'RunAtLoad': True, 'KeepAlive': None}

$ brew list --cask
adoptopenjdk copilot-cli gcloud-cli gimp meld oracle-jdk visualdiffer wailbrew wireshark-chmodbpf
```

The offline catalogue's record for that cask (`raw_artifacts`) is:

```json
[[":uninstall", {":early_script": {":executable": "/usr/sbin/installer",
   ":args": ["-pkg", "$HOMEBREW_PREFIX/Caskroom/wireshark-chmodbpf/4.6.8/Uninstall ChmodBPF.pkg",
             "-target", "/"], ":sudo": true, ":must_succeed": false},
   ":pkgutil": "org.wireshark.ChmodBPF.pkg"}],
 [":pkg", ["Install ChmodBPF.pkg"]]]
```

Catalogue-wide counts (offline, derived):

```
casks in catalog: 7720
casks whose raw_artifacts name the launchd control tool: 431
casks whose raw_artifacts mention :pkg: 687
wireshark-chmodbpf in that set: False
```

And in `brew info --json=v2`, casks have **no** `uninstall` or `zap` key at
all — observed top-level cask keys:

```
['artifacts', 'auto_updates', 'autobump', 'bundle_short_version',
 'bundle_version', 'caveats', 'caveats_rosetta', 'conflicts_with',
 'container', 'depends_on', 'deprecate_args', 'deprecated',
 'deprecation_date', 'deprecation_reason', 'deprecation_replacement_cask',
 'deprecation_replacement_formula', 'desc', 'disable_args', 'disable_date',
 'disable_reason', 'disable_replacement_cask', 'disable_replacement_formula',
 'disabled', 'full_token', 'homepage', 'installed', 'installed_time',
 'languages', 'name', 'no_autobump_message', 'old_tokens', 'outdated',
 'pinned', 'pinned_version', 'rename', 'ruby_source_checksum',
 'ruby_source_path', 'sha256', 'skip_livecheck', 'tap', 'tap_git_head',
 'token', 'url', 'url_specs', 'version']
```

### E-6 — Homebrew: where stateful data lives

Caveats of installed formulae (`brew info --json=v2 --installed`, `caveats`
field): 26 of 180 have caveats; exactly one mentions a `var` path:

```
=== httpd ===
DocumentRoot is $HOMEBREW_PREFIX/var/www.

The default ports have been set in $HOMEBREW_PREFIX/etc/httpd/httpd.conf to 8080 and in
$HOMEBREW_PREFIX/etc/httpd/extra/httpd-ssl.conf to 8443 so that httpd can run without sudo.
```

From the local `homebrew/core` checkout
(`/usr/local/Homebrew/Library/Taps/homebrew/homebrew-core/Formula/`, revision
`e045d7385fd`, 2026-09-02), verbatim:

```ruby
# Formula/p/postgresql@17.rb
  def postgresql_datadir
    var/name
  end
  def postgresql_log_path
    var/"log/#{name}.log"
  end
  def caveats
    <<~EOS
      This formula has created a default database cluster with:
        initdb --locale=en_US.UTF-8 -E UTF-8 #{postgresql_datadir}
      When uninstalling, some dead symlinks are left behind so you may want to run:
        brew cleanup --prune-prefix
    EOS
  end

# Formula/m/mysql.rb
  service do
    run [opt_bin/"mysqld_safe", "--datadir=#{var}/mysql"]
    keep_alive true
    working_dir var/"mysql"
  end

# Formula/m/mariadb.rb
  service do
    run [opt_bin/"mariadbd-safe", "--datadir=#{var}/mysql"]
    keep_alive true
    working_dir var
  end

# Formula/r/redis.rb
  %w[run db/redis log].each { |p| (var/p).mkpath }
  s.gsub! "dir ./", "dir #{var}/db/redis/"
  service do
    run [opt_bin/"redis-server", etc/"redis.conf"]
    keep_alive true
    error_log_path var/"log/redis.log"
    log_path var/"log/redis.log"
    working_dir var
  end

# Formula/v/valkey.rb  — same shape, var/db/valkey
# Formula/c/couchdb.rb
  (var/"couchdb/data").mkpath
  inreplace "rel/couchdb/etc/default.ini", "./data", "#{var}/couchdb/data"

# Formula/n/nginx.rb
  --error-log-path=#{var}/log/nginx/error.log
  --http-log-path=#{var}/log/nginx/access.log
  --pid-path=#{var}/run/nginx.pid
  Docroot is: #{var}/www

# Formula/i/influxdb.rb   "--data-dir", var/"lib/influxdb"
# Formula/g/grafana.rb    cfg:default.paths.data=#{var}/lib/grafana
# Formula/p/prometheus.rb --storage.tsdb.path #{var}/prometheus
# Formula/r/rabbitmq.rb   (var/"lib/rabbitmq").mkpath
```

Catalogue-wide derivation (offline): formulae whose `service_run_args`,
`service_args` or `caveats` name a `$HOMEBREW_PREFIX/var/<x>` path that is
**not** `log/…` or `run/…` — 86 of 8586:

```
arcadedb autobrr caddy cassandra cayley cc-connect derby djl-serving dolt
ejabberd eslint_d forgejo fourstore gitea gitea-runner gpsd grafana
grafana-alloy hbase httpd i2pd immudb influxdb ircd-hybrid ircd-irc2 lighttpd
lunarr mariadb mariadb@10.11 mariadb@10.5 mariadb@10.6 mariadb@11.4
mariadb@11.8 meilisearch metabase minio mosquitto mycorrhiza mysql mysql@8.0
mysql@8.4 mysql@9.7 neo4j nginx node-red nodebrew nsq nuvie opensearch
opensearch-dashboards opentsdb orientdb percona-server percona-server@8.0
pgbadger pinot pocket-id postgresql@12 postgresql@13 postgresql@14
postgresql@15 postgresql@16 postgresql@17 postgresql@18 questdb recc runit ry
sdlpop seaweedfs sftpgo sickchill sing-box solr solr@8.11 tika tile38 tinyice
transmission-cli tronbyt-server typedb varnish victorialogs victoriametrics
vulcain zeroclaw
```

Explicit misses of that heuristic, checked individually:

```
redis     run args reference only etc/redis.conf ; caveats mention var: False
valkey    run args reference only etc/valkey.conf; caveats mention var: False
couchdb   run args: ["$HOMEBREW_PREFIX/opt/couchdb/bin/couchdb"]; caveats mention var: False
rabbitmq  caveats mention var: False
prometheus caveats mention var: False
```

Their data directories are set by `inreplace`/`mkpath` in the formula body
(E-6, `redis.rb`, `valkey.rb`, `couchdb.rb`) and are therefore **not**
recoverable from provider metadata at all — only from the formula Ruby source.

Existing prefix state on this machine:

```
$ ls -1 /usr/local/var
cache
db
homebrew
locate
log
run
www
$ ls -1 /usr/local/var/log
httpd
php-fpm.log
```

### E-7 — MacPorts: StartupItems (cited only; MacPorts is not installed)

```
$ ls -d /opt/local
ls: /opt/local: No such file or directory
$ command -v port
port: not found
```

`https://guide.macports.org/chunked/reference.startupitems.html`, read
2026-09-02:

> "A StartupItem is a MacPorts facility to run 'daemons,' a Unix term for
> programs that run continuously in the background, rather than under the
> direct control of a user; for example, mail servers, network listeners, etc.
> Ports that use StartupItem keywords create scripts for launchd, which is the
> Apple facility introduced with Mac OS X 10.4 to replace xinetd for starting
> and managing daemons. To support launchd, a program named daemondo is
> provided by MacPorts base that serves as an adapter between launchd and
> daemons ('executable' StartupItems) or traditional Unix startup scripts that
> start daemons ('script' StartupItems)."
>
> "During port installation a MacPorts StartupItem creates a .plist file in
> `${prefix}/etc/LaunchDaemons/`" … "places a symbolic link to the .plist file
> within `/Library/LaunchDaemons/` if `${startupitem.install}` is true."
>
> `startupitem.create` — "Trigger the creation of a StartupItem. Default: no"
>
> `startupitem.install` — "Whether to install a link to the StartupItem in the
> appropriate subdirectory of /Library (see startupitem.location) so that it
> can be launched automatically after rebooting. Default: yes"
>
> `startupitem.location` — "Chooses the subdirectory in which to install the
> StartupItem. Also affects how it will be loaded: LaunchDaemons must be
> loaded as root, and only one instance will run for the whole system.
> LaunchAgents are loaded as a normal user, and one instance per user can run."

The same page gives the full path pair as
`${prefix}/etc/LaunchDaemons/org.macports.<name>/org.macports.<name>.plist`
and `/Library/LaunchDaemons/org.macports.<name>.plist` (symlink).

`macports-base`, `src/port1.0/portstartupitem.tcl`, read 2026-09-02 via
`https://raw.githubusercontent.com/macports/macports-base/master/src/port1.0/portstartupitem.tcl`:

```tcl
options startupitems startupitem.autostart startupitem.debug \
        startupitem.create startupitem.custom_file \
        startupitem.executable startupitem.group \
        startupitem.init startupitem.install startupitem.location \
        startupitem.logevents startupitem.logfile \
        startupitem.logfile.stderr startupitem.name \
        startupitem.netchange startupitem.pidfile startupitem.plist \
        startupitem.requires startupitem.restart startupitem.start \
        startupitem.stop startupitem.type startupitem.uniquename \
        startupitem.user startupitem.daemondo.verbosity

default startupitem.autostart   no
default startupitem.create      {[portstartupitem::get_startupitem_type]}
default startupitem.install     {$system_options(startupitem_install)}
default startupitem.location    LaunchDaemons
default startupitem.name        {${subport}}
default startupitem.uniquename  {org.macports.${startupitem.name}}
default startupitem.plist       {${startupitem.uniquename}.plist}
```

The user note it generates references `"sudo port load ${subport}"`.

`macports-base`, `doc/port.1`, read 2026-09-02 via
`https://raw.githubusercontent.com/macports/macports-base/master/doc/port.1`:

```roff
.PP
load
.RS 4
Provides a shortcut to using launchctl to load a port\(cqs daemon (as installed in /Library/LaunchDaemons)\&. It runs:
.sp
launchctl load \-w /Library/LaunchDaemons/org\&.macports\&.${port}\&.plist
.RE
.PP
unload
.RS 4
A shortcut to launchctl, like load, but unloads the daemon\&.
.RE
```

### E-8 — MacPorts: there is no `port info --startupitem`

`doc/port.1` (read 2026-09-02) documents the `info` action's field selection
as: "Specific meta-information may be requested through an option such as
`--maintainer` or `--category`" plus `--line`, `--pretty`, `--index`, and
"Recognized field names are those from the PortIndex, see 'port help info' for
a complete list."

`macports-base`, `src/port/port.tcl` (read 2026-09-02): `action_info`
iterates over options with the `ports_info_` prefix; the default set is

```tcl
set opts_todo {
    ports_info_heading ports_info_replaced_by ports_info_subports
    ports_info_variants ports_info_skip_line
    ports_info_long_description ports_info_homepage
    ports_info_skip_line ports_info_depends_fetch ports_info_depends_extract
    ports_info_depends_patch ports_info_depends_build
    ports_info_depends_lib ports_info_depends_run ports_info_depends_test
    ports_info_conflicts ports_info_platforms ports_info_license
    ports_info_maintainers
}
```

and the source data is chosen by `ports_info_index`:

```tcl
set index_only 0
if {[dict exists $options ports_info_index] && [dict get $options ports_info_index]} {
    set index_only 1
}
...
if {!$index_only} { ... mportopen $porturl $options $merged_variations ... }
```

The string `startupitem` does not appear in `src/port/port.tcl`.

Read-only actions available instead, `doc/port.1` (read 2026-09-02):

> `contents` — "Lists the files installed by *portname*."
> `notes` — "Displays notes for *portname* which usually contain useful
> information concerning setup and use of the port."
> `installed` — "Show the installed version, variants and activation status
> for each *portname*. If no arguments are given, all installed ports are
> displayed."
> `file` — "Displays the path to the Portfile for *portname*."
> `cat` — "Concatenates and prints the contents of *Portfile* on stdout."

### E-9 — MacPorts: stateful data paths, from Portfiles

All read 2026-09-02 from `https://raw.githubusercontent.com/macports/macports-ports/master/…`:

`databases/postgresql17/Portfile` — no `startupitem` lines; its notes say
"To use the postgresql server as a startup item install the ${name}-server
port." Only `--datadir=${prefix}/share/${name}` (shared files, not a cluster).

`databases/postgresql17-server/Portfile`:

```tcl
set dbdir           ${prefix}/var/db/${rname}/defaultdb
set dbhome          ${prefix}/var/db/${rname}
startupitem.create  yes
startupitem.init    "PGCTL=${libdir}/bin/pg_ctl"
startupitem.start   "sudo -u ${dbuser} /bin/sh -c \"\${PGCTL} -D \${POSTGRESQL17DATA:=${dbdir}}..."
startupitem.stop    "sudo -u ${dbuser} /bin/sh -c \"\${PGCTL} -D \${POSTGRESQL17DATA:=${dbdir}} stop..."
notes "\nTo create a database instance, after install do\n\
        sudo port select postgresql ${rname}\n\
        sudo mkdir -p ${dbdir}\n\
        sudo chown ${dbuser}:${dbgrp} ${dbdir}\n\
        sudo -u ${dbuser} /bin/sh -c \'cd ${dbhome} && ${libdir}/bin/initdb -D ${dbdir}\' "
```

`databases/mysql8/Portfile` (startupitem in the `mysql8-server` subport):

```tcl
startupitem.create      yes
startupitem.executable  ${prefix}/lib/${name_mysql}/bin/mysqld
startupitem.user        ${mysqluser}
startupitem.group       ${mysqluser}
startupitem.logfile     ${log_file}
-DMYSQL_DATADIR:PATH="${prefix}/var/db/${name_mysql}"
${destroot}${prefix}/var/db/${name_mysql} \
${destroot}${prefix}/var/db/${name_mysql}-files \
${destroot}${prefix}/var/db/${name_mysql}-keyring
"Your databases in ${prefix}/var/db/${name_mysql} are left untouched by an upgrade."
```

`databases/redis/Portfile`:

```tcl
startupitem.create      yes
startupitem.executable  ${prefix}/bin/redis-server ${prefix}/etc/redis.conf
destroot.keepdirs   ${destroot}${prefix}/var/db/redis
xinstall -d ${prefix}/var/log
touch ${prefix}/var/log/redis.log
```

`www/nginx/Portfile`:

```tcl
startupitem.create      yes
startupitem.pidfile     auto ${nginx_pidfile}
startupitem.executable  ${prefix}/sbin/nginx -g "daemon off;"
set nginx_logdir    ${prefix}/var/log/${name}
set nginx_rundir    ${prefix}/var/run/${name}
```

### E-10 — Runtimes on this machine

```
$ brew list --formula | grep -E '^(python|node|ruby|go|rust|openjdk|php|perl|lua|llvm|mise|nvm|pyenv|rbenv|uv|pipx|phpbrew)'
go-task llvm lua mise node nvm openjdk openjdk@21 php@8.4 phpbrew pipx
python@3.12 python@3.13 python@3.14 uv uvwasi
```

(`go-task`, `uvwasi` are false positives of the prefix match; `ruby`, `go`,
`rust` are not installed via Homebrew here.)

From the installed JSON:

```
llvm        aliases ['llvm@23']              versioned_formulae ['llvm@22','llvm@21',...,'llvm@14']  keg_only True  reason :provided_by_macos
node        aliases ['node.js','node@26','nodejs','npm']  versioned_formulae ['node@24','node@22','node@20','node@18']  keg_only False
openjdk     aliases ['java','openjdk@26']    versioned_formulae ['openjdk@25','openjdk@21','openjdk@17','openjdk@11','openjdk@8']  keg_only True  reason :shadowed_by_macos
openjdk@21  aliases []                       versioned_formulae ['openjdk@25','openjdk@17','openjdk@11','openjdk@8']  keg_only True  reason :versioned_formula
php@8.4     aliases []                       versioned_formulae ['php@8.3','php@8.2','php@8.1']  keg_only True  reason :versioned_formula
python@3.13 aliases []                       versioned_formulae ['python@3.14','python@3.12','python@3.11','python@3.10','python@3.9']  keg_only False
```

Note `php@8.4` appears in **both** E-1 (has a `service` block) and here.

## Findings

1. **Homebrew `SERVICE` detection, online-or-offline, two independent signals.**
   (a) `brew info --json=v2` always emits a `service` key for every formula;
   it is `null` for non-service formulae (E-1: 180/180 have the key, 175 are
   null). The detection rule is therefore **`service != null`**, *not*
   "`service` key present". A rule written as "has a `service` key" matches
   every formula and is a bug. (b) Independently, Homebrew writes
   `homebrew.mxcl.<name>.plist` into the keg at install time; its presence
   under `<Cellar>/<name>/<version>/` is a pure-filesystem signal that agreed
   exactly with (a) on this machine (E-4).

2. **The `service` block's shape** (E-1, E-2) is: `run` (string or array of
   strings, absolute paths in `--json=v2`), `run_type`, `keep_alive` (an
   object, e.g. `{"always": true}` — not a bare boolean in JSON), `working_dir`,
   `log_path`, `error_log_path`, `environment_variables`, `require_root`, and
   less commonly `interval`, `process_type`, `stop_timeout`, `cron`, `sockets`,
   `input_path`, `macos_legacy_timers`, `nice`. 353 of 8586 core formulae
   (4.1%) define a service.

3. **`require_root` is the LaunchDaemon-vs-LaunchAgent determinant, and it is
   in the metadata** (E-1 `unbound`; E-2, 55 core formulae). Combined with the
   manpage (E-3), the destination is `/Library/LaunchDaemons/` when the service
   is started with `sudo` and `~/Library/LaunchAgents/` otherwise; Homebrew's
   own `dest_dir` is `System.root? ? boot_path : user_path`. `require_root` is
   a statement about the *formula's* requirement, not a guarantee about where a
   given user's plist actually landed.

4. **The launchd label is `homebrew.mxcl.<formula name>` by default and is
   overridable** (E-3, `default_plist_name`; the DSL exposes
   `name macos: "…"`). Across the whole core catalogue **zero** formulae
   override it today (E-2). Homebrew's own matcher accepts both
   `homebrew.mxcl.X` and `homebrew.X`
   (`/homebrew(?>\.mxcl)?\.([\w+-.@]+)(\.plist|\.service)?\z/`, E-3); a
   detector scanning launch directories should use the same two-form pattern
   rather than assuming `mxcl`.

5. **Started-state is out of scope and stays out.** Whether a service is
   *loaded* is a launchd fact, readable only through `brew services list` or
   `launchctl`, both of which this project refuses. What *is* readable without
   either is the presence of a matching plist in `~/Library/LaunchAgents` or
   `/Library/LaunchDaemons` (E-4). That is "a plist has been installed", which
   is sufficient for INV-19 (refuse to migrate) and is **not** the same as
   "currently running". The distinction must be surfaced in the UI wording.
   On this machine: five formulae define services, none has an installed plist
   in either launch directory (E-4).

6. **The offline catalogue carries the service data under a different
   schema** (E-2). `packages.<tag>.jws.json.payload` has no `service` key at
   all; it has `service_run_args` and `service_args` (a list of
   `[":symbol", value]` pairs, Ruby-symbol-prefixed), with `$HOMEBREW_PREFIX`
   left unexpanded. **Consequence for INV-15:** service detection *is*
   answerable offline for the whole catalogue, but only by a second parser
   targeting this internal schema. A snapshot must therefore capture the
   *normalised* service record, not the raw bytes of whichever source produced
   it; and the internal schema is undocumented, so it must be treated as
   unstable (see "Re-check when").

7. **Homebrew `STATEFUL` cannot be detected reliably from provider metadata.**
   The best metadata-only heuristic — a `$HOMEBREW_PREFIX/var/<x>` path in
   `service_run_args`, `service_args` or `caveats`, excluding `log/` and
   `run/` — yields 86 of 8586 formulae (E-6) and **misses redis, valkey,
   couchdb, rabbitmq and prometheus**, all of which are unambiguously stateful.
   Their data directories are established by `inreplace` and `mkpath` calls in
   the formula Ruby body, which no JSON output exposes. Only 1 of 180 installed
   formulae here had a `var` path in its caveats (E-6). Conclusion: caveats
   text is a weak signal, service arguments are a moderate one, and neither is
   sufficient.

8. **Recommended split for `STATEFUL`** (INV-19 + INV-20):
   - **Curated policy data (primary).** A dated, sourced list of
     `(provider, package) -> data paths, rationale, source URL, read date`.
     This is the authority. INV-19's refusal fires on a curated hit.
   - **Metadata heuristic (secondary, additive only).** The E-6 rule plus
     "a service block whose `working_dir` is under `<prefix>/var` and is not
     `var`, `var/log` or `var/run`". A hit that is *not* in the curated list
     produces `STATEFUL` with lower confidence and a "not curated" marker —
     it must never *remove* a curated classification.
   - **Filesystem corroboration (tertiary, evidence only).** Existence of
     `<prefix>/var/<name>` or `<prefix>/var/db/<name>` with non-trivial
     content. This raises confidence and supplies the concrete path to warn
     about; it must never be the sole basis, because an empty directory
     survives an uninstall and a populated one may predate the provider.

9. **Recommended seed for the curated stateful list**, every entry traceable
   to E-6 or E-9. Homebrew paths are relative to `brew --prefix`; MacPorts to
   `${prefix}` (`/opt/local` by default).

   | Package (family) | Homebrew formula → data path | MacPorts port → data path |
   |---|---|---|
   | PostgreSQL | `postgresql@NN` → `var/postgresql@NN` (cluster; `var/log/postgresql@NN.log`) | `postgresqlNN-server` → `var/db/postgresqlNN/defaultdb` (the *client* port `postgresqlNN` has no startupitem) |
   | MySQL | `mysql`, `mysql@8.0/8.4/9.7`, `percona-server*` → `var/mysql` | `mysqlN-server` → `var/db/mysqlN`, plus `-files`, `-keyring` |
   | MariaDB | `mariadb`, `mariadb@*` → `var/mysql` | `mariadb-*-server` → `var/db/mariadb-*` (**not verified — see Cannot determine**) |
   | Redis | `redis` → `var/db/redis` (from `redis.conf` `dir`), `var/log/redis.log` | `redis` → `var/db/redis`, `var/log/redis.log` |
   | Valkey | `valkey` → `var/db/valkey` | `valkey` (**not verified**) |
   | MongoDB | not in homebrew/core (E-2); `mongodb/brew` tap → `THIRD_PARTY_SOURCED` | (**not verified**) |
   | nginx | `nginx` → `var/www` (docroot), `etc/nginx/`, `var/log/nginx/` | `nginx` → `var/log/nginx`, `var/run/nginx`, `etc/nginx/` |
   | httpd | `httpd` → `var/www`, `etc/httpd/`, `var/cache/httpd` | (**not verified**) |
   | CouchDB | `couchdb` → `var/couchdb/data` | (**not verified**) |
   | RabbitMQ | `rabbitmq` → `var/lib/rabbitmq`, `var/log/rabbitmq` | (**not verified**) |
   | InfluxDB | `influxdb` → `var/lib/influxdb` | (**not verified**) |
   | Grafana | `grafana` → `var/lib/grafana` (incl. `plugins`) | (**not verified**) |
   | Prometheus | `prometheus` → `var/prometheus` (TSDB) | (**not verified**) |
   | OpenSearch | `opensearch` → `var/lib/opensearch`, `var/opensearch/plugins` | (**not verified**) |
   | Cassandra | `cassandra` → `var/lib/cassandra` | (**not verified**) |
   | Solr | `solr`, `solr@8.11` → `var/lib/solr` | (**not verified**) |
   | Gitea / Forgejo | `gitea` → `var/gitea`; `forgejo` → `var/forgejo` | (**not verified**) |
   | MinIO | `minio` → `var/minio` | (**not verified**) |
   | Neo4j | `neo4j` → `var/neo4j` | (**not verified**) |
   | Meilisearch | `meilisearch` → `var/meilisearch/data.ms` | (**not verified**) |
   | Varnish | `varnish` → `var/varnish` | (**not verified**) |
   | Transmission | `transmission-cli` → `var/transmission` | (**not verified**) |

   The remaining names in the 86-strong E-6 list are legitimate second-round
   candidates; they are omitted from the seed only because their MacPorts
   counterpart was not checked. The seed's own generation command (the E-6
   derivation) should ship alongside it so the list is regenerable.

10. **MacPorts `SERVICE` detection.** There is **no `port info --startupitem`**
    (E-8: `startupitem` does not appear in `src/port/port.tcl`; the `info`
    field flags map to `ports_info_*` names drawn from the PortIndex). The
    available read-only mechanisms, in descending order of directness:
    - **For an installed port:** `port -q contents <name>` and look for
      `${prefix}/etc/LaunchDaemons/org.macports.<name>/org.macports.<name>.plist`
      (E-7, E-8). Equivalently, list `${prefix}/etc/LaunchDaemons/` directly —
      no `port` invocation needed, and this works with no network.
    - **For an installed *or* available port:** `port notes <name>`; the
      generated startupitem note contains `sudo port load <subport>` (E-7).
      Weaker (a maintainer's hand-written note could also say that) and needs
      the ports tree.
    - **For an available port:** `port cat <name>` / `port file <name>` then
      match `^\s*startupitem\.create\s+yes` in the Portfile (E-7, E-8). Needs
      a synced ports tree; `port sync`/`selfupdate` are forbidden, so this is
      "works if the tree is already there".
    - **The launch-machinery signal:** `/Library/LaunchDaemons/org.macports.<name>.plist`
      (or `/Library/LaunchAgents/...` when `startupitem.location LaunchAgents`)
      — a symlink into the prefix, present when `startupitem.install` is true,
      which is the default (E-7).

11. **The MacPorts label is `org.macports.${startupitem.name}` where
    `startupitem.name` defaults to `${subport}`** (E-7). Because it defaults to
    the *subport* and can be set explicitly, the plist basename is not reliably
    the port name the user typed. `port contents` is authoritative;
    filename-pattern matching is a fallback.

12. **The service/data split differs structurally between the providers, and
    that is exactly why INV-19 exists.** Homebrew ships one `postgresql@17`
    formula that both provides the binaries and creates a cluster in
    `var/postgresql@17`. MacPorts splits it: `postgresql17` (no startupitem, no
    cluster) and `postgresql17-server` (startupitem, cluster at
    `var/db/postgresql17/defaultdb`, created *manually* by the user following
    the port's notes) (E-9). So `postgresql@17` → MacPorts is a **1→2** mapping
    where one of the two targets performs no data initialisation at all. Data
    paths also disagree in *shape*: `var/mysql` vs `var/db/mysql8`,
    `var/postgresql@17` vs `var/db/postgresql17/defaultdb`. No automatic
    migration is possible from this metadata, which is the finding INV-19
    encodes.

13. **`RUNTIME` detection under Homebrew is metadata-driven and offline**
    (E-10): `versioned_formulae` (non-empty ⇒ the formula heads a versioned
    family), `aliases` (e.g. `node` aliased `node@26`, `openjdk` aliased
    `java`/`openjdk@26`), and `keg_only_reason.reason == ":versioned_formula"`
    (⇒ this formula *is* a versioned family member). This is a better rule than
    matching `@` in the name, because it catches unversioned family heads
    (`node`, `openjdk`, `llvm`) and excludes `@`-named non-runtimes
    (`icu4c@78`, `openssl@3`). Homebrew's own keg-only reasons
    `:provided_by_macos` / `:shadowed_by_macos` additionally flag runtimes that
    duplicate an Apple-supplied one.

14. **Classes are not mutually exclusive.** Observed on this machine (E-1,
    E-10): `php@8.4` is simultaneously a `RUNTIME` (versioned language family,
    `keg_only_reason :versioned_formula`) and a `SERVICE` (php-fpm service
    block, plist in the keg). From E-6, `postgresql@17` is simultaneously
    `STATEFUL` and `SERVICE`; `httpd` is `STATEFUL` and `SERVICE`; a
    third-party-tapped `mongodb-community` would be `THIRD_PARTY_SOURCED`,
    `STATEFUL` and `SERVICE` at once. The domain model's table reads as one
    class per package.

15. **Casks can install launch machinery that no cask metadata names**
    (E-5). `wireshark-chmodbpf` is installed here and
    `/Library/LaunchDaemons/org.wireshark.ChmodBPF.plist` exists with label
    `org.wireshark.ChmodBPF`, yet that cask's `raw_artifacts` do not mention
    the launchd control tool at all — the daemon arrives inside an embedded
    `.pkg`. Catalogue-wide, 431 of 7720 casks *do* name launchd labels in their
    uninstall stanza and 687 install a `.pkg`. Separately,
    `brew info --json=v2` exposes **no** `uninstall`/`zap` key for casks at
    all, so even the 431 are invisible to the v2 JSON and must be read from the
    offline catalogue's `raw_artifacts` or the cask Ruby source. This reinforces
    `02-domain-model.md`'s existing rule that casks are reported, not migrated:
    the tool cannot enumerate what a cask has planted outside the Caskroom.

16. **Offline vs network (load-bearing for INV-15):** everything in findings
    1–15 was obtained with no network refresh. The Homebrew API cache mtimes
    were unchanged by the `brew info --json=v2 --installed` run (E-0), and the
    per-formula Ruby reads came from the already-present `homebrew/core`
    checkout. Service detection, runtime-family detection and the stateful
    heuristic are all offline-capable. What is **not** offline-capable: reading
    a formula's Ruby body for a package that is neither installed nor in a
    local tap, and reading a MacPorts Portfile with no synced ports tree.

## Cannot determine

- `UNRESOLVED` — **MacPorts behaviour on a real install.** Everything in E-7
  to E-9 is cited, not observed: MacPorts is not installed on this machine.
  Unverified in particular: the exact `port -q contents` output format for a
  startupitem port; whether `${prefix}/etc/LaunchDaemons/` exists when no
  startupitem port is installed; whether the `/Library/LaunchDaemons` entry is
  a symlink or a copy in practice; the actual default of
  `startupitem_install` in a shipped `macports.conf`. *A future attempt needs
  a machine with MacPorts installed and at least one startupitem port
  (`redis` or `nginx` are cheap).*
- `UNRESOLVED` — **MacPorts data paths for most of the seed list.** Only
  postgresql17-server, mysql8, redis and nginx Portfiles were read (E-9). The
  entries marked "not verified" in Finding 9 have no MacPorts-side citation.
  *A future attempt needs the corresponding Portfiles from
  `github.com/macports/macports-ports`; each is one fetch.*
- `UNRESOLVED` — **Stability of the offline catalogue's internal schema.**
  `service_run_args` / `service_args` (E-2) appear in
  `~/Library/Caches/Homebrew/api/internal/packages.<tag>.jws.json.payload`,
  which is a cache implementation detail with no documentation at
  `docs.brew.sh` that I could find. *A future attempt needs either a
  `docs.brew.sh` page documenting it, or a decision to derive the offline path
  from `brew info --json=v2` output captured into a snapshot instead of from
  the cache file.*
- Whether a formula anywhere (including third-party taps) overrides the
  launchd label via `name macos:`. Zero in homebrew/core today (E-2); taps not
  surveyed.
- Whether `port select` groups are the right MacPorts analogue of Homebrew's
  `versioned_formulae` for `RUNTIME` detection. The MacPorts guide page I
  fetched (`using.common-tasks.html`) does not cover `port select`; I did not
  locate the correct page within this note's budget. Finding 13 is therefore
  Homebrew-only.
- `mongodb-community` is absent from the core catalogue (E-2) — confirming it
  is tap-sourced — but the `mongodb/brew` tap itself was not inspected, so its
  data path is uncited.

## Impact on the specification

- **Confirms INV-19.** Finding 12 is direct evidence for the invariant's stated
  rationale: the same database has a different data path, a different
  package split and a different initialisation story under each provider.
  Nothing here weakens it.
- **Confirms `02-domain-model.md` (Package classes)** for `SERVICE` and
  `RUNTIME`: both are detectable from provider metadata, offline, under
  Homebrew (Findings 1, 13). No change needed to their definitions.
- **Contradicts `02-domain-model.md` (Package classes), single-class table.**
  Finding 14: `php@8.4` on this machine is both `RUNTIME` and `SERVICE`;
  `postgresql@17` is both `STATEFUL` and `SERVICE`. The table reads as a
  one-of. The model needs either a *set* of classes per package or an explicit,
  documented precedence (with `MANUAL_REVIEW`-producing classes dominating, so
  that classifying `php@8.4` as `RUNTIME` cannot suppress the INV-19 refusal
  its service block requires). **Reported, not edited** — this is a Tier 3
  change and belongs in `docs/spec-evolution.md`.
- **Qualifies `02-domain-model.md` (Snapshot).** Finding 6: the same fact
  (a formula's service definition) has two incompatible on-disk schemas
  depending on the source. The snapshot must store the normalised record and
  name which source produced it, or `analyze --from-snapshot` will disagree
  with a live run.
- **Supports `02-domain-model.md`'s existing cask rule** with new evidence
  (Finding 15) — casks may install launch daemons the metadata does not
  mention, so "reported, not migrated" should be stated as a *safety*
  property, not only a mapping one.
- **Feeds INV-20 policy data.** Finding 9 is the proposed seed for a
  `stateful-packages` data file; Finding 8 is the proposed detection/curation
  split. Each row needs its source URL and read date carried in the data file
  itself.
- **Correction to the research brief, not to the spec:** the
  `Homebrew/homebrew-services` README is no longer a usable source (E-3). Any
  future note or spec text pointing at it should point at `docs.brew.sh/Manpage`
  and `Homebrew/brew`'s `Library/Homebrew/services/` instead.

## Addendum, 2026-10-10 — the startup-item location, observed (pass 08 slice 3)

MacPorts 2.12.6 is now installed on the machine this note was written on, so
part of E-7 moves from cited to read in source. All read-only, 2026-10-10.

1. **Where an item lives.** `port1.0/portstartupitem.tcl` (sha256
   `cc4d2c99b1e303d4677f9d562f8c3a9f076bea2d8e28263f099b805f0ee592b3`): the item
   directory is `${prefix}/etc/${startupitem.location}/${uniquename}` (line
   290); `startupitem.location` defaults to `LaunchDaemons` (line 108) and may
   be `LaunchAgents` (lines 546-560); `uniquename` defaults to
   `org.macports.<subport>` and the plist to `<uniquename>.plist`, every one
   overridable per port, and a port may declare several items (lines 112-121,
   158-166). The directory is created only when an item is (lines 130-133,
   513).
2. **A port that uses its own name.** `devel/dbus/Portfile` (sha256
   `c5e92c79a8fe40d84ae5a2e8da30937924340d20687c0f54062576c84805c244`, the
   rsync ports tree as synced on this machine) declares two items with
   `uniquename` `org.freedesktop.dbus-system` (`LaunchDaemons`) and
   `org.freedesktop.dbus-session` (`LaunchAgents`), with `startupitem.create
   no` and the system plist written by the Portfile itself. A scan that reads
   only `org.macports.<port>` reports `dbus` as defining no service.
3. **Deactivation removes the item.** Deactivating a port deletes its files,
   the item directory included (`registry2.0/portimage.tcl`,
   `_deactivate_files`), so the absence of an item says nothing about an
   inactive port.
4. **The ordinary state is no location at all.** On this machine
   `${prefix}/etc` holds only `macports`: no port has created an item.

Consequence (implemented in `crates/adapters/src/macports/service.rs`, commit
`ffdd3b3`, then attributed by the registry after the end-of-slice review): the
scan reads both locations and answers "these ports define a service" only when
every entry is a real directory holding a plist of its own name that
`port -q provides` attributes to an installed port, the prefix's `etc/macports`
is a real directory, and every installed port is active. The item belongs to
the port the registry names, whatever it is called — `dbus`'s `org.freedesktop.*`
items, and a subport that sets `startupitem.name` to its parent's name
(`databases/mysql5/Portfile`, sha256
`d8fa9bb9b07560d1cb6a526b8cf8bc458d8f3c5e18c457cd243d8555076b6287`, line 173:
`mysql5-server` writes `org.macports.mysql5`;
security review, pass 08 slice 3, M1). Anything it cannot attribute is
**unknown**, which the planner treats as unknown rather than as "no service". Whether a port that *declares* an item but materialises
none (`startupitem.type none`) should count is DP-032. The system
`macports.conf` (which can set `startupitem_type none`) is not read; a user
`macports.conf` is already a `PROVIDER_CONFIGURATION_RISK` (DP-027).

## Re-check when

- **Homebrew's major version changes** (currently 6.0.21). The `--json=v2`
  `service` shape and the `homebrew.mxcl.` default label are both
  implementation details of `Library/Homebrew/service.rb`.
- **Immediately, if the internal API cache schema is relied on.**
  `service_run_args` / `service_args` (E-2) is an undocumented cache format and
  the most likely thing in this note to break silently. It carries
  `homebrew_version: 6.0.21-28-gfd18738` and `generated_at: 1788350020`
  (2026-09-02) — treat a mismatch in `homebrew_version` as a re-check trigger.
- **E-8 and E-9 on a MacPorts machine** — MacPorts 2.12.6 is now installed
  here and E-7's location rules are read in source (addendum, 2026-10-10); E-8
  and E-9 are still cited only, and the three MacPorts `UNRESOLVED` items above
  stay open until they are observed.
- **When MacPorts' minor version changes** (2.12.6 at the addendum):
  `portstartupitem.tcl`'s location and naming rules, by its sha256.
- **When the curated stateful list is first written** — regenerate the 86-name
  E-6 candidate list against the then-current catalogue rather than copying
  this note's list, and record the new revision and date.
- Not time-sensitive in the support-tier sense: nothing here depends on a
  published deadline.
