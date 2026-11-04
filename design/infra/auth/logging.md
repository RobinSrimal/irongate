# Function Logging

Source: [infra/auth/logging.ts](../../../infra/auth/logging.ts).

## What

Shared logging arguments for the existing public/admin SST Functions.

## Why

Typed stdout audit events need finite retention and a compatible Lambda application-level filter.

## How

Returns logging.retention/format plus the supported function transform. It resolves loggingConfig
through $util.output, preserves the log group/system settings, and sets JSON with INFO application
level. Existing SST groups/roles and stream/write permissions remain in use; no SDK client or extra
permission is added. None is a runtime audit mode, not a request to remove operational logs.
