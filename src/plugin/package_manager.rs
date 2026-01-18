use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::collections::{HashMap, HashSet};
use crate::error::{Result, FKVimError};
use crate::config::{Config, LuaConfig};
use crate::config::lua_config::PluginConfig;
use crate::plugin::{PluginManager};
use crate::plugin::lua::LuaEnv;

/// 插件包管理器
pub struct PackageManager {
    /// 编辑器配置
    config: Config,
    
    /// 插件配置
    plugin_configs: Vec<PluginConfig>,
    
    /// 插件目录
    plugin_dir: PathBuf,
    
    /// 临时目录
    temp_dir: PathBuf,
    
    /// 已安装的插件
    installed_plugins: HashMap<String, PathBuf>,
    
    /// 已处理的依赖项（防止循环依赖）
    processed_deps: HashSet<String>,
}

impl PackageManager {
    /// 创建新的插件包管理器
    pub fn new(config: Config, plugin_configs: Vec<PluginConfig>) -> Self {
        let plugin_dir = config.plugin_dir.clone();
        let temp_dir = plugin_dir.join("_temp");
        
        // 确保目录存在
        let _ = fs::create_dir_all(&plugin_dir);
        let _ = fs::create_dir_all(&temp_dir);
        
        Self {
            config,
            plugin_configs,
            plugin_dir,
            temp_dir,
            installed_plugins: HashMap::new(),
            processed_deps: HashSet::new(),
        }
    }
    
    /// 从 Lua 配置中加载插件
    pub fn load_from_lua_config(config: Config, lua_config: &LuaConfig) -> Self {
        Self::new(config, lua_config.plugins.clone())
    }
    
    /// 初始化包管理器
    pub fn init(&mut self) -> Result<()> {
        // 扫描已安装的插件
        self.scan_installed_plugins()?;
        
        // 检查是否需要安装插件
        if self.has_missing_plugins() {
            println!("发现未安装的插件，开始安装...");
            self.install_plugins()?;
        }
        
        Ok(())
    }
    
