# FontLoader

[English](README.md) | [简体中文](README.zh-CN.md)

## Highlights

- Lightweight Windows desktop utility focused on temporary font loading.
- Minimize to the system tray and keep running in the background with very low resource usage.
- Drag and drop `.ttf`, `.otf`, and `.ttc` files to import them.
- Import and unload fonts in batches, or search, select, and manage them individually.
- Load fonts for the current Windows session without copying them to the system font directory or installing them permanently.
- Restore fonts from the previous session once, then unload managed fonts automatically before exit.

FontLoader loads `.ttf`, `.otf`, and `.ttc` files through the Windows session-level font API. Fonts become available to other applications during the current session without being copied to the system font directory or installed permanently.

> When you close the window or choose “卸载全部并退出” (Unload all and exit) from the tray menu, FontLoader destroys the WebView and removes its tray icon before silently unloading managed fonts in the background. The process may remain briefly after both disappear and exits automatically when cleanup finishes. If unloading fails, the tray icon and window return so you can retry or choose to exit anyway.

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

1. Open FontLoader and choose or drag in one or more `.ttf`, `.otf`, or `.ttc` files.
2. Review the loaded fonts in the list. Duplicate files are skipped. Copy any family name with its adjacent button. TTC collections group faces by family; expand the list or copy all unique names, one per line.
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
