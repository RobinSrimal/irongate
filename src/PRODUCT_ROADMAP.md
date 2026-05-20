# Irongate Product Roadmap

## Product Direction

Irongate is a repo-first, Rust-based OpenAuth implementation for developers who want a self-hosted OAuth/OIDC server with secure defaults and a simple AWS deployment path.

The core product should remain usable directly from the repository. Any CLI or desktop app should be an optional operator layer that makes deployment and operations safer, faster, and easier. A dashboard is only worth building when it helps developers operate a live auth service; generic charts are not enough.

## Product Principles

- Keep auth focused on identity, OAuth/OIDC correctness, token lifecycle, providers, clients, and security auditability.
- Keep AWS deployment simple, repeatable, and inspectable.
- Prefer repo and CLI workflows first; use a Tauri app as a guided interface over the same Rust operator core.
- Avoid storing long-lived AWS credentials. Prefer AWS profiles, SSO, and short-lived credentials.
- Keep payments independent of auth. Billing can consume Irongate subject IDs later, but it should not become part of the auth core.

## Target Users

- Solo developers and small teams that want self-hosted auth without running a large identity platform.
- Rust-oriented teams that prefer a small, auditable auth server over a SaaS dependency.
- AWS users who are comfortable owning infrastructure but do not want to manually wire Lambda, API Gateway, DynamoDB, provider secrets, and OAuth clients.

## Roadmap

### Phase 1: Core Auth Server

Goal: make the Rust auth server stable enough to deploy and test locally.

- Finish OAuth authorization code and refresh-token flows.
- Keep mandatory client registration and exact redirect URI matching.
- Support password, code, GitHub, Google, and Apple provider paths.
- Keep DynamoDB and in-memory storage behind the same storage adapter.
- Maintain local development with `cargo run`, DynamoDB Local, and the test client.
- Expand audit events for login success/failure, provider callback failures, client changes, token rotation, token reuse, and admin actions.

Success criteria:

- A developer can run Irongate locally, register a client, complete login, refresh a token, and revoke tokens.
- The security defaults are documented and covered by focused tests.

### Phase 2: AWS Deployment Baseline

Goal: make the repository deployment path reliable without a companion app.

- Harden the CDK stack for Lambda, API Gateway, DynamoDB, IAM, logging, and outputs.
- Make `ISSUER_URL`, provider configuration, trusted proxies, and dev/prod mode explicit.
- Document first deployment, bootstrap, client registration, provider setup, and rollback.
- Add a `doctor` checklist that can be run manually or from a future CLI.

Success criteria:

- A developer can deploy from the repo and get a working issuer URL with clear post-deploy steps.
- Common setup mistakes are documented with direct fixes.

### Phase 3: Operator CLI

Goal: create a practical operations layer before building a GUI.

Commands to target:

- `irongate deploy`: validate AWS identity, synth/deploy the stack, capture outputs, and run post-deploy checks.
- `irongate doctor`: verify AWS credentials, stack outputs, API reachability, JWKS, DynamoDB access, provider callback URLs, and admin API access.
- `irongate bootstrap`: create or import the admin key and store it safely.
- `irongate clients`: list, create, update, disable, and rotate client secrets.
- `irongate audit`: inspect recent security events and token reuse warnings.
- `irongate upgrade`: show stack/runtime version drift and guide safe upgrades.

Success criteria:

- The CLI removes most manual AWS/CDK/admin API steps.
- The same command flow works for local development and deployed AWS environments where possible.

### Phase 4: Tauri Operator App

Goal: provide a guided desktop UI over the same Rust operator core.

Primary workflows:

- Connect to AWS using a profile, SSO session, or temporary credentials.
- Choose region, stack name, issuer URL, domain, and provider configuration.
- Deploy or connect to an existing Irongate stack.
- Store deployment metadata and the admin key in the OS keychain.
- Manage OAuth clients and provider settings.
- Run health checks and show actionable repair steps.
- View security-relevant audit events.

The app should not be positioned as an analytics dashboard. The home view should answer:

- Is the auth server reachable?
- Is discovery/JWKS valid?
- Are providers configured correctly?
- Are there recent security warnings?
- Are clients and redirect URIs configured as intended?
- Is the deployed version current?

Success criteria:

- A developer can deploy, verify, and operate Irongate without touching CDK directly.
- The UI surfaces operational problems with clear fixes instead of passive charts.

### Phase 5: Operational Signals

Goal: make Day 2 operation genuinely useful.

Useful metrics and signals:

- Registered clients.
- Active refresh tokens.
- Distinct subjects seen through token issuance.
- Password-provider accounts, when the password provider is enabled.
- Recent logins, failed logins, provider callback failures, token reuse detections, and token revocations.
- Rate-limit events and suspicious spikes.
- Stack health, Lambda errors, API latency, and DynamoDB throttling.

Non-goals:

- Full product analytics.
- Business intelligence charts.
- User management for arbitrary application databases.
- Replacing CloudWatch for deep infrastructure debugging.

Success criteria:

- The operator helps answer whether auth is healthy, secure, and correctly configured.
- Every metric shown has a clear operational decision attached to it.

### Phase 6: Optional Billing Integration

Goal: support payment-aware applications without coupling billing to auth.

- Keep Stripe or other payment providers outside the auth server.
- Provide an optional integration pattern that maps Irongate subjects to billing customers and entitlements.
- Document how an application should combine Irongate identity with its own authorization and billing state.
- Consider a separate crate or example service only after the auth/deploy/operator story is stable.

Success criteria:

- Developers can integrate payments without changing Irongate's OAuth semantics.
- Billing remains an application concern, not an auth-server responsibility.

## Near-Term Priorities

1. Stabilize the Rust auth server and tests.
2. Make AWS deployment from the repo boring and repeatable.
3. Add richer audit events and admin endpoints needed by future operations tooling.
4. Build a small Rust operator core and CLI around deploy, doctor, bootstrap, clients, and audit.
5. Build the Tauri app only after the CLI workflows prove useful.

## Open Questions

- Should the first operator package live inside this repository or as a separate workspace member?
- Should deployment use CDK directly, CloudFormation templates generated from CDK, or a Rust-native AWS provisioning path?
- How much CloudWatch data should the operator read versus relying on Irongate admin APIs and DynamoDB audit records?
- What is the minimum provider configuration UX for the first Tauri version: password-only, GitHub, Google, or all v1 providers?
