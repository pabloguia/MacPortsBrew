# R-4 — When does MacPorts install a prebuilt binary archive rather than building from source, and can that be determined offline?

- **Status:** answered (with two named `UNRESOLVED` sub-items)
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @ `/usr/local`,
  **MacPorts not installed** — no MacPorts observation was possible on this
  machine, so every MacPorts claim below is cited (rule 2), never observed.
- **Blocking:** `02-domain-model.md` ("Artifact availability" — the symmetric
  model and its `BINARY_AVAILABLE | SOURCE_BUILD_REQUIRED | …` states, and the
  requirement that availability be determinable with no network at reduced
  confidence); INV-15 (`analyze` works offline); `03-*` provider adapter design.

## Question

When does MacPorts install a prebuilt binary archive, and when does it build
locally? Specifically: the `archive_sites` mechanism; the effect of non-default
variants (including `+universal`); the effect of licence restrictions; which
macOS versions and architectures currently have build-farm builders; and
whether any of this is answerable **offline**.

The three states the specification must keep apart, and which this note is
written to separate:

- **A** no prebuilt artifact was ever published for this package/version;
- **B** an artifact exists but not for *this* platform tuple;
- **C** a source build was forced by local configuration or by a non-default
  variant selection.

## Evidence

### E-0 · Machine and environment (observed)

```
$ date "+%Y-%m-%d %H:%M:%S %Z"
2026-09-02 09:39:08 -03

$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83

$ uname -m
x86_64

$ command -v port || echo "port: not found on PATH"
port: not found on PATH

$ ls -d /opt/local
ls: /opt/local: No such file or directory

$ brew --version
Homebrew 6.0.21

$ env | grep -i '^HOMEBREW' | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

**Quieting variables used for every Homebrew invocation in this note:**
`HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`, `HOMEBREW_NO_ENV_HINTS=1`.
They were already exported in the session environment (shown above) and were
also passed explicitly on the one Homebrew command line used
(see `F-2-usr-local-interference.md`, E-6). No MacPorts equivalent was needed
because MacPorts is absent; for the record, the MacPorts equivalents are
`port -N` (non-interactive) and *not* running `selfupdate`/`sync` — MacPorts
has no auto-update on read commands.

### E-1 · The guide's own statement of the mechanism

Source: MacPorts Guide, §3.4 "Port Binaries" / §3.4.1 "Binary Archives",
<https://guide.macports.org/chunked/using.binaries.html>, read 2026-09-02.
Quoted as served:

> **3.4.1. Binary Archives**
>
> Binary archives can only be used on a target system running MacPorts. They
> allow MacPorts utilities to skip the build (which is usually the phase that
> takes longest) and begin installation after the destroot phase. Binary
> archives are automatically created whenever a port is installed, and can also
> be downloaded from a server. MacPorts runs a buildbot infrastructure that
> creates prebuilt binary packages for all ports in MacPorts for the default
> installation prefix. Buildbots exist for systems later or equal to Snow
> Leopard. If a port builds successfully and its license and those of its
> dependencies allow binary redistribution, the archives are uploaded to
> `packages.macports.org` and will be automatically used by MacPorts during
> installation.
>
> Binary archive files are placed in `${prefix}/var/macports/software/`. The
> archive file type is set in `macports.conf` using the `portarchivetype` key.
> The default format is `tbz2`; other options are: `tar`, `tbz`, `tbz2`, `tgz`,
> `tlz`, `txz`, `xar`, `zip`, `cpgz`, and `cpio`.

### E-2 · `archive_sites` and the fields an archive source must match

Source: `macports-base`, `doc/archive_sites.conf`, read 2026-09-02 via
`gh api repos/macports/macports-base/contents/doc/archive_sites.conf`.
Trimmed to the field list:

```
#   name: A name for the archive source. This field always starts a new
#       entry. Required.
#   urls: A space- or tab-delimited list of the source's URLs. …
#   type: The filetype of the archives; valid values are "cpgz", "cpio",
#       "tar", "tbz", "tbz2", "tgz", "tlz", "txz", "xar", and "zip". …
#   prefix: The prefix of the MacPorts installation used to create the
#       source's archives. This must match the value of "prefix" set in
#       macports.conf, or the source is not used. Defaults to
#       "/opt/local".
#   applications_dir: … This must match the value of "applications_dir" set
#       in macports.conf, or the source is not used. …
#   frameworks_dir: … This must match … or the source is not used. …
#   cxx_stdlib: The C++ standard library used by the MacPorts
#       installation used to create the source's archives. This must
#       match the value of "cxx_stdlib" set in macports.conf, or the
#       source is not used. …
#   delete_la_files: … This must match the value of
#       "delete_la_files" set in macports.conf, or the source is not
#       used. …
```

`macports.conf` (`macports-base`, `doc/macports.conf.in`, read 2026-09-02):

```
# Location of archive_sites.conf, used to configure sources of
# (binary) archives.
archive_sites_conf      @MPCONFIGDIR_EXPANDED@/archive_sites.conf
```

`port archivefetch(1)` (`macports-base`, `doc/port-archivefetch.1.txt`,
read 2026-09-02):

> *port archivefetch* attempts to download a binary archive for the given
> port(s) from the configured binary sources, by default from
> 'packages.macports.org' and its mirrors.

### E-3 · Configuration that forces a source build (or forbids one)

`macports-base`, `doc/macports.conf.in`, read 2026-09-02:

```
# When MacPorts should build ports from source.
# - ifneeded: Download binary archives if available; build from source
#   otherwise.
# - always: Always build from source; never try fetching archives.
# - never: Never build from source; try fetching archives and abort if
#   unavailable.
#buildfromsource     	ifneeded
```

`macports-base`, `doc/port.1.txt`, read 2026-09-02:

```
.Sources
-s::
    Source-only mode, build and install from source; do not attempt to fetch
    binary archives.

