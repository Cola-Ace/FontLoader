# FontLoader

[English](README.md) | [简体中文](README.zh-CN.md)

FontLoader is a small Windows desktop utility for loading fonts temporarily into the current Windows session.

It loads `.ttf`, `.otf`, and `.ttc` files through the Windows session-level font API. Fonts become available to other applications during the current session without being copied to the system font directory or installed permanently.

## Features

- Load multiple TrueType, OpenType, and TrueType Collection files at once.
- Search, select, and unload individual fonts or batches of fonts.
- Keep the Rust background process and system tray available after the main window is minimized.
- Offer a one-time restore action for fonts from the previous session.
- Unload managed fonts before the application exits.

## Requirements

- Windows 10 version 1803 or later, or Windows 11 (currently targeting x64).
- Node.js and pnpm `10.33.2`.
- A Rust toolchain and the Windows prerequisites required by Tauri 2.
- Microsoft Edge WebView2 Runtime.

## Quick Start

Install the JavaScript dependencies and launch the desktop application:

```powershell
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm tauri dev` starts the Vite development server through the Tauri configuration and opens the desktop application. The frontend-only server can be started with `pnpm dev` at `http://localhost:1420`.

## Commands

| Command | Description |
| --- | --- |
| `pnpm install --frozen-lockfile` | Install dependencies from the pnpm lockfile. |
| `pnpm dev` | Start the Vite development server on port `1420`. |
| `pnpm tauri dev` | Run the Windows desktop application in development mode. |
| `pnpm build` | Type-check and build the frontend into `dist/`. |
| `pnpm test` | Run the Vitest test suite. |
| `pnpm tauri build --no-bundle` | Build a release executable without creating an installer bundle. |

The release executable is written to Cargo's release target directory, normally `src-tauri/target/release/FontLoader.exe`. If `CARGO_TARGET_DIR` is configured, use its `release` subdirectory instead.

## Usage

1. Open FontLoader and choose one or more `.ttf`, `.otf`, or `.ttc` files.
2. Review the loaded fonts in the list. Duplicate files are skipped.
3. Use search and selection controls to find fonts and unload them individually or in batches.
4. Minimize the window to keep the app in the system tray. Open it again from the tray when needed.
5. When the app detects fonts from an earlier session, choose whether to restore or dismiss them.

Font registration is temporary: FontLoader does not copy font files, modify the registry, or provide permanent font installation. A restart of Windows removes the session-level registrations.

## Project Structure

```text
src/              Vue 3 and TypeScript frontend
src-tauri/        Tauri 2 and Rust backend
```

The backend uses Win32 font APIs and `ttf-parser` for font metadata. The frontend is built with Vue 3, Vite, and TypeScript.
