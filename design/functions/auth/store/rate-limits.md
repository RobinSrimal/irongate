# Rate Limits

Implementation: [store/rate_limits.rs](../src/store/rate_limits.md),
[policies](../src/config/rate_limits.md) and [source identity](../src/ratelimit/middleware.md).

## What And Why

Persisted fixed-window counters enforce independent source and secondary budgets across Lambda
instances. Changing emails, client IDs or provider sessions cannot reset the source ceiling;
changing source IP cannot reset the normalized-email ceiling.

## How

Defaults use 60-second windows, separately for each endpoint:

| Endpoint | Source | Secondary |
| --- | ---: | --- |
| Register, login, forgot | 60 | 5 per normalized email across sources |
| Verify, reset completion | 5 | None |
| Authorize | 100 | 100 per configured client/source |
| Token | 100 | 50 per configured client/source |
| Revoke | 60 | 20 per configured client/source |
| Google/Apple start | 60 combined | 20 per provider/session-digest/source |

Only API Gateway v1/v2 context supplies source IP. Parsed IPs are canonicalized, including
IPv4-mapped IPv6. Invalid or absent context shares one unknown-source identity; forwarded headers
are ignored. Unknown OAuth client names share a structural `None` secondary identity, distinct from
every registered name. Malformed emails charge only source. No account lookup chooses a budget.

Identifiers use separate HMAC families for source, email, client/source and provider/source, with
JSON tuple framing for compound inputs. Counter keys contain a dimension label plus digest, not raw
IPs, emails or flow secrets. Counter values contain count and window start. Limiter diagnostics omit
identifiers and backend error text; this does not change other audit logging.

Admission charges source first, then the secondary counter. Source denial allocates no secondary
row. Secondary denial or failure retains the source charge: this is conservative accounting, not
an atomic multi-counter transaction. Each bucket uses a consistent read and at most eight CAS
attempts. Runtime window expiry is independent of cleanup TTL; successful writes set TTL to the
evaluation time plus twice the window.

Exhaustion returns 429 with positive remaining-window Retry-After, rounded up. CAS exhaustion returns
429 with a one-second retry. Operational/serialization failures and malformed counters return
generic 503. A lost write response does not admit a request or refund a possibly committed charge.
Enabled policies require correct dimensions, counts in 1..=1,000,000 and windows in 1..=86,400 seconds.
Missing/invalid policies, wrong identity type or an empty secret fail closed before counter I/O.
Only explicit `enabled=false` bypasses accounting.

## Boundaries

These are application admission limits after request parsing, not an edge DDoS control. Email limits
can temporarily deny a targeted address; source limits affect users behind shared NAT/BFF egress.
API Gateway sees the BFF's outbound IP, not its browser user's IP. Two-dimensional checks normally
need two reads and conditional writes, with bounded retries. Defaults are starting points, not load
measurements. Shared [memory](../tests/rate_limits.md) and
[DynamoDB Local](../tests/dynamodb_storage/rate_limits.md) contracts cover races and failures without
claiming production availability or complete storage equivalence.
