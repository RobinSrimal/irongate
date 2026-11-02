# Runtime Performance

Implementation: [Lambda definitions](api.md), [runtime state](../../functions/auth/src/config.md)
and [provider clients](../../functions/auth/providers/README.md).

## What And Why

The public ARM64 Lambda uses 256 MB and the admin Lambda 128 MB; both have a 30-second timeout.
Public auth performs password hashing, signing, provider exchanges and email delivery, while the
admin entrypoint performs account lifecycle operations. These are configuration defaults, not
benchmark-derived service-level guarantees.

## How

Runtime initialization constructs shared DynamoDB, email/provider and signing state. Warm invocations
reuse those instances. Google/Apple clients cache JWKS for ten minutes and allow one forced refresh
on an unknown key ID. Local signing uses configured key material; KMS signing adds an AWS request.

External HTTP calls do not all have explicit application-level timeouts/retry policies.
Lambda timeout is a total execution bound, not a per-provider retry strategy.

Current tests check behavior with local fixtures/fakes. There is no checked-in load-test harness or
current cold-start/throughput report establishing these memory settings as optimal. Operational
logging must not include token, password, link or private-key material.
