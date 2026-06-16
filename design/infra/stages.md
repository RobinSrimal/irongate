# Infra Stages

Target code: `sst.config.ts`

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

The setup script rewrites the project name and default profile names after a repository is created from the template.

Non-secret stage defaults live in:

```text
infra/stage-config.ts
```

This file is version-controlled and should contain reviewed defaults such as email sender names, verification/reset URL bases, audit log mode, log retention, table KMS mode, signing mode, signing key id, and admin lifecycle settings. Secret values stay in SST secrets per stage/account.

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
- Stage-specific provider credentials must not be shared across accounts.
- Issuer URL must be stable per stage.
