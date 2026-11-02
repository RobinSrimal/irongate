# Irongate authentication improvements handoff

Update Irongate with the reusable authentication improvements developed in Knights & Forks.
The user wants to start a fresh Plaza project from the improved template afterwards. Start by
comparing the repositories and identifying the actual gaps; this handoff is not an implementation
slice or a decision to migrate Plaza's infrastructure.

## Repositories and starting state

- Working repository: `/Users/robinsrimal/Desktop/irongate`.
- Reference repository: `/Users/robinsrimal/Desktop/64x64` (Knights & Forks); read-only for this task.
- Future consumer: `/Users/robinsrimal/Desktop/plaza`; no changes or migration authorized here.
- Irongate baseline inspected on 2026-11-02:
  `4f805446bc81b4c6e18304aceaba80bfe6df6544`.
- Knights & Forks reference:
  `4e7207887f86aacba19d1bcf7ffea312055d5620`.

Irongate was clean before this documentation handoff. The initial pending edits are `AGENTS.md`,
`design/README.md` and this file. Preserve them. Check both repositories' current status and HEAD
before work; pin comparisons to the reference commit unless newer changes are deliberately reviewed.
No Irongate implementation, build, test or deployment has been performed by this handoff.

Read `AGENTS.md`, `README.md`, `design/README.md`, `design/overview.md`, current manifests and
relevant `docs/setup/` pages first. Source paths below are relative to the Knights & Forks repository.
Historical slice docs explain intent; the current source is the reference for final behaviour.
Some older status paragraphs are stale, and some commits include unrelated product work.

## Start with a focused comparison

1. Compare auth/admin source, tests, infrastructure, config and setup tooling in both repositories.
   Identify shared ancestry if present, but do not assume the repositories have matching histories.
2. Classify differences as reusable auth improvements, optional client/example work, product-only
   behaviour or deployment-specific values. Record dependencies and any unresolved design choices.
3. Run a relevant Irongate baseline before changing implementation. Record existing failures and
   local integration prerequisites without using live customer accounts or deployed tables.
4. Report the gaps and propose the smallest coherent first slice. Use `design/implementation/`
   only for the current implementation slice, following `AGENTS.md`. Do not write a multi-slice
   roadmap or pre-create future slice documents.

Do not copy the entire source repository or cherry-pick mixed product commits blindly. Irongate
already includes Google/Apple JWKS caching from `4094d4a`; compare its behaviour before claiming
that feature is missing. Preserve existing improvements that remain appropriate.

## Reference areas to inspect

This is a source inventory, not an implementation order or approval for new infrastructure.

| Area | Source references | Boundary to preserve |
| --- | --- | --- |
| Email-first setup and HTTPS email actions | `packages/functions/auth/src/api/email_actions.rs`, `src/api/providers/password.rs`, `src/providers/password.rs`, `src/email/`, `infra/auth/domain.ts`; slices 02 and 04 | Optional pages in the existing auth Lambda; scanner-safe GETs, deliberate completion, validated origins and configured branding. No additional web runtime required. |
| DynamoDB correctness and refresh expiry | Auth `src/storage/`, `src/store/`, `tests/storage_contract.rs`, `tests/dynamodb_storage.rs`; slice 76, commit `28158b8` | Consistent auth reads, explicit conditional full-record writes, correct SDK error classification and renewal of refresh-family logical expiry and TTL. |
| Atomic password recovery | Auth `src/store/recovery.rs`, `src/store/credential_generation.rs`, password flows and recovery test support; slice 77, `69a92d1` | Password replacement and proof consumption commit together; old recovery links, codes and refresh sessions cannot remain usable. |
| JWT validation and key rotation | Auth provider validators and tests; `packages/functions/product/src/auth/`; slice 78, `027c3b5` | Independent claim-rejection tests and bounded key trust. Product-side access-token caching belongs in a suitable verifier/example, not a copied chess API. |
| Abuse protection | Auth `src/ratelimit/`, `src/config/rate_limits.rs`, `src/store/rate_limits.rs`; slice 79, `42cfec8` | Independent account/source limits and tests that vary email, client or session instead of only repeating identical requests. |
| Audit logging | Auth `src/audit.rs`, `src/config/audit.rs`, `infra/auth/logging.ts`; slice 80, `2de9aa5` | Structured safe events, bounded retention and visible emitter failures using existing Lambda logging. Do not add an audit service or copy the old persistence design. |
| Shared-account resolution | Auth `src/store/shared_identity.rs`, `src/store/email_ownership.rs`, `src/api/providers/email_confirmation.rs` and identity tests; slice 123, `31ee948` | Stable internal subjects, trustworthy email ownership and transactional linking without merging unrelated populated accounts. |
| Native provider sign-in | Auth `src/api/providers/google_native.rs`, `apple_native.rs`, `native_session.rs`; `packages/auth-browser/`; slices 127, 130 and 131 | iOS system-browser Google, native iOS Apple and Android Credential Manager Google, with browser Apple on Android. Bind native responses to the original auth transaction. |
| Secure native sessions | `apps/mobile/src-tauri/src/auth/`, especially `storage.rs`, `storage_android.rs`, `oauth.rs`, `session.rs` | Reusable Keychain/Keystore, PKCE, cancellation and session handling belong in a neutral example/package; tokens stay outside JavaScript. |
| Multiple Android Google clients | Auth Google native handler/config/startup tests; `infra/auth/api.ts`, `infra/shared/stage-config.ts`; slice 136, `4e72078` | A validated explicit presenter allowlist supports debug and Play signing identities while preserving the web/server audience and nonce/PKCE checks. Use template values, not the product IDs. |
| Account deletion and Apple revocation | Auth account lifecycle, `src/store/apple_grants.rs`, `packages/functions/accounts/`, `infra/accounts/index.ts`; slice 133 and final refactor `d01d4d9` | Extract auth semantics only after reviewing product coupling and permission boundaries; the current dedicated Lambda is not a generic drop-in template component. |

