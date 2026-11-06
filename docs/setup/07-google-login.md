# Google Login

## Goal

Enable Google OIDC login through Irongate.

## Inputs Needed

- Google OAuth web client ID.
- Google OAuth client secret.
- Authorized redirect URI for Irongate's Google callback.
- Google Cloud project with OAuth consent screen configured.

## Files To Edit

- `infra/shared/stage-config.ts`

## Stage Config

Set the non-secret client ID:

```ts
auth: {
  googleClientId: "<google oauth web client id>",
}
```

The Google client secret does not go in this file.

## SST Secret

```bash
npx sst secret set GoogleClientSecret "<google oauth client secret>" --stage dev
```

Repeat for production with the production secret:

```bash
npx sst secret set GoogleClientSecret "<prod google oauth client secret>" --stage production
```

## Google Console

Google login needs:

1. An OAuth consent screen.
2. A Web application OAuth client.
3. Irongate's Google callback URL registered as an authorized redirect URI.

### 1. Configure OAuth Consent

In Google Cloud:

1. Create or select a Google Cloud project.
2. Open `APIs & Services` or `Google Auth Platform`.
3. Configure the OAuth consent screen.
4. Set the app name, support email, developer contact email, and authorized domains.
5. Keep the app in testing while validating, or publish it when ready for production.
6. If the app is in testing mode, add the Google accounts that should be allowed to test login.

For Irongate's current Google login, the expected scopes are:

```text
openid
email
profile
```

Avoid adding broader Google API scopes unless the application actually needs Google API access.

### 2. Create A Web OAuth Client

In Google Cloud:

1. Open `APIs & Services` or `Google Auth Platform`.
2. Go to `Clients` or `Credentials`.
3. Create an OAuth client.
4. Choose `Web application`.
5. Name it for the stage, such as `irongate-dev` or `irongate-production`.
6. Add the deployed Irongate callback URL as an authorized redirect URI.

Use the Irongate auth API URL, not the example web app URL:

```text
<ApiUrl>/google/callback
```

For example:

```text
https://abc123.execute-api.eu-central-1.amazonaws.com/google/callback
```

If `ISSUER_URL` is set to a custom auth domain, use that custom domain:

```text
https://auth.example.com/google/callback
```

Google redirect URIs must match exactly. A mismatch usually fails with:

```text
redirect_uri_mismatch
```

`Authorized JavaScript origins` are not required for Irongate's server-side OIDC callback. Add them
only if a separate browser integration calls Google directly. The web example does not do that.

### 3. Copy Values Into Irongate

Copy the generated client ID into `infra/shared/stage-config.ts`:

```ts
auth: {
  googleClientId: "<google oauth web client id>",
}
```

Store the generated client secret as an SST secret:

```bash
npx sst secret set GoogleClientSecret "<google oauth client secret>" --stage dev
```

Google redirects back to Irongate first. Irongate validates the Google response, prepares the
issuer/subject identity and account snapshot, then atomically commits identity/account work and an
Irongate authorization code before redirecting to the OAuth client callback in `auth.clients.toml`.
Returning identities retain their subject even if email changes or is absent. New identities
require a verified email and either exact `gmail.com` or a valid signed Google hosted-domain (`hd`)
claim. A matching active email owner can come from password, Apple or Google; otherwise an available
email creates a fresh account/owner. Existing accounts are never merged or moved by later claims.

First-time Google identities using third-party addresses without that authority are rejected with
`invalid_grant`. The internal confirmation store and rate-limited code-email operation exist, but
no public callback or route invokes them. Confirmation transport and code-verification attempt
limits are not implemented. There is no separate-account fallback or configuration bypass. Adding an
`hd` request parameter cannot supply signed authority.
Use Gmail or a Google-hosted organization account for first-time Google acceptance testing.

New/reusable identities guard or create account, email owner and email index, then write identity,
identity index and code together in six operations. Missing/inactive owner accounts or malformed
bindings reject rather than selecting an alternative account. Returning
identities update metadata and create the code only while the exact prepared identity and active
account/generation still match. The consumed session must match Google and remain unexpired at
preparation; expiry is rechecked before dispatch. No additional secrets, setup values, routes,
or infrastructure are required. Password-first then Google can share a subject; adding a password to a
provider-first account is not supported, and pending registration cannot install a credential there.
There are no compatibility paths or backfills. See the [callback design](../../design/functions/auth/api/providers/google.md).

## Google Values Mapping

