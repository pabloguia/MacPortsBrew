# Troubleshooting

## Start here

```sh
macportsbrew doctor
```

It checks the things that are wrong most often, and each finding carries its
reason — and, for anything derived from provider policy, the source and the
date that policy was read.

```
[Warning] macports's prefix /opt/local is not on PATH.
[Warning] homebrew support for this machine is Degraded
          (source: https://docs.brew.sh/Support-Tiers, read 2026-09-02).
[OK] Command Line Tools are installed.
[OK] homebrew 6.0.21 is present.
```

For an issue report, `macportsbrew doctor --report <file>` writes the same
information in a redacted file — no usernames, home paths, environment variable
values or tokens. Read it before you share it; MacPortsBrew never uploads it.

## The wrong version of a tool runs

Both providers have it, and `PATH` order decides. `macportsbrew analyze` shows
the effective resolution per package: what runs, and which provider owns it.
`macportsbrew doctor` shows the `PATH` order and flags duplicate entries.

See [Primary Provider](Primary-Provider.md) for the four ways to change it,
least invasive first. This build reports; it does not change what runs.

## A package compiles instead of installing

There is no prebuilt artifact for your machine, or non-default options were
requested, or a licence restriction applies. `macportsbrew analyze` names which:

```
gh  homebrew  2.98.0  Builds from source (no prebuilt artifact for this machine)
```

This is normal and increasingly common on Intel. See
[Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md),
and [Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md) for why.

Check the Command Line Tools are installed — `doctor` says — because both
providers need them to compile and neither says so clearly until a build fails.

## "Needs a human decision" on everything

That is usually correct rather than broken. Run:

```sh
macportsbrew explain <package>
```

The most common causes, in rough order of frequency:

- **Reverse dependencies.** Other packages under the same provider depend on
  this one.
- **The package holds data or runs as a service.** Never migrated
  automatically, in either direction.
- **The mapping confidence is below Strong.** Acting needs Strong or better.
- **Availability is unknown.** Something could not be determined offline, and
  an unknown is not a yes.
- **The destination provider is not installed.** Installing one is a separate
  step needing explicit consent.

## A package I have is not listed

Three likely reasons:

1. **It is a dependency**, and normal mode hides those. Try
   `macportsbrew analyze --advanced`.
2. **It is a GUI app or cask.** Casks are reported and labelled, and excluded
   from migration planning — MacPorts has no equivalent concept.
3. **It was not installed by either provider.** MacPortsBrew inventories
   Homebrew and MacPorts and nothing else.

## A provider is installed but reported absent

Detection looks in the standard prefixes: `/opt/homebrew` and `/usr/local` for
Homebrew, `/opt/local` for MacPorts (<https://docs.brew.sh/Installation>,
<https://guide.macports.org>, both read 2026-09-02). Both Homebrew candidates
are probed on every machine, so an Intel Homebrew under Rosetta on an Apple
Silicon Mac is found rather than missed.

If you have relocated a prefix, detection may miss it. That is worth a
[provider compatibility report](../.github/ISSUE_TEMPLATE/provider-compatibility.yml)
— include the actual prefix.

If the provider is present but not working, `doctor` reports it as broken with
the reason, and the analysis **degrades rather than failing**: the other
provider is still analysed.

## An error I do not understand

Every error carries a stable identifier and a remediation string. The exit code
tells you the category:

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | generic failure |
| 2 | usage error |
| 3 | partial success |
| 4 | blocked by a safety check |
| 5 | provider unavailable or unhealthy |
| 6 | user aborted |
| 7 | lock held by another instance |
| 8 | stale or unusable input |

**Exit 4 from `plan` is normal**, not a fault: it means the plan contains a step
where a human has to decide. The plan is printed anyway, because it is the
useful output.

**Exit 4 from `migrate --yes` is expected for every plan in this build.** The
plan is put to the execution gate, which refuses every plan this build produces
and lists why; nothing runs and nothing changes.

`"error: exit status 1"` is never acceptable output from this tool. If you see
something like it, that is a bug worth reporting.

## Something looks wrong in the numbers

Snapshot it and attach the snapshot:

```sh
macportsbrew snapshot --out state.json
macportsbrew analyze --from-snapshot state.json
```

Because analysis is a pure function of the snapshot, that file lets anyone
reproduce your exact result. It is redacted the same way `doctor --report` is.

## Both providers seem to be interfering with each other

Look for build variables. If `CPPFLAGS`, `LDFLAGS`, `CFLAGS` or
`PKG_CONFIG_PATH` name one provider's prefix, packages the other provider
compiles can pick up the wrong headers and libraries — and the result installs
cleanly and breaks later. `doctor` names any such variable. MacPortsBrew never
manages them for you.

See
[Using Homebrew and MacPorts Together](Using-Homebrew-and-MacPorts-Together.md).

## Reporting a problem

Pick the matching template in
[`../.github/ISSUE_TEMPLATE`](../.github/ISSUE_TEMPLATE). Each asks for your
macOS version, architecture, provider versions, MacPortsBrew version and policy
version — `doctor --report` collects all of them. **Never include secrets or
tokens.** For a security problem, do not open an issue; see
[`../SECURITY.md`](../SECURITY.md).