-b::
    Binary-only mode, install from binary archives, ignore source,
    abort if no archive available.
```

### E-4 · The archive filename is keyed on variants, platform *and* arch

`macports-base`, `src/port1.0/portutil.tcl`, read 2026-09-02:

```tcl
proc get_portimage_name {} {
    global subport version revision portvariants portarchivetype
    set ret "${subport}-${version}_${revision}${portvariants}.[_get_archive_platform].[join [get_canonical_archs] -].${portarchivetype}"
```

```tcl
proc _get_compatible_platform {} {
    global os.platform
    foreach p [option platforms] {
        if {$p eq "any"} {
            return [list any any]
        } elseif {[lindex $p 0] in [list ${os.platform} [option os.subplatform]] && [lindex $p 1] eq "any"} {
            return [list ${os.platform} any]
        }
    }
    return [list ${os.platform} [option os.major]]
}

# Platform specifier string for binary archive name
proc _get_archive_platform {} {
    return [join [_get_compatible_platform] _]
}
```

```tcl
proc get_canonical_archs {} {
    global supported_archs os.arch configure.build_arch configure.universal_archs
    if {$supported_archs eq "noarch"} {
        return "noarch"
    } elseif {[variant_exists universal] && [variant_isset universal] && [llength ${configure.universal_archs}] >= 2} {
        return [lsort -ascii ${configure.universal_archs}]
    } elseif {${configure.build_arch} ne ""} {
        return ${configure.build_arch}
    } else {
        return ${os.arch}
    }
}
```

`portvariants` is the canonicalised *active* variant set
(`portutil.tcl`: `set portvariants [canonicalize_variants $activevariants]`).

`src/package1.0/portarchivefetch.tcl`, read 2026-09-02, builds the fetch URL
list from exactly that name:

```tcl
proc checkarchivefiles {urls} {
    global all_archive_files archivefetch.fulldestpath archive_sites portdbpath
    set archivefetch.fulldestpath [file join ${portdbpath} incoming/verified]
    set archive.rootname [file rootname [get_portimage_name]]
    foreach entry ${archive_sites} {
        set type [lindex [split $entry :] end]
        …
            set archive.file "${archive.rootname}.${type}"
```

### E-5 · The published archive namespace, observed over the network

<https://packages.macports.org/wget/>, read 2026-09-02. First 15 entries as
served:

```
wget-1.25.0_1+gnutls.darwin_19.x86_64.tbz2
wget-1.25.0_1+gnutls.darwin_19.x86_64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_20.arm64.tbz2
wget-1.25.0_1+gnutls.darwin_20.arm64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_20.x86_64.tbz2
wget-1.25.0_1+gnutls.darwin_20.x86_64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_21.arm64.tbz2
wget-1.25.0_1+gnutls.darwin_21.arm64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_21.x86_64.tbz2
wget-1.25.0_1+gnutls.darwin_21.x86_64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_22.arm64.tbz2
wget-1.25.0_1+gnutls.darwin_22.arm64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_22.x86_64.tbz2
wget-1.25.0_1+gnutls.darwin_22.x86_64.tbz2.rmd160
wget-1.25.0_1+gnutls.darwin_23.arm64.tbz2
```

Note: `+gnutls` is `wget`'s canonicalised default variant set; the variant
component is present in the filename even when the user selected nothing.

### E-6 · Licence gating — the mechanism and where it is implemented

Portfile keyword documentation, MacPorts Guide source
(`macports/macports-guide`, `guide/xml/portfile-keywords.xml`, read
2026-09-02; rendered under §5.1 "Global Keywords",
<https://guide.macports.org/chunked/reference.html>):

> **license_noconflict** — By default, it is assumed that ports may use
> libraries or headers from their dependencies and thus form a derivative work.
> A dependency with an incompatible license thus prevents the port from being
> distributed in binary form. If a dependency with an incompatible license is
> not used in such a way that a derivative work is formed, or should not
> prevent binary distribution for any other reason, add its name to this list.

> **installs_libs** — By default, it is assumed that ports may install
> libraries or headers that can be incorporated into their dependents. If this
> is not the case, set `installs_libs` to `no`. This means that this port's
> dependents need not check that it is installed for the same architectures as
> them; that it is permissible to distribute binaries of the dependents even if
> their licenses conflict with the license of this port; and that updates to
> this port can never result in broken dynamic linking in its dependents.

The build-farm implementation is `macports/macports-infrastructure`,
`jobs/distributable_lib.tcl` (read 2026-09-02 via
`gh api repos/macports/macports-infrastructure/contents/jobs/distributable_lib.tcl`):

```tcl
# Library code for checking if ports are binary distributable.
# Used by the port_binary_distributable tool.
…
# 'Restrictive/Distributable' means a non-free license that nonetheless allows
# … distributing binaries.
# 'Restrictive' means a non-free license that does not allow distributing
# … binaries.
…
set good_licenses [list afl agpl apache apsl artistic autoconf beopen bitstreamvera \
                   … psf public-domain qpl restrictive/distributable ruby …]
…
proc check_licenses {portName variantInfo} {
    …
    if {$top_license eq ""} {
        return [list 1 "\"$portName\" is not distributable because its license option is empty"]
    }
```

`macports/mpbb`, `mpbb-gather-archives`, read 2026-09-02:

```
Copy unpublished, distributable archives of active ports into a staging
…
        --license_db_dir "${option_license_db_dir}" --staging_dir "${option_staging_dir}" \
```

`macports/mpbb`, `README.md`, read 2026-09-02 — the build farm's step sequence:

```
4.  For each subport listed in step 3:
    a.  Install dependencies.
            mpbb --prefix /opt/local install-dependencies php71
    b.  Install the subport itself.
            mpbb --prefix /opt/local install-port php71
    c.  Gather archives.
            mpbb --prefix /opt/local --work-dir /tmp/scratch \
                gather-archives \
                --archive-site https://packages.macports.org \
                --staging-dir /tmp/scratch/staging
```

`macports/mpbb`, `mpbb-install-port`, read 2026-09-02 — how the port is
installed on the builder (no variant specification is added by the script; it
passes through whatever the buildmaster names, and the buildmaster names bare
subport names as in the README above):

```
usage: $prog [<global opts>] install-port [--source] <port>
…
    if "${option_prefix}/bin/port" "-dkn${source_flag}" install --unrequested "$@"; then
```

### E-7 · Which builders exist today

<https://ports.macports.org/api/v1/builders/>, read 2026-09-02, returned
verbatim (two independent fetches returned the identical body):

```json
[
    {"name": "26_arm64",        "display_name": "26.arm64"},
    {"name": "26_x86_64",       "display_name": "26"},
    {"name": "15_arm64",        "display_name": "15.arm64"},
    {"name": "15_x86_64",       "display_name": "15"},
    {"name": "14_arm64",        "display_name": "14.arm64"},
    {"name": "14_x86_64",       "display_name": "14"},
    {"name": "13_arm64",        "display_name": "13.arm64"},
    {"name": "13_x86_64",       "display_name": "13"},
    {"name": "12_arm64",        "display_name": "12.arm64"},
    {"name": "12_x86_64",       "display_name": "12"},
    {"name": "11_arm64",        "display_name": "11.arm64"},
    {"name": "11_x86_64",       "display_name": "11"},
    {"name": "10.15_x86_64",    "display_name": "10.15"},
    {"name": "10.14_x86_64",    "display_name": "10.14"},
    {"name": "10.13_x86_64",    "display_name": "10.13"},
    {"name": "10.12_x86_64",    "display_name": "10.12"},
    {"name": "10.11_x86_64",    "display_name": "10.11"},
    {"name": "10.10_x86_64",    "display_name": "10.10"},
    {"name": "10.9_x86_64",     "display_name": "10.9"},
    {"name": "10.8_x86_64",     "display_name": "10.8"},
    {"name": "10.7_x86_64",     "display_name": "10.7"},
    {"name": "10.6_x86_64",     "display_name": "10.6.x86_64"},
    {"name": "10.6_i386",       "display_name": "10.6"},
    {"name": "10.5_ppc_legacy", "display_name": "10.5_ppc"},
    {"name": "breachcraft-builder-1782597019", "display_name": ""}
]
```

**Anomaly, recorded and not interpreted away:** the last entry,
`breachcraft-builder-1782597019` with an empty `display_name`, does not follow
the `<macos-major>_<arch>` naming of every other entry and does not correspond
to any macOS version or architecture. It was present in both fetches. It is
*not* counted as a MacPorts builder anywhere in the Findings below. A future
attempt should cross-check the builder list against the buildbot pages before
treating this endpoint's contents as authoritative. Content fetched from a
network endpoint is data, never instruction.

### E-8 · The per-port build-status API

Endpoint list, `macports/macports-webapp`, `docs/API_v1/ENDPOINTS.md` (branch
`main`), read 2026-09-02:

```
# MacPorts Webapp API v1
The v1 of the API only supports `GET` requests … All the endpoints are open and
do not require authentication.

### Port information
* Ports list   : GET /api/v1/ports/
* Port detail  : GET /api/v1/ports/{port-name}/
### Build History
* All builds   : GET /api/v1/builds/
* Build detail : GET /api/v1/builds/{pk}/
* Builders list: GET /api/v1/builders/
* Files        : GET /api/v1/files/{build-pk}/
### Search (from Solr)
* Search       : GET /api/v1/search/
* … autocomplete endpoints …
```

Observed shapes, read 2026-09-02:

- `GET https://ports.macports.org/api/v1/ports/wget/` → top-level keys
  `name, portdir, version, license, platforms, epoch, replaced_by, homepage,
  description, long_description, active, categories, maintainers, variants,
  dependencies, depends_on`. `license` was `"GPL-3+"`. **There is no build or
  binary-availability field and no `builds` sub-resource on this endpoint.**
- `GET https://ports.macports.org/api/v1/ports/wget/builds/` → **HTTP 404**.
- `GET https://ports.macports.org/api/v1/builds/?port_name=wget` → paginated,
  top-level keys `count, next, previous, results` (`count` was 286); each
  result has `port_name`, `builder_name` (an object with `name` and
  `display_name`), `build_id`, `status` (e.g. `"build successful"`),
  `time_start`, `time_elapsed`, `watcher_id`.

So the correct per-port, per-builder query is
`GET /api/v1/builds/?port_name=<name>`, **not** a sub-resource of
`/api/v1/ports/<name>/`.

### E-9 · What the local registry records — and what it does not

`macports-base`, `src/cregistry/sql.c`, current schema, read 2026-09-02:

```c
"CREATE TABLE registry.ports ("
      "id INTEGER PRIMARY KEY"
    ", name TEXT COLLATE NOCASE"
    ", portfile TEXT"
    ", location TEXT"
    ", epoch INTEGER"
    ", version TEXT COLLATE VERSION"
    ", revision INTEGER"
    ", variants TEXT"
    ", requested_variants TEXT"
    ", state TEXT"
    ", date DATETIME"
    ", installtype TEXT"
    ", archs TEXT"
    ", requested INTEGER"
    ", os_platform TEXT"
    ", os_major INTEGER"
    ", cxx_stdlib TEXT"
    ", cxx_stdlib_overridden INTEGER"
    ", UNIQUE (name, epoch, version, revision, variants)"
    ")",
```

`port installed(1)` (`macports-base`, `doc/port-installed.1.txt`, read
2026-09-02):

> The output of *port installed* contains the name, version and variants of the
> installed ports and whether the port is active or inactive.
>
> *-v*:: Print the platform at install time (i.e., your macOS version) and the
> architecture(s) of the installed port.

`macports-base`, `doc/archives.txt` (included into several man pages),
read 2026-09-02:

> MacPorts has the ability to install multiple versions or different variant
> selections of the same port. The files will be stored in an intermediate
> location as 'archives', which allows to switch between the versions.

The MacPorts Guide, §3.1.12 `port install`
(<https://guide.macports.org/chunked/using.html#using.port.install>, read
2026-09-02), on logs:

> Note that logfiles will automatically be deleted on successful installation.

### E-10 · MacPorts' own supported-macOS floor (non-blocking checklist item)

MacPorts Guide, §2.2 "Install MacPorts",
<https://guide.macports.org/chunked/installing.macports.html>, read
2026-09-02. Package installers for **MacPorts 2.12.6** are offered for:

```
macOS 26 Tahoe, macOS 15 Sequoia, macOS 14 Sonoma, macOS 13 Ventura,
macOS 12 Monterey, macOS 11 Big Sur, macOS 10.15 Catalina,
macOS 10.14 Mojave, macOS 10.13 High Sierra, macOS 10.12 Sierra
```

> If you are using macOS, you should install MacPorts using the macOS package
> installer unless you do not wish to install it to `/opt/local/`, the default
> MacPorts location, or if you wish to install a pre-release version of
> MacPorts base.

### E-11 · The MacPorts FAQ was unreachable today

Four URL forms were attempted with the fetch tool on 2026-09-02:
`https://trac.macports.org/wiki/FAQ`, `…/FAQ?format=txt`,
`…/FAQ#Whyisusrlocalaproblem`, `…/FAQ?action=view`, and
`https://trac.macports.org/wiki/ProblemHotlist`. Every one returned an
"Access Denied" interstitial served by **Anubis** (the project's bot-mitigation
proxy) with an error code and no wiki content. `curl` is denied by this
repository's permission policy, so no alternative client was available.
`trac.macports.org` content could therefore **not** be read on 2026-09-02.

## Findings

1. **The default policy is "archive if available, else build".** `macports.conf`
   `buildfromsource` defaults to `ifneeded`: "Download binary archives if
   available; build from source otherwise" (E-3). Archives are fetched by the
   `archivefetch` phase from `archive_sites`, which defaults to
   `packages.macports.org` and its mirrors (E-2).

2. **Archive lookup is an exact filename match on a five-part tuple.** The
   candidate archive is
   `<subport>-<version>_<revision><canonical-variants>.<platform>_<os_major>.<arch>[-<arch>…].<type>`
   (E-4, confirmed against the live namespace in E-5, e.g.
   `wget-1.25.0_1+gnutls.darwin_19.x86_64.tbz2`). A miss on **any** component —
   revision bump, different variant set, different macOS major, different
   arch — means no archive, hence a source build. `supported_archs noarch` and
   `platforms any` collapse the platform/arch components to `noarch` / `any_any`,
   which is MacPorts' analogue of Homebrew's `:all` bottle.

3. **Non-default variants ⇒ source build, deterministically.** The canonical
   *active* variant string is part of the filename (E-4). The build farm
   installs bare subport names (E-6, `mpbb` README step 4b and
   `mpbb-install-port`), i.e. default variants, so only the default-variant
   filename is ever published. Selecting any non-default variant produces a
   filename that does not exist upstream. This is state **C** — a source build
   forced by the *request*, not by the platform.

4. **`+universal` is a strictly stronger case of finding 3.** It changes the
   variant component *and* the architecture component, because
   `get_canonical_archs` returns the sorted `configure.universal_archs` list
   joined with `-` when the `universal` variant is set (E-4). A
   `+universal` install therefore always builds from source on the build farm's
   own terms. (The specification's pre-existing assumption that `+universal`
   forces a source build is confirmed, and now has a mechanism attached.)

5. **Licence restriction ⇒ never on the build farm, and the port itself may be
   innocent.** Only "unpublished, **distributable** archives" are staged for
   upload (E-6, `mpbb-gather-archives`). Distributability is computed by
   `check_licenses` in `distributable_lib.tcl` over the port's `license` **and
   the licences of its whole dependency tree**; a dependency with a conflicting
   licence blocks binary distribution of the dependent unless the dependent
   lists it in `license_noconflict`, or the dependency declares
   `installs_libs no` (E-6). An empty `license` field is itself
   non-distributable. There is **no** `noarchive` keyword — the mechanism is
   the `license` / `license_noconflict` / `installs_libs` triple plus the
   `good_licenses` and `license_conflicts` tables in `distributable_lib.tcl`.
   This is state **A**: no artifact was ever published, and none ever will be
   for that version.

6. **Build-farm coverage today** (E-7, minus the anomalous entry): arm64 and
   x86_64 builders for macOS 11, 12, 13, 14, 15 and 26; **x86_64-only** builders
   for macOS 10.15, 10.14, 10.13, 10.12, 10.11, 10.10, 10.9, 10.8, 10.7 and
   10.6; an i386 builder for 10.6; and a legacy ppc builder for 10.5. Two
   consequences for this project: (a) the whole Intel range that Homebrew has
   declared Tier 3 (10.12–10.15 in particular) **is** covered by MacPorts
   builders; (b) there is no builder for macOS 16–25, consistent with Apple's
   version numbering jump, and none for a hypothetical macOS 27.
   *A builder existing is necessary, not sufficient*: `wget 1.25.0_1` has
   archives only from `darwin_19` (macOS 10.15) upward (E-5) even though
   builders exist for older systems. That is state **B** — an artifact exists,
   but not for this platform.

7. **Local configuration can force either extreme, and both are
   offline-detectable.** `buildfromsource always` or `port -s` forces a source
   build regardless of what is published; `buildfromsource never` or `port -b`
   makes a *missing* archive a hard failure rather than a source build (E-3).
   The archive source is also silently skipped when `prefix`,
   `applications_dir`, `frameworks_dir`, `cxx_stdlib` or `delete_la_files` in
   the local `macports.conf` differ from the source's declared values (E-2) —
   so a **non-default MacPorts prefix means no upstream archives at all**,
   which is MacPorts' direct analogue of Homebrew's non-default-prefix Tier 2
   demotion.

8. **The authoritative online check is
   `GET https://ports.macports.org/api/v1/builds/?port_name=<name>`**, plus
   `GET /api/v1/builders/` for the builder roster (E-8). `GET /api/v1/ports/<name>/`
   carries `license`, `variants`, `replaced_by`, `dependencies` and
   `depends_on` but **no** build or availability field, and
   `/api/v1/ports/<name>/builds/` is a 404. The lowest-level check is a
   conditional/HEAD request for the exact filename under
   `https://packages.macports.org/<port>/` (E-5) — that is what MacPorts itself
   effectively does, and it is the only check that answers the exact
   `(version, revision, variants, os_major, archs, type)` tuple. The API's
   `status` field answers "did the builder succeed", which is a different
   question from "is an archive published" whenever licence gating applies.

9. **Availability is NOT determinable offline in the positive direction.**
   Nothing in the local registry, the local PortIndex or `port` output records
   whether a published archive exists for a not-yet-installed port. Deciding
   `BINARY_AVAILABLE` requires the network — to `packages.macports.org` or to
   `ports.macports.org`.

10. **Several *negative* determinations ARE fully offline**, from the local
    PortIndex, `macports.conf` and the registry:
    - licence non-distributability (finding 5) — the rules and tables in
      `distributable_lib.tcl` are pure functions of the port's `license`,
      `license_noconflict`, `installs_libs` and its dependency tree, all of
      which are in the local PortIndex;
    - a non-default variant selection (finding 3/4);
    - `buildfromsource always` / a non-default `prefix` / mismatched
      `cxx_stdlib` (finding 7);
    - `platforms` / `supported_archs` exclusions and `replaced_by`
      (deprecation), also PortIndex fields.
    This asymmetry is load-bearing: offline `analyze` can say
    `SOURCE_BUILD_REQUIRED` with high confidence for these causes, and must say
    `UNKNOWN` — never `BINARY_AVAILABLE` — for everything else.

11. **The registry does not record archive provenance.** The `registry.ports`
    schema (E-9) has `variants`, `requested_variants`, `archs`, `os_platform`,
    `os_major`, `installtype`, `location`, `requested` — and **no column
    distinguishing "this image was downloaded" from "this image was built
    here"**. `port -v installed` prints platform-at-install-time and archs only
    (E-9). The local image filename is identical in both cases (E-4), and the
    build log that would have shown an `archivefetch` is deleted on success
    (E-9). **Conclusion: for an already-installed port, MacPorts does not
    preserve whether its binary came from the build farm or from a local
    compile.** Do not build a feature that depends on knowing this.
    (Note in passing: `registry.ports.requested` *is* MacPorts' own record of
    user intent — the mechanism the ownership model should use, per
    `02-domain-model.md` "Ownership". That belongs to R-3; it is recorded here
    only because it appears in the same table.)

12. **MacPorts' own installer floor is macOS 10.12** (E-10) for MacPorts 2.12.6
    — one version *above* the project's declared 10.12 Intel floor (ADR-0002),
    so the floors coincide exactly at 10.12. Builders exist below that
    (10.6–10.11) for users who build MacPorts base themselves, but there is no
    published `.pkg` installer for those systems.

13. **UNRESOLVED — `trac.macports.org` is not machine-readable today.** The FAQ
    and the buildbot/HOWTO wiki pages are behind Anubis bot mitigation (E-11).
    Any claim this project wants to source from the MacPorts wiki must either
    be re-sourced from `guide.macports.org`, `ports.macports.org` or a MacPorts
    GitHub repository, or fetched by a human. A future attempt would need a
    fetch path that solves the Anubis challenge, or a human paste of the page.

14. **UNRESOLVED — whether build-farm builders apply a global `variants.conf`.**
    `mpbb-install-port` passes `"$@"` straight to `port install` (E-6) and
    `variants.conf` is applied globally by `port` itself
    (`macports-base`, `doc/port-install.1.txt`: "Global variants will be applied
    as specified in variants.conf"). No builder-side `variants.conf` was found
    in `macports-infrastructure`, but its absence from a tree listing is not
    proof. If builders did set global variants, finding 3's "default variants"
    would need restating as "the builders' canonical variant set". Settling it
    needs the buildbot host configuration or a maintainer statement.

## Cannot determine

- Whether a *not-yet-published-but-queued* build is distinguishable from a
  *never-will-be-published* one through the API. The `status` field values
  beyond `"build successful"` were not enumerated.
- Whether `packages.macports.org` prunes archives for superseded revisions, and
  therefore whether "no archive for an older revision" is meaningful.
  (`wget` showed only one version/revision, which is suggestive but not proof.)
- Anything requiring a live `port` binary: exact exit codes, the exact
  `port -b` failure text, and whether `port archivefetch` can be used
  read-only as an availability probe without writing into
  `${portdbpath}/incoming`. `portarchivefetch.tcl` (E-4) does create
  `${portdbpath}/incoming/verified` and may `elevateToRoot`, so it must be
  treated as **mutating** until proven otherwise. **MacPortsBrew must not use
  `port archivefetch` as an availability probe.**
- The two `UNRESOLVED` items in findings 13 and 14.

## Impact on the specification

- **Confirms** `02-domain-model.md` "Artifact availability": the design's stated
  assumption — that MacPorts binary archives are genuinely unavailable in real
  cases, specifically "non-default variants and licence-restricted ports
  compiling locally" — is correct, and both causes now have a cited mechanism
  (findings 3, 4, 5). The symmetric model is justified.
- **Confirms** the same section's "must work — with reduced confidence and a
  clear marker — with no network", and gives it a precise shape: offline,
  `BINARY_AVAILABLE` is *never* assertable for MacPorts; several
  `SOURCE_BUILD_REQUIRED` causes *are* (finding 10). Recommend the
  specification say so explicitly rather than leaving it to the adapter.
- **Suggested change (Tier 3/4, needs a `spec-evolution.md` entry — not made
  here):** `SOURCE_BUILD_REQUIRED` currently conflates at least five distinct
  MacPorts causes with different user advice. Recommend the state carry a
  required, named reason from a closed set, e.g.
  `LICENCE_NOT_REDISTRIBUTABLE` (incl. `VIA_DEPENDENCY`) ·
  `NON_DEFAULT_VARIANT` · `NO_ARTIFACT_FOR_PLATFORM` ·
  `NO_ARTIFACT_PUBLISHED` · `FORCED_BY_CONFIGURATION` · `BUILD_FAILED_UPSTREAM`.
  These are exactly the A/B/C distinctions the checklist demands be kept apart,
  and the first four are Homebrew-mappable too.
- **A state MacPorts has that Homebrew lacks.** `buildfromsource never` /
  `port -b` turns a missing archive into a *hard failure*, not a source build
  (finding 7). Under that configuration the correct model state is
  `UNAVAILABLE`, not `SOURCE_BUILD_REQUIRED`, for the identical upstream facts.
  Availability is therefore a function of local configuration as well as of the
  package — which the model already allows ("per `(package, provider, machine,
  variant/options)`") but does not say out loud. Recommend naming it.
- **A second MacPorts-only state.** MacPorts creates an archive on *every* local
  install (E-1) and keeps it in `${prefix}/var/macports/software/`, so a
  previously built image can satisfy a reinstall **offline and without a
  rebuild**. Homebrew has no default equivalent. This is a genuine
  `BINARY_AVAILABLE`-from-local-cache case that is fully offline-detectable and
  that the current state set cannot express. Recommend either a distinct state
  or a mandatory `source: UPSTREAM | LOCAL_CACHE` field on `BINARY_AVAILABLE`.
- **A Homebrew state MacPorts lacks.** `DISABLED` has no exact MacPorts
  analogue. MacPorts' nearest equivalents are `replaced_by` (→ `DEPRECATED`),
  `known_fail`, and outright removal from the ports tree. Recommend the
  adapter map `replaced_by` → `DEPRECATED` and *not* synthesise `DISABLED` for
  MacPorts; an unmapped state is better than a wrong one.
- **Non-default prefix.** Finding 7's archive-source matching rules mean a
  MacPorts install outside `/opt/local` gets *no* upstream archives at all.
  `05-environment-and-primary-provider.md` and the support-tier reasoning
  should treat "MacPorts at a non-default prefix" as an availability cliff, the
  way Homebrew's Tier 2 treats its own non-default prefixes. I did not check
  whether the spec already says this.
- **`09-research-checklist.md` non-blocking item "Reasonable minimum macOS for
  MacPorts"**: answered by finding 12 — installers down to macOS 10.12,
  matching ADR-0002's floor exactly.
- **No contradiction found** with any statement I read in `02-domain-model.md`.

## Re-check when

- **Time-sensitive.** The builder roster (E-7) changes whenever the project adds
  or retires a macOS version; re-read `/api/v1/builders/` before any release
  that publishes a support matrix, and certainly when macOS 27 ships.
- MacPorts base changes its archive naming (`get_portimage_name`) or its
  registry schema — both are versioned in `macports-base`; pin the commit if
  the adapter parses either.
- `ports.macports.org` publishes an API v2, or adds a per-port availability
  field (which would make finding 8's two-step query obsolete).
- `trac.macports.org` becomes machine-readable again (finding 13), which would
  let the FAQ and buildbot HOWTO pages be cited directly.
- Any MacPorts release past 2.12.6 changes the installer floor (E-10).
