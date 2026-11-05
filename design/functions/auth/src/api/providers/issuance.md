# Provider Callback Storage Errors

Source: [packages/functions/auth/src/api/providers/issuance.rs](../../../../../../packages/functions/auth/src/api/providers/issuance.rs).

## What

A private, shared mapping from typed storage errors to sanitized Google/Apple callback responses.

## Why

Confirmed conditional rejection is different from an outage, conflict or uncertain transaction
outcome. Neither kind of failure may expose storage details or a first-party authorization code.

## How

`ConditionFailed` and `AlreadyExists` map to a fixed invalid_grant (400). Every other storage variant
maps to a fixed server_error (500), including transaction conflicts and lost responses. Error
contents are discarded, not serialized or logged. The helper does not retry, compensate, redirect
or imply rollback; callbacks use it at state/session consumption and issuance preparation/finish.
