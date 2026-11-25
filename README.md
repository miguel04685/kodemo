<div align="center">

<img src="assets/banner.png" alt="Kodemo's banner with icon and Kodemi mascot" width="75%" />

---

**A native code editor that respects your machine.**

Written in 100% Rust · Built with egui · No Electron · No telemetry

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows%20%7C%20macos-lightgrey.svg)](#installation)

[Features](#features) · [Requirements](#requirements) · [Installation](#installation) · [Building](#building-from-source) · [Packaging](#packaging-a-deb) · [Configuration](#configuration)

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

VS Code–style layout, syntax highlighting, line numbers, font ligatures, command palette, file tree, right-click **Select All**, and async I/O that never blocks the UI.

### 🌍 Languages

Rust, Python, JavaScript, TypeScript, HTML, CSS, JSON, Markdown, TOML, YAML, Shell, SQL, C, C++, C#, Java, Go, PHP, Kotlin, Lua, Swift, Ruby, Dart, Scala, R, Perl, Elixir, Haskell, Zig, Nim, Julia, PowerShell, Assembly, Solidity, Objective-C and Groovy. Local word suggestions include language keywords, standard-library names, identifiers from the open file, and ready-to-run compile/test commands on an empty line.

### 🤖 Local AI ghost autocomplete

**Qwen 2.5 Coder 0.5B** running locally via `llama.cpp`. No cloud, no API key, no network after the one-time model download. FIM prompting, context-aware multi-line suggestions, prompt caching, and a CPU priority boost for the local server. Loads on first request and **unloads when idle**.

If no model is installed, enabling ghost text downloads Q4_K_M or Q4_0 automatically. The `.deb` installer can also fetch it during setup.

### 🌐 Built-in WebView

Embedded WebKitGTK panel for live-previewing HTML, CSS, and JS. Serves your workspace on a local port, and **suspends when hidden** so it never burns memory in the background.

### 🎨 Themes

17 built-in themes, import VS Code color themes as JSON, full light/dark support, and an AMOLED black mode.

### 🌍 Internationalization

**English** and **Brazilian Portuguese** out of the box, with instant switching. The spell checker uses a Brazilian Portuguese dictionary.

### ⚡ Auto-scaffold

Create an empty file and Kodemo fills in idiomatic boilerplate. One undo removes it. Multi-file scaffolds via `!axum`, `!react`, `!fastapi`.

### 🧩 Snippets

Emmet-compatible (`!`, `html:5`) plus **Bootstrap 5** components: `bs5`, `bs:navbar`, `bs:card`, `bs:modal`, `bs:grid`, `bs:form`, `bs:table`, `bs:alert`, `bs:btn`, and `bsi:` for icons.

### 🔋 Energy Saver

Battery-aware profile that throttles CPU, suspends the WebView, and disables ghost text on low power. Fully configurable.

---

## Requirements

Kodemo targets machines that other IDEs have left behind.

### Minimum

| | |
| :--- | :--- |
| CPU | 64-bit dual-core, 1.6 GHz (x86-64 or ARM64) |
| RAM | **1 GB** (2 GB recommended for the local AI) |
| Disk | 40 MB without AI · **~550 MB** with the Q4_K_M model |
| GPU | OpenGL 2.0, or Mesa's software renderer |
| Display | 1024 × 600 |
| OS | Linux (X11; Wayland through XWayland), Windows 10, macOS 11 |
| Libraries | `libgl1`, `libx11-6`, `libxkbcommon0`, `libfontconfig1` |
| Optional | `libwebkit2gtk-4.1-0` for the website preview, `curl` for model downloads, `git` for the Git panel |

### Recommended

| | |
| :--- | :--- |
| CPU | Quad-core 2.0 GHz or better |
| RAM | 4 GB |
| Disk | 1 GB free for the editor, model and projects |
| GPU | Any OpenGL 3.3-capable GPU |
| OS | Debian 12 / Ubuntu 22.04 or newer, Windows 11, macOS 13+ |

A 2-core / 1.3 GB machine runs the editor and the 0.5B model comfortably with the WebView closed. Keeping a Electron-class browser open at the same time is what makes a small machine swap.

---

## Installation

### Debian / Ubuntu

Download the `.deb` for your architecture (`amd64` or `arm64`) and install it:

```sh
sudo apt install ./kodemo_0.1.0_amd64.deb
```

The installer asks, through debconf:

1. which **color theme** to start with;
2. whether to use **hardware (OpenGL) or software rendering**;
3. whether to enable the **WebKitGTK website preview** (the package recommends `libwebkit2gtk-4.1-0` and reuses the system copy when present);
4. whether to **download the local Qwen model** now, and whether to use **Q4_K_M** (better) or **Q4_0** (smaller).

It then prints — and, if `yad` is available, shows a small window with — **“Your setup is done!”** and Kodemi jumping (`kodemi-jump.png`).

Skip the model during setup and Kodemo will offer to download it the first time you enable ghost suggestions.

### Other platforms

Prebuilt Windows and macOS binaries are on the roadmap. For now, build from source.

---

## Building from source

### 1. Install the toolchain

Rust **1.85 or newer**:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# or: sudo apt install cargo rustc
```

Linux build dependencies:

```sh
sudo apt install build-essential pkg-config libgl1-mesa-dev libx11-dev \
    libxkbcommon-dev libfontconfig1-dev
```

Optional, for the embedded preview helper:

```sh
sudo apt install libwebkit2gtk-4.1-dev
```

### 2. Build

```sh
git clone https://github.com/miguel04685/kodemo.git
cd kodemo
cargo build --release --offline --bins
```

The `--offline` flag is supported and recommended for reproducible builds; delete it if you need Cargo to fetch crates for the first time.

Binaries land in `target/release/`:

- `kodemo` — the editor;
- `kodemo-webview` — the WebKit helper (optional at runtime).

Run it:

```sh
./target/release/kodemo
./target/release/kodemo ~/projects/my-app
```

If the GPU driver refuses to create a context, Kodemo relaunches itself with Mesa's software rasterizer. Force it with:

```sh
kodemo --software
```

### 3. Test

```sh
cargo test --offline
```

---

## Packaging a .deb

```sh
./packaging/build-deb.sh
```

This builds both binaries, stages the desktop file, icons, the jumping-Kodemi image and — when available — the bundled `llama.cpp` runtime, then writes:

```
target/kodemo_0.1.0_amd64.deb
```

Cross-building is intentionally refused: the package embeds native shared libraries, so build the ARM64 package on an ARM64 machine (or in an ARM64 container). The same script then produces `kodemo_0.1.0_arm64.deb`.

Package contents, installation size and dependency prompts live in `packaging/debian/` (`control.in`, `postinst`, `templates`).

---

## Configuration

Settings live in `~/.config/kodemo/settings.json` (or `$XDG_CONFIG_HOME/kodemo/settings.json`) and can be edited by hand or through **File → Settings**.

Useful environment variables:

| Variable | Effect |
| :--- | :--- |
| `KODEMO_CONFIG_DIR` | Use a different settings folder (portable installs, tests) |
| `KODEMO_WEBVIEW_HELPER` | Point at a specific `kodemo-webview` binary |
| `KODEMO_SOFTWARE_CHILD` | Internal flag used by the software-rendering relaunch |
| `LIBGL_ALWAYS_SOFTWARE=1` | Force Mesa's software renderer |

Model files are discovered in `~/.local/share/kodemo/models/`; `llama-server` in `~/.local/share/kodemo/llama/`, `~/.local/bin/`, `/usr/lib/kodemo/llama/` or `PATH`.

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

*Measured as RSS for the whole process tree on Linux. The CPU/RAM indicator in the top bar sums Kodemo, the WebView helper, the Qwen server and any terminal child process.*

---

## License

Kodemo is dual-licensed under **MIT OR Apache-2.0**. See `LICENSE-MIT` and `LICENSE-APACHE`.

Bundled third-party assets keep their own licenses:

- **Inter**, **JetBrains Mono**, **Space Grotesk** — SIL Open Font License 1.1;
- **Phosphor Icons** — MIT;
- **Brazilian Portuguese word list** — MPL-2.0 (see `assets/dict/pt_BR-LICENSE.txt`);
- **llama.cpp**, when bundled in a package — MIT.

The local Qwen model is downloaded separately and is subject to the Qwen license.
