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

## Stage Defaults

| Setting | Dev | Production |
| --- | --- | --- |
| `DEV_MODE` | Allowed when explicit | false |
| KMS | AWS owned acceptable | Customer managed recommended |
| Email | Console or provider | Provider required |
| CORS | Localhost allowed | Explicit origins |
| Logs | More verbose | Structured, no secrets |

## Security Invariants

- Production must not inherit local or dev-only behavior.
- Stage-specific provider credentials must not be shared across accounts.
- Issuer URL must be stable per stage.
