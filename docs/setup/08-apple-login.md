# Apple Login

## Goal

Enable Sign in with Apple through Irongate.

## Inputs Needed

- Apple Services ID.
- Apple Team ID.
- Apple Key ID.
- Apple private key `.p8`.
- Apple return URL for Irongate's Apple callback.
- A primary Apple App ID with Sign in with Apple enabled.

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

The `clientId` is the Apple Services ID identifier, for example:

```text
com.example.auth
```

The Apple private key itself does not go in this file.

## SST Secret

```bash
npx sst secret set ApplePrivateKey --stage dev < AuthKey_<KEY_ID>.p8
```

Repeat for production with the production key:

```bash
npx sst secret set ApplePrivateKey --stage production < AuthKey_<KEY_ID>.p8
```

The `.p8` file should include the PEM headers:

```text
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
```

## Apple Developer Console

Apple web login needs three Apple-side objects:

1. A primary App ID.
2. A Services ID for the web/OIDC client.
3. A Sign in with Apple private key associated with the primary App ID.

### 1. Enable Sign In With Apple On The Primary App ID

In Apple Developer:

1. Open `Certificates, Identifiers & Profiles`.
2. Go to `Identifiers`.
3. Create or select an App ID for the project.
4. Open the App ID's app services/capabilities.
5. Enable `Sign in with Apple`.
6. Save the App ID.

The App ID is the Apple-side primary app that the Services ID is associated with. Even if Irongate
is being used for a web app first, Apple still expects the web Services ID to be associated with a
primary App ID.

### 2. Create And Configure The Services ID

In Apple Developer:

1. Open `Certificates, Identifiers & Profiles`.
2. Go to `Identifiers`.
3. Add a new `Services ID`.
4. Use a reverse-DNS identifier such as `com.example.auth`.
5. Register the Services ID.
6. Open the Services ID.
7. Enable `Sign in with Apple`.
8. Click `Configure`.
9. Select the primary App ID from step 1.
10. Add the Irongate auth API domain under website domains.
11. Add the deployed Irongate Apple callback URL under return URLs.

Use the Irongate auth API URL, not the example web app URL:

```text
<ApiUrl>/apple/callback
```

For example:

```text
https://abc123.execute-api.eu-central-1.amazonaws.com/apple/callback
```

If `ISSUER_URL` is set to a custom auth domain, use that custom domain:

```text
https://auth.example.com/apple/callback
```

The website domain entry is the host only:

```text
auth.example.com
```

Apple redirects back to Irongate first. Irongate then validates the Apple response and redirects to
the OAuth client callback configured in `auth.clients.toml`.

### 3. Create The Sign In With Apple Private Key

In Apple Developer:

1. Open `Certificates, Identifiers & Profiles`.
2. Go to `Keys`.
3. Add a new key.
4. Enable `Sign in with Apple`.
5. Configure the key for the same primary App ID.
6. Register and download the `.p8` file.
7. Copy the Key ID into `infra/shared/stage-config.ts`.
8. Store the `.p8` file in SST with `ApplePrivateKey`.

Apple only lets you download the private key once. Treat it like a production secret.

## Apple Values Mapping

| Apple value | Irongate field |
| --- | --- |
| Services ID identifier | `auth.apple.clientId` |
| Team ID | `auth.apple.teamId` |
| Key ID | `auth.apple.keyId` |
| `.p8` private key | SST secret `ApplePrivateKey` |
| Return URL | `<ApiUrl>/apple/callback` |

## Validation

- Deploy the stage.
- Start login with `provider=apple`.
- Confirm Apple posts back to Irongate and Irongate returns to the OAuth client callback.

## Done When

- Apple login creates or reuses a persisted Irongate identity.
- Apple private key material is stored only as an SST secret.

## References

- Apple: Configure Sign in with Apple for the web:
  <https://developer.apple.com/help/account/capabilities/configure-sign-in-with-apple-for-the-web/>
- Apple: Register a Services ID:
  <https://developer.apple.com/help/account/identifiers/register-a-services-id/>
- Apple: Enable app services:
  <https://developer.apple.com/help/account/identifiers/enable-app-services/>
- Apple: Create a Sign in with Apple private key:
  <https://developer.apple.com/help/account/capabilities/create-a-sign-in-with-apple-private-key/>
