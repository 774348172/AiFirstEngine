---
name: game-engine-construction
description: Execute, validate, repair, freeze, or report construction work in G:\gameEngin. Use for active construction documents, Rust engine/editor changes, construction Gate failures, exact-commit qualification, production composition checks, Local CI, and construction completion.
---

# Game Engine Construction

Keep construction governance outside runtime and AI-facing Tool interfaces. Apply only the assurance
needed by the current change and claim; do not turn these rules into a fixed recipe.

## Establish Scope

1. Read the project skill and current repository entries.
2. Confirm this lane's active construction document and scope registered in 54, current source state, user
   authorization, and the other lane's ownership boundaries. The project permits at most three concurrent lanes (user update 2026-10-26);
   follow the project skill's isolation and integration rules. For the user-authorized 371 S5 window on
   2026-10-26, the main lane owns conflicting files/integration/build/measurement first; the secondary lane
   preserves work and waits until explicit release in 54. Continue unrelated work. Otherwise stop on a material conflict.
3. Identify the concrete change, its owners and consumers, external state it may touch, and the claim that
   validation must support.
4. Treat historical failures as evidence for relevant risks, not as a permanent suite that every later change
   must replay.

Do not modify or validate unrelated systems merely because they share a workspace. Preserve user changes and
isolate only state that can contaminate the result.

## Prevent Over-Construction

### Continue Routine Repairs Autonomously

Within an authorized construction task, diagnose and repair ordinary implementation defects, stale test
fixtures, build failures, evidence gaps, and recoverable environment issues without ending the turn to ask
whether to continue. Preserve the first failure, verify the repair against the intended contract (never weaken
assertions merely to obtain a pass), and resume the remaining authorized work. Report progress while working.
Pause for user discussion when the repair requires changing the framework design or an architectural boundary.
This preference does not authorize unrelated external changes or override an actual unavailable capability or
execution-policy restriction; identify such blockers precisely after exhausting in-scope alternatives.

Before adding a schema version, owner, transaction, migration, diagnostic family, compatibility layer, broad
test matrix, or production workflow, prove that it is necessary to fix the observed failure or satisfy the
explicit user claim.

1. Separate confirmed causes from suspected robustness gaps. A confirmed local defect must not automatically
   turn a nearby architectural concern into a construction prerequisite.
2. Write the smallest counterfactual first: identify the minimum input, branch, or owner change that would have
   prevented the failure while preserving all other observed state. Start the repair at that boundary.
3. Trace existing behavior and retained evidence before proposing new infrastructure. If the supposedly missing
   artifact, validation, rollback, or authority already existed and the failure used a different key or input,
   fix the key or input mismatch instead of redesigning the producer.
4. Keep speculative hardening separate. Record it as deferred follow-up unless the current repair can reproduce
   a failure that requires it. Do not make a generic protocol upgrade, new schema generation, migration matrix,
   or exhaustive fault matrix block a one-line or single-owner bug fix.
5. Scale tests to changed behavior. Do not run owner suites for owners that were not modified, add report fields
   that restate existing proof, or require release-level evidence for a source-level claim.
6. Prefer deleting or freezing unjustified Gates over preserving them because they were already documented or
   activated. When review disproves a construction premise, correct the formal design and active construction
   document before implementation.

For every construction document, include a short necessity and validation-economy audit before the Gate list:

```text
requested claim or confirmed failure
minimum causal change and files that must change
explicitly excluded or deferred work
Gate -> risk/failure contract -> cheapest red-capable test -> terminal evidence
reused evidence -> consumed identities -> invalidation condition
estimated Gate cost -> stop/review condition
```

Every Gate, test, runner, and helper must map to one explicit risk or failure contract. Remove anything that
cannot be connected to the requested claim or confirmed failure through this audit. Architectural completeness
is not, by itself, authorization to expand construction. If actual cost materially exceeds the estimate, stop
the expensive branch and review its premise before adding time, infrastructure, or validation scope.

## Keep Small Diagnostics Small

Treat a defect as a small diagnostic while the observed failure can still be explained and repaired inside one
owner without changing a schema, external state, production binary, or release claim.

1. Use the shortest real reproduction needed to confirm the user's symptom, then convert the captured condition
   into a deterministic owner-level red test. Do not keep the real Editor, GPU, or installed composition as the
   inner repair loop after the owner test can represent the failure.
2. Before adding a temporary scenario, runner, or source-linked real-window test, verify that the existing test
   binary actually contains the required project runtime, features, and composition. If it does not, stop that
   branch immediately; missing composition is evidence that the harness is ineligible, not permission to build a
   candidate, qualify an artifact, or enter promotion work.
