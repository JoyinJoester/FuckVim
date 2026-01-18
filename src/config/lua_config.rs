use std::path::Path;
use std::collections::HashMap;
use std::path::PathBuf;
use mlua::{Lua, Value, Table, Function};
use serde::{Deserialize, Serialize};
use crate::error::{Result, FKVimError};
use super::Config;

/// Lua 配置处理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuaConfig {
    pub theme: String,
    pub tab_width: usize,
    pub use_spaces: bool,
    pub show_line_numbers: bool,
    pub syntax_highlight: bool,
    pub auto_indent: bool,
    pub auto_save: u64,
    pub neovim_compat: NeovimCompatLuaConfig,
    pub mappings: HashMap<String, HashMap<String, String>>,
    pub commands: HashMap<String, String>, // 存储 Lua 函数引用的字符串表示
    pub plugins: Vec<PluginConfig>, // 插件配置列表
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeovimCompatLuaConfig {
    pub enabled: bool,
    pub plugin_dir: Option<String>,
    pub load_runtime: bool,
    pub support_vimscript: bool,
    pub package_manager: String,
    pub auto_install_dependencies: bool,
}

/// 插件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,         // 插件名称或 GitHub 仓库
    pub enabled: bool,        // 是否启用
    pub priority: Option<u32>, // 加载优先级
    pub path: Option<String>, // 插件路径
    pub config: Option<String>, // 配置函数引用
    pub opts: HashMap<String, String>, // 插件选项
}

impl LuaConfig {
    /// 将 Lua 配置转换为应用程序配置
    pub fn to_config(&self) -> Result<Config> {
        let config_dir = super::get_default_config_dir();
        let plugin_dir = config_dir.join("plugins");
        
        let neovim_plugin_dir = self.neovim_compat.plugin_dir.as_ref().map(|p| {
            let path = if p.starts_with("~/") {
                if let Some(home_dir) = dirs::home_dir() {
                    home_dir.join(p.trim_start_matches("~/"))
                } else {
                    PathBuf::from(p)
                }
            } else {
                PathBuf::from(p)
            };
            path
        });
        
        // 转换按键映射
        let mut keymaps = HashMap::new();
        for (mode, maps) in &self.mappings {
            let mut mode_maps = HashMap::new();
            for (key, cmd) in maps {
                mode_maps.insert(key.clone(), cmd.clone());
            }
            keymaps.insert(mode.clone(), mode_maps);
        }
        
        // 解析包管理器类型
        let package_manager = match self.neovim_compat.package_manager.as_str() {
            "packer" => super::NeovimPackageManagerType::Packer,
            "lazy" => super::NeovimPackageManagerType::Lazy,
            "vim-plug" => super::NeovimPackageManagerType::VimPlug,
            _ => super::NeovimPackageManagerType::None,
        };
        
        Ok(Config {
            config_dir,
            plugin_dir,
            theme: self.theme.clone(),
            tab_width: self.tab_width,
            use_spaces: self.use_spaces,
            show_line_numbers: self.show_line_numbers,
            syntax_highlight: self.syntax_highlight,
            auto_indent: self.auto_indent,
            auto_save: self.auto_save,
            neovim_compat: super::NeovimCompatConfig {
                enabled: self.neovim_compat.enabled,
                plugin_dir: neovim_plugin_dir,
                load_runtime: self.neovim_compat.load_runtime,
                support_vimscript: self.neovim_compat.support_vimscript,
                package_manager,
                auto_install_dependencies: self.neovim_compat.auto_install_dependencies,
            },
            keymaps,
        })
    }
}