| Google value | Irongate field |
| --- | --- |
| Web OAuth client ID | `auth.googleClientId` |
| Web OAuth client secret | SST secret `GoogleClientSecret` |
| Authorized redirect URI | `<ApiUrl>/google/callback` |
| OAuth client app callback | `auth.clients.toml` redirect URI |

## Browser Transaction

Navigate the browser to Irongate's authorize URL. `/google/authorize` sets a short-lived
`__Host-ig-google-<state-HMAC>` cookie on the HTTPS auth issuer before redirecting to Google.
The cookie is Secure, HttpOnly, SameSite=Lax, Path=/ and has no Domain. Its lifetime cannot exceed
the original authorize session or provider-state TTL. It is not the web app's login session.

Google returns to that same issuer in the same browser. The callback requires the matching cookie
even for a declined sign-in. Fetching provider start from a BFF/native HTTP client and copying its
cookie to the Worker or loopback host will not work. The existing web redirect and Tauri system-browser
flow already follow this contract; they do not need the cookie exposed to JavaScript or Rust.

Fresh sign-in is required for transactions created without a browser binding. There is no unbound
fallback, migration or data cleanup. After confirmed consumption the callback clears only its own
cookie; independent attempts have separate names. No additional settings or secrets are required.
Public mailbox confirmation remains unavailable.

## Validation

- Use a separately authorized deployed stage and disposable test identity.
- Start login with `provider=google`.
- Confirm Irongate returns to the configured OAuth client callback.
- Confirm the generated Irongate account has a Google identity record.
- Confirm the client redirect contains the authorization code and original state, not access,
  refresh or ID tokens; exchange the code through the existing client flow. Repeat sign-in and
  confirm the subject remains stable. Do not copy codes, tokens or callback URLs into notes.

Useful manual authorization URL shape:

```text
<ApiUrl>/authorize?response_type=code&client_id=<client_id>&redirect_uri=<registered_client_callback>&scope=openid%20email%20profile&state=<random>&code_challenge=<s256_challenge>&code_challenge_method=S256&provider=google
```

For the Cloudflare web example, the button is shown only when `auth.googleClientId` is non-empty and
the example has been redeployed.

## Common Failures

- `redirect_uri_mismatch`: the Google client does not contain the exact Irongate callback URL.
- `origin_mismatch`: a browser integration is calling Google directly from an unregistered origin.
  The Irongate web example should not need this.
- Google login button missing in the web example: `auth.googleClientId` is empty or the example was
  not redeployed after changing stage config.
- Provider not configured: `GoogleClientSecret` is missing for the deployed SST stage.
- Invalid/expired browser transaction: restart from the app in the same browser and verify that
  navigation reaches the HTTPS auth issuer, not a server-side fetch. Missing, malformed, duplicate
  or mismatched cookies cannot be substituted with query parameters or the old authorize cookie.
- Test user blocked: the consent screen is in testing mode and the Google account is not listed as a
  test user.
- Delayed changes: Google says OAuth client changes can take several minutes, sometimes longer, to
  propagate.
- Confirmed stale/conditional issuance or blocked identity reuse: `400 invalid_grant` with
  `provider sign-in is no longer valid`. Start a fresh sign-in.
- New Google identity without authoritative email: the same safe `invalid_grant` response; repeating
  sign-in does not establish mailbox ownership. The internal confirmation operations are not a
  user-facing flow or a configuration option.
- Issuance storage outage, transaction conflict or uncertain outcome: `500 server_error` with
  `provider sign-in unavailable`. This is not invalid credentials or proof that nothing committed.

Invalid browser proof leaves provider/session state untouched. After valid browser admission,
provider state and authorize session are consumed before Google exchange/validation, outside the
identity/code transaction. Later failed callbacks require restarting sign-in, not refreshing/replaying the
callback. There is one application-level issuance attempt, with no automatic re-resolution or
upstream exchange retry. Do not delete accounts to compensate for an error: a lost response may
follow a complete commit, and any unreturned code expires normally. See the
[smoke guide](../operations/smoke-test.md) for authorized checks and limitations.

## Done When

- Google login creates or reuses a persisted Irongate identity.
- Tokens are issued by Irongate, not by the web example.
- Google client secret material is stored only as an SST secret.

## References

- Google: Using OAuth 2.0 for Web Server Applications:
  <https://developers.google.com/identity/protocols/oauth2/web-server>
- Google: OpenID Connect:
  <https://developers.google.com/identity/openid-connect/openid-connect>
- Google Cloud Help: Manage OAuth clients:
  <https://support.google.com/cloud/answer/15549257>
