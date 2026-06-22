# Google Login

## Goal

Enable Google OIDC login through Irongate.

## Inputs Needed

- Google OAuth web client ID.
- Google OAuth client secret.
- Authorized redirect URI for Irongate's Google callback.

## Files To Edit

- `infra/shared/stage-config.ts`

## Stage Config

Set the non-secret client ID:

```ts
auth: {
  googleClientId: "<google oauth web client id>",
}
```

## SST Secret

```bash
npx sst secret set GoogleClientSecret "<google oauth client secret>" --stage dev
```

## Google Console

Add the deployed Irongate callback URL as an authorized redirect URI:

```text
<ApiUrl>/google/callback
```

## Validation

- Deploy the stage.
- Start login with `provider=google`.
- Confirm Irongate returns to the configured OAuth client callback.

## Done When

- Google login creates or reuses a persisted Irongate identity.
- Tokens are issued by Irongate, not by the web example.