/// 从 Lua 配置文件加载配置
pub fn load_lua_config(config_file: &Path) -> Result<LuaConfig> {
    let lua = Lua::new();
    
    // 添加模拟的 vim 全局对象以支持 Neovim 风格的配置
    setup_vim_compat(&lua)?;
    
    // 加载配置文件
    let config_content = std::fs::read_to_string(config_file)
        .map_err(|e| FKVimError::ConfigError(format!("无法读取配置文件: {}", e)))?;
    
    // 执行配置脚本并获取返回值
    let config_table: Table = lua.load(&config_content)
        .set_name("config")
        .eval()
        .map_err(|e| FKVimError::ConfigError(format!("Lua 配置错误: {}", e)))?;
    
    // 提取配置选项
    let theme = get_string_or(&config_table, "theme", "default")?;
    let tab_width = get_int_or(&config_table, "tab_width", 4)? as usize;
    let use_spaces = get_bool_or(&config_table, "use_spaces", true)?;
    let show_line_numbers = get_bool_or(&config_table, "show_line_numbers", true)?;
    let syntax_highlight = get_bool_or(&config_table, "syntax_highlight", true)?;
    let auto_indent = get_bool_or(&config_table, "auto_indent", true)?;
    let auto_save = get_int_or(&config_table, "auto_save", 0)? as u64;
    
    // 提取 Neovim 兼容性配置
    let neovim_compat = extract_neovim_compat(&config_table)?;
    
    // 提取按键映射
    let mappings = extract_mappings(&config_table)?;
    
    // 提取命令
    let commands = extract_commands(&lua, &config_table)?;
    
    // 提取插件配置
    let plugins = extract_plugins(&lua, &config_table)?;
    
    Ok(LuaConfig {
        theme,
        tab_width,
        use_spaces,
        show_line_numbers,
        syntax_highlight,
        auto_indent,
        auto_save,
        neovim_compat,
        mappings,
        commands,
        plugins,
    })
}

/// 设置 vim 兼容全局对象
fn setup_vim_compat(lua: &Lua) -> Result<()> {
    let globals = lua.globals();
    
    // 创建 vim 表
    let vim_table = lua.create_table()?;
    
    // 添加常用的 vim 函数
    let command_fn = lua.create_function(|_, cmd: String| {
        // 在实际实现中，这里会执行命令
        println!("执行命令: {}", cmd);
        Ok(())
    })?;
    vim_table.set("command", command_fn)?;
    
    // 设置到全局
    globals.set("vim", vim_table)?;
    
    Ok(())
}

/// 提取 Neovim 兼容性配置
fn extract_neovim_compat(config_table: &Table) -> Result<NeovimCompatLuaConfig> {
    match config_table.get::<_, Value>("neovim_compat")? {
        Value::Table(compat_table) => {
            let enabled = get_bool_or(&compat_table, "enabled", true)?;
            let plugin_dir = match compat_table.get::<_, Value>("plugin_dir")? {
                Value::String(s) => Some(s.to_str()?.to_string()),
                _ => None,
            };
            let load_runtime = get_bool_or(&compat_table, "load_runtime", true)?;
            let support_vimscript = get_bool_or(&compat_table, "support_vimscript", false)?;
            let package_manager = get_string_or(&compat_table, "package_manager", "default")?;
            let auto_install_dependencies = get_bool_or(&compat_table, "auto_install_dependencies", false)?;
            
            Ok(NeovimCompatLuaConfig {
                enabled,
                plugin_dir,
                load_runtime,
                support_vimscript,
                package_manager,
                auto_install_dependencies,
            })
        },
        _ => Ok(NeovimCompatLuaConfig {
            enabled: true,
            plugin_dir: None,
            load_runtime: true,
            support_vimscript: false,
            package_manager: "default".to_string(),
            auto_install_dependencies: false,
        }),
    }
}

/// 提取按键映射
fn extract_mappings(config_table: &Table) -> Result<HashMap<String, HashMap<String, String>>> {
    let mut result = HashMap::new();
    
    match config_table.get::<_, Value>("mappings")? {
        Value::Table(mappings_table) => {
            // 遍历模式（normal, insert 等）
            for pair in mappings_table.pairs::<String, Table>() {
                let (mode, mode_maps) = pair?;
                let mut mode_map = HashMap::new();
                
                // 遍历该模式的所有映射
                for map_pair in mode_maps.pairs::<String, String>() {
                    let (key, action) = map_pair?;
                    mode_map.insert(key, action);
                }
                
                result.insert(mode, mode_map);
            }
        },
        _ => {}
    }
    
    Ok(result)
}

