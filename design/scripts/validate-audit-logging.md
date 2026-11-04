# Audit Infra Validation

Source: [scripts/validate-audit-logging.mjs](../../scripts/validate-audit-logging.mjs).

## What

A local resolved-config harness included in npm run test:infra.

## Why

Static strings alone do not prove both functions consume the logging settings.

## How

Loads actual stage/config/logging/API modules in isolated VMs, stubbing only side-effecting
bindings, packaging and resource constructors. Dev/production must resolve two functions with
finite retention, JSON/INFO, the expected mode and preserved group/system settings. Invalid overrides
and injected checked-in defaults must fail. Installed SST source checks supplement resolved tests
for managed logging roles and retention. Uses fake bindings and no cloud credentials; cannot prove
deployed IAM, CloudWatch delivery or retention enforcement.
