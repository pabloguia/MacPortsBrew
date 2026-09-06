# R-11 — What the signing and notarisation decision assumes, and which of it holds

- **Status:** partially answered — **three of five questions closed from
  official documentation, and one of R-11's own premises is wrong.** Two
  questions remain open and are marked `UNRESOLVED`. Nothing here is observed:
  this repository has no Apple Developer account, no certificate and no CI run,
  so every answer below is cited, never run.
- **Answered on:** 2026-09-06
- **Machine:** irrelevant to the answers — none required a local observation,
  which is why they could be closed at all.
- **Blocking:** the implementation pass that writes the release workflow.
  **Not** ADR-0005, which is decided. Q3's answer changes what ADR-0005 must
  provision; Q2's answer has a cost consequence that touches DP-007.

## Sources

All read 2026-09-06.

- Apple, *Customizing the notarization workflow* —
  `https://developer.apple.com/documentation/security/customizing-the-notarization-workflow`
- Apple, *Packaging Mac software for distribution* —
  `https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution`
- GitHub, *runner-images* `README.md`, `main` —
  `https://raw.githubusercontent.com/actions/runner-images/main/README.md`
- GitHub, *runner-images* `images/macos/macos-15-Readme.md`, `main`
- GitHub Docs, *GitHub Actions billing* —
  `https://docs.github.com/en/billing/concepts/product-billing/github-actions`
- GitHub Docs, *Larger runners reference* —
  `https://docs.github.com/en/actions/reference/runners/larger-runners`

Apple's documentation site is a JavaScript application; a plain HTTP fetch
returns an empty shell. The pages above were read rendered. Anyone re-verifying
by `curl` will get nothing and should not conclude the pages are gone.

## Q1 — Does stapling a `.pkg` or `.dmg` cover a binary copied out of it?

**No, and the reason is stronger than the question assumed: a bare binary
cannot carry a ticket at all.** Apple, *Customizing the notarization workflow*:

> To attach a ticket to your app, bundle, disk image, or flat installer
> package, use the `stapler` tool

and, in the same section:

> While you can notarize a ZIP archive, you can't staple to it directly. […]
> **Although tickets are created for standalone binaries, it's not currently
> possible to staple tickets to them.**

