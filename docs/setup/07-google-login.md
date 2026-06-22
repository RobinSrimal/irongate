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

Google redirects back to Irongate first. Irongate validates the Google response, resolves or creates
the Irongate identity, then redirects to the OAuth client callback configured in
`auth.clients.toml`.

## Google Values Mapping

| Google value | Irongate field |
| --- | --- |
| Web OAuth client ID | `auth.googleClientId` |
| Web OAuth client secret | SST secret `GoogleClientSecret` |
| Authorized redirect URI | `<ApiUrl>/google/callback` |
| OAuth client app callback | `auth.clients.toml` redirect URI |

## Validation

- Deploy the stage.
- Start login with `provider=google`.
- Confirm Irongate returns to the configured OAuth client callback.
- Confirm the generated Irongate account has a Google identity record.

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
- Test user blocked: the consent screen is in testing mode and the Google account is not listed as a
  test user.
- Delayed changes: Google says OAuth client changes can take several minutes, sometimes longer, to
  propagate.

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
