use std::path::{Path, PathBuf};
use std::fs;
use crate::error::{Result};
use crate::config::Config;
use crate::plugin::lua::LuaEnv;

/// Neovim 兼容性管理器
pub struct NeovimCompat {
    /// 编辑器配置
    config: Config,
    
    /// Neovim 运行时路径
    runtime_path: Vec<PathBuf>,
    
    /// 是否已加载运行时
    runtime_loaded: bool,
}

impl NeovimCompat {
    /// 创建 Neovim 兼容性管理器
    pub fn new(config: Config) -> Self {
        let mut runtime_path = Vec::new();
        
        // 如果配置了 Neovim 插件目录，添加相关路径
        if config.neovim_compat.enabled {
            if let Some(ref nvim_plugin_dir) = config.neovim_compat.plugin_dir {
                // 添加标准 Neovim 运行时路径
                runtime_path.push(nvim_plugin_dir.clone());
                
                // Neovim 的标准插件路径
                runtime_path.push(nvim_plugin_dir.join("pack"));
                
                // 如果存在 after 目录，也添加
                let after_dir = nvim_plugin_dir.join("after");
                if after_dir.exists() {
                    runtime_path.push(after_dir);
                }
            }
        }
        
        Self {
            config,
            runtime_path,
            runtime_loaded: false,
        }
    }
    
    /// 初始化 Neovim 兼容性环境
    pub fn init(&mut self, lua_env: &mut LuaEnv) -> Result<()> {
        if !self.config.neovim_compat.enabled {
            return Ok(());
        }
        
        // 设置 runtimepath
        self.setup_runtime_path(lua_env)?;
        
        // 加载 Neovim 运行时文件
        if self.config.neovim_compat.load_runtime {
            self.load_runtime(lua_env)?;
        }
        
        Ok(())
    }
    
