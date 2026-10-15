# Better Auth Feature Comparison

Reviewed on 2026-10-15 against:

- Irongate commit `4094d4afc9310edc789f658de2d093c64eee74bb`.
- Better Auth checkout commit `ef3175ae52cb56d447b9102b5c71c17d8e46d86c`, whose main
  package declares version `1.7.9`. This is a source snapshot, not a claim that every
  feature on its main branch is available in the corresponding published release.

This is a feature-family inventory based on source, route, configuration, and documentation
inspection. It is not a security audit, protocol certification, or exhaustive comparison of every
configuration option. The local `better-auth/` reference checkout is ignored by Git; the source links
below are pinned so this document remains usable without that directory.

## Assessment

Irongate has the foundation for a standalone authentication service: password and Google/Apple
authentication, OAuth authorization codes, PKCE, ID tokens, JWT access tokens, refresh rotation,
logout, account lifecycle controls, and an AWS deployment. It does not yet have Better Auth's breadth
of account management, authentication methods, organizations, enterprise integrations, or developer
tooling.

Matching the feature families is feasible on Lambda and DynamoDB, but it is a substantial expansion
of the product. Changing the database alone would not close the gap.

Better Auth is not PostgreSQL-only or inherently incompatible with serverless deployments. Its
adapters cover several SQL databases and MongoDB, and it documents stateless sessions and serverless
use. Irongate's distinction is the integrated Rust, API Gateway, Lambda, DynamoDB, IAM, SST, and
optional KMS deployment model. [Better Auth database documentation][ba-database]

For this assessment, parity means equivalent application capabilities. It does not mean compatibility
with Better Auth's TypeScript configuration, npm plugins, database schema, client SDK, or exact HTTP
API. Those would be additional compatibility projects.

## Existing Foundation

These capabilities exist in Irongate; the later sections describe differences within each family.

| Capability | Current implementation |
| --- | --- |
| Password authentication | Registration, required email verification, login, forgot/reset flows, Argon2id hashing, configurable verification/reset templates, Resend delivery. |
| Social authentication | Google and Apple code exchange and ID-token validation, provider state/nonce, cached provider JWKS with refetch for missing keys. |
| OAuth/OIDC | Authorization Code + S256 PKCE, public/confidential clients, exact registered redirects, desktop loopback ports, discovery, JWKS, ID tokens, userinfo. |
| Token lifecycle | Configurable lifetimes, HMAC refresh-token lookup, transactional creation/rotation, reuse detection, family revocation. |
| User identity | Persisted internal subject IDs, provider identities, active/disabled/deleted account states and deletion tombstones. |
| Operator administration | Separate IAM-protected get/disable/enable/delete/revoke-sessions routes. |
| Security and operations | Typed DynamoDB operations, TTL plus runtime expiry, source-context rate limits, configurable CloudWatch audit logging, SST secrets, local or KMS ES256. |
| Example integrations | Cloudflare web BFF with server-side sessions and a Tauri desktop app with OS keychain storage. These are examples, not reusable SDKs. |

Evidence: [public routes][ig-routes], [password domain][ig-password], [admin routes][ig-admin],
[records][ig-records], [refresh storage][ig-refresh], and [client registry][ig-clients].

## User And Session Management

Better Auth exposes account and session operations directly to authenticated applications.
Irongate currently exposes a smaller public surface. [User/account source][ba-core]
[Account documentation][ba-users] [Session documentation][ba-sessions]

