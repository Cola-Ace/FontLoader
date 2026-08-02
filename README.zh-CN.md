# FontLoader

[English](README.md) | 简体中文

FontLoader 是一个 Windows 桌面临时字体加载工具，用于将字体加载到当前 Windows 会话中。

它通过 Windows 会话级字体 API 加载 `.ttf`、`.otf` 和 `.ttc` 文件。字体在当前会话中可供其他应用使用，不会复制到系统字体目录，也不会永久安装。

## 功能

- 一次选择并加载多个 TrueType、OpenType 和 TrueType Collection 字体文件。
- 搜索、选择并单独或批量卸载字体。
- 最小化主窗口后保留 Rust 后台进程和系统托盘。
- 检测上次会话中的字体，并提供一次性恢复入口。
- 应用退出前卸载由 FontLoader 管理的字体。

## 环境要求

- Windows 10 1803 或更高版本，或 Windows 11（当前目标为 x64）。
- Node.js 和 pnpm `10.33.2`。
- Rust 工具链，以及 Tauri 2 所需的 Windows 开发环境。
- Microsoft Edge WebView2 Runtime。

## 快速开始

安装 JavaScript 依赖并启动桌面应用：

```powershell
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm tauri dev` 会根据 Tauri 配置自动启动 Vite 开发服务器并打开桌面应用。只启动前端开发服务器时，可以使用 `pnpm dev`，地址为 `http://localhost:1420`。

## 常用命令

| 命令 | 说明 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 根据 pnpm 锁文件安装依赖。 |
| `pnpm dev` | 在 `1420` 端口启动 Vite 开发服务器。 |
| `pnpm tauri dev` | 以开发模式运行 Windows 桌面应用。 |
| `pnpm build` | 执行类型检查并将前端构建到 `dist/`。 |
| `pnpm test` | 运行 Vitest 测试套件。 |
| `pnpm tauri build --no-bundle` | 构建 release 可执行文件，不生成安装器。 |

release 可执行文件会输出到 Cargo 的 release 目标目录，通常为 `src-tauri/target/release/FontLoader.exe`。如果配置了 `CARGO_TARGET_DIR`，请到该目录下的 `release` 子目录查找。

## 使用方式

1. 打开 FontLoader，选择一个或多个 `.ttf`、`.otf` 或 `.ttc` 文件。
2. 在列表中查看已加载字体，重复文件会自动跳过。
3. 使用搜索和选择控件查找字体，然后单独或批量卸载。
4. 最小化窗口后，应用会保留在系统托盘中；需要时从托盘重新打开。
5. 如果检测到上次会话遗留的字体，可以选择恢复或忽略。

字体注册是临时的：FontLoader 不会复制字体文件、不修改注册表，也不提供永久字体安装。Windows 重启后，会话级字体注册会失效。

## 项目结构

```text
src/              Vue 3 和 TypeScript 前端
src-tauri/        Tauri 2 和 Rust 后端
```

后端使用 Win32 字体 API，并通过 `ttf-parser` 读取字体元数据；前端使用 Vue 3、Vite 和 TypeScript 构建。
