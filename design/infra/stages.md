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
examples.enabled = false
examples.authWeb = false
examples.webSpa = false
examples.resourceApi = false
```

Example infrastructure is not part of the auth core and must not deploy unless a stage enables it deliberately.

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