| Capability | Irongate status | Work needed for parity |
| --- | --- | --- |
| Update profile and custom user fields | Missing public management API. Identity properties exist, but are not a configurable user-profile system. | Authenticated reads/updates, field validation, writable versus server-owned fields, and claim propagation rules. |
| Change email | Missing. | Prove ownership, reserve the new normalized email, move lookup records atomically, handle collisions, notify/verify as appropriate, and retain the subject. |
| Change password while signed in | Reset exists; current-password change does not. | Reauthentication, current-password verification, password update, and explicit session-revocation policy. |
| Add password to a social account | Missing. | Attach a verified password credential to an existing subject without creating a second account. |
| Verification resend | Partial: registration can send another verification message for an unverified user. | Dedicated resend operation with its own delivery and abuse policy. |
| List, link, and unlink login identities | Missing public operations. Matching emails across providers currently remain separate accounts. | Explicit proof of both identities, unique ownership, safe unlinking, and protection against removing the final login method. |
| Self-service account deletion | Operator deletion exists; user deletion does not. | Recent authentication or verification, public authorization checks, and reuse of the existing deletion behavior. |
| Read the current application session | Example-specific BFF behavior; core offers userinfo and refresh families. | A reusable application session contract and client integration appropriate to the BFF/native model. |
| List devices/sessions and revoke one, others, or all | Current-token family revocation and operator-wide revocation exist. | Public subject-authorized session listings, safe session identifiers, device/activity metadata, and selective revocation. |
| Session freshness and reauthentication | No general freshness contract. | Record actual authentication time and proof level separately from token issuance/refresh; require fresh proof for sensitive operations. |
| Multiple signed-in accounts and account switching | Missing. Multiple refresh families are not account-switching support. | Track and switch several authenticated identities in the same application/browser. |
| Upstream provider token management | Login uses provider tokens but offers no linked-account token API. | Encrypted storage, retrieval, refresh, scopes, and unlink/revocation rules for applications calling Google or other provider APIs. |

Irongate's `irongate_session` authorize cookie identifies a short-lived login transaction. It is not
a reusable logged-in identity-provider session. Likewise, the web example's cookie belongs to that
BFF, not to a central SSO session shared by arbitrary clients.

There is also a semantic difference in revocation: Irongate stops refreshes and checks account status
on userinfo, but a resource API validating an issued JWT offline continues accepting it until expiry.
Matching immediate invalidation of database-backed sessions would require an online status check or
another revocation mechanism. Better Auth's caching/stateless options have their own invalidation
tradeoffs; not every Better Auth configuration guarantees immediate invalidation either.

## Authentication Methods And Security Features

These are separate features, not automatic consequences of the existing password or OAuth code.
Evidence: [bundled plugin exports][ba-plugin-source], [plugin documentation][ba-plugin-docs],
[social-provider registry][ba-social], and [passkey package][ba-passkey].

| Capability | Irongate status | Work needed for parity |
| --- | --- | --- |
| Two-factor authentication | Missing. | TOTP enrollment/verification, delivered OTP factors, recovery codes, trusted devices, recovery/reset rules, attempt limits, and a login continuation that cannot issue tokens before all required factors succeed. |
| Passkeys/WebAuthn | Missing. | Registration and authentication ceremonies, challenge storage, RP ID/origin validation, credentials and authenticator metadata, listing/renaming/removal, passkey-first registration, and browser/native support. |
| Passwordless magic-link login | Missing. Verification/reset links do not authenticate the user. | Login-specific, single-use proof and continuation bound to the requesting flow. |
| Email OTP | Missing. | OTP sign-in, verification, password reset and email-change flows with strict attempt/expiry rules. |
| Phone authentication | Missing. | Phone identity normalization, SMS delivery, verification, sign-in, number changes/removal, and phone-based recovery. |
| Username authentication | Missing. | Unique normalized usernames, validation, lookup, and password login by username. |
| Anonymous users and upgrade/linking | Missing. | Guest subjects and safe transfer or linking of guest-owned state after authentication. |
| Additional social providers | Only Google and Apple. | Provider adapters and tests for GitHub, Microsoft, GitLab, Discord, Facebook, Slack, and the other providers in the reference registry. |
| Generic OAuth/OIDC login | Missing. | Configurable endpoints/discovery, provider-specific identity validation and profile mapping, scopes, credentials, and safe outbound requests. |
| One Tap and direct provider ID-token login | Redirect flows exist; these entry points do not. | Validate externally acquired tokens, audience and nonce binding, then integrate with normal account and factor policy. |
| Sign in with Ethereum | Missing. | Wallet challenge/signature verification, replay protection, and wallet identity management. |
| CAPTCHA and breached-password checks | Missing. Existing rate limits are separate controls. | Pluggable CAPTCHA verification and compromised-password policy on password creation/change. |
| Last login method | No dedicated feature. | Persist/expose the last authentication method and integrate it into clients. |
| General one-time session handoff token | Missing public feature. Internal verification/code records are purpose-specific. | Issue and consume a constrained single-use handoff token with explicit audience, purpose, and session behavior. |

