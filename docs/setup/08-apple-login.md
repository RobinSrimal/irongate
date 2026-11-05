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

Apple redirects back to Irongate first. Irongate validates the Apple response, prepares the
issuer/subject identity and account snapshot, then atomically commits identity/account work and an
Irongate authorization code before redirecting to the OAuth client callback in `auth.clients.toml`.
Returning identities retain their subject if signed email changes or is absent. For a new Apple
identity, a signed verified email joins the active account recorded as that email's owner; an
available email creates a fresh account and owner. A verified private-relay address is its own
address, not the user's hidden real email. Unsigned profile fields never authorize linking.

New/reusable identities guard or create account/email ownership, then write identity, subject index
and code in the same transaction. Missing/inactive owner accounts or malformed owner/index state
reject rather than creating an alternative account. Returning
identities update metadata and create the code only while the exact prepared identity and active
account/generation still match. The consumed session must match Apple and remain unexpired at
preparation; expiry is rechecked before dispatch. No additional secrets, setup values, routes,
infrastructure are required. Password-first then Apple can share a subject; adding a password to an
Apple-first account remains unsupported. A pending password request never installs a credential
during Apple sign-in. Google may share the owner through its own authoritative-email checks;
existing accounts are never merged. See the
[callback design](../../design/functions/auth/api/providers/apple.md).

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

- Use a separately authorized deployed stage and disposable test identity.
- Start login with `provider=apple`.
- Confirm Apple posts back to Irongate and Irongate returns to the OAuth client callback.
- Confirm the redirect contains the authorization code and original state, not access, refresh or
  ID tokens; exchange it through the existing client flow. Repeat sign-in and confirm the subject
  remains stable without relying on profile fields being resent. Do not copy codes, tokens or
  callback URLs into notes.

## Callback Failures

- Confirmed stale/conditional issuance or blocked identity reuse: `400 invalid_grant` with
  `provider sign-in is no longer valid`. Start a fresh sign-in.
- Issuance storage outage, transaction conflict or uncertain outcome: `500 server_error` with
  `provider sign-in unavailable`. This is not invalid credentials or proof that nothing committed.

Provider state and authorize session are consumed before Apple exchange/validation, outside the
identity/code transaction. Failed callbacks require restarting sign-in, not refreshing/reposting
the callback. There is one application-level issuance attempt, with no automatic re-resolution or
upstream exchange retry. Do not delete accounts to compensate for an error: a lost response may
follow a complete commit, and any unreturned code expires normally. See the
[smoke guide](../operations/smoke-test.md) for authorized checks and limitations.

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
