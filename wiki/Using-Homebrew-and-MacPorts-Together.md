# Using Homebrew and MacPorts Together

Having both installed is **a supported end state**, not a mess to clean up.
MacPortsBrew treats coexistence as a first-class answer — `Keep both` is one of
the outcomes the policy engine can produce, and there is an invariant and a
test behind that (INV-18).

What follows is what coexistence actually requires, and the three ways it goes
wrong.

## The rules that make it work

**Each provider owns its own prefix, completely.** Homebrew's is
`/opt/homebrew` on Apple Silicon or `/usr/local` on Intel; MacPorts' is
`/opt/local`. Nothing MacPortsBrew does ever writes into either
(<https://docs.brew.sh/Installation> and <https://guide.macports.org>, both
read 2026-09-02).

**One provider never satisfies the other's dependency.** If both need `openssl`,
both get their own. This looks wasteful and is not: it is what keeps each
provider able to reason about its own tree, upgrade it, and remove from it
safely. MacPortsBrew will never propose making one provider's library serve the
other's package (INV-08), and it will never create a deliberate cross-provider
dependency (INV-07).

**Duplication is correct.** Two copies of the same tool under two providers is
a normal, healthy state. MacPortsBrew reports it and does not offer to
"consolidate" it, because consolidation is exactly the operation that breaks
one of the two trees (INV-09).

**Exactly one of the duplicates runs.** That is the only genuinely interesting
question coexistence raises, and it is `PATH` order plus what each provider
linked. See [Primary Provider](Primary-Provider.md).

## The three ways it goes wrong

### 1. `PATH` order you did not choose

If both providers ship `git`, whichever prefix appears first on `PATH` wins.
That is fine when it is what you wanted and confusing when it is not, because
nothing announces it.

```sh
macportsbrew doctor
```

reports the order providers resolve in, flags a prefix that is not on `PATH` at
all, and flags duplicate `PATH` entries. `macportsbrew analyze` shows the
effective resolution per package: which executable runs, which provider owns
it, and whether that ownership was confirmed by asking the provider or merely
inferred from the path prefix.

### 2. Build variables leaking one prefix into the other's builds

This is the failure mode that produces genuinely broken packages, and it is
silent. If `CPPFLAGS`, `LDFLAGS`, `CFLAGS`, `PKG_CONFIG_PATH` or similar name
one provider's prefix, a package the *other* provider compiles from source can
pick up headers and libraries from the wrong tree. It links, it installs, and
it breaks later — often after the library it borrowed is upgraded or removed.

```sh
macportsbrew doctor
```

names any build variable that points at a provider prefix. **MacPortsBrew never
manages these variables for you**; it tells you they are there, because a tool
that silently rewrote your build environment would be doing the thing this page
warns about.

The fix is yours: unset them, or scope them to the specific build that needs
them rather than exporting them from your shell profile.

### 3. Assuming a shared dependency

Installing a tool under one provider and expecting it to satisfy the other's
requirement does not work, and neither provider will report the situation
usefully when it fails. Let each provider install what it needs.

## What MacPortsBrew does about your shell

Nothing, in this build. When it does, it will be through a single **managed
block** — one clearly delimited region in one shell file, added with your
consent, removable with `macportsbrew env --revert`. It will never edit a shell
file outside that block, and it will never touch your `PATH` silently.

The repository's own agent tooling denies writes to shell configuration files
outright, which is the same rule applied inward.

## Deciding what lives where

There is no general answer, and MacPortsBrew will not invent one. What it will
do is answer per package, with reasons:

```sh
macportsbrew analyze
macportsbrew explain <package>
```

A useful pattern, not a rule: keep the thing you *use* under whichever provider
gives you a prebuilt artifact for it on your machine, and stop worrying about
the dependencies underneath — they are each provider's business, and
MacPortsBrew hides them unless you ask for `--advanced`.

## Coexistence and the Intel deadline

If you are on Intel, having MacPorts installed alongside Homebrew is a
reasonable position to be in ahead of the timeline in
[Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md), rather than a
transitional state to be resolved. `Keep both` with a chosen primary is a
destination.

## See also

- [Primary Provider](Primary-Provider.md) — which copy runs.
- [Dependencies and Safety](Dependencies-and-Safety.md) — why dependencies stay
  inside their own provider.
- [Troubleshooting](Troubleshooting.md) — when something already broke.
