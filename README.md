# FuckVim - 一个基于rust的 Vim 替代品(未补完)

[![GitHub license](https://img.shields.io/github/license/JoyinJoester/Fuckvim)](https://github.com/JoyinJoester/Fuckvim/blob/main/LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/JoyinJoester/Fuckvim)](https://github.com/JoyinJoester/Fuckvim/stargazers)
[![GitHub issues](https://img.shields.io/github/issues/JoyinJoester/Fuckvim)](https://github.com/JoyinJoester/Fuckvim/issues)

## 简介

FuckVim 是一个使用 Rust 编写的现代化文本编辑器，旨在提供 Vim 的强大功能，同时融合现代编辑器的用户体验。它支持多标签页、分屏编辑、语法高亮、文件浏览器以及集成终端等功能，同时保持了轻量级和高性能的特点，此项目使用github copilot进行编译纠错

## 特性

- 🚀 基于 Rust 构建，性能卓越且内存安全
- 🔌 插件系统支持 Lua 脚本
- 🌈 语法高亮支持各种编程语言
- 📑 多标签页和分屏编辑
- 🗂️ 内置文件浏览器
- 📦 缓冲区管理
- 🎮 模态编辑（Normal、Insert、Visual 等模式）
- 🔍 文本搜索和替换
- ⚡ 快速启动和响应
- 🖥️ 集成终端
- 🔄 Neovim 插件兼容层

## 安装

### 从源码安装

```bash
git clone https://github.com/JoyinJoester/Fuckvim.git
cd Fuckvim
cargo build --release
```

编译完成后，可执行文件将位于 `target/release/` 目录中。

### 使用预编译的二进制文件

您可以从 [releases](https://github.com/JoyinJoester/Fuckvim/releases) 页面下载适用于您操作系统的预编译二进制文件。

## 快速入门

### 基本操作

FuckVim 保留了大部分 Vim 的按键映射，因此如果您熟悉 Vim，您将很快适应 FuckVim：

- `i` - 进入插入模式
- `Esc` - 返回普通模式
- `h`, `j`, `k`, `l` - 光标移动
- `:w` - 保存文件
- `:q` - 退出
- `:wq` 或 `:x` - 保存并退出

### 窗口管理

- `:split` 或 `:sp` - 水平分割窗口
- `:vsplit` 或 `:vs` - 垂直分割窗口
- `Ctrl+w` 然后 `h`, `j`, `k`, `l` - 在窗口间移动
- `:close` 或 `:clo` - 关闭当前窗口

### 标签页

- `:tabnew` 或 `:tabe` - 创建新标签页
- `:tabnext` 或 `:tabn` - 切换到下一个标签页
- `:tabprevious` 或 `:tabp` - 切换到上一个标签页
- `:tabclose` 或 `:tabc` - 关闭当前标签页

### 缓冲区管理

- `:buffer` 或 `:b` [number] - 切换到指定缓冲区
- `:bnext` 或 `:bn` - 切换到下一个缓冲区
- `:bprevious` 或 `:bp` - 切换到上一个缓冲区

### 集成终端

- `:toggleterm` - 切换终端可见性
- `:focusterm` - 将焦点移至终端
- `:exitterm` - 退出终端模式
- `:sendterm [命令]` - 向终端发送命令

## 配置

FuckVim 使用 Lua 进行配置，配置文件位于：

- Linux/macOS: `~/.config/fkvim/config.lua`
- Windows: `%USERPROFILE%\.config\fkvim\config.lua`

示例配置：

```lua
-- 基本设置
vim.opt.number = true
vim.opt.relativenumber = true
vim.opt.tabstop = 4
vim.opt.shiftwidth = 4
vim.opt.expandtab = true

-- 按键映射
vim.keymap.set('n', '<C-s>', ':w<CR>', { silent = true })
vim.keymap.set('n', '<F5>', ':toggleterm<CR>', { silent = true })

-- 插件配置
require('plugins').setup {
    packages = {
        -- 添加您的插件
        { 'nvim-treesitter/nvim-treesitter', config = function()
            require('nvim-treesitter.configs').setup {
                ensure_installed = { "rust", "lua", "vim", "javascript" },
                highlight = { enable = true },
            }
        end },
    },
}
```

## 插件系统

FuckVim 支持通过 Lua 脚本扩展功能。插件可以放置在以下目录：

- Linux/macOS: `~/.local/share/fkvim/plugins/`
- Windows: `%USERPROFILE%\.local\share\fkvim\plugins\`

FuckVim 还提供了 Neovim 插件兼容层，允许您使用大多数 Neovim 插件。

## 贡献

欢迎贡献代码、报告问题或提出新功能请求！请查看 [贡献指南](CONTRIBUTING.md) 了解更多信息。

## 许可证

FuckVim 采用 [MIT 许可证](LICENSE)。

## 致谢

FuckVim 的开发受到了以下项目的启发：

- [Neovim](https://neovim.io/)
- [Helix](https://helix-editor.com/)
- [Xi Editor](https://xi-editor.io/)

## 联系方式

- GitHub: [JoyinJoester](https://github.com/JoyinJoester)
- Email: [Joyin8888@foxmail.com]
