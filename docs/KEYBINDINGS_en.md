# FuckVim Keybindings Reference

> 🎯 **Intent-First Editor** - Designed for the AI Era

---

## Mode Switching

| Key | Current Mode | Action |
|-----|--------------|--------|
| `i` | Normal | Enter **Insert** mode |
| `Esc` | Insert | Return to **Normal** mode |
| `:` | Normal | Enter **Command** mode |
| `Esc` | Command | Cancel, return to **Normal** |
| `Ctrl+C` | Any | ❌ **Cancel** (use :q to quit) |

---

## Window Navigation (Spatial)

> i3wm / Tmux style directional navigation

| Position | Key (Ctrl + Dir) | Target |
|----------|------------------|--------|
| **Editor** (right) | `Ctrl+H` / `Left` | 👈 **Sidebar** |
| **File Tree** (top-left) | `Ctrl+L` / `Right` | 👉 **Editor** |
| | `Ctrl+J` / `Down` | 👇 **Git Panel** |
| **Git Panel** (bottom-left) | `Ctrl+L` / `Right` | 👉 **Editor** |
| | `Ctrl+K` / `Up` | 👆 **File Tree** |

---

## Normal Mode (Navigation)

| Key | Action |
|-----|--------|
| `h/j/k/l` | Move cursor |
| `Shift+H/L` | Switch tabs |
| `0` / `$` | Line start/end |
| `p` | 📋 Paste |
| `Ctrl+P` | 🔍 Fuzzy finder |
| `Ctrl+T` | 📟 Terminal |
| `Space` | ⌨ WhichKey menu |

---

## ⌨ WhichKey Menu (Leader Key)

| Key | Action |
|-----|--------|
| `f` | 🔍 Find files |
| `e` | 📂 File tree |
| `g` | 🐙 Git status |
| `w` | 💾 Save |
| `q` | ❌ Quit |
| `v` / `s` | Split vertical/horizontal |
| `t` | 📟 Terminal |
| `T` | 🔢 Toggle line numbers |
| `l` | 🌐 Switch language |
| `?` | 💡 Help |

---

## Insert Mode (Editing)

| Key | Action |
|-----|--------|
| Any char | Insert character |
| `Ctrl+V` | 📋 Paste |
| `Enter` | New line (smart indent) |
| `Backspace` | Delete (auto-pairs aware) |
| `Tab` | Accept completion |
| `↑/↓` | Navigate completions |

---

## Command Mode

| Command | Action |
|---------|--------|
| `:q` | Quit |
| `:w` | Save |
| `:wq` | Save & quit |
| `:vsp [file]` | Vertical split |
| `:sp [file]` | Horizontal split |
| `:tabnew` | New tab |
| `:tree` | Toggle file tree |
| `:git` | Toggle Git panel |

---

## File Tree (Yazi-style)

| Key | Action |
|-----|--------|
| `j/k` | Navigate |
| `Enter` | Open file/folder |
| `Backspace` | Go up |
| `a` | ➕ New file (add `/` for folder) |
| `d` | 🗑️ Delete |
| `r` | ✏️ Rename |

---

## Git Panel

| Key | Action |
|-----|--------|
| `Space` | ✅ Stage/Unstage |
| `c` | 💾 Commit (staged only) |
| `C` | 🚀 Stage all + Commit |
| `P` | 📤 Push |
| `r` | 🔄 Refresh |
