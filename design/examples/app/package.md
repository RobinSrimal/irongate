# App Frontend Package

Source: [packages/examples/app/package.json](../../../packages/examples/app/package.json).

## What

React, Vite and Tauri CLI dependencies and package scripts.

## Why

Frontend compilation and native invocation have an explicit local toolchain.

## How

Build runs TypeScript and Vite. Tests compile auth helpers for Node and then build the frontend; tauri delegates to the native CLI. This is a private optional workspace.
