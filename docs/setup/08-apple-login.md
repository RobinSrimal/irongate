# Apple Login

## Goal

Enable Sign in with Apple through Irongate.

## Inputs Needed

- Apple Services ID.
- Apple Team ID.
- Apple Key ID.
- Apple private key `.p8`.
- Apple return URL for Irongate's Apple callback.

## Files To Edit

- `infra/shared/stage-config.ts`

## Stage Config

Set non-secret Apple identifiers:

```ts
apple: {
  enabled: true,
  clientId: "<services id>",
  teamId: "<team id>",
  keyId: "<key id>",
}
```

## SST Secret

```bash
npx sst secret set ApplePrivateKey --stage dev < AuthKey_<KEY_ID>.p8
```

## Apple Developer Console

Add the deployed Irongate Apple callback URL:

```text
<ApiUrl>/apple/callback
```

## Validation

- Deploy the stage.
- Start login with `provider=apple`.
- Confirm Apple posts back to Irongate and Irongate returns to the OAuth client callback.

## Done When

- Apple login creates or reuses a persisted Irongate identity.
- Apple private key material is stored only as an SST secret.