    /// 设置 Neovim 运行时路径
    fn setup_runtime_path(&self, lua_env: &mut LuaEnv) -> Result<()> {
        // 将运行时路径转换为 Lua 字符串
        let runtime_paths = self.runtime_path.iter()
            .filter_map(|p| p.to_str())
            .collect::<Vec<_>>()
            .join(",");
        
        // 设置 vim.opt.runtimepath
        let runtime_setup = format!(r#"
            vim.opt.runtimepath = "{}"
            vim.opt.packpath = "{}"
        "#, runtime_paths, runtime_paths);
        
        lua_env.execute(&runtime_setup)?;
        
        Ok(())
    }
    
    /// 加载 Neovim 运行时文件
    fn load_runtime(&mut self, lua_env: &mut LuaEnv) -> Result<()> {
        if self.runtime_loaded {
            return Ok(());
        }
        
        // 加载 Neovim 兼容性的 Lua 模块
        self.load_nvim_compat_modules(lua_env)?;
        
        // 标记为已加载
        self.runtime_loaded = true;
        
        Ok(())
    }
    
    /// 加载 Neovim 兼容性的 Lua 模块
    fn load_nvim_compat_modules(&self, lua_env: &mut LuaEnv) -> Result<()> {
        // 加载基本的 Neovim 兼容层
        let compat_lua = r#"
        -- FKVim Neovim 兼容层
        
        -- 设置全局变量
        vim.g = vim.g or {}
        vim.b = vim.b or {}
        vim.w = vim.w or {}
        vim.t = vim.t or {}
        vim.v = vim.v or {}
        vim.env = vim.env or {}
        
        -- 简单的命令定义函数
        function vim.create_user_command(name, fn, opts)
            -- 在 FKVim 中注册命令
            local cmd_name = name:gsub("^%l", string.upper)
            fkvim.register_command(cmd_name, function(args)
                fn(args)
            end)
        end
        
        -- 键盘映射函数
        function vim.keymap.set(mode, lhs, rhs, opts)
            opts = opts or {}
            local modes = type(mode) == "string" and {mode} or mode
            
            -- 在 FKVim 中注册键盘映射
            for _, m in ipairs(modes) do
                -- 这里会调用 FKVim 的按键映射系统
                print(string.format("映射按键: %s 模式 %s -> %s", m, lhs, 
                    type(rhs) == "string" and rhs or "函数"))
            end
        end
        
        -- 自动命令 API
        vim.api.nvim_create_autocmd = vim.api.nvim_create_autocmd or function(event, opts)
            -- 在 FKVim 中注册自动命令
            local event_name = type(event) == "string" and event or table.concat(event, ",")
            print(string.format("创建自动命令: %s", event_name))
            return 1 -- 命令 ID
        end
        
        -- 高亮组 API
        function vim.api.nvim_set_hl(ns_id, name, opts)
            -- 设置高亮组
            print(string.format("设置高亮组: %s", name))
        end
        
        -- 插件管理器 API 桥接
        vim.fn.has = vim.fn.has or function(feature)
            local supported = {
                "nvim", "lua", "timers", "terminal", "syntax"
            }
            
            for _, f in ipairs(supported) do
                if f == feature then
                    return 1
                end
            end
            
            return 0
        end
        
        -- 文件类型检测
        vim.filetype = vim.filetype or {}
        vim.filetype.add = function(option)
            -- 添加文件类型规则
            print("添加文件类型规则")
        end
        
        -- 确保常用的 Neovim 模块存在
        vim.diagnostic = vim.diagnostic or {}
        vim.highlight = vim.highlight or {}
        vim.loader = vim.loader or {}
        
        -- 补全框架 API 存根
        vim.lsp = vim.lsp or {}
        vim.lsp.handlers = vim.lsp.handlers or {}
        
        -- 告诉用户我们在兼容模式下运行
        print("FKVim 在 Neovim 兼容模式下运行")
        "#;
        
        lua_env.execute(compat_lua)?;
        
        Ok(())
    }
    
    /// 搜索 Neovim 插件目录
    pub fn find_neovim_plugins(&self) -> Vec<PathBuf> {
        let mut plugins = Vec::new();
        
        if !self.config.neovim_compat.enabled {
            return plugins;
        }
        
        // 如果配置了 Neovim 插件目录
        if let Some(ref nvim_plugin_dir) = self.config.neovim_compat.plugin_dir {
            // 查找 pack/*/start/* 目录下的插件
            let pack_dir = nvim_plugin_dir.join("pack");
            if pack_dir.exists() {
                if let Ok(entries) = fs::read_dir(&pack_dir) {
                    for entry in entries.filter_map(|res| res.ok()) {
                        let path = entry.path();
                        if path.is_dir() {
                            let start_dir = path.join("start");
                            if start_dir.exists() {
                                if let Ok(plugins_entries) = fs::read_dir(&start_dir) {
                                    for plugin_entry in plugins_entries.filter_map(|res| res.ok()) {
                                        let plugin_path = plugin_entry.path();
                                        if plugin_path.is_dir() {
                                            plugins.push(plugin_path);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // 查找独立插件目录
            let plugin_dir = nvim_plugin_dir.join("plugin");
            if plugin_dir.exists() {
                plugins.push(plugin_dir);
            }
        }
        
        plugins
    }
    
    /// 处理 VimScript 命令
    pub fn handle_vim_command(&self, cmd: &str, lua_env: &mut LuaEnv) -> Result<()> {
        // 将 VimScript 命令转换为 Lua 等效命令
        let lua_cmd = format!("vim.cmd([[{}]])", cmd);
        lua_env.execute(&lua_cmd)?;
        
        Ok(())
    }
    
    /// 运行 Neovim 插件初始化脚本
    pub fn run_plugin_init_script(&self, plugin_path: &Path, lua_env: &mut LuaEnv) -> Result<()> {
        // 检查插件目录中是否有 plugin/*.vim 或 plugin/*.lua 脚本
        let plugin_vim_dir = plugin_path.join("plugin");
        if plugin_vim_dir.exists() {
            // 处理 *.lua 脚本
            if let Ok(entries) = fs::read_dir(&plugin_vim_dir) {
                for entry in entries.filter_map(|res| res.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "lua" {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    // 执行 Lua 脚本
                                    lua_env.execute(&content)?;
                                }
                            } else if ext == "vim" {
                                // TODO: 实现 VimScript 解析器或调用外部的 Vim/Neovim
                                println!("发现 VimScript 文件: {:?}，但当前不支持直接执行", path);
                            }
                        }
                    }
                }
            }
        }
        
        // 检查并加载 ftdetect/*.vim 和 ftdetect/*.lua 文件
        let ftdetect_dir = plugin_path.join("ftdetect");
        if ftdetect_dir.exists() {
            if let Ok(entries) = fs::read_dir(&ftdetect_dir) {
                for entry in entries.filter_map(|res| res.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "lua" {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    // 执行 Lua 脚本
                                    lua_env.execute(&content)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 加载插件的 after/ 目录
        let after_dir = plugin_path.join("after");
        if after_dir.exists() {
            let after_plugin_dir = after_dir.join("plugin");
            if after_plugin_dir.exists() {
                if let Ok(entries) = fs::read_dir(&after_plugin_dir) {
                    for entry in entries.filter_map(|res| res.ok()) {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(ext) = path.extension() {
                                if ext == "lua" {
                                    if let Ok(content) = fs::read_to_string(&path) {
                                        // 执行 Lua 脚本
                                        lua_env.execute(&content)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}