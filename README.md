# FKVim - 一个现代化的 Vim 编辑器替代品

[![GitHub license](https://img.shields.io/github/license/JoyinJoester/Fuckvim)](https://github.com/JoyinJoester/Fuckvim/blob/main/LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/JoyinJoester/Fuckvim)](https://github.com/JoyinJoester/Fuckvim/stargazers)
[![GitHub issues](https://img.shields.io/github/issues/JoyinJoester/Fuckvim)](https://github.com/JoyinJoester/Fuckvim/issues)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

<p align="center">
  <img src="https://raw.githubusercontent.com/JoyinJoester/Fuckvim/main/assets/logo.png" alt="FKVim Logo" width="200" height="200" onerror="this.style.display='none'"/>
</p>

> 🚀 强大的模态编辑器，结合了 Vim 的高效与现代编辑器的友好体验

## 📋 功能概览

FKVim 是一个基于 Rust 构建的现代化文本编辑器，旨在提供 Vim/Neovim 的强大功能，同时融合现代编辑器的用户体验和更友好的界面：

- ⚡ **高性能** - 基于 Rust 构建，启动迅速，即使处理大文件也能保持流畅
- 🔍 **强大的编辑能力** - 保留 Vim 的模态编辑和快捷键理念
- 🧩 **灵活的插件系统** - 支持 Lua 脚本扩展，兼容部分 Neovim 插件
- 🖥️ **内置终端** - 无需离开编辑器即可使用命令行
- 🌈 **语法高亮** - 基于 Tree-sitter 的高级语法解析和高亮显示
- 📑 **多标签页和分屏** - 灵活的窗口管理，提高工作效率
- 📁 **文件浏览器** - 方便的文件导航和管理
- 🔄 **缓冲区管理** - 高效处理多个文件

## 🚀 快速开始

### 安装

#### 使用预编译二进制文件

从 [Releases](https://github.com/JoyinJoester/Fuckvim/releases) 页面下载适用于您操作系统的最新版本。

#### 从源码编译

确保您已安装 [Rust 工具链](https://www.rust-lang.org/tools/install)，然后执行：

```bash
# 克隆仓库
git clone https://github.com/JoyinJoester/Fuckvim.git
cd Fuckvim

# 编译
cargo build --release

# 安装（可选）
cargo install --path .
```

### 基本用法

#### 启动编辑器

```bash
# 打开编辑器
fkvim

# 打开指定文件
fkvim path/to/file.txt

# 打开多个文件
fkvim file1.txt file2.txt
```

#### 基本模式

- **普通模式 (Normal)**: 默认模式，用于导航和执行命令
- **插入模式 (Insert)**: 用于输入文本
- **可视模式 (Visual)**: 用于选择文本
- **命令模式 (Command)**: 用于执行命令行命令

#### 常用命令

| 命令 | 功能 |
|------|------|
| `:q` | 退出 |
| `:w` | 保存 |
| `:wq` 或 `:x` | 保存并退出 |
| `:e <文件>` | 编辑文件 |
| `:help` | 显示帮助 |//待补全
| `:split` 或 `:sp` | 水平分割窗口 |
| `:vsplit` 或 `:vs` | 垂直分割窗口 |
| `:tabnew` 或 `:tabe` | 新建标签页 |//待补全

## ⚙️ 配置

FKVim 使用 Lua 进行配置，配置文件位于：

- **Linux/macOS**: `~/.config/fkvim/config.lua`
- **Windows**: `%USERPROFILE%\.config\fkvim\config.lua`

### 示例配置

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
        { 'nvim-treesitter/nvim-treesitter', 
          config = function()
            require('nvim-treesitter.configs').setup {
                ensure_installed = { "rust", "lua", "vim" },
                highlight = { enable = true },
            }
          end 
        },
    },
}
```

## 🧩 插件系统

### 插件目录

插件可以放置在以下目录：

- **Linux/macOS**: `~/.local/share/fkvim/plugins/`
- **Windows**: `%USERPROFILE%\.local\share\fkvim\plugins\`

### 创建插件

FKVim 插件使用 Lua 编写。一个基本的插件结构如下：

```lua
-- myplugin.lua
local M = {}

function M.setup(opts)
    -- 插件初始化代码
    print("My plugin initialized with options: " .. vim.inspect(opts))
end

function M.my_command()
    -- 插件功能实现
    print("执行自定义命令")
end

return M
```

## 🔄 快捷键

### 导航

| 快捷键 | 功能 |
|--------|------|
| `h`, `j`, `k`, `l` | 左、下、上、右移动 |
| `w` | 向前跳转一个单词 |
| `b` | 向后跳转一个单词 |
| `gg` | 跳转到文件开头 |
| `G` | 跳转到文件末尾 |
| `0` | 跳转到行首 |
| `$` | 跳转到行尾 |

### 编辑

| 快捷键 | 功能 |
|--------|------|
| `i` | 进入插入模式 |
| `a` | 在光标后进入插入模式 |
| `o` | 在下方新行进入插入模式 |
| `O` | 在上方新行进入插入模式 |
| `x` | 删除字符 |
| `dd` | 删除行 |
| `yy` | 复制行 |
| `p` | 粘贴 |
| `u` | 撤销 |
| `Ctrl+r` | 重做 |

### 窗口管理
//待补全
| 快捷键 | 功能 |
|--------|------|
| `Ctrl+w` + `h/j/k/l` | 在窗口间移动 |
| `Ctrl+w` + `s` | 水平分割窗口 |
| `Ctrl+w` + `v` | 垂直分割窗口 |
| `Ctrl+w` + `c` | 关闭当前窗口 |
| `Ctrl+w` + `o` | 关闭其他窗口 |

### 终端集成
//待补全
| 命令 | 功能 |
|------|------|
| `:toggleterm` | 切换终端可见性 |
| `:focusterm` | 聚焦到终端 |
| `:exitterm` | 退出终端模式 |
| `:sendterm <命令>` | 向终端发送命令 |
| `:clearterm` | 清空终端 |
| `:restartterm` | 重启终端 |

## 🤝 贡献指南

欢迎贡献代码、报告问题或提出功能请求！

1. Fork 本仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建一个 Pull Request

## 📄 许可证

FKVim 基于 [MIT 许可证](LICENSE) 发布。

## 👥 致谢

FKVim 的开发受到了以下项目的启发：

- [Neovim](https://neovim.io/)
- [Helix Editor](https://helix-editor.com/)
- [Xi Editor](https://xi-editor.io/)

## 📞 联系方式

- **作者**: JoyinJoester
- **GitHub**: [JoyinJoester](https://github.com/JoyinJoester)
- **Email**: Joyin8888@foxmail.com

---

<p align="center">
  使用 ❤️ 和 Rust 构建
</p>
