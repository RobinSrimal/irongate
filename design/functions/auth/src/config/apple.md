# Apple Configuration

Source: [packages/functions/auth/src/config/apple.rs](../../../../../packages/functions/auth/src/config/apple.rs).

## What

Fixed Apple OIDC endpoints and optional Services ID, team, key and private-key configuration.

## Why

Provider enablement must not leave a partially configured authentication path.

## How

Absent provider inputs disable Apple; partial inputs fail. The secret resolver loads a PKCS#8 key with redacted Debug output. Client-secret lifetime is validated with a one-day default and six-month upper bound.
