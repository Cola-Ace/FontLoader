# Repository instructions

- Use PowerShell 7 for repository commands.
- Prefer `pnpm` and `pnpx`; do not generate or commit `package-lock.json`.
- Keep frontend TypeScript formatted with 2-space indentation, double quotes, semicolons, trailing commas, and no parentheses around a single arrow parameter.
- Keep the product version synchronized in `package.json`, `src-tauri/Cargo.toml`, the root `font-loader` package entry in `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`.
- Every version update must increment the version number and use a matching `vX.Y.Z` release tag.
- Update `.gitignore` when adding generated or local-only files. Do not commit `dist/`, `.build-target/`, `src-tauri/target/`, or local environment files.
- Before a release, run `pnpm build`, `pnpm test`, and `pnpm tauri build --no-bundle`.
