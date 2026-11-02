# Web OAuth Tests

Source: [packages/examples/web/tests/oauth.test.ts](../../../../packages/examples/web/tests/oauth.test.ts).

## What

Pure tests of authorize/callback construction and PKCE helpers.

## Why

Client request parameters can regress independently of UI presentation.

## How

Node tests check provider selection, redirect, scopes and PKCE/state parameters using local values. They do not perform provider sign-in.
