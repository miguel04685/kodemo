# Welcome to Kodemo

Kodemo is a native desktop IDE written in Rust with `egui`. It is designed for
small machines — the kind that still have a spinning disk, two cores and a
legacy Intel GPU.

## A five-minute tour

1. **Explorer** — press `Ctrl+B` to show or hide the sidebar. Right-click files
   to create, rename or delete them.
2. **Language suggestions** — start typing in an HTML, CSS, JavaScript, Python,
   PHP or Java file. A rounded list of language words and document identifiers
   appears; use `↑`/`↓` to choose and `Tab` or `Enter` to accept.
3. **Ghost completion** — keep typing in `app.js`. When the local Qwen model is
   available, a grey suggestion appears. Press `Tab` to accept it.
4. **Spell check** — misspelled words in comments, strings and Markdown get a
   red squiggle. Turn it off in **File → Settings** if you prefer.
5. **Live preview** — press `Ctrl+Alt+P`. Edit `index.html`, `styles.css` or
   `app.js`, save with `Ctrl+S`, and the preview reloads by itself.
6. **Command palette** — press `Ctrl+Shift+P` for every action in the IDE.
7. **Quick open** — press `Ctrl+P` and type part of a file name.
8. **Themes** — open **File → Settings** and pick a light or dark theme, or tune
   every color yourself.

## Keyboard shortcuts

| Action | Shortcut |
| --- | --- |
| Command palette | `Ctrl+Shift+P` |
| Quick open file | `Ctrl+P` |
| Save | `Ctrl+S` |
| Save as | `Ctrl+Shift+S` |
| Close editor | `Ctrl+W` |
| Toggle sidebar | `Ctrl+B` |
| Toggle bottom panel | `Ctrl+J` |
| Toggle preview | `Ctrl+Alt+P` |
| Settings | `Ctrl+,` |
| Find in files | `Ctrl+Shift+F` |
| Go to line | `Ctrl+G` |
| Zoom in / out | `Ctrl+=` / `Ctrl+-` |

## Local AI

Kodemo looks for `llama-server` in `~/.local/share/kodemo/llama/`,
`~/.local/bin/` and `PATH`. It looks for the Qwen 0.5B GGUF model in
`~/.local/share/kodemo/models/`. Both paths can be set explicitly in Settings.

The model is queried through llama.cpp's fill-in-the-middle interface. The
endpoint must accept llama.cpp's `/completion` payload.

## Files in this workspace

- `index.html` — this page.
- `styles.css` — all of the visual design.
- `app.js` — a tiny interactive counter, saved to localStorage.
- `snippets.py` — a small Python scratchpad.
- `README.md` — the guide you are reading.

Enjoy the quiet.
