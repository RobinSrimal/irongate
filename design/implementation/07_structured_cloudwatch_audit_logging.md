# 07_structured_cloudwatch_audit_logging

## Status

Implemented and locally verified on 2026-11-04 following the user's implementation request.
Builds on [independent rate limits](06_independent_source_and_account_rate_limits.md), committed as
`923bedb`. No deployment, cloud reconfiguration or historical-data cleanup was performed.

## Goal

Make `AUTH_AUDIT_LOG_MODE=cloudwatch` emit safe structured security events through the existing
public/admin Lambda logs, with finite retention and visible local emission failures.

Keep authentication and lifecycle outcomes independent of best-effort audit delivery. Remove the
misnamed DynamoDB audit persistence path without adding a service, destination, queue or new flow.

## Reviewed Sources

Irongate baseline: `923bedb`, clean before this definition. Reference repository:
`/Users/robinsrimal/Desktop/64x64`, pinned to `4e7207887f86aacba19d1bcf7ffea312055d5620`.
The definition was reviewed against that reference. At implementation start the reference HEAD was
`e1a625df38e993d5aff99f0a7f95d896d5d0e19f` with a clean worktree; the port remained pinned to the
reviewed commit. Newer reference work and product-specific behavior were not imported.

Reference files inspected at that commit:

- `packages/functions/auth/src/audit.rs`, `src/audit/events.rs` and `src/audit/tests.rs`.
- `packages/functions/auth/tests/admin_lifecycle/audit.rs` and `oauth_refresh_revoke/audit.rs`.
- `infra/auth/logging.ts` and `scripts/validate-audit-logging.mjs`.
- `design/implementation/80_irongate_audit_logging.md`; implementation commit `2de9aa5`.

Adapt reusable emitter, event and verification behavior, not the reference's email-first fixtures,
provider-email-confirmation event, product configuration or deployment evidence. Irongate retains
its password-first contract, credential-generation guards and independent rate limits.

## Starting Gaps

| Area | Current Irongate behavior | Required correction |
| --- | --- | --- |
| Destination | Enabled audit writes `audit` rows to AuthTable | Emit to existing Lambda application logs |
| Retention | Audit rows have no TTL; CloudWatch retention does not cover them | New events use existing finite function-log retention |
| Schema | Arbitrary event name, raw IP and free-form detail are accepted | Typed events with bounded, allowlisted fields |
| Failure handling | OAuth/admin callers discard write errors | One centralized safe fallback diagnostic on local failure |
| Admin attribution | Events identify the target but not the operator | Separate actor derived only from trusted IAM context |
| Rotation ordering | Rotation event follows response signing | Emit confirmed storage rotation before signing can fail |
| Evidence | Disabled/random-token tests assert only absence of table rows | Assert actual emitted events alongside state and responses |

Both entrypoints already configure JSON operational tracing. Both SST functions already have JSON
logging, a shared finite-retention setting and SST-managed roles. Checked-in dev and production
retention defaults are 30 days. The retention parser validates environment overrides but casts the
checked-in fallback; validate both paths. Preserve these resources and defaults.

## Design To Honor

- [Overview](../overview.md), [threat model](../functions/auth/threat-model.md) and
  [testing](../functions/auth/testing.md).
- [Audit overview](../functions/auth/observability/audit.md),
  [implementation mirror](../functions/auth/src/audit.md) and
  [mode parsing](../functions/auth/src/config/audit.md).
- [Public state](../functions/auth/src/config.md),
  [runtime configuration](../functions/auth/src/config/environment.md),
  [auth startup](../functions/auth/src/main.md) and
  [admin startup](../functions/admin/src/main.md).
- [Typed store](../functions/auth/src/store/mod.md),
  [token](../functions/auth/src/oauth/token.md), [revoke](../functions/auth/src/oauth/revoke.md),
  [admin implementation](../functions/auth/src/api/admin.md) and
  [admin API](../functions/admin/api.md).
- [API infrastructure](../infra/auth/api.md), [infra configuration](../infra/auth/config.md),
  [stage configuration](../infra/shared/stage-config.md) and
  [test instructions](../../packages/functions/auth/tests/README.md).