Provider breadth also includes profile mapping and access to upstream API tokens. Implementing more
buttons alone would not establish full social-provider parity.

## Organizations, Enterprise, And Administration

| Capability | Irongate status | Work needed for parity |
| --- | --- | --- |
| Organizations and memberships | Missing. | Organization CRUD, user-to-organization listings, membership lifecycle, active organization, and tenant isolation. |
| Invitations and teams | Missing. | Invite/accept/reject/cancel/expiry flows and email delivery; team CRUD and membership. |
| Application roles and permissions | OAuth scopes exist; no organization/application authorization system. | Built-in/custom roles, permission checks, dynamic role management, and revocation semantics when membership changes. |
| Enterprise SSO | Missing. Google/Apple are not tenant-configured enterprise federation. | Tenant OIDC/SAML connections, discovery/metadata, domain verification, claim mapping, provisioning, assertion validation/replay defense, and certificate rotation. |
| SCIM directory provisioning | Missing. | Authenticated directory connections, Users/Groups discovery and CRUD/PATCH, filtering/pagination, activation/deactivation, membership/role mapping, and reconciliation. |
| API keys | Missing. OAuth client secrets and AWS IAM credentials are different credentials. | User/organization-owned keys, scopes/permissions, expiry, quotas/rate limits, listing, update and revocation, and hashed lookup. |
| Expanded administration | Partial: operator lifecycle API only. | User creation/listing/filtering/updating, password setting, roles, temporary bans, session listing and individual revocation, and impersonation controls. |

Evidence: [organization plugin][ba-organizations], [SSO package][ba-sso], [SCIM package][ba-scim],
[API key package][ba-api-key], and [admin plugin][ba-admin].

AWS IAM protects infrastructure operators. It does not supply the application role model for a
customer inviting members or managing their organization. Those operations would need their own
authorization based on the authenticated Irongate subject. Operator IAM can remain in place.

Impersonation is part of Better Auth's admin feature set, but supporting it safely is an explicit
product choice requiring audit, actor attribution, scope limits, and restricted eligibility.

## OAuth Provider And Signing

Irongate already acts as an authorization server. Better Auth's OAuth provider package goes beyond
the subset Irongate exposes. [OAuth provider source][ba-oauth-source]
[OAuth provider documentation][ba-oauth-docs]

