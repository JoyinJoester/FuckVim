use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::config::Config;
use mlua::{Lua, Table, Value, Function};
use crate::error::{Result, FKVimError};
use std::sync::{Arc, Mutex};

/// Lua 环境，管理 Lua 状态和插件加载
pub struct LuaEnv {
    /// Lua 状态
    lua: Lua,
    
    /// 编辑器配置
    config: Config,
    
    /// 已注册的命令
    commands: HashMap<String, Arc<Mutex<Box<dyn Fn(Vec<String>) -> Result<()>>>>>,
    
    /// 已加载的模块
    loaded_modules: HashMap<String, bool>,
    
    /// 已加载的 Neovim 插件
    loaded_nvim_plugins: HashMap<String, PathBuf>,
}

impl LuaEnv {
    /// 创建新的 Lua 环境
    pub fn new(config: &Config) -> Result<Self> {
        let lua = Lua::new();
        let mut lua_env = Self {
            lua,
            config: config.clone(),
            commands: HashMap::new(),
            loaded_modules: HashMap::new(),
            loaded_nvim_plugins: HashMap::new(),
        };
        
        // 设置全局 API
        lua_env.setup_globals()?;
        
        // 确保vim表及其子表存在，即使不启用neovim兼容
        {
            let globals = lua_env.lua.globals();
            let vim_table = lua_env.lua.create_table()?;
            
            // 创建常用的vim子表
            let fs_table = lua_env.lua.create_table()?;
            vim_table.set("fs", fs_table)?;
            
            let opt_table = lua_env.lua.create_table()?;
            vim_table.set("opt", opt_table)?;
            
            let log_table = lua_env.lua.create_table()?;
            vim_table.set("log", log_table)?;
            
            // 创建keymap表
            let keymap_table = lua_env.lua.create_table()?;
            vim_table.set("keymap", keymap_table)?;
            
            // 创建其他常用表
            let api_table = lua_env.lua.create_table()?;
            vim_table.set("api", api_table)?;
            
            let fn_table = lua_env.lua.create_table()?;
            vim_table.set("fn", fn_table)?;
            
            let g_table = lua_env.lua.create_table()?;
            vim_table.set("g", g_table)?;
            
            let cmd_fn = lua_env.lua.create_function(|_, cmd: String| {
                println!("执行 Vim 命令: {}", cmd);
                Ok(())
            })?;
            vim_table.set("cmd", cmd_fn)?;
            
            globals.set("vim", vim_table)?;
        }
        
        // 加载预设模块
        lua_env.load_prelude()?;
        
        // 初始化 Neovim 兼容层
        if config.neovim_compat.enabled {
            lua_env.setup_neovim_compat()?;
        }
        
        Ok(lua_env)
    }
    
    /// 执行 Lua 代码
    pub fn execute(&self, code: &str) -> Result<()> {
        self.lua.load(code).exec().map_err(|e| e.into())
    }
    
    /// 执行存储的命令
    pub fn execute_command(&self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }
        