- [Stage setup](../../docs/setup/03-stage-config.md),
  [deploy instructions](../../docs/setup/09-deploy-auth-core.md) and
  [operational smoke guide](../../docs/operations/smoke-test.md).

Update authoritative documents when their implementation changes. Until then they should continue
to describe the current DynamoDB behavior honestly, not advertise this proposal as complete.

## Scope And Alternatives

Use one small injectable emitter shared by auth/admin. Production writes bounded JSON lines to
stdout; an independent stderr path reports local failures. Lambda owns downstream log delivery.
Tests inject capturing/failing writers through the same serialization path.

A plain `tracing::info!` call is simpler, but ordinary tracing filters and its output-error handling
would obscure the explicit audit enablement/failure contract. A small synchronous writer keeps that
boundary observable. Keeping DynamoDB and adding TTL would retain a second persistence mechanism
and still contradict the selected `cloudwatch` mode. Do not implement both paths.

No direct CloudWatch SDK calls, background task, buffer for later delivery, new log group, new AWS
service, additional audit-retention setting or runtime permission expansion. Only existing Lambda
logging settings may be adjusted in source; applying them requires separate deployment approval.
If implementation discovers a need for a larger architecture change, stop and discuss it first.

Out of scope: new event coverage, login-attempt history, dashboards, alarms, S3 export, account
linking, email actions, native auth, provider changes, Cloudflare/example changes, general request-log
sanitization, database migrations and live operations. Existing unrelated logging risks must not be
described as fixed merely because the audit emitter is safer.

## Emission Contract

### State And Modes

- Keep `cloudwatch | none`, defaulting to `cloudwatch`; reject unknown values during startup.
- Add the emitter to public/admin application state. Each production entrypoint explicitly installs
  its real output/fallback writers using the parsed mode. No silent no-op production default.
- The emitter owns mode enforcement. `none` invokes neither sink and writes no audit database rows;
  it does not disable operational tracing, API Gateway logs or their retention.
- Remove AuthStore audit methods and storage-dependent audit functions. Use one current interface,
  with no persistence alias, dual write or runtime backend-selection framework.
- Adapt test constructors narrowly. They must explicitly choose capture/disabled/failing emitters;
  do not refactor unrelated provider/store fixtures just because AppState gains a field.

### Records And Privacy

Emit one compact newline-terminated JSON record with `record_type=auth_audit`, `schema_version=1`,
a generated UUID, RFC3339 timestamp, `level=INFO` and a fixed typed `event_type`.
Serialize, validate and enforce a total size strictly below 8 KiB before writing.

Keep only the ten existing event kinds:

| Event | Allowed event fields | Confirmed boundary |
| --- | --- | --- |
| `authorization_code_exchanged` | client_id, subject | Successful exchange preparation, before response return |
| `refresh_token_issued` | client_id, subject, token_hash | Initial refresh credential persisted |
| `refresh_token_rotated` | client_id, subject, token_hash | Rotation committed, before response signing |
| `refresh_token_reuse` | client_id, token_hash | Store returns the existing reuse-detected outcome |
| `refresh_token_revoked` | client_id, token_hash, typed reason | Revoked or already-revoked result for that client |
| `admin_account_read` | actor, subject | Account successfully read |
| `admin_account_disabled` | actor, subject, revoked_refresh_families, deleted_password_secrets | Existing disable/cleanup sequence succeeds |
| `admin_account_enabled` | actor, subject, revoked_refresh_families, deleted_password_secrets | Existing enable/cleanup sequence succeeds |
| `admin_account_deleted` | actor, subject, revoked_refresh_families, deleted_identities, deleted_password_users, deleted_password_secrets | Existing deletion sequence succeeds |
| `admin_subject_sessions_revoked` | actor, subject, revoked_refresh_families | Subject session revocation succeeds |

Use enums and numeric counts instead of arbitrary names or free-form detail. Revoke reason is
`revoked | already_revoked`. Subjects come from confirmed internal account/credential records;
client IDs come from the configured registry. Token references remain validated 64-character HMAC
digests, never bearer values. Bound identifier lengths and reject invalid/control-character values
without truncating them into collisions. Do not change OAuth client-validation rules just to fit
a copied audit regex; audit validation failure uses the safe diagnostic path, not an auth failure.