| Capability | Irongate status | Work needed for parity |
| --- | --- | --- |
| Code/refresh grants, PKCE, discovery, ID tokens, userinfo | Present subset. | Preserve it while extending the capabilities below; OIDC compatibility is not a claim of complete conformance. |
| Client credentials | Missing; rejected by the client model. | Machine identities, grant policy, audience/scopes and client authentication without an end-user account. |
| Device authorization | Missing. | Device/user codes, approval/denial, bounded polling, expiry, client binding and token issuance. |
| Runtime OAuth client management and registration | Config-only registry. | Authorized client CRUD, secret lifecycle, registration policy, validation, and persistence. |
| Consent and grant management | Missing. | Requested versus approved scopes/claims, consent storage, grant listing/revocation, and application-owned consent screens. |
| Reusable SSO session and OIDC interaction controls | Missing. | Central login session plus `prompt`, account selection, `max_age`, and authentication-context handling. |
| Rich claims and pairwise subjects | Public subject plus limited claims. | Claims requests, scope-based profile claims, custom claim hooks, client-specific pairwise subjects, and authentication/session claims. |
| Resource-specific token audiences | One configured access-token audience. | Resource registration/binding, resource indicators, per-resource scopes/claims, and protected resource metadata. |
| Token introspection and opaque access tokens | Missing. | Protected token-status endpoint and persisted access-token state if opaque tokens are offered. |
| OIDC logout | Refresh-family revoke only. | RP-initiated end-session and back-channel logout, session identifiers, and registered logout destinations. |
| `private_key_jwt` client authentication | Missing; shared-secret and public clients exist. | Registered client JWKS, assertion validation, audience/lifetime checks and assertion replay storage. |
| DPoP | Missing. | Proof validation, replay checks, sender-bound code/token/refresh state and resource-side enforcement. |
| Rotation retry grace | Strict refresh-token reuse detection exists. | Optional, bounded replay of the same encrypted response for legitimate retry, bound to the same client/scope/resource/key. This is a policy option, not a reason to weaken strict replay detection by default. |
| Signing-key rotation with overlap | One configured signer and public key. | Publish and verify old keys while new keys sign; retire keys only after the relevant token/cache lifetimes. |
| Signing algorithms and configurable signing hooks | Local/KMS ES256 present. | Additional algorithms and extension points if these are part of the chosen parity contract. |
| MCP authorization and client metadata discovery | Missing. | Resource metadata/challenges and binding, consent, client discovery such as CIMD, and the relevant protocol integration. |

The signing design mentions rotation overlap, but the current runtime holds one signer and returns
one public key. The `expires_at` field on local key metadata is not an implemented rotation mechanism.
This is an existing implementation/design mismatch as well as a parity gap.
[Irongate signing implementation][ig-signing]

The configured client allowlist can carry scope strings, but this is not a complete resource and
permission registry. Discovery advertises a fixed scope list, and userinfo does not implement a
general profile service. [Irongate claims][ig-claims] [Irongate userinfo][ig-userinfo]

## Developer Experience And Extensibility

| Capability | Irongate status | Work needed for parity |
| --- | --- | --- |
| Reusable client SDK | Example-specific integration code. | A documented, typed client with error handling, session lifecycle, and separate browser/BFF/native usage. |
| Framework integrations | React/Tauri and web examples. | Reusable React/Vue/Svelte/Solid clients and server/framework integration guides or adapters, rather than copying example code. |
| Mobile/desktop SDK coverage | Desktop-first Tauri example with mobile notes. | Tested mobile redirect and secure-storage integration; equivalents of Better Auth's Expo/Electron integrations where needed. |
| Hooks and plugin API | Developers can edit the template; no supported plugin contract. | Stable hooks for auth/account/session events, validation, custom routes, claims and storage, with documented trust boundaries. |
| Custom user/session schema | Fixed records with some JSON properties. | Validated extension fields, safe input/output rules, and typed SDK exposure. |
| Configurable delivery and password callbacks | Email templates and an internal sender trait exist; the deployed sender is Resend. Password hashing is Argon2id. | Supported delivery/lifecycle hooks and configurable hash/verify behavior for applications importing other credential formats or using different messaging services. |
| API schema and OpenAPI tooling | No generated public API specification. | OpenAPI/schema generation, reference documentation, and client generation. |
| Setup, migrations, and test helpers | Setup script, design/setup docs and internal test support exist. | Versioned upgrade tooling, deployment/config diagnostics, and reusable consumer integration-test helpers. |
| Localization | Configurable email content; no general translated API error system. | Stable error codes, locale negotiation, translations and client handling. |
| Custom session response and caching modes | BFF sessions and JWTs exist, with a different contract. | Session enrichment hooks and explicit consistency/freshness semantics; literal parity would also cover Better Auth's cookie-cache/stateless/secondary-storage options. |
| Preview OAuth proxy and popup flows | Missing reusable features. | Safe preview-origin OAuth handoff and popup coordination/communication. |
| Portable instrumentation | CloudWatch audit events and request tracing exist. | Documented spans/events and integration with application telemetry comparable to Better Auth's instrumentation API. |

