# R-14 — What actually reaches a user of a published binary

- **Status:** partially answered — **two of four closed, and the withdrawn
  claim was closer to right than the review that withdrew it.** One
  operationally important thing turned up that neither the plan nor the review
  knew about. Two questions remain `UNRESOLVED`.
- **Answered on:** 2026-09-06
- **Machine:** irrelevant — all four are questions about GitHub's behaviour, and
  the two closed ones are closed from GitHub's own documentation. Nothing here
  is observed: this project has no remote, no release and no advisory.
- **Blocking:** how much of `SECURITY.md`'s withdrawal runbook is a real channel.
  Raised 2026-09-04 by the security review of DP-009's plan.

## Sources

All read 2026-09-06, all GitHub Docs.

- *Repository security advisories* —
  `/en/code-security/security-advisories/working-with-repository-security-advisories/about-repository-security-advisories`
- *Publishing a repository security advisory* — same section
- *Managing releases in a repository* —
  `/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository`
- *Immutable releases* —
  `/en/code-security/concepts/supply-chain-security/immutable-releases`

## Q1 — Does a repository advisory for a project with no ecosystem package reach the global database?

**Yes, and the withdrawn claim was nearer the mark than its withdrawal.** The
distinction the plan and the review both missed is between *being in the
database* and *producing an alert*:

> GitHub will review each published security advisory, **add it to the GitHub
> Advisory Database**, and *may* use the security advisory to send Dependabot
> alerts to affected repositories. If the security advisory comes from a fork,
> we'll only send an alert if the fork owns a package, published under a unique
> name, on a public package registry. This process can take up to 72 hours […]

So publication does syndicate: the advisory is added, unconditionally, subject
to review. The ecosystem-package condition quoted above is about **forks** and
about **alerts**, not about database inclusion.

**But the reach is still near zero, for a different reason than the review
gave.** Dependabot alerts fire at repositories that *depend on* the affected
package. MacPortsBrew ships a signed binary through Releases and publishes no
ecosystem package, so no repository can declare a dependency on it, so there is
nothing for Dependabot to alert. The advisory is a **public record**, findable
by someone who looks. It is not a push channel.

That is worth keeping in `SECURITY.md` and worth describing accurately: an
advisory is how the disclosure becomes citable and searchable, not how a user
finds out.

## Q2 — What does a "Watch → Releases only" subscriber receive, and when? `UNRESOLVED`

Not closed. The notification pages read here describe watching and custom
subscriptions in general terms and do not state the delivery guarantee, the
latency, or whether a *deleted* release generates or retracts anything. Since
this is the one channel in the runbook that actually pushes, it deserves better
than an inference, and the answer is cheap to observe once a remote exists.

## Q3 — Does `releases.atom` include deleted or de-listed releases? `UNRESOLVED`

Not documented anywhere found. This is an empirical question about a feed and
should be answered by observation on the real repository rather than by
reading — publish a release, read the feed, delete it, read the feed again.

## Q4 — Does deleting a release leave the tag, the assets and the attestations?

**Partly answered, and the answer arrives with a feature the plan was written
without: immutable releases.**

For an ordinary release, the docs give the procedure and say nothing about what
survives it. For an **immutable** release they are explicit, and the constraints
run the opposite way from what a withdrawal runbook wants:

> **Git tags cannot be moved**: Once an immutable release is published, its
> associated Git tag is locked to a specific commit, cannot be changed, and
> cannot be deleted while the release exists. **If you delete the immutable
> release, you can delete the tag, but you cannot reuse the same tag name.**
>
> **Release assets cannot be modified or deleted**: All files attached to the
> release (such as binaries and archives) are protected from modification or
> deletion.

and, on provenance:

> creating an immutable release automatically generates a **release
> attestation**, which is a cryptographically verifiable record of a release
> containing the release tag, commit SHA, and release assets.

with a further constraint that matters to anyone imagining a clean restart:

> Even if you delete a repository and create a new one with the same name, you
> cannot reuse tags that were associated with immutable releases in the original
> repository.

**What this does to the runbook.** Under immutable releases, the plan's step 2
becomes coarse: assets cannot be pulled individually, so withdrawing one bad
artifact means deleting the whole release, and the version number it carried is
then **permanently unusable** — no `v1.2.3` re-cut with a fixed binary, ever,
under that name or in a same-named successor repository. A withdrawal plan that
assumes "replace the asset" or "re-tag" does not survive contact with this.

Against that, the feature gives the plan something it wanted: attestations are
generated automatically rather than needing a workflow step, and an attacker
who compromises the account cannot quietly swap a published asset — which is the
threat ADR-0005 is most concerned with.

**Still not cited:** whether the attestation itself survives deletion of the
release, which is precisely what the plan's step 2 leans on. The documentation
describes attestation creation and says nothing about its lifetime.

## What this changes

1. **`SECURITY.md`'s advisory claim can be restored, in corrected form.** The
   advisory does reach the GitHub Advisory Database; what it does not do is
   alert anyone, because nothing depends on this project as a package. Say that,
   rather than either the original claim or its blanket withdrawal.
2. **Immutable releases are a decision nobody has taken.** They are strictly
   good for supply-chain integrity and strictly bad for withdrawal
   manoeuvrability, they change what step 2 of the runbook can do, and they
   permanently burn a version number on any withdrawal. That belongs in ADR-0005
   or its own entry — it is not a detail of the release workflow.
3. **Two questions need a remote, not a reading.** Q2 and Q3 are both observable
   in minutes once one exists, and neither should be guessed at before then.
