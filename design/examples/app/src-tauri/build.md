# Tauri Build Hook

Source: [packages/examples/app/src-tauri/build.rs](../../../../packages/examples/app/src-tauri/build.rs).

## What

The tauri-build invocation.

## Why

Tauri generates platform context and validates its configuration during native builds.

## How

main calls tauri_build::build; it does not create cloud resources or load deployment secrets.
