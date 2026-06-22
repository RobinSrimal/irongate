# Template Setup

## Goal

Create a new repository from the Irongate template and rewrite project names.

## Inputs Needed

- New GitHub repository URL.
- Project name, for example `my-app`.

## Files To Edit

Usually none manually. The setup script updates names across the template.

## Commands

```bash
git clone <REPO_URL> my-app
cd my-app
npm run setup -- my-app
npm install
```

If no name is passed to `npm run setup`, the script uses the checkout folder name.

## Validation

```bash
npm run test:setup
npm run typecheck
```

## Common Failures

- Running setup in the original template repo instead of a new repository.
- Using a project name with shell-special characters.
- Forgetting `npm install` after setup.

## Done When

- `package.json`, `sst.config.ts`, and package names use the new project name.
- `npm run test:setup` passes.
