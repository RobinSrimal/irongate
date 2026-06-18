# Infra Stages

Target code: `sst.config.ts`, `infra/shared/stage-config.ts`

## Owns

- Stage names.
- AWS account/profile mapping.
- Project naming.
- Production defaults.

## Target Behavior

The template uses separate AWS accounts or profiles for dev and production:

```text
<project>-dev
<project>-prod
```

Only these stage names are supported by default:

```text
dev
production
```

`--stage dev` uses the dev profile and removable dev defaults. `--stage production` uses the prod profile, retained resources, and protection enabled. `--stage prod` and unknown stages fail with a clear error so production-like deploys cannot silently inherit dev configuration.

The setup script rewrites the project name and default profile names after a repository is created from the template.

Non-secret stage defaults live in:

```text
infra/shared/stage-config.ts
```

This file is version-controlled and should contain reviewed defaults such as email sender names, verification/reset URL bases, audit log mode, log retention, table KMS mode, signing mode, signing key id, and admin lifecycle settings. Secret values stay in SST secrets per stage/account.

Optional example deployment settings also live in this file and default to disabled:

```text
dev.examples.enabled = true
dev.examples.web.enabled = true
production.examples.enabled = false
production.examples.web.enabled = false
examples.web.clientId = "web"
examples.web.baseUrl = optional override
examples.app.enabled = false
```

Example infrastructure is not part of the auth core. In this repo's dev stage, the web example is
enabled so it can be smoke-tested. Production keeps examples disabled unless deliberately enabled.

The web example derives its base URL from the incoming request origin by default. Stages may
optionally configure `examples.web.baseUrl` for custom domains, tunnels, or unusual proxy setups.
Those values must not replace auth-core issuer, client, or secret configuration. Example settings
remain opt-in per stage.

Generated `workers.dev` origins are acceptable for first dev deploys. Production examples should use
an explicit domain and exact Irongate redirect URI registration.

## Stage Defaults

| Setting | Dev | Production |
| --- | --- | --- |
| `DEV_MODE` | Allowed when explicit | false |
| KMS | AWS owned acceptable | Customer managed recommended |
| Email | Resend required | Resend required |
| CORS | Localhost allowed | Explicit origins |
| Audit logs | CloudWatch by default, explicit `none` allowed | CloudWatch by default, explicit `none` allowed |
| Log retention | Configurable | Configurable |

## Security Invariants

- Production must not inherit local or dev-only behavior.
- Unknown or ambiguous stage names must not fall back to dev.
- Stage-specific provider credentials must not be shared across accounts.
- Issuer URL must be stable per stage.
- Production example web origins should be explicit and allowlisted.