Evidence: [client library][ba-client], [hooks][ba-hooks], [plugin API][ba-plugin-api],
[package integrations][ba-packages], [OpenAPI plugin][ba-openapi], and
[instrumentation documentation][ba-instrumentation].

Rust need not load JavaScript plugins to provide equivalent capabilities. However, modifying source
for each application is a different developer experience from installing and configuring a plugin.
Similarly, a remote auth service cannot directly provide Better Auth's in-process `auth.api` and
TypeScript schema inference; an SDK and explicit schemas would serve that role.

## Separate Ecosystem Scope

The Better Auth documentation also includes capabilities outside its main bundled auth library.
These should remain visible in a literal parity assessment without being mistaken for core features.

| Surface | Reference capability | Irongate status |
| --- | --- | --- |
| Stripe package in this checkout | Customer/subscription lifecycle, billing operations, organization subscriptions and webhook integration. | Missing. |
| External billing/marketing integrations | Polar, Autumn, Dodo Payments, Creem, Chargebee, Commet and Dub. These are documented integrations, not all implemented in this checkout. | Missing; each integration needs its own scope. |
| Agent Auth | Separately maintained package for agent identity, capability authorization and approval flows; the checked-in docs explicitly label the evolving protocol unstable. | Missing; documentation was reviewed, implementation is outside this checkout. |
| Paid Better Auth Infrastructure | Hosted management dashboard, analytics, queryable audit logs, abuse detection, email/SMS services and operational integrations. | CloudWatch audit and Resend delivery provide parts of the outcome, not the hosted product. |
| Community plugins | Open-ended third-party ecosystem. | No compatible plugin ecosystem. Literal parity cannot be a bounded target without a named list. |

Evidence: [Stripe package][ba-stripe], [documented plugins][ba-plugin-docs],
[Agent Auth documentation][ba-agent], and [Infrastructure documentation][ba-infra].
The paid infrastructure offering does not mean SSO and SCIM are paid-only: their source packages are
also present in the inspected repository.

## What AWS And DynamoDB Change

These are engineering implications of the gaps above, not already implemented features:

1. **Access patterns need explicit design.** Organizations, invitations, devices, API keys, client
   registrations and directory connections require new keyed records and indexes. Generic list/search
   filters cannot simply be translated into scans of the auth table. Broad SCIM/admin filtering may
   require extra projections or a search service, or a clearly documented capability limit.
2. **Uniqueness and relationships need transactions.** Linking credentials, moving email addresses,
   inviting members, changing roles and deleting tenants require conditional ownership checks and
   consistent indexes. Large cascades need resumable work rather than an unbounded Lambda request.
3. **Some new secrets must be recoverable.** TOTP seeds and upstream provider refresh tokens require
   encryption and restricted decrypt access. HMAC-only lookup is appropriate for one-time bearer
   secrets, but cannot replace encryption when the server must use the original value later.
4. **Challenges need a common lifecycle.** MFA, WebAuthn, device approval and reauthentication all need
   durable, expiring, single-use state and a clear distinction between partial proof and completed
   authentication. Lambda memory can cache public data; it cannot be the authority for these flows.
5. **Authorization freshness becomes more important.** Organization membership, role changes,
   disabled API keys and device revocation need explicit enforcement rules. Putting permissions in
   a long-lived JWT alone does not immediately enforce changes.
6. **Delivery and background work broaden.** SMS needs a delivery integration. Directory sync,
   webhook retries, notifications and large cleanup jobs may warrant queues/workers. These can stay
   serverless, but they are additional infrastructure and operational work.
