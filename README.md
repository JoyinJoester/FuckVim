# FuckVim 🚀

**English** | [中文](README_zh.md)

A lightweight, modern terminal code editor written in Go with Vim-style keybindings.

![Go](https://img.shields.io/badge/Go-1.24+-00ADD8?style=flat&logo=go)
![License](https://img.shields.io/badge/License-MIT-green)

## ✨ Features

- **Vim-style Modal Editing** - Normal, Insert, Command modes
- **Split Panes** - Horizontal (`:sp`) and Vertical (`:vsp`) splits
- **Tab Pages** - Multiple workspaces with `Shift+H/L` navigation
- **File Tree** - Sidebar file browser with `:tree`
- **Git Integration** - Status panel, staging, commit, push
- **Fuzzy Finder** - Telescope-style file search with `Ctrl+P`
- **Syntax Highlighting** - Powered by Chroma (100+ languages)
- **Smart Completion** - Context-aware autocomplete with Auto-Pairs
- **LSP Support** - Language Server Protocol integration (gopls)
- **WASM Plugins** - Extensible plugin system
- **WhichKey Menu** - `Space` leader key with hints
- **I18n** - English and Chinese language support

## 📦 Dependencies

### Required
- **Go 1.24+** - [Download](https://go.dev/dl/)

### Optional (for LSP features)
```bash
go install golang.org/x/tools/gopls@latest
```

## 🚀 Installation

```bash
git clone https://github.com/joyins/fuckvim.git
cd fuckvim
go build -o fuckvim .
./fuckvim [filename]
```

## ⌨️ Key Bindings

See full keybindings: [docs/KEYBINDINGS_en.md](docs/KEYBINDINGS_en.md)

| Key | Action |
|-----|--------|
| `i` | Enter Insert mode |
| `Esc` | Return to Normal mode |
| `:w` / `:q` | Save / Quit |
| `Space` | Open WhichKey menu |
| `Ctrl+P` | Fuzzy file finder |

## 📁 Project Structure

```
fuckvim/
├── main.go          # Core editor logic
├── completion.go    # Autocomplete engine
├── lsp_client.go    # LSP client
├── lsp_types.go     # LSP protocol types
├── docs/            # Documentation
└── plugin/          # WASM plugin system
```

## 📄 License

MIT License - see [LICENSE](LICENSE)
