# Native App Crate

Source: [packages/examples/app/src-tauri/Cargo.toml](../../../../packages/examples/app/src-tauri/Cargo.toml).

## What

Tauri runtime, HTTP, crypto and keyring dependencies.

## Why

Protocol and secure-storage operations run in the Rust side of the example.

## How

Declares Tauri 2 with PNG/tray support, opener, keyring, reqwest, rand, SHA-256 and URL helpers. Library artifact types support the scaffold; the existing auth implementation is desktop-oriented.