So the staplable set is: app bundle, `.dmg`, flat `.pkg`. Not a ZIP, and **not a
Mach-O executable** — which is exactly what MacPortsBrew ships. A ticket *is*
generated for it (notarisation is recursive: "The notary service generates a
ticket for the top-level file that you specify, as well as for each nested
file"), and that ticket lives only on Apple's servers.

**This inverts R-11's own framing.** R-11 supposed that if stapling covers only
the container then "the `.dmg` buys nothing the bare binary does not and is
dropped". The opposite follows: the `.dmg` buys the *only* offline-verifiable
path there is, because it is the only artifact of the three that can hold a
ticket. A user who copies the binary out of the mounted image and keeps it has
an unstapled binary from that moment on.

**Consequence for the charter's user.** Gatekeeper without a stapled ticket
"finds the ticket online"; stapling "ensures that Gatekeeper can find the ticket
even when a network connection isn't available". A bare notarised binary can
therefore never be verified offline. That is the state of exactly the user this
project is written for — an older Intel Mac, possibly offline — and it is a
property of Apple's tooling, not of any choice this project can make.

## Q2 — Are the tools on the runner, and is that runner free?

**The premise is wrong. `macos-15-intel` is not a standard runner and is not
free.** GitHub's `runner-images` README lists it as an alias, not an image:

> `| macOS 15 | x64 | macos-15-large, or macos-15-intel | [macOS-15] |`

`macos-15` alone is the **arm64** image; the x64 image answers to
`macos-15-large` and `macos-15-intel`. The `-large` suffix makes it a larger
runner, and GitHub Docs is unambiguous about what that costs:

> **Larger runners are always charged for, even when used by public
> repositories or when you have quota available from your plan.**

The x64 macOS larger runner is the 12-core / 30 GB row of *Specifications for
general larger runners*. So **every minute of Intel macOS CI is billed, public
repository or not.** DP-007 chose public GitHub; that decision is unaffected for
ordinary CI on standard runners, but an Intel *release* job is a recurring cost
the release workflow must justify or avoid. The arm64 `macos-15` standard runner
remains free for public repositories.

**Tooling.** `images/macos/macos-15-Readme.md` lists **Xcode Command Line Tools
16.4.0.0.1.1747106510** under Tools, plus Xcode itself. `codesign`, `notarytool`
and `stapler` are not enumerated individually because they ship inside Xcode and
the Command Line Tools rather than as separate packages — **that last step is an
inference, not a citation**, and it is the one thing here a first CI run settles
for free. R-10's Xcode floor question is therefore narrowed but not closed:
`notarytool` requires Xcode 13 or later (*Customizing the notarization
workflow*: "Xcode 13 or later supports this tool"), and the image is far above
that.

## Q3 — Which certificate signs the `.pkg`?

**A different one from everything else, and R-11 was right to reopen this.**
Apple, *Packaging Mac software for distribution*:

> If you're distributing a product independently, use a **Developer ID
> Installer**-signing identity. This is named `Developer ID Installer: <TeamID>`

and, for the disk image:

> `<CodeSigningIdentity>` is your **Developer ID Application** code-signing
> identity […] Use a code-signing identity, not an installer-signing identity.

The same page warns that the two are not interchangeable and are not even listed
together:

> Installer-signing identities are different from code-signing identities, so
> the `-p codesigning` option filters out installer-signing identities.

**So shipping all three artifacts needs two certificates**: Developer ID
Application for the Mach-O and the `.dmg`, Developer ID Installer for the
`.pkg`. R-9 C-3's list — Mach-O files, disk images, command-line tools — is
correct and simply does not include `.pkg`, which is why treating this as
answered was wrong. ADR-0005 must provision both, and DP-008 ("is the `.pkg`
also provisional") acquires a concrete cost: a second certificate to obtain,
store and rotate.

One further constraint from the same page, easy to miss and fatal at
notarisation time: a distributed `.dmg` must be "a UDIF-format read-only
zip-compressed disk image (type UDZO)".

## Q4 — Does the hardened runtime need an entitlement for a tool that spawns `brew` and `port`? `UNRESOLVED`

Not researched. The expectation stated in R-11 — none needed — is probably
right, because the hardened runtime's restrictions concern code *loaded into*
the process (library validation, JIT, `DYLD_*`) rather than child processes
created by `exec`. **That is reasoning, not a citation, and this project does not
accept reasoning as a third option.** It needs either Apple's hardened-runtime
documentation read against the specific case or, better, a first notarisation
that either succeeds or names the entitlement it wanted.

## Q5 — What does a quarantined, notarised, **unstapled** binary do offline? `UNRESOLVED`

Q1 settles half of it: a bare binary cannot be stapled, and Gatekeeper falls
back to the network when no ticket is attached. What is *not* established is the
part that decides whether this matters in practice — whether a command-line tool
launched from a shell is subject to the same Gatekeeper evaluation as a
double-clicked application, and whether it carries `com.apple.quarantine` at
all. A binary fetched with `curl` or `git` carries no quarantine attribute; one
downloaded by a browser does. The outcomes differ, Apple does not document the
CLI case clearly, and the difference is the whole question.

**This one cannot be closed by reading.** It needs observation on a machine with
the quarantine attribute set and the network off — which is a test the release
pass should run before promising anything about offline use.

## What this changes

1. **The `.dmg` is not droppable on Q1's grounds** — it is the only staplable
   container that delivers a single binary, so it is the offline story.
2. **ADR-0005 needs a second certificate** (Developer ID Installer) if the
   `.pkg` ships, and DP-008 should be re-read with that cost attached.
3. **An Intel release job costs money on a public repository.** Nothing in the
   free tier covers `macos-15-intel`. Either the release builds Intel on a paid
   larger runner, cross-compiles from the free arm64 standard runner, or the
   project ships arm64 first and says so.
4. **Two questions stay open**, both needing something other than reading: one
   notarisation attempt, and one offline machine.
