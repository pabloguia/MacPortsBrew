# Understanding Bottles and Source Builds

When you install a package, you either get a **prebuilt artifact** — something
already compiled for your machine, which installs in seconds — or the provider
**compiles it from source** on your machine, which can take minutes or hours
and needs a working compiler toolchain.

Homebrew calls its prebuilt artifacts **bottles**. MacPorts calls its
**binary archives**. Same idea, different names and different mechanics.

This distinction is the single most practical difference between the two
providers on any given machine, and it is one of the main things MacPortsBrew
reports.

## What MacPortsBrew tells you

`macportsbrew analyze` has an **Availability** column:

```
Package     Availability
git         Prebuilt
gh          Builds from source (no prebuilt artifact for this machine)
gnutls      Builds from source (non-default options were requested)
gemini-cli  Deprecated
gimp        Unknown
```

The states, and what each actually means:

| State | Meaning |
|---|---|
| **Prebuilt** | A prebuilt artifact applies to this machine. Installing is a download and an unpack. |
| **Builds from source** | It will compile here. The reason is given: no artifact for this platform, non-default options were requested, a licence restriction, or the configuration forces it. |
| **Deprecated** | The provider has marked it deprecated. It still works; it is on the way out. |
| **Disabled** | No longer obtainable from this provider. |
| **Unsupported platform** | The provider does not offer it for this machine at all. |
| **Unavailable** | Nothing published. |
| **Unknown** | It could not be determined. **This is not a synonym for "no".** |

## Why `Unknown` matters

Some of this cannot be answered offline, and MacPortsBrew never guesses at it.

Homebrew 6 fetches a per-platform API document, and the bottle information in
it lists **only the tag selected for this machine** — it is a *resolution
result*, not a *publication record*. So a formula can report "no bottle" from
the local cache while five bottles exist upstream. Telling "nothing is
published at all" apart from "something is published but not for this machine"
needs the tap or the network
([`../docs/research/R-2-homebrew-artifact-availability.md`](../docs/research/R-2-homebrew-artifact-availability.md),
2026-09-02; of 778 formulae with no bottle for the example machine's tag, 773
were the second case and only 5 the first).

When MacPortsBrew cannot tell, it says `Unknown` and marks it, rather than
reporting a confident "no". A policy rule then treats the unknown as a reason
for a human to look — never as a reason to act. You will see this in a plan as:

```
- Whether homebrew would supply postgresql17-server ready-built or compile it
  here could not be determined. An unknown is not a yes.
```

## Homebrew bottle tags, if you want the detail

Homebrew names the artifact for your machine with a **tag**. On Intel macOS the
tag is the bare macOS codename — `sonoma`, `tahoe`; on Apple Silicon it is the
codename with an `arm64_` prefix — `arm64_sonoma`. `all` means
platform-independent
([`../docs/research/R-2-homebrew-artifact-availability.md`](../docs/research/R-2-homebrew-artifact-availability.md),
2026-09-02, from Homebrew's own source).

There is **no `brew` subcommand that prints the tag.** MacPortsBrew detects it
from Homebrew's own API cache filename, which is capability detection rather
than a version table it would have to maintain.

Homebrew also has a documented **older-macOS fallback**: a bottle for an older
macOS with the same architecture is accepted on a newer one. This is why Intel
machines on macOS 26 still install `sonoma` bottles and why Intel coverage has
not visibly collapsed despite the tier — see
[Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md).

One thing worth knowing if you have read it elsewhere:
**`HOMEBREW_BUILD_FROM_SOURCE` does not exist.** It is absent from Homebrew's
tree and from the 6.0.21 manual page, and setting it changes nothing. The real
switches are per-invocation flags — `--build-from-source`, `--build-bottle`,
`--interactive`, `--cc`, `--HEAD`, and any `--with-*`/`--without-*` option.

## MacPorts binary archives

MacPorts distributes prebuilt archives too, and MacPorts 2.12.6 ships package
installers down to macOS 10.12 Sierra
([`../docs/research/R-4-macports-binary-archives.md`](../docs/research/R-4-macports-binary-archives.md),
read 2026-09-02).

The thing that most often forces a MacPorts source build is **variants**. If
you install a port with non-default variants, there is generally no prebuilt
archive matching that combination, so it compiles. MacPortsBrew reports that
as "Builds from source (non-default options were requested)", which is a
different situation from "no artifact exists" and is reported as such.

Licence restrictions are the other common cause: some ports cannot be
distributed prebuilt and must be compiled locally.

## If you need to compile

Both providers need Apple's **Command Line Tools** to build from source.
`macportsbrew doctor` checks for them:

```
[OK] Command Line Tools are installed.
```

If they are missing, that becomes a warning, because both providers need them
and neither will say so clearly until a build fails.

## How this affects recommendations

A source build is not a fault; it is a cost. The policy engine weighs it: one
of the rules, `SOURCE_NO_PREBUILT_DESTINATION_PREBUILT`, fires when the
provider holding a package would compile it and another provider has one ready
built. Like every rule here, it names no provider — it fires in whichever
direction the artifacts point.

```sh
macportsbrew explain <package>
```

shows both providers' availability among the facts, and whether that rule
fired.

## See also

- [Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md) — where the Intel
  coverage numbers come from.
- [Package Mapping](Package-Mapping.md) — availability is assessed per
  provider, so it depends on the mapping being right first.