7. **Cost changes with the workload.** DynamoDB reads/writes, indexes, KMS operations, logs and
   email/SMS delivery all contribute. The stack avoids an always-on relational database requirement;
   it does not establish that every equivalent workload is cheaper without measurements.

Matching user-facing features does not require supporting PostgreSQL, Redis or MongoDB as Irongate
runtime backends. Literal database-adapter/plugin compatibility would conflict with a DynamoDB-only
product boundary and should be treated separately from capability parity.

## Suggested Priorities

This ordering is an assessment, not an accepted implementation roadmap:

1. Define the parity contract: core plus named first-party plugins, separate from hosted services
   and third-party integrations; equivalent capabilities versus drop-in API compatibility.
2. Complete account/session foundations: authenticated account APIs, recent-authentication proof,
   email/password changes, linking, and device/session management. Add signing-key overlap alongside
   these security foundations.
3. Add MFA and passkeys on that foundation; add a reusable SDK as the public contract develops.
4. Add common login choices and integrations: more social providers, generic OIDC, magic links,
   email OTP, phone/username login and abuse controls.
5. Add application authorization: organizations, invitations, teams, roles and API keys, followed
   by enterprise SSO and SCIM once their identity and membership foundations exist.
6. Extend the authorization-server surface: machine/device grants, consent, client lifecycle,
   resource binding, richer OIDC/logout, DPoP and MCP, according to the agreed parity scope.
7. Cover the remaining SDK/tooling, optional plugins and integrations against the inventory above.

Several gaps reflect current deliberate boundaries: config-only clients, minimal profiles,
operator-only account lifecycle and application-owned sessions. Feature parity would require
explicitly expanding those boundaries. The current design docs should change alongside each
implemented capability, not claim the full target now.

## Sources

Better Auth links are pinned to the inspected checkout. Irongate links refer to this repository and
were evaluated at the revision recorded above.

[ig-routes]: ../../packages/functions/auth/src/routes.rs
[ig-password]: ../../packages/functions/auth/src/providers/password.rs
[ig-admin]: ../../packages/functions/auth/src/api/admin.rs
[ig-records]: ../../packages/functions/auth/src/store/records.rs
[ig-refresh]: ../../packages/functions/auth/src/store/refresh.rs
[ig-clients]: ../../packages/functions/auth/src/core/clients.rs
[ig-signing]: ../../packages/functions/auth/src/crypto/signing.rs
[ig-claims]: ../../packages/functions/auth/src/core/tokens.rs
[ig-userinfo]: ../../packages/functions/auth/src/oauth/userinfo.rs
[ba-database]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/concepts/database.mdx
[ba-core]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/better-auth/src/api/index.ts
[ba-users]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/concepts/users-accounts.mdx
[ba-sessions]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/concepts/session-management.mdx
[ba-plugin-source]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/better-auth/src/plugins/index.ts
[ba-plugin-docs]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/plugins
[ba-social]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/core/src/social-providers/index.ts
[ba-passkey]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/passkey/src
[ba-organizations]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/better-auth/src/plugins/organization
[ba-sso]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/sso/src
[ba-scim]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/scim/src
[ba-api-key]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/api-key/src
[ba-admin]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/better-auth/src/plugins/admin
[ba-oauth-source]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/oauth-provider/src
[ba-oauth-docs]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/plugins/oauth-provider.mdx
[ba-client]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/better-auth/src/client
[ba-hooks]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/concepts/hooks.mdx
[ba-plugin-api]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/concepts/plugins.mdx
[ba-packages]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages
[ba-openapi]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/better-auth/src/plugins/open-api
[ba-instrumentation]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/reference/instrumentation.mdx
[ba-stripe]: https://github.com/better-auth/better-auth/tree/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/packages/stripe
[ba-agent]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/plugins/agent-auth.mdx
[ba-infra]: https://github.com/better-auth/better-auth/blob/ef3175ae52cb56d447b9102b5c71c17d8e46d86c/docs/content/docs/infrastructure/introduction.mdx
