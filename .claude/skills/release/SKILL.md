---
name: release
description: Create a GitHub release with tag and changelog
disable-model-invocation: true
user-invocable: true
argument-hint: version
---

Create a GitHub release.

## Steps

1. Fetch remote tags: `git fetch --tags`
2. Determine version:
   - If $ARGUMENTS provided, use it (e.g., `1.5.1`)
   - Otherwise detect from branch name `release/<version>`
   - Otherwise get latest tag and suggest next version
3. Get previous tag: `git tag --sort=-v:refname | head -1`
4. Generate changelog: `git log <previous-tag>..HEAD --oneline --no-merges`
5. Pull latest main: `git pull origin main`

## Tag convention

Tags: `v<version>` (e.g., `v1.5.1`)

## Release title

`Release v<version>`

## Release notes format

```markdown
## What's Changed

### Features
- <feat commits>

### Bug Fixes
- <fix commits>

### Other Changes
- <chore/refactor/ci/docs commits>

**Full Changelog**: https://github.com/<owner>/<repo>/compare/<previous-tag>...v<version>
```

Omit empty sections. Do NOT include AI attribution.

## Execution

Use the gh CLI at `"/c/Program Files/GitHub CLI/gh.exe"`.

Show the release notes, then ask "Quer que eu crie?".
When user confirms, create with:
```bash
"/c/Program Files/GitHub CLI/gh.exe" release create v<version> --title "Release v<version>" --target main --notes "<notes>"
```

Return the release URL when done.