Admin actor comes from the existing trusted API Gateway IAM authorizer context, preferably user ARN
with a deterministic caller/user-ID fallback consistent with the current guard. Never use headers,
body fields or the target subject as actor evidence. Keep IAM authorization unchanged. Operator ARN,
client ID, subject and digest are identifying correlation data, not anonymous information; an IAM
session identifier may itself contain operator-identifying text.

Remove raw-IP and free-form-detail fields entirely. Do not emit end-user email claims, passwords,
raw codes/tokens/session IDs, cookies, headers, request bodies, secret-bearing URLs, provider payloads,
credentials or raw errors. This restriction applies to the audit output and its failure diagnostic.

### Ordering And Failure

Perform one synchronous local emission attempt before the audited handler returns. Hold a
process-wide output lock over complete writes and flush, including partial-write handling, so
concurrent emitter instances do not interleave their records. Do not spawn detached logging work.

Centralize serialization, validation, size and output failures. Attempt one bounded
`record_type=audit_emit_failed`, `level=ERROR` JSON diagnostic through an independent fallback writer.
Include only envelope metadata, generated event ID, fixed event kind and a fixed failure category;
never include the rejected payload, actor/subject, token digest or underlying error string.
If fallback also fails, terminate without recursion, panic or unbounded retries.

Emission failure does not roll back a completed change, turn success into an auth error or turn a
replay rejection into success. Remove caller-side discarded audit results by making this behavior
the emitter's responsibility. Preserve the existing OAuth and admin response contracts.

Move rotation emission immediately after confirmed storage rotation, before JWT signing. Keep
initial issuance after persistence and exchange success after successful preparation. Rejected code
exchange, unknown/foreign refresh revocation and denied admin calls must not produce success events.
Already-revoked tokens remain idempotent and may emit the corresponding typed result.

Do not change credential/lifecycle transactions to make audit atomic. Admin cleanup is multi-step;
a partial failure can leave changes without a completed-action event. Likewise, response delivery
can fail after a confirmed event. A successful local write is not a CloudWatch acknowledgement;
delivery is not transactional, exactly once, tamper-proof or compliance-grade. Partial output can
leave a broken line, and total output failure cannot reliably report itself.

## Existing Logging Infrastructure

Reuse `infra.logRetentionDays` / `AUTH_LOG_RETENTION_DAYS` and both current function log groups.
Validate checked-in fallback values as well as overrides against the supported finite day counts.
Retain 30-day defaults and independent API Gateway access logging.

The installed SST Function component exposes `logging.retention`/`logging.format`, supplies a log
group and attaches its normal logging policy. It does not expose an application-level filter in
that high-level object. A small `infra/auth/logging.ts` helper may use the supported function
transform to preserve the resolved logging config and explicitly set JSON/INFO on both existing
Lambdas. Do not overwrite their log-group names, system settings, roles or other transforms.

Audit emission must remain independent of ordinary tracing verbosity. INFO audit and ERROR fallback
records need compatible function filtering. Verify actual local resolved configuration rather than
inventing an SST option or assuming access-log retention also configures the functions. Preserve
SST's existing stream/write permissions; no runtime CloudWatch client is needed.

