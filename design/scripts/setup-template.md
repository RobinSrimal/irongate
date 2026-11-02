# Template Setup

Sources: [setup-template.sh](../../scripts/setup-template.sh) and [setup-template.test.sh](../../scripts/setup-template.test.sh).

## What

The project-name substitution script and its disposable-fixture regression tests.

## Why

A consumer can rename template packages, display names and default profile references without editing each file by hand.

## How

The script normalizes a slug, derives or accepts PROJECT_DISPLAY_NAME and replaces Irongate/irongate tokens in selected text files. It prunes git, SST, dependency, build, script and editor directories and reports changed files. IRONGATE_TEMPLATE_ROOT selects an explicit root for tests; running setup changes files and is not a read-only validation. The shell test builds temporary fixtures and checks naming, preservation and repeated-run behaviour. It does not deploy resources or generate runtime secrets.
