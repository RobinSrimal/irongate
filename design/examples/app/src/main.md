# React Entrypoint

Source: [packages/examples/app/src/main.tsx](../../../../packages/examples/app/src/main.tsx).

## What

Mounting the App component into the HTML root.

## Why

Frontend bootstrapping stays separate from authentication commands.

## How

Creates a React root and renders App under StrictMode. It has no token persistence logic.
