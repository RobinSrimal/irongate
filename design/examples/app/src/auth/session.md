# Frontend Session Types

Source: [packages/examples/app/src/auth/session.ts](../../../../../packages/examples/app/src/auth/session.ts).

## What

AppSession, stored-session status and display helpers.

## Why

The React view has a typed boundary to Tauri commands.

## How

AppSession currently includes access_token and optional id_token plus userinfo. Only refresh tokens are excluded from the JavaScript response. Display helpers return subject/email fallbacks without validating identity.
