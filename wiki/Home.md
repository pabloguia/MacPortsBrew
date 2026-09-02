# MacPortsBrew

> Safely migrate and manage packages between Homebrew and MacPorts on macOS.

MacPortsBrew looks at what you have installed under Homebrew and under
MacPorts, works out which packages are equivalent, explains which provider is
the better choice for each one **on your machine**, and builds a reviewable
plan to move it — in either direction — without ever removing something before
its replacement is installed and verified.

## Homebrew and MacPorts are peers here

Neither is globally better, and MacPortsBrew does not have a favourite. Which
one wins for a given package depends on your machine, that package, and what
each provider currently offers for it. Two consequences worth stating up front:

- **Moving back is a first-class operation.** The same planner, the same policy
  engine and the same safety checks serve
  [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md) and
  [MacPorts to Homebrew](MacPorts-to-Homebrew-Migration.md).
- **Keeping both is a supported answer**, not a failure to decide. See
  [Using Homebrew and MacPorts Together](Using-Homebrew-and-MacPorts-Together.md).

The name puts MacPorts first because it makes a better word. Read nothing into
the order.

## Start here

| If you want to | Read |
|---|---|
| Install it and see what it says about your machine | [Getting Started](Getting-Started.md) |
| Move packages from Homebrew to MacPorts | [Homebrew to MacPorts Migration](Homebrew-to-MacPorts-Migration.md) |
| Move packages from MacPorts to Homebrew | [MacPorts to Homebrew Migration](MacPorts-to-Homebrew-Migration.md) |
| Run both without breaking either | [Using Homebrew and MacPorts Together](Using-Homebrew-and-MacPorts-Together.md) |
| Understand the Intel deadline | [Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md) |
| Know why a command said what it said | [CLI Reference](CLI-Reference.md) |
| Fix something | [Troubleshooting](Troubleshooting.md) |

## Understanding the answers

- [Package Mapping](Package-Mapping.md) — how an equivalent is found, and why
  "no equivalent" is a real answer rather than a failure.
- [Dependencies and Safety](Dependencies-and-Safety.md) — what stops a
  migration, and why.
- [Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md)
  — prebuilt artifacts, source builds, and how to tell which you will get.
- [Primary Provider](Primary-Provider.md) — when both providers have the same
  tool, which one actually runs.
- [Undoing a Migration](Undoing-a-Migration.md) — snapshots, history and the
  restore path.

## What this build does

Version 0.1.0 is **read-only**. It inventories, analyses, maps, explains and
plans. It executes nothing, elevates nothing, writes nothing into either
provider's prefix, and makes no network calls.

`plan` prints exactly what would happen and stops. There is no `migrate`
command yet; execution is a later pass. Every page here that describes a
migration says plainly where the plan ends and the not-yet-written executor
would begin.

## Privacy

No telemetry, and no network call that reports anything about you.
`macportsbrew doctor --report` writes a **redacted** diagnostic file locally —
no usernames, home paths, environment variable values or tokens — which you
read and choose to share. MacPortsBrew never uploads it.

## Not affiliated

MacPortsBrew is an independent project, **not affiliated with, endorsed by, or
sponsored by** the Homebrew project or the MacPorts Project. It interacts with
both only through their public, supported command-line interfaces.

## Elsewhere

- [Architecture](Architecture.md) — how the program is put together.
- [Contributing](Contributing.md) — how to change it.
- [`../docs/spec/README.md`](../docs/spec/README.md) — the specification.
- [`../docs/support-matrix.md`](../docs/support-matrix.md) — what is tested
  versus what is believed to work.
