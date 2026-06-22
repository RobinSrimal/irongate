# AWS Accounts And SST

## Goal

Configure AWS profiles so SST deploys `dev` and `production` to the intended AWS accounts.

## Inputs Needed

- AWS organization/account setup.
- Dev AWS account access.
- Production AWS account access.
- Desired local AWS profile names.

## Files To Edit

- `sst.config.ts` only if changing default profile names.

## Commands

The template defaults are:

```text
<project>-dev
<project>-prod
```

Configure SSO profiles:

```bash
aws configure sso --profile <project>-dev
aws configure sso --profile <project>-prod
```

Login when needed:

```bash
aws sso login --profile <project>-dev
aws sso login --profile <project>-prod
```

Deploy commands:

```bash
npm run deploy -- --stage dev
npm run deploy -- --stage production
```

## Validation

```bash
aws sts get-caller-identity --profile <project>-dev
aws sts get-caller-identity --profile <project>-prod
```

`AWS_PROFILE` should be unset during SST deploys unless deliberately overriding:

```bash
unset AWS_PROFILE
```

## Common Failures

- Using `--stage prod`; the template expects `--stage production`.
- Leaving `AWS_PROFILE` set to the wrong account.
- SSO session expired before deploy.

## Done When

- Dev and production profiles resolve to the intended AWS accounts.
- SST deploys `dev` with the dev profile and `production` with the prod profile.