    /// 扫描已安装的插件
    fn scan_installed_plugins(&mut self) -> Result<()> {
        self.installed_plugins.clear();
        
        // 扫描主插件目录
        if self.plugin_dir.exists() {
            for entry in fs::read_dir(&self.plugin_dir)
                .map_err(|e| FKVimError::PluginError(format!("无法读取插件目录: {}", e)))? {
                    
                let entry = entry.map_err(|e| {
                    FKVimError::PluginError(format!("无法读取插件目录条目: {}", e))
                })?;
                
                let path = entry.path();
                if path.is_dir() && !path.file_name().unwrap_or_default().to_string_lossy().starts_with("_") {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    self.installed_plugins.insert(name, path);
                }
            }
        }
        
        // 如果启用了 Neovim 兼容，也扫描 Neovim 插件目录
        if self.config.neovim_compat.enabled {
            if let Some(nvim_dir) = &self.config.neovim_compat.plugin_dir {
                // 扫描 pack/*/start/* 目录
                let pack_dir = nvim_dir.join("pack");
                if pack_dir.exists() {
                    // 修复: 使用 match 处理 Result 而不是 unwrap_or_else
                    if let Ok(entries) = fs::read_dir(&pack_dir) {
                        for entry in entries {
                            if let Ok(entry) = entry {
                                let path = entry.path();
                                if path.is_dir() {
                                    let start_dir = path.join("start");
                                    if start_dir.exists() {
                                        // 修复: 同样使用 match 处理 Result
                                        if let Ok(plugin_entries) = fs::read_dir(&start_dir) {
                                            for plugin_entry in plugin_entries {
                                                if let Ok(plugin_entry) = plugin_entry {
                                                    let plugin_path = plugin_entry.path();
                                                    if plugin_path.is_dir() {
                                                        let name = plugin_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                                        self.installed_plugins.insert(name, plugin_path);
                                                    }
                                                }
                                            }
                                        }
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
    
    /// 检查是否有未安装的插件
    fn has_missing_plugins(&self) -> bool {
        for plugin in &self.plugin_configs {
            let plugin_name = self.extract_plugin_name(&plugin.name);
            if !self.installed_plugins.contains_key(&plugin_name) {
                return true;
            }
        }
        false
    }
    
    /// 从插件规范中提取插件名称（例如从 GitHub 仓库 URL）
    fn extract_plugin_name(&self, spec: &str) -> String {
        // 处理 GitHub 格式: user/repo
        if spec.contains('/') {
            let parts: Vec<&str> = spec.split('/').collect();
            if parts.len() >= 2 {
                return parts[1].to_string();
            }
        }
        
        // 否则直接使用完整名称
        spec.to_string()
    }
    
    /// 安装插件
    fn install_plugins(&mut self) -> Result<()> {
        // 创建一个插件配置的副本，避免循环引用和所有权问题
        let plugin_configs = self.plugin_configs.clone();
        
        for plugin in &plugin_configs {
            self.processed_deps.clear();
            self.install_plugin(plugin, false)?;
        }
        
        Ok(())
    }
    
    /// 安装单个插件
    fn install_plugin(&mut self, plugin: &PluginConfig, is_dependency: bool) -> Result<()> {
        let plugin_name = self.extract_plugin_name(&plugin.name);
        
        // 如果已经安装，跳过
        if self.installed_plugins.contains_key(&plugin_name) {
            return Ok(());
        }
        
        // 如果是依赖项且已经处理过，避免循环依赖
        if is_dependency && self.processed_deps.contains(&plugin_name) {
            return Ok(());
        }
        
        if is_dependency {
            self.processed_deps.insert(plugin_name.clone());
        }
        
        println!("安装插件: {}", plugin_name);
        
        // 确定目标目录
        let target_dir = if self.config.neovim_compat.enabled && self.config.neovim_compat.plugin_dir.is_some() {
            let nvim_dir = self.config.neovim_compat.plugin_dir.as_ref().unwrap();
            let pack_dir = nvim_dir.join("pack").join("fkvim");
            if !plugin.enabled {  // 使用 enabled 替代 lazy
                pack_dir.join("opt").join(&plugin_name)
            } else {
                pack_dir.join("start").join(&plugin_name)
            }
        } else {
            self.plugin_dir.join(&plugin_name)
        };
        
        // 创建目标目录
        fs::create_dir_all(&target_dir).map_err(|e| {
            FKVimError::PluginError(format!("无法创建插件目录 {}: {}", target_dir.display(), e))
        })?;
        
        // 安装插件
        if let Some(local_path) = &plugin.path {  // 使用 path 替代 local
            // 本地插件：创建符号链接或复制
            let local_path = PathBuf::from(local_path);
            if local_path.exists() {
                // 简单复制内容
                copy_dir_contents(&local_path, &target_dir)?;
            } else {
                return Err(FKVimError::PluginError(format!(
                    "本地插件路径不存在: {}", local_path.display()
                )));
            }
        } else {
            // 远程插件：从 GitHub 克隆
            let url = format!("https://github.com/{}.git", plugin.name);
            
            // 构建 git 命令
            let mut git_cmd = Command::new("git");
            git_cmd.arg("clone");
            git_cmd.arg("--depth=1"); // 浅克隆以加快速度
            
            // 添加URL和目标目录
            git_cmd.arg(&url).arg(&target_dir);
            
            // 执行 git 克隆
            let output = git_cmd.output().map_err(|e| {
                FKVimError::PluginError(format!("执行 git clone 失败: {}", e))
            })?;
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                return Err(FKVimError::PluginError(format!(
                    "克隆插件 {} 失败: {}", plugin.name, error
                )));
            }
        }
        
        // 记录已安装的插件
        self.installed_plugins.insert(plugin_name.clone(), target_dir);
        
        // 在实际项目中，这里需要处理依赖，但为简化当前实现，先不添加
        
        Ok(())
    }
    
    /// 查找插件配置
    fn find_plugin_config(&self, name: &str) -> Option<&PluginConfig> {
        // 首先尝试精确匹配
        for config in &self.plugin_configs {
            if config.name == name {
                return Some(config);
            }
        }
        
        // 然后尝试匹配提取的名称
        let extracted_name = self.extract_plugin_name(name);
        for config in &self.plugin_configs {
            let config_name = self.extract_plugin_name(&config.name);
            if config_name == extracted_name {
                return Some(config);
            }
        }
        
        None
    }
    
    /// 加载所有插件
    pub fn load_plugins(&self, _plugin_manager: &mut PluginManager, lua_env: &mut LuaEnv) -> Result<()> {
        // 首先加载非懒加载的插件
        for plugin in &self.plugin_configs {
            if plugin.enabled {  // 使用 enabled 替代 !lazy
                let plugin_name = self.extract_plugin_name(&plugin.name);
                if let Some(plugin_path) = self.installed_plugins.get(&plugin_name) {
                    // 确定插件类型
                    if plugin_path.join("lua").exists() || plugin_path.join("plugin").exists() {
                        // 这是一个 Neovim 插件
                        lua_env.load_plugin(plugin_path)?;
                    } else if plugin_path.join("init.lua").exists() {
                        // 这是一个 FKVim Lua 插件
                        lua_env.load_plugin(plugin_path)?;
                    }
                    
                    // 如果有配置函数，执行它
                    if let Some(_config_fn) = &plugin.config {
                        // 在实际实现中，我们会从存储的函数引用中找到配置函数并执行
                        // 这里简化，只输出信息
                        println!("执行插件 {} 的配置函数", plugin_name);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 加载懒加载插件
    pub fn load_lazy_plugin(&self, name: &str, plugin_manager: &mut PluginManager, lua_env: &mut LuaEnv) -> Result<bool> {
        let plugin_name = self.extract_plugin_name(name);
        
        // 查找插件配置
        if let Some(plugin) = self.find_plugin_config(&plugin_name) {
            if let Some(plugin_path) = self.installed_plugins.get(&plugin_name) {
                // 加载插件
                if plugin_path.join("lua").exists() || plugin_path.join("plugin").exists() {
                    // Neovim 插件
                    lua_env.load_plugin(plugin_path)?;
                } else if plugin_path.join("init.lua").exists() {
                    // FKVim Lua 插件
                    lua_env.load_plugin(plugin_path)?;
                }
                
                // 如果有配置函数，执行它
                if let Some(_config_fn) = &plugin.config {
                    println!("执行懒加载插件 {} 的配置函数", plugin_name);
                }
                
                return Ok(true);
            }
        }
        
        // 尝试使用插件管理器的通用方法加载
        plugin_manager.load_lazy_plugin(&plugin_name, lua_env)
    }
}

/// 复制目录内容
fn copy_dir_contents(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Err(FKVimError::PluginError(format!(
            "源目录不存在: {}", from.display()
        )));
    }
    
    if !to.exists() {
        fs::create_dir_all(to).map_err(|e| {
            FKVimError::PluginError(format!("无法创建目标目录: {}", e))
        })?;
    }
    
    for entry in fs::read_dir(from).map_err(|e| {
        FKVimError::PluginError(format!("无法读取目录: {}", e))
    })? {
        let entry = entry.map_err(|e| {
            FKVimError::PluginError(format!("无法读取目录条目: {}", e))
        })?;
        
        let path = entry.path();
        let target = to.join(path.file_name().unwrap());
        
        if path.is_dir() {
            copy_dir_contents(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| {
                FKVimError::PluginError(format!("无法复制文件: {}", e))
            })?;
        }
    }
    
    Ok(())
}