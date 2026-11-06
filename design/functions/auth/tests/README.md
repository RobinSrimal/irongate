# Test Layout Guide

Source: [packages/functions/auth/tests/README.md](../../../../packages/functions/auth/tests/README.md).

## What

The contributor guide to domain-oriented integration tests.

## Why

Tests are organized around observable auth contracts rather than delivery history.

## How

Explains source-local pure tests versus cross-module router/store tests, support fixtures and the api_routes/password child module. It documents naming and execution, not a claim that every storage race is covered.

Documents ordinary SDK wire tests and explicitly enabled DynamoDB Local transaction tests, Java
prerequisites, in-memory startup and Cargo commands. The guide separates emulator coverage from
production guarantees and explains dummy credentials, loopback restrictions, unique-table cleanup,
timeouts, ignored tests and the emulator's network exposure. SDK wire fixtures check that both
query paths reject missing/non-string returned keys without exposing sensitive diagnostics.

The local suite includes a refresh child module for TTL renewal, revoked-state retention, runtime
expiry and pre-commit races. It distinguishes raw physical TTL inspection from asynchronous cleanup
and uses real backend conditions rather than fabricated transaction failures or call counts.

The guide also describes recovery invalidation/rejection, ordered credential and lifecycle races,
and controlled pre-dispatch versus post-commit response-loss faults. A recovery-only clock tests
expiry without sleeps. It distinguishes observed database state from assumptions based on responses.

Independent admission cases share one contract between memory and Local targets. The guide includes
focused commands, source/email race gates, conservative failure accounting and physical counter TTL
checks. Route children vary identities and verify rejection before protected work. Advancing limiter
evaluation time is explicitly separate from storage time and TTL cleanup.

Documents shared registration contracts and their focused memory/Local commands. Real
HMAC keys and password hashes, eight independently rejected operations, both proof races,
stale/recreated bindings, expiry and uncertain outcomes are checked through the actual adapter.
Raw Local reads
separately check proof TTL, cancellation retention and all seven persistent activation rows,
including email ownership and its subject index. Pending/resend state makes no ownership claim.
The guide explains the crate-root recovery fixture used for login verification, its single shared
`crate::support::registration` import, and both runners' `mod support` declarations. The Local
runner continues to supply `DynamoStorage`; importing support does not change its backend. It also
explains the need for loopback-only binding when execution approval disallows the standard
launcher's all-interface listener.

Documents shared ownership contracts in `support/email_ownership.rs` and its lifecycle/races
children, run by `registration_contract.rs` and the Local `ownership` module. Coverage includes
active-owner rejection without linking, malformed records/read failures, retention boundaries,
snapshot conditions, lifecycle cleanup/retry and ordered deletion/reuse/stale-index races. The
focused Local command uses `--test dynamodb_storage ownership:: -- --ignored`. Raw Local cases
check no physical TTL, subjectless tombstones and malformed index values failing closed through
both query paths. Retrying deletion after fixture-only repair must complete cleanup without
pretending earlier deletion steps rolled back. These use fresh acceptance data, not historical
backfill, and do not claim whole-deletion atomicity or exhaustive TTL/corruption coverage.

Documents shared provider-issuance contracts with focused memory/Local commands and child
modules for outcomes/races and shared provider ownership. Coverage includes single-use bound codes, stable
subjects and authoritative Google/Apple resolution without credential replacement,
all independent conditions,
exact orphan checks, deleted reuse policy and both race orders. Controlled session clocks and
pre-dispatch/lost-response faults preserve the distinction between rejection and outage. The Local
child checks physical TTL, persistent email ownership and raw rollback. Both providers' cases include
malformed owner/index state, proof binding, relay separation, deletion/reuse, competing claims and
both password completion orders. The guide explicitly limits identity-only revocation,
upstream JWT/live-provider guarantees and rollback of earlier state/session consumption. Google
fixtures explicitly model Workspace authority; signed callback cases cover missing/invalid authority,
unsigned overrides, cross-provider sharing and stable returning identity. Cross-provider races cover
both competing fresh claims and valid concurrent attachments to an unchanged owner.

Includes audit commands and the split between byte-level emitter tests, real-router outcomes and
local resolved infra validation. Production stream checks use a subprocess; neither those nor the
inert-resource harness establish deployed log delivery, permissions or retention.

Documents shared Google confirmation contracts, reconstructed stores, proof/request bindings,
independent transaction conditions, replay, deleted reuse, deterministic expiry and lifecycle/claim
races. The memory runner checks confirmed codes through `/token`; the Local child verifies physical
TTL, creation's returned deadline and raw cancellation. Focused commands run both targets without
live services. The guide distinguishes storage contracts from a public confirmation flow.

Documents internal Google confirmation delivery contracts with a captured sender, actual generated
proof completion, independent send budgets and bounded transaction/delivery gates. Invalid inputs,
admission/storage outages, committed response loss, accepted-then-failed delivery and expiry retain
their distinct outcomes without retry/refund/cleanup. Renderer checks cover leading zeros, exact
deadline and escaping. The guide does not claim public code-attempt protection, browser acceptance
or live email delivery; public Google callbacks remain unchanged.

Documents shared provider-state memory/Local contracts, typed browser/provider admission,
exact-value delete/replay, expiry and deterministic races/faults. Local checks raw TTL and rejection
retaining state. Google HTTP tests exercise separate host cookies, both completion orders,
invalid/duplicate proofs, cleanup and one winning exchange; existing signed-token tests retain
valid cookies to reach validation. The guide distinguishes HTTP fixtures from browser enforcement
and live acceptance, and records consolidation of duplicate state tests from the start suite.
