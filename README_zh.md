# FuckVim 🚀

[English](README.md) | **中文**

一个用 Go 编写的轻量级终端代码编辑器，支持 Vim 风格快捷键。

## ✨ 功能特性

- **Vim 模态编辑** - Normal、Insert、Command 模式
- **分屏** - 水平 (`:sp`) 和垂直 (`:vsp`) 分屏
- **标签页** - 多工作区，`Shift+H/L` 切换
- **文件树** - `:tree` 打开侧边栏
- **Git 集成** - 状态面板、暂存、提交、推送
- **模糊搜索** - `Ctrl+P` 快速定位文件
- **语法高亮** - Chroma 驱动，支持 100+ 语言
- **智能补全** - 上下文感知 + 自动括号配对
- **LSP 支持** - 语言服务器协议 (gopls)
- **WASM 插件** - 可扩展插件系统
- **WhichKey** - 空格键菜单提示
- **多语言** - 中英文界面

## 📦 依赖

### 必需
- **Go 1.24+** - [下载](https://go.dev/dl/)

### 可选 (LSP)
```bash
go install golang.org/x/tools/gopls@latest
```

## 🚀 安装

```bash
git clone https://github.com/joyins/fuckvim.git
cd fuckvim
go build -o fuckvim .
./fuckvim [文件名]
```

## ⌨️ 快捷键

查看完整快捷键：[docs/KEYBINDINGS_zh.md](docs/KEYBINDINGS_zh.md)

| 按键 | 动作 |
|------|------|
| `i` | 进入插入模式 |
| `Esc` | 返回普通模式 |
| `:w` / `:q` | 保存 / 退出 |
| `Space` | WhichKey 菜单 |
| `Ctrl+P` | 模糊搜索 |

## 📄 许可证

MIT License