        let cmd_name = parts[0];
        if let Some(cmd) = self.commands.get(cmd_name) {
            let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
            let cmd = cmd.lock().unwrap();
            cmd(args)?;
            Ok(())
        } else {
            // 尝试从 Lua 执行
            let globals = self.lua.globals();
            if let Ok(vim) = globals.get::<_, Table>("vim") {
                if let Ok(cmd_fn) = vim.get::<_, Function>("cmd") {
                    match cmd_fn.call::<_, ()>(command) {
                        Ok(_) => Ok(()),
                        Err(e) => Err(FKVimError::LuaError(e)),
                    }
                } else {
                    Err(FKVimError::CommandError(format!("未知命令: {}", command)))
                }
            } else {
                Err(FKVimError::CommandError(format!("未知命令: {}", command)))
            }
        }
    }
    
    /// 加载 Lua 插件
    pub fn load_plugin(&mut self, plugin_path: &Path) -> Result<()> {
        let init_lua = plugin_path.join("init.lua");
        if !init_lua.exists() {
            return Err(FKVimError::PluginError(format!(
                "插件初始化文件不存在: {:?}", init_lua
            )));
        }
        
        // 读取插件内容
        let plugin_code = std::fs::read_to_string(&init_lua)
            .map_err(|e| FKVimError::PluginError(format!(
                "无法读取插件文件: {:?}, 错误: {}", init_lua, e
            )))?;
        
        // 执行插件代码
        self.lua.load(&plugin_code)
            .set_name(&format!("plugin:{}", plugin_path.display()))
            .exec()
            .map_err(|e| FKVimError::PluginError(format!(
                "加载插件失败: {:?}, 错误: {}", init_lua, e
            )))?;
        
        Ok(())
    }
    
    /// 设置全局 API
    fn setup_globals(&mut self) -> Result<()> {
        let globals = self.lua.globals();
        
        // 创建 fkvim 全局表
        let fkvim_table = self.lua.create_table()?;
        
        // 添加 API 函数
        let version_fn = self.lua.create_function(|_, ()| {
            Ok("FKVim 0.1.0")
        })?;
        fkvim_table.set("version", version_fn)?;
        
        // 创建命令表
        let commands_table = self.lua.create_table()?;
        self.lua.globals().set("__fkvim_commands", commands_table)?;
        
        // 注册命令 API
        let register_command_fn = self.lua.create_function(|lua, (name, callback): (String, Function)| {
            // 使用 Lua 环境存储命令
            if let Ok(global_commands) = lua.globals().get::<_, Table>("__fkvim_commands") {
                global_commands.set(name, callback)?;
            }
            
            Ok(())
        })?;
        fkvim_table.set("register_command", register_command_fn)?;
        
        // 设置缓冲区 API
        let buffer_table = self.lua.create_table()?;
        // TODO: 实现缓冲区 API 函数
        fkvim_table.set("buffer", buffer_table)?;
        
        // 设置窗口 API
        let window_table = self.lua.create_table()?;
        // TODO: 实现窗口 API 函数
        fkvim_table.set("window", window_table)?;
        
        // 设置全局表
        globals.set("fkvim", fkvim_table)?;
        
        Ok(())
    }
    
    /// 加载预设模块
    fn load_prelude(&mut self) -> Result<()> {
        // 注意：标准库应该在 Lua 实例创建时加载，而不是通过 load 方法
        
        // 加载预设的实用函数
        let prelude = r#"
        -- FKVim 预载模块
        
        -- 实用函数
        function vim.split(inputstr, sep)
            if sep == nil then
                sep = "%s"
            end
            local t = {}
            for str in string.gmatch(inputstr, "([^"..sep.."]+)") do
                table.insert(t, str)
            end
            return t
        end
        
        -- 路径处理
        function vim.fs.normalize(path)
            -- 简单的路径归一化
            return path:gsub("\\", "/"):gsub("/+", "/")
        end
        
        -- 日志功能
        vim.log.levels = {
            TRACE = 0,
            DEBUG = 1,
            INFO = 2,
            WARN = 3,
            ERROR = 4,
        }
        
        function vim.log.info(msg)
            print("[INFO] " .. msg)
        end
        
        function vim.log.error(msg)
            print("[ERROR] " .. msg)
        end
        "#;
        
        self.lua.load(prelude).exec()?;
        
        Ok(())
    }
    
    /// 设置 Neovim 兼容层
    fn setup_neovim_compat(&mut self) -> Result<()> {
        let globals = self.lua.globals();
        
        // 获取已存在的 vim 表
        let vim_table = globals.get::<_, Table>("vim").map_err(|e| {
            FKVimError::LuaError(e)
        })?;
        
        // 创建 vim.api 表
        let api_table = self.lua.create_table()?;
        
        // 添加 Neovim API 函数
        
        // nvim_get_current_buf
        let get_current_buf = self.lua.create_function(|_, ()| {
            // 实际实现中，这会返回当前缓冲区的ID
            Ok(1)
        })?;
        api_table.set("nvim_get_current_buf", get_current_buf)?;
        
        // nvim_buf_get_lines
        let buf_get_lines = self.lua.create_function(|lua, (_buf_id, _start, _end_, _strict): (i64, i64, i64, bool)| {
            // 实际实现中，这会返回指定缓冲区的行内容
            let lines = lua.create_sequence_from(vec!["line 1", "line 2", "line 3"])?;
            Ok(lines)
        })?;
        api_table.set("nvim_buf_get_lines", buf_get_lines)?;
        
        // nvim_buf_set_lines
        let buf_set_lines = self.lua.create_function(|_, (_buf_id, _start, _end_, _strict, _lines): (i64, i64, i64, bool, Vec<String>)| {
            // 实际实现中，这会设置指定缓冲区的行内容
            Ok(())
        })?;
        api_table.set("nvim_buf_set_lines", buf_set_lines)?;
        
        // 增加更多 API 函数以支持 Neovim 插件
        
        // nvim_create_buf - 创建新缓冲区
        let create_buf = self.lua.create_function(|_, (_listed, _scratch): (bool, bool)| {
            // 返回新创建的缓冲区 ID
            Ok(2)
        })?;
        api_table.set("nvim_create_buf", create_buf)?;
        
        // nvim_buf_set_option - 设置缓冲区选项
        let buf_set_option = self.lua.create_function(|_, (buf_id, name, value): (i64, String, Value)| {
            // 设置缓冲区选项
            println!("设置缓冲区 {} 选项 {} 为 {:?}", buf_id, name, value);
            Ok(())
        })?;
        api_table.set("nvim_buf_set_option", buf_set_option)?;
        
        // nvim_get_current_win - 获取当前窗口
        let get_current_win = self.lua.create_function(|_, ()| {
            // 返回当前窗口 ID
            Ok(1)
        })?;
        api_table.set("nvim_get_current_win", get_current_win)?;
        
        // nvim_open_win - 打开浮动窗口
        let open_win = self.lua.create_function(|_lua, (_buf_id, _enter, _config): (i64, bool, Table)| {
            // 创建浮动窗口，返回窗口 ID
            Ok(2)
        })?;
        api_table.set("nvim_open_win", open_win)?;
        
        // nvim_win_set_option - 设置窗口选项
        let win_set_option = self.lua.create_function(|_, (win_id, name, value): (i64, String, Value)| {
            // 设置窗口选项
            println!("设置窗口 {} 选项 {} 为 {:?}", win_id, name, value);
            Ok(())
        })?;
        api_table.set("nvim_win_set_option", win_set_option)?;
        
        // nvim_command - 执行 ex 命令
        let command = self.lua.create_function(|_, cmd: String| {
            // 执行 ex 命令
            println!("执行命令: {}", cmd);
            Ok(())
        })?;
        api_table.set("nvim_command", command)?;
        
        // nvim_set_keymap - 设置按键映射
        let set_keymap = self.lua.create_function(|_, (mode, lhs, rhs, _opts): (String, String, String, Table)| {
            // 设置按键映射
            println!("设置按键映射: {} 模式 {} -> {}", mode, lhs, rhs);
            Ok(())
        })?;
        api_table.set("nvim_set_keymap", set_keymap)?;
        
        // nvim_create_autocmd - 创建自动命令
        let create_autocmd = self.lua.create_function(|_lua, (events, _opts): (Value, Table)| {
            // 创建自动命令
            let events_str = match events {
                Value::String(s) => s.to_str().unwrap_or("").to_string(),
                Value::Table(_t) => "多个事件".to_string(),
                _ => "未知事件".to_string(),
            };
            println!("创建自动命令: {} 事件", events_str);
            Ok(1) // 返回自动命令 ID
        })?;
        api_table.set("nvim_create_autocmd", create_autocmd)?;
        
        // 设置 vim.api 表
        vim_table.set("api", api_table)?;
        
        // 添加 vim.cmd 函数
        let cmd_fn = self.lua.create_function(|_, cmd: String| {
            // 实际实现中，这会执行 Vim 命令
            println!("执行 Vim 命令: {}", cmd);
            Ok(())
        })?;
        vim_table.set("cmd", cmd_fn)?;
        
        // vim.fn 表用于调用 Vim 函数
        let fn_table = self.lua.create_table()?;
        // 设置常用的 Vim 函数
        vim_table.set("fn", fn_table)?;
        
        Ok(())
    }

    /// 设置 Neovim 风格的 require 函数
    pub fn setup_neovim_require(&mut self) -> Result<()> {
        let globals = self.get_globals()?;
        
        // 创建线程安全的已加载模块列表
        let loaded_modules = Arc::new(Mutex::new(HashMap::<String, bool>::new()));
        let loaded_modules_clone = loaded_modules.clone();
        
        let require_fn = self.lua.create_function(move |lua_ctx, module_name: String| {
            // 检查模块是否已加载
            let mut modules_map = loaded_modules_clone.lock().unwrap();
            
            if modules_map.contains_key(&module_name) {
                // 已加载，返回空表作为简化实现
                return Ok(lua_ctx.create_table()?);
            }
            
            // 尝试从各个标准路径加载
            let paths = vec![
                format!("lua/{}.lua", module_name.replace(".", "/")),
                format!("lua/{}/init.lua", module_name.replace(".", "/")),
            ];
            
            for path in paths {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // 尝试加载模块
                    if let Ok(chunk) = lua_ctx.load(&content).set_name(&path).into_function() {
                        if let Ok(value) = chunk.call::<_, mlua::Value>(()) {
                            modules_map.insert(module_name.clone(), true);
                            match value {
                                mlua::Value::Table(table) => return Ok(table),
                                _ => return Ok(lua_ctx.create_table()?)
                            }
                        }
                    }
                    
                    return Err(mlua::Error::RuntimeError(
                        format!("加载模块 '{}' 失败", module_name)
                    ));
                }
            }
            
            // 如果所有尝试都失败，返回错误
            Err(mlua::Error::RuntimeError(format!("Module '{}' not found", module_name)))
        })?;
        
        // 设置全局的 require 函数
        globals.set("require", require_fn)?;
        
        // 显式释放对全局表的引用，避免借用冲突
        drop(globals);
        
        Ok(())
    }

    /// 获取 Lua 全局变量
    pub fn get_globals(&self) -> Result<Table> {
        Ok(self.lua.globals())
    }

    /// 设置配置选项
    pub fn set_config(&mut self, option: &str, value: &str) -> Result<()> {
        // 更新内部配置
        match option {
            "theme" => self.config.theme = value.to_string(),
            "tab_width" => {
                if let Ok(width) = value.parse::<usize>() {
                    self.config.tab_width = width;
                }
            },
            "use_spaces" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.use_spaces = val;
                }
            },
            "show_line_numbers" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.show_line_numbers = val;
                }
            },
            "syntax_highlight" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.syntax_highlight = val;
                }
            },
            "auto_indent" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.auto_indent = val;
                }
            },
            "auto_save" => {
                if let Ok(seconds) = value.parse::<u64>() {
                    self.config.auto_save = seconds;
                }
            },
            "neovim_compat.enabled" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.neovim_compat.enabled = val;
                }
            },
            "neovim_compat.load_runtime" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.neovim_compat.load_runtime = val;
                }
            },
            "neovim_compat.support_vimscript" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.neovim_compat.support_vimscript = val;
                }
            },
            "neovim_compat.auto_install_dependencies" => {
                if let Ok(val) = value.parse::<bool>() {
                    self.config.neovim_compat.auto_install_dependencies = val;
                }
            },
            _ => return Err(FKVimError::ConfigError(format!("未知配置选项: {}", option))),
        };
        
        // 在Lua环境中更新对应的配置
        let globals = self.lua.globals();
        let option_parts: Vec<&str> = option.split('.').collect();
        
        if option_parts.len() == 1 {
            // 顶级选项
            if let Ok(vim_table) = globals.get::<_, Table>("vim") {
                if let Ok(opt_table) = vim_table.get::<_, Table>("opt") {
                    let _ = match option {
                        "tab_width" | "auto_save" => {
                            if let Ok(val) = value.parse::<i64>() {
                                opt_table.set(option, val)
                            } else {
                                Ok(())
                            }
                        },
                        "use_spaces" | "show_line_numbers" | "syntax_highlight" | "auto_indent" => {
                            if let Ok(val) = value.parse::<bool>() {
                                opt_table.set(option, val)
                            } else {
                                Ok(())
                            }
                        },
                        _ => opt_table.set(option, value),
                    };
                }
            }
        } else if option_parts.len() == 2 {
            // 嵌套选项
            if let Ok(vim_table) = globals.get::<_, Table>("vim") {
                if let Ok(opt_table) = vim_table.get::<_, Table>("opt") {
                    if let Ok(parent_table) = opt_table.get::<_, Table>(option_parts[0]) {
                        let _ = match option_parts[1] {
                            "enabled" | "load_runtime" | "support_vimscript" | "auto_install_dependencies" => {
                                if let Ok(val) = value.parse::<bool>() {
                                    parent_table.set(option_parts[1], val)
                                } else {
                                    Ok(())
                                }
                            },
                            _ => parent_table.set(option_parts[1], value),
                        };
                    }
                }
            }
        }
        
        Ok(())
    }
}