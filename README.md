<div align="center">

<img src="assets/banner.png" alt="Kodemo's banner with icon and Kodemi mascot" width="75%" />

---

**A native code editor that respects your machine.**

Written in 100% Rust · Built with egui · No Electron · No telemetry

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows%20%7C%20macos-lightgrey.svg)](#installation)

[Features](#features) · [Installation](#installation) · [Performance](#performance) · [Configuration](#configuration) · [Building](#building-from-source)

</div>

---

## Why Kodemo?

Most modern editors are built on Electron and quietly send usage data to a server. Kodemo is not.

- **Native Rust binary.** One executable, no runtime, no Node.js.
- **Lean on RAM.** Lighter than Electron-based editors. Heavy features unload when idle.
- **Zero telemetry.** Kodemo sends nothing to any server. Audit it yourself — it's open source.
- **Runs on old hardware.** From OpenGL 2.0 GPUs to modern ones, with software fallback.
- **Cross-platform.** Linux, Windows and macOS — same editor everywhere.

---

## Features

### 🖥️ Native and cross-platform

Linux (`.deb`, AppImage), Windows (`.exe`), macOS (`.dmg`). Runs on OpenGL 2.0 hardware with automatic software-rendering fallback.

### 📝 Editor

VS Code–style layout, tree-sitter syntax highlighting, font ligatures, command palette, file tree, and async I/O that never blocks the UI.

### 🤖 Local AI ghost autocomplete

**Qwen 2.5 0.5B** running locally via `llama.cpp`. No cloud, no API key, no network. Loads on first request and **unloads when idle** — memory returns to baseline. FIM prompting for multi-line, context-aware suggestions.

### 🌐 Built-in WebView

Embedded WebKitGTK panel for live-previewing HTML, CSS, and JS. Serves your workspace on a local port, and **suspends when hidden** so it never burns memory in the background.

### 🎨 Themes

Import VS Code color themes as JSON. Dark+ and Light+ built in, with full light/dark support.

### 🌍 Internationalization

**English** and **Brazilian Portuguese** out of the box, with instant switching.

### ⚡ Auto-scaffold

Create an empty file and Kodemo fills in idiomatic boilerplate. One undo removes it. Multi-file scaffolds via `!axum`, `!react`, `!fastapi`.

### 🧩 Snippets

Emmet-compatible (`!`, `html:5`) plus **Bootstrap 5** components: `bs5`, `bs:navbar`, `bs:card`, `bs:modal`, `bs:grid`, `bs:form`, `bs:table`, `bs:alert`, `bs:btn`, and `bsi:` for icons.

### 🔋 Energy Saver

Battery-aware profile that throttles CPU, suspends the WebView, and disables ghost text on low power. Fully configurable.

---

## Performance

Kodemo releases resources when they aren't in use:

| State | Approximate memory |
| :--- | :--- |
| Idle, start page | **~70 MB** |
| Project open, AI off, WebView closed | **~100 MB** |
| + Local AI loaded | **~400 MB** |
| + WebView showing a real website | **~700 MB** |
| **After AI unload + WebView suspend** | **~120 MB** |

*Measured as RSS for the whole process tree on Linux. The same workflow in an Electron-based editor with Copilot and a browser open typically sits between 1.5 GB and 2.5 GB — and does not return to baseline when idle.*

---

## Installation

### Coming soon...
