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