AWS documents Lambda capture subject to execution-role logging permissions and custom-runtime JSON
level/timestamp handling. Local configuration tests do not establish deployed delivery or IAM.
Sources checked for this definition: [Lambda log delivery](https://docs.aws.amazon.com/lambda/latest/dg/monitoring-cloudwatchlogs.html),
[log-level filtering](https://docs.aws.amazon.com/lambda/latest/dg/monitoring-cloudwatchlogs-log-level.html)
and [SST Function](https://sst.dev/docs/component/aws/function/), alongside the installed
`.sst/platform/src/components/aws/function.ts` implementation.

Stop creating DynamoDB audit rows. Existing rows without TTL are outside CloudWatch retention and
remain untouched. Document that limitation; do not scan, backfill, export or delete them here.
No backwards-compatible reader or automatic cleanup is required.

## Bounded Change Surface

Production changes belong in:

```text
packages/functions/auth/src/audit.rs
packages/functions/auth/src/audit/events.rs            focused typed schema if useful
packages/functions/auth/src/config.rs
packages/functions/auth/src/config/audit.rs
packages/functions/auth/src/config/environment.rs     only if mode-loading tests require it
packages/functions/auth/src/main.rs
packages/functions/auth/src/store/mod.rs              remove audit facade
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/revoke.rs
packages/functions/auth/src/api/admin.rs
packages/functions/admin/src/main.rs
infra/auth/api.ts
infra/auth/config.ts
infra/auth/logging.ts                                 small shared function config helper
```

Keep emitter tests in a focused child module if needed. Put route coverage in
`tests/admin_lifecycle/audit.rs` and `tests/oauth_refresh_revoke/audit.rs`, with a small shared capture
fixture under `tests/support/audit.rs`. The refresh/revoke root is already 812 lines; do not append
the new scenarios there. Keep all maintained source/test files below 1,000 lines.

Existing AppState/AdminAppState test constructors will need mechanical updates. No new dependencies
or dependency upgrades should be necessary. A focused `scripts/validate-audit-logging.mjs`, wired
into `npm run test:infra`, may resolve real infra modules with inert resource constructors and fake
bindings. Stub deployment side effects, not the configuration under test; do not port product/domain
stubs that Irongate does not need. Update existing static checks only where this small refactor
changes their assumptions. No generic test-runner or infrastructure rewrite.

## Verification

1. Run the affected existing OAuth/admin/startup tests and infra checks as baseline. Add a positive
   captured-output check showing the current enabled-destination gap before wiring the emitter.
2. Exercise all ten events through real OAuth/admin routes and assert emitted fields plus response
   and persisted state. Include nonzero cleanup counts, trusted/forged actor context, failed actions,
   reuse rejection, idempotent versus random/foreign revocation and code replay. Adapt current
   password-first/generation-aware fixtures, not reference product fixtures.
3. For both modes assert no new audit database writes. Pair absence checks with positive enabled
   emission so an accidentally disconnected emitter cannot pass. Disabled mode must touch neither
   output nor fallback. Audit removal must not alter credential transactions or limiter behavior.
4. Inject failed output and failed fallback; keep successful auth/admin outcomes and stored changes,
   preserve rejection outcomes, and observe only safe failure categories. Force signing failure
   after committed rotation and prove the rotation event still exists alongside the stored change.
5. Capture real serializer/writer bytes: parse complete JSON lines, validate envelope fields, exercise
   concurrent/partial writes, write/flush failure, invalid identifiers and oversize limits. Sensitive
   marker values must not reach either stream. Use a subprocess for production stdout/stderr under
   a restrictive tracing filter if necessary; a mocked sink alone does not prove production wiring.
6. Test auth/admin startup mode loading, including default, none and invalid mode. Resolve both
   functions' config for dev/production and retention overrides, including invalid checked-in
   fallback values, and confirm JSON/INFO plus preserved group/system settings and existing roles.
   Source-string assertions supplement but cannot replace resolved-config tests or certify AWS.
7. Replace database-row-only audit assertions, preserving unrelated lifecycle/recovery/provider and
   limiter regressions. Temporarily bypass a positive event and the safe error report to show the
   new checks detect both omissions; restore and rerun. No blanket test deletion or lint cleanup.

Suggested baseline targets:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test admin_lifecycle --test admin_deletion --test oauth_refresh_revoke \
  --test oauth_token_userinfo --test startup_config
npm run test:infra
```

Final checks:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo test --locked --manifest-path packages/functions/admin/Cargo.toml
cargo check --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
cargo clippy --locked --manifest-path packages/functions/admin/Cargo.toml --all-targets
npm run typecheck
npm run test:infra
git diff --check
```

Use offline mode when local dependencies allow it. Check touched-file rustfmt, exact mirrors,
relative links and source lengths. Run disposable DynamoDB Local regressions if store/transaction
behavior changes beyond removing audit facade methods; otherwise record that they were not rerun
and why. Setup/example/native builds are unnecessary unless their inputs change. Distinguish
existing warnings from new failures and do not reuse the reference's test/deployment results.

No cloud credentials, live accounts, email, UI tests or deployment are needed. If any local fixture
server is started, stop it and clean up only test-owned resources.

## Documentation And Completion

Mirror every touched source/test/config/script at its exact mapped design path, including new
schema, capture, child-test and logging-helper files. Update the audit overview, auth/admin state
and startup mirrors, affected route/store mirrors, infra config/logging docs, test guide and threat
model to describe implemented what/why/how. Remove the obsolete DynamoDB-audit explanation only
after cutover is verified; retain the historical-row and unrelated security limitations.

Update stage/deploy/smoke instructions and stale current-state audit descriptions in
`docs/comparisons/better-auth.md`. Clarify mode none versus operational logs, finite function-log
retention, actor privacy, failure markers, costs of log ingestion/storage and absence of durable
delivery guarantees. No live smoke result should be claimed. No setup secret changes are needed.

Completion requires actual enabled output, explicit suppression, no new audit table writes, typed
private records, visible safe local failures, unchanged auth/admin outcomes, verified event ordering,
valid finite logging configuration and recorded local check results. Record any remaining deployment
validation separately; do not mark local checks as CloudWatch delivery evidence.

## Implementation Result

- Replaced the database writer with typed synchronous audit output and independent safe diagnostics.
  Public/admin state and both entrypoints explicitly install emitters; AuthStore has no audit API.
- Preserved the ten existing outcomes, adding trusted IAM actor attribution and structured counts.
  Rotation is emitted immediately after commit, before signing. Unknown/foreign revocation and
  failed actions do not become success records. Audit failure does not alter auth/lifecycle results.
- Reused the existing Lambda groups/roles. Both functions resolve JSON/INFO and finite retention
  through the supported SST transform. Checked-in fallback retention and environment overrides are
  validated; both 30-day stage defaults are unchanged.
- Adapted reference output/route tests to Irongate's password-first contract. Removed redundant
  row-absence-only tests, retaining unrelated lifecycle, provider and limiter coverage.
- Updated exact mirrors for all 29 touched maintained source/test/config/script/README files, plus
  current audit, threat, admin, stage and setup/smoke/comparison docs. No dependency changed.

### Verification Evidence

All Cargo commands below used `--locked --offline` with the relevant manifest path.

| Check | Actual result |
| --- | --- |
| Affected auth/admin/OAuth/startup baseline | 44 passed |
| Existing infra baseline | All five original validators passed |
| Positive destination-gap test before cutover | Failed as expected: successful enabled admin read produced no output record |
| Auth full suite | 279 passed, 29 opt-in DynamoDB Local cases ignored |
| Admin suite | 1 startup-mode test passed |
| Auth/admin cargo check | Both passed |
| Auth/admin Clippy, all targets | Both exited 0; existing style warnings remain in auth code/fixtures |
| Final admin audit tests after fixture lint correction | 4 passed |
| TypeScript typecheck | Passed |
| Infra checks | All six validators passed, including resolved audit logging config |
| Touched-file rustfmt, diff whitespace | Passed |
| Exact mirror, relative-link and source-length checks | Passed; longest touched source is 790 lines |

The first sandboxed full run could not bind loopback ports for five existing SDK-wire tests.
The complete rerun with local-server permission passed; it used local fixtures, not cloud accounts.
The new startup test initially assumed different error wording and was corrected to assert the
actual typed audit-config rejection.

Two temporary mutations were exercised and restored: omitting the admin-read event failed the
positive output test, and omitting the fallback diagnostic failed the output-error test. Restored
code passed the full suite. Production stream tests exercised real stdout/stderr independently of
tracing filters. Route tests covered all ten event kinds alongside state/results, identifier privacy,
actor fallback/forged headers, idempotent/foreign revocation and signing failure after rotation.

DynamoDB Local tests were not rerun: no credential transaction or adapter behavior changed, only
the obsolete audit facade was removed. Setup scripts, examples and native builds were not rerun
because their inputs were unchanged. No UI tests or live mail/provider requests ran.

### Operational Limits

CloudWatch delivery, deployed IAM/filtering and actual retention have not been validated here.
Applying the logging settings requires a separately authorized deployment and smoke check.
Emission remains best effort; admin cleanup remains multi-step and unrelated operational logging
is not comprehensively sanitized. Historical DynamoDB audit rows without TTL remain untouched.
No next slice or roadmap was created.
