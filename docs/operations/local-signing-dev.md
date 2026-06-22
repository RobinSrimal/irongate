# Local ES256 Signing For Dev

## Goal

Use local ES256 signing in dev to avoid KMS signing cost during normal development.

## Inputs Needed

- ES256 P-256 private key PEM.
- `signing.mode = "local-es256"` in `infra/shared/stage-config.ts`.
- `signing.keyId` value for the public JWKS `kid`.

## Files To Edit

- `infra/shared/stage-config.ts`

## Commands

Generate a local key if needed:

```bash
openssl ecparam -name prime256v1 -genkey -noout -out signing-dev.pem
openssl pkcs8 -topk8 -nocrypt -in signing-dev.pem -out signing-dev.pk8.pem
mv signing-dev.pk8.pem signing-dev.pem
```

Set the SST secret:

```bash
npx sst secret set AuthSigningPrivateKey --stage dev < signing-dev.pem
```

Deploy:

```bash
npm run deploy -- --stage dev
```

## Validation

```bash
curl "<ApiUrl>/.well-known/jwks.json"
```

Expected:

- JWKS contains the configured `kid`.
- JWKS contains public key material only.

## Common Failures

- Private key is not PKCS#8 PEM.
- `AuthSigningPrivateKey` was set in the wrong stage.
- `signing-dev.pem` was accidentally committed. The template ignores `signing-*.pem`.

## Done When

- Dev deployment signs JWTs with the configured local ES256 key.
- No dev KMS signing key is created for token signing.