Also read `design/research/irongate-aws-email-actions-migration.md` in the source repository for
the earlier product-neutral email-action contract. Treat it as reference, not a command to restore
old hostnames or override the user's current clean-change preference.

## Security behaviour that must survive the transfer

- Resolve existing provider identities by validated issuer/subject before considering email.
  Later changed or missing email claims must not move an already-linked identity to another account.
- For new Google identities, automatic matching needs verified Gmail ownership or a valid signed
  Workspace `hd` claim. A non-Google-hosted address needs the existing fresh email-confirmation
  flow. A domain suffix or `email_verified` alone does not establish Google's authority over it.
- Treat Apple's verified private-relay address as its own address. Never infer the real address,
  strip aliases, or match by display name. Unverified/unsigned values cannot authorize linking.
- Pending registrations cannot reserve another person's email or install an attacker's pending
  password when the legitimate owner signs in through a provider.
- Preserve one-use, expiry, client/redirect, state/nonce, PKCE and credential-generation checks.
  Exercise competing claims and recovery/disable/delete races through the actual storage boundary.
- DynamoDB TTL is cleanup, not authorization. Conditional races, operational outages and uncertain
  write outcomes need distinct handling; a failed response does not prove a write was rolled back.
- Keep admin IAM isolation, refresh replay detection, safe logging and explicit disabled/deleted
  account rejection. Do not imply immediate revocation of already-issued self-contained JWTs when
  only future token issuance/refresh is blocked.

## Account deletion needs a deliberate boundary

The final Knights & Forks implementation uses one dedicated request-handling Lambda with a durable
receipt, short lease and retryable cleanup. It does not use FIFO/SQS, a Pipe or a second worker.
Do not resurrect the superseded queue implementation from earlier commits.

That Lambda also knows about player identities, tournament history, venue references, photos and
CloudFront invalidation. Those are product rules. Irongate should retain reusable credential
invalidation, identity cleanup, Apple grant storage/revocation and truthful completion/retry
semantics without importing chess tables or granting auth broad product access.

Before selecting an orchestration/extension API, explain the smallest product-neutral option,
permission scope, timeout/retry behaviour and simpler alternatives. Follow the architecture-approval
rule in `AGENTS.md`; do not introduce a generic hooks framework, worker or queue speculatively.

## Keep the template reusable

- Keep Knights & Forks names, domains, AWS account/profile IDs, tables, OAuth client IDs, Apple team
  IDs, native bundle/package names and sender addresses out of template defaults. The native bridge
  currently includes `com.knightsandforks` namespaces; inspect all bindings before generalising it.
- Make app identity, allowed issuers/callbacks, branding and provider setup explicit configuration.
  Update template setup scripts, examples, manifests and operator docs together.
- Do not import tournaments, venues, ratings, city catalogues, marketing pages, screenshot fixtures,
  store-release assets or private credentials. Review shared dependencies before adding packages.
- Preserve Irongate's independent optional examples. The user considered an AWS-only Plaza but has
  not selected that migration. Do not remove the Cloudflare BFF example or change either deployment.
- Backwards-compatibility layers are not required for this transfer. Use one coherent current
  contract, remove superseded names and update all in-scope callers; keep protocol interoperability.
- Keep UI copy brief and directed at the app user. No explanatory developer text in auth screens.

## Verification and completion evidence

Use existing meaningful tests before adding more. Prefer signed provider fixtures, controlled clocks,
deterministic race gates and shared memory/DynamoDB Local contracts. Delete or consolidate tests
only when their coverage is redundant or does not test a useful behaviour. Avoid source-string
checks and mocked success cases as substitutes for security or transaction tests.

Run relevant auth/admin builds, Rust formatting/Clippy, configuration/setup checks and changed
example/native checks. Use this repository's manifests to choose commands. Port disposable local
integration helpers where needed; never use a deployed table as the integration-test target.
Do not claim that copying passing tests establishes a passing port, or that build success proves
Google/Apple real-device login. Knights & Forks device acceptance was user-led, not fully certified.

For each completed slice, report changed behaviour, source references, design mirrors, actual
verification results and unresolved decisions. Keep source files under 1,000 lines and finish one
slice before writing another. The destination is a coherent, product-neutral Irongate template that
a fresh project can configure without Knights & Forks services or credentials. No cloud deployment,
Plaza rewrite, push or store action is part of this handoff.