/// 提取命令
fn extract_commands(_lua: &Lua, config_table: &Table) -> Result<HashMap<String, String>> {
    let mut commands = HashMap::new();
    
    match config_table.get::<_, Value>("commands")? {
        Value::Table(commands_table) => {
            // 遍历所有命令
            for pair in commands_table.pairs::<String, Function>() {
                let (name, _) = pair?;
                // 将函数转为字符串引用
                commands.insert(name, format!("command_{}", commands.len()));
            }
        },
        _ => {}
    }
    
    Ok(commands)
}

/// 提取插件配置
fn extract_plugins(_lua: &Lua, config_table: &Table) -> Result<Vec<PluginConfig>> {
    let mut plugins = Vec::new();
    
    // 检查是否有 plugins 配置
    match config_table.get::<_, Value>("plugins")? {
        Value::Table(plugins_table) => {
            // 遍历插件配置表
            for i in 1..=plugins_table.len()? {
                if let Ok(plugin_value) = plugins_table.get::<_, Value>(i) {
                    if let Value::Table(plugin_table) = plugin_value {
                        // 获取插件名称（必需）
                        let name = match plugin_table.get::<_, Value>(1)? {
                            Value::String(s) => s.to_str()?.to_string(),
                            _ => {
                                if let Ok(name_str) = get_string_or(&plugin_table, "name", "") {
                                    if !name_str.is_empty() {
                                        name_str
                                    } else {
                                        continue; // 跳过没有名称的插件
                                    }
                                } else {
                                    continue; // 跳过没有名称的插件
                                }
                            }
                        };
                        
                        // 获取启用状态
                        let enabled = get_bool_or(&plugin_table, "enabled", true)?;
                        
                        // 获取优先级
                        let priority = match plugin_table.get::<_, Value>("priority")? {
                            Value::Integer(i) => Some(i as u32),
                            Value::Number(n) => Some(n as u32),
                            _ => None,
                        };
                        
                        // 获取路径
                        let path = match plugin_table.get::<_, Value>("path")? {
                            Value::String(s) => Some(s.to_str()?.to_string()),
                            _ => None,
                        };
                        
                        // 获取配置函数
                        let config = match plugin_table.get::<_, Value>("config")? {
                            Value::Function(_) => Some(format!("plugin_config_{}", plugins.len())),
                            _ => None,
                        };
                        
                        // 获取选项
                        let mut opts = HashMap::new();
                        match plugin_table.get::<_, Value>("opts")? {
                            Value::Table(opts_table) => {
                                for pair in opts_table.pairs::<String, Value>() {
                                    let (key, value) = pair?;
                                    // 将值转为字符串
                                    match value {
                                        Value::String(s) => opts.insert(key, s.to_str()?.to_string()),
                                        Value::Integer(i) => opts.insert(key, i.to_string()),
                                        Value::Number(n) => opts.insert(key, n.to_string()),
                                        Value::Boolean(b) => opts.insert(key, b.to_string()),
                                        _ => opts.insert(key, "null".to_string()),
                                    };
                                }
                            },
                            _ => {}
                        }
                        
                        // 创建插件配置
                        let plugin_config = PluginConfig {
                            name,
                            enabled,
                            priority,
                            path,
                            config,
                            opts,
                        };
                        
                        plugins.push(plugin_config);
                    }
                }
            }
        },
        _ => {}
    }
    
    Ok(plugins)
}

// 辅助函数：从表中获取字符串或默认值
fn get_string_or(table: &Table, key: &str, default: &str) -> Result<String> {
    match table.get::<_, Value>(key)? {
        Value::String(s) => Ok(s.to_str()?.to_string()),
        _ => Ok(default.to_string()),
    }
}

// 辅助函数：从表中获取整数或默认值
fn get_int_or(table: &Table, key: &str, default: i64) -> Result<i64> {
    match table.get::<_, Value>(key)? {
        Value::Integer(i) => Ok(i),
        Value::Number(n) => Ok(n as i64),
        _ => Ok(default),
    }
}

// 辅助函数：从表中获取布尔值或默认值
fn get_bool_or(table: &Table, key: &str, default: bool) -> Result<bool> {
    match table.get::<_, Value>(key)? {
        Value::Boolean(b) => Ok(b),
        _ => Ok(default),
    }
}