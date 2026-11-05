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
timeouts, ignored tests and the emulator's network exposure.

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

Documents the eight shared registration contracts and their focused memory/Local commands. Real
HMAC keys and password hashes, six independently rejected writes, both proof races, stale/recreated
bindings, expiry and uncertain outcomes are checked through the actual adapter. Raw Local reads
separately prove proof TTL, cancellation retention and persistent activation. The guide explains
the crate-root recovery fixture used for login verification, its single shared
`crate::support::registration` import, and both runners' `mod support` declarations. The Local
runner continues to supply `DynamoStorage`; importing support does not change its backend. It also
explains the need for loopback-only binding when execution approval disallows the standard
launcher's all-interface listener.

Documents eight shared provider-issuance contracts with focused memory/Local commands and child
modules for outcomes/races. Coverage includes single-use bound codes, stable subjects, separation
from both other providers and an unchanged verified password account, all independent conditions,
exact orphan checks, deleted reuse policy and both race orders. Controlled session clocks and
pre-dispatch/lost-response faults preserve the distinction between rejection and outage. The Local
child checks physical TTL and raw rollback. The guide explicitly limits identity-only revocation,
upstream JWT/live-provider guarantees and rollback of earlier state/session consumption.

Includes audit commands and the split between byte-level emitter tests, real-router outcomes and
local resolved infra validation. Production stream checks use a subprocess; neither those nor the
inert-resource harness establish deployed log delivery, permissions or retention.