3. Set a short exploration budget for each new diagnostic branch. When the branch exceeds that budget without
   improving the red signal or excluding an owner, stop it, preserve the useful evidence, and return to the
   smallest reproducible boundary. Do not preserve a failed exploratory branch merely because time was already
   spent on it.
4. Separate preparation/Cook, build/export, and actual Player/test time in reports. File publication boundaries
   are stage estimates, not proof of compiler-only duration. Reuse the established owner cache and effective
   build environment; Compiler export and direct engine Cargo builds have different cache owners (see
   [371 export procedure](references/371-construction-lessons.md#6-compiler导出输入与构建环境2026-10-21)).
   A failed attempt's total duration is not all avoidable waste. Compilation or tool timeout without a test
   verdict does not prove a runtime failure; do not widen the suite to compensate for that cost or classify
   an allocation failure as environmental without causal evidence.
5. Reject a new helper, runner, or temporary workspace when compiling or executing it covers materially more
   code than the behavior under test. Return to an existing owner seam, already-qualified artifact, or narrower
   harness. Delete throwaway harnesses after use or mark them as run-owned; do not promote them into permanent
   architecture without a separately demonstrated recurring need.
6. Keep documentation synchronization out of the diagnosis loop. Update the active document only when scope or
   evidence materially changes; write completion records and indexes once after the source claim is proven.
7. Require a new claim and the corresponding authorization before escalating a source repair into project-linked
   composition, exact-commit qualification, binary sealing, cache rebuild, promotion, production replacement, or
   installed-state validation.
8. Report source completion and installed availability separately. A green source regression does not mean the
   running production Editor contains the fix.

Use this default path for a one-owner UI or runtime defect:

```text
one bounded real reproduction
-> deterministic owner red test
-> one-owner fix
-> owner test + direct consumer test
-> affected formatting/check
-> one completion-document sync
```

Expand this path only when a failing signal proves another owner or when the user's explicit claim requires a
different environment. Never expand it merely to make the evidence look more release-like.

## Select Assurance By Claim

Choose the smallest applicable level. A construction document may combine levels, but must not require every
level by default.

### Development

Use a fast red-capable test at the owner boundary while implementing or diagnosing. Run directly affected
consumers when a shared contract changes. Dirty-worktree results are useful feedback but are not release
evidence.

### Integration

Use affected-domain tests when the change crosses crates, processes, fixtures, generated contracts, or platform
boundaries. Bind the result to a clean exact commit only when the result is claimed for that commit.

### Release Or Activation

Use production composition, complete declared matrices, installed binaries, real Editor sessions, or external
configuration only when the user-visible claim depends on them. State the profile, features, platform, binary,
configuration, and source identity actually being qualified. Require explicit user authorization before changing
real external configuration or consuming a one-shot real-world acceptance attempt.

Targeted tests cannot prove a different environment or composition. Conversely, a release-level run is not a
debugging loop.

## Diagnose Repeated Performance And Build Blockers

Use these rules when a scale/performance Gate fails or successive windows repair the same symptom. For the
observed 371 failures and their evidence, read [371 construction lessons](references/371-construction-lessons.md)
when reviewing a stalled construction or a similar blocker; do not replay its historical suites.

- Check the harness against the authoritative measurement contract before another optimization. Separate
  diagnostic feedback, correctness checks, and formal qualification. A Trace/probe cost may block formal
  measurement only when the contract explicitly makes it a prerequisite; a correctness or composition failure
  still blocks an ineligible run. Repair a contradictory harness with a red test preserving the formal limits.
  Historical diagnostic failures do not prove formal performance failure when the formal window never ran.
- When successive fixes leave the same scale symptom unresolved, review the whole affected path before choosing
  another local hotspot. Reuse existing stage data and bounded counters to distinguish preparation, update,
  graph validation/compile, encoding, submit, and GPU execution as relevant. Verify which prior optimizations
  already work. Inspect work that scales per draw/resource/frame, and rank measured residual costs; do not
  prescribe a new cache, arena, scheduler, or backend fork from an unconfirmed suspicion. Nested spans cannot be
  added, and Debug timings cannot be treated as Release qualification.
- Evaluate the diagnostic's total cost, including Cook and rebuilding the composition. A seconds-long Player
  inside a tens-of-minutes build is not a cheap inner loop. Prefer existing qualified reports/binaries where
  their identities and instrumentation suffice, or an existing owner seam for a deterministic reproduction.
  Keep a necessary full-composition reproduction bounded, then return to the owner loop. Minimal inputs locate
  causes; the original frozen load remains required for formal scale qualification.
- For allocation failure, LLVM OOM, stack overflow, or an abnormal build exit, preserve stderr, exit status,
  last successful phase, produced artifacts, and the owned process tree. Distinguish Cargo/compiler, export,
  launcher, and Player failures using actual evidence. Obtain bounded phase/process memory observations on a
  justified diagnostic attempt where available; post-failure free memory is not failure-time peak evidence.
  Do not assume a clone, pagefile, machine capacity, or stack root cause from an exit code alone.
- Retry an expensive build only after stating what cause, input, launch method, or observability changed and
  what the retry will resolve. If it again fails before Player without a narrower causal signal, stop blind
  full builds and isolate the failing owner/phase. Limiting compile concurrency can address compiler pressure
  but does not prove that export/package memory is fixed. No new build or Player means runtime qualification
  is pending; do not seal or substitute an older binary as the failed attempt's output.
- Keep permission/instrumentation blockers separate from product correctness and performance. An unavailable
  OS event chain does not justify indefinitely expanding private decoders or inventing frame correspondence.
  Review the required evidence and existing observation paths, record unmatched/pending results honestly, and
  change an architectural requirement only through the existing design process and user scope.

## Eliminate Duplicate Work

- Do not rerun a filtered test when a later suite executes the same test under the same inputs and environment.
- Do not run both a preflight suite and an authoritative suite merely to obtain two labels for identical evidence.
- Run feature or platform variants only for their owners and affected dependency/consumer closure unless a
  release policy explicitly requires a broader matrix.
- Stop an expensive sequence when an upstream failure already makes the claimed result ineligible. Continue only
  when the additional stage is intentionally collecting distinct evidence.
- Reuse valid evidence when all inputs it consumed are unchanged. Record the reuse instead of rerunning the
  producer.
- Keep a Compiler operation's consumed source unchanged from prepare through publication/verification.
  Finish edits and cheap owner checks first; edit the next variant only after the operation ends, or in a
  separately bound project copy. Preserve drift rejection; refresh/prepare after intentional source changes.
- Keep effective toolchain, features, profile and build environment consistent across reusable artifacts.
  A previous shell call's environment is not persistent; pass required values in the same invocation as the
  producer. On an unexpected cache miss, compare its report and consumed identities before another full build.
- Prefer one repository-owned runner when separate runners would execute the same stages and consume the same
  inputs. Do not create a construction runner unless orchestration itself is reusable product-independent
  infrastructure.
- After a broad suite fails, diagnose with the narrowest reproducible owner test. Prove the repair with that test,
  then run the required terminal suite once; do not repeatedly use the full crate, workspace, or real environment
  as the edit loop.
- Make high-cost suites fail fast. Before starting one, reserve enough time to preserve evidence and clean owned
  state; do not start it merely because it appears in an older construction document.

## Bind Evidence To Its Inputs

Track identities separately:

```text
product source and dependencies
test or verification harness
generated contract or fixture
production binary and launch composition
external configuration or installed state
```

A change invalidates only evidence that consumed the changed identity. A test-only change does not automatically
invalidate an unchanged production binary or a receipt proving that binary's behavior. A source-level pass does
not prove an installed or feature-gated binary when those identities differ.

Use a clean exact-commit source when publishing commit-bound integration or release evidence. Development testing
does not require a repository-external worktree unless existing changes, parallel work, or test side effects can
contaminate the result.

## Preserve Actionable Failure Evidence

- Capture the first failing assertion or diagnostic with enough stdout, stderr, trace, fixture, and environment
  context to reproduce it. A test name and exit code alone are invalid diagnostic evidence.
- Build a tight owner-level reproduction before changing code. Do not use Local CI or a complete workspace as the
  repair loop.
- Identify the observed failing stage and classify its cause as design, construction-document/harness,
  implementation, environment, or pre-existing baseline when evidence supports it. Keep the root cause unknown
  when not established; this must not prevent a bounded in-scope diagnosis.
- After repair, run the reproduction and affected consumers. Renew only the qualification evidence invalidated by
  the repair.
- Govern retries by side effects and evidence identity. A non-mutating test may be rerun after its cause or
  evidence gap is addressed; a real one-shot attempt still requires fresh eligibility and authorization.
- One explicit Local CI construction authorization covers the initial run and any later Local CI reruns in the
  same continuous construction window. A rerun does not require another user authorization, but it must use a
  fresh run root and follow owner-level diagnosis/repair when the previous run exposed a real failure. Do not use
  Local CI as the repair loop. The authorization ends when the work completes, the agent stops or reports back,
  external-state scope changes, or the three-hour limit is reached; continuing afterward requires a new user
  authorization.

## Protect External State

When a test launches processes or mutates files, ports, pipes, devices, installations, or configuration:

- give each concurrent role exclusive mutable state;
- keep ownership explicit so cleanup never targets unrelated state;
- preserve required evidence before cleanup;
- terminate owned descendants on timeout or abnormal exit;
- make configuration changes transactional and report both persistence and rollback outcomes;
- never claim a configuration reload or typed Tool activation that the real host has not observed.

Standing user authorization (2026-10-15): clean verified construction-owned temporary test directories,
including exact owned children of the system temporary directory outside the repository, without asking again.
For these tests, declare the resolved system temporary directory as the allowed parent and prove ownership of
that exact disposable child; never clean the shared temporary parent or unrelated siblings. Preserve required
evidence and apply the path, reparse-point, and process checks below. This authorization excludes user projects,
installed files/configuration, shared caches/evidence roots, and other tasks' directories. If the exact target is
already absent, record `missing` rather than claiming deletion or retention. User authorization does not override
host execution policy: an explicit policy rejection must not be bypassed by another API, shell, or helper.

Recursive cleanup is permitted for a completed role-owned temporary root. Resolve the literal absolute target
first and prove that it is a strict child of the declared repository-external role root. Reject workspace roots,
home directories, drive roots, shared artifact roots, unresolved variables, globs, and any target or ancestor
whose reparse point can escape the role root. Preserve required evidence and confirm that no owned descendant is
still running. Then use one native PowerShell
`Remove-Item -LiteralPath <exact-target> -Recurse -Force` command and verify that the target no longer exists.
If deletion fails because of an ordinary filesystem error, diagnose that error within the same verified scope.
If host policy rejects the action, record `retained_by_host_policy` only when the target still exists, preserve the
exact rejection, and stop cleanup; do not retry through a different API or shell to bypass policy.

When the user explicitly authorizes partial recursive cleanup, apply the same proof separately to each named
role-owned `build`, `cargo-target`, `temp`, or failed-state child directory. List the exact literal targets before
deletion; require every target to be a strict child of one exact role root; reject the role root itself, production
build targets, user projects, another role root, and any unlisted sibling. Check the target and every ancestor up
to the role root for symlink, junction, or other reparse-point escape. Preserve the role's evidence directory and
any state not explicitly classified as failed or disposable. Record each target as `removed`, `missing`, or
`retained_by_host_policy`. A cleanup failure does not invalidate or rerun already valid qualification evidence.

Binary sealing, reopen verification, configuration lineage, and byte-exact rollback are required only when the
claim includes that binary or configuration transaction.

## Apply Repository Quality Contracts

Use repository-owned producers for committed quality truth. For the Rust lint/debt contract, use:

```powershell
Set-Location G:\gameEngin\rust
cargo run -p quality_gate --locked -- propose-ledger
```

Review reported drift; never auto-apply a generated ledger. Run format, lint, architecture, suppression, default,
feature, platform, and hygiene stages according to the affected scope or the explicitly declared release policy.
No stage is universal merely because it exists.

## Enforce The Three-Hour Limit

No continuous construction authorization may exceed three hours including implementation, tests, waits, evidence,
and cleanup.

- Split work before starting when the conservative estimate cannot fit.
- Do not start a stage whose deadline plus evidence and cleanup reserve exceeds the remaining window.
- At 2 hours 45 minutes, start no new implementation or validation; preserve evidence, restore external state,
  terminate owned processes, and clean required temporary state.
- At 3 hours, stop and report `stopped_by_time_limit`. A new user authorization is required to resume.

The time limit is an operational boundary, not a reason to fragment a short construction into artificial Gates.

## Finish

Define a user-requested large block by explicit exit outcomes and remaining dependencies, not by one local fix
or a window's test count. On continuation, reuse completed evidence and name the unresolved outcomes. If a block
does not close, report it as partial with the actual blocker; do not repeatedly relabel its next hotspot as a
new completed large block. Distinguish source functionality, formal scale/performance, platform observation,
tool consumption, and installed availability. Do not present elapsed time limits as the root cause of defects
or inefficient validation. Do not promise remaining duration without measured stage costs and unknowns.

Report the changed scope, relevant source and artifact identities, tests actually run, failures and reruns, reused
evidence, elapsed time, external-state restoration, remaining risks, and omitted tests with reasons. Synchronize
and archive construction documents only when their required claims are proven.
