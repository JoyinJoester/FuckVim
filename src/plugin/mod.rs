pub mod lua;
pub mod nvim_compat;
pub mod package_manager;

use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use crate::error::{Result, FKVimError};
use crate::config::Config;

/// 插件类型
pub enum PluginType {
    /// Lua 插件
    Lua,
    /// Neovim 兼容插件
    Neovim,
    /// Rust 动态库插件
    RustDynlib,
}

/// 插件源类型
#[derive(Clone)]
pub enum PluginSource {
    /// 本地路径
    Local(PathBuf),
    /// Git 仓库
    Git {
        /// 仓库 URL
        url: String,
        /// 分支，标签或提交哈希
        version: Option<String>,
    },
}

/// 插件元数据
pub struct PluginMetadata {
    /// 插件名称
    pub name: String,
    
    /// 插件版本
    pub version: String,
    
    /// 插件作者
    pub author: String,
    
    /// 插件描述
    pub description: String,
    
    /// 插件类型
    pub plugin_type: PluginType,
    
    /// 插件路径
    pub path: PathBuf,
    
    /// 插件源
    pub source: PluginSource,
    
    /// 插件是否懒加载
    pub lazy: bool,
    
    /// 插件依赖
    pub dependencies: Vec<String>,
}

/// 插件管理器
pub struct PluginManager {
    /// 加载的插件列表
    plugins: Vec<PluginMetadata>,
    
    /// 配置
    config: Config,
    
    /// 插件目录
    plugin_dir: PathBuf,
    
    /// Neovim 插件目录结构
    nvim_plugin_dirs: Option<NvimPluginDirs>,
    
    /// 已声明但未安装的插件
    pending_plugins: HashMap<String, PluginSource>,
}

/// Neovim 插件目录结构
pub struct NvimPluginDirs {
    /// 根目录
    pub root: PathBuf,
    
    /// start 目录 (自动加载的插件)
    pub start: PathBuf,
    
    /// opt 目录 (懒加载插件)
    pub opt: PathBuf,
}

impl PluginManager {
    /// 创建新的插件管理器
    pub fn new(config: Config) -> Self {
        let plugin_dir = config.plugin_dir.clone();
        
        // 如果启用了 Neovim 兼容性，设置 Neovim 插件目录
        let nvim_plugin_dirs = if config.neovim_compat.enabled {
            if let Some(nvim_dir) = &config.neovim_compat.plugin_dir {
                // Neovim 插件目录结构: <root>/pack/*/start/* 和 <root>/pack/*/opt/*
                // 创建目录结构
                let start_dir = nvim_dir.join("pack").join("fkvim").join("start");
                let opt_dir = nvim_dir.join("pack").join("fkvim").join("opt");
                
                // 创建 Neovim 插件目录结构（如果不存在）
                let _ = fs::create_dir_all(&start_dir);
                let _ = fs::create_dir_all(&opt_dir);
                
                Some(NvimPluginDirs {
                    root: nvim_dir.clone(),
                    start: start_dir,
                    opt: opt_dir,
                })
            } else {
                None
            }
        } else {
            None
        };
        
        Self {
            plugins: Vec::new(),
            config,
            plugin_dir,
            nvim_plugin_dirs,
            pending_plugins: HashMap::new(),
        }
    }
    
    /// 注册插件
    pub fn register_plugin(&mut self, name: &str, source: PluginSource, lazy: bool) -> Result<()> {
        // 将插件添加到待安装列表
        self.pending_plugins.insert(name.to_string(), source.clone());
        
        // 懒加载插件的处理逻辑
        if lazy {
            println!("注册懒加载插件: {}", name);
            
            // 如果是 Git 源且有 Neovim 插件目录结构，直接放入 opt 目录
            if let (PluginSource::Git { url, version: _ }, Some(dirs)) = (&source, &self.nvim_plugin_dirs) {
                let opt_dir = dirs.opt.join(name);
                
                // 检查目录是否已存在
                if !opt_dir.exists() {
                    println!("将在需要时安装懒加载插件 {} 从 {}", name, url);
                    // 在实际应用中，我们会在插件首次加载时处理安装
                }
            }
        }
        
        Ok(())
    }
    
    /// 安装所有注册的插件
    pub fn install_plugins(&mut self) -> Result<()> {
        for (name, source) in self.pending_plugins.clone() {
            self.install_plugin(&name, &source)?;
        }
        
        // 清理安装完成的插件
        self.pending_plugins.clear();
        
        Ok(())
    }
    
    /// 安装单个插件
    fn install_plugin(&mut self, name: &str, source: &PluginSource) -> Result<()> {
        match source {
            PluginSource::Local(path) => {
                // 本地插件，检查路径是否存在
                if !path.exists() {
                    return Err(FKVimError::PluginError(format!(
                        "本地插件路径不存在: {:?}", path
                    )));
                }
                
                // 获取插件元数据并添加到已安装列表
                let metadata = self.create_plugin_metadata(name, path, source.clone(), false)?;
                self.plugins.push(metadata);
            }
            PluginSource::Git { url, version: _version } => {
                // 确定安装目录
                let install_dir = if let Some(dirs) = &self.nvim_plugin_dirs {
                    dirs.start.join(name)
                } else {
                    self.plugin_dir.join(name)
                };
                
                // 检查目录是否已存在
                if install_dir.exists() {
                    // 如果已存在，可以考虑更新
                    println!("插件 {} 已安装，跳过...", name);
                } else {
                    // 克隆 Git 仓库
                    println!("安装插件 {} 从 {}", name, url);
                    
                    // 在实际应用中使用 git2 或运行 git 命令克隆仓库
                    // 这里简化为创建目录
                    fs::create_dir_all(&install_dir).map_err(|e| {
                        FKVimError::PluginError(format!("创建插件目录失败: {}", e))
                    })?;
                    
                    // TODO: 实际克隆 Git 仓库的代码
                    // 例如：run_git_clone(url, &install_dir, version)?;
                }
                
                // 获取插件元数据并添加到已安装列表
                let metadata = self.create_plugin_metadata(name, &install_dir, source.clone(), false)?;
                self.plugins.push(metadata);
            }
        }
        
        Ok(())
    }
    
    /// 创建插件元数据
    fn create_plugin_metadata(&self, name: &str, path: &Path, source: PluginSource, lazy: bool) -> Result<PluginMetadata> {
        // 确定插件类型
        let plugin_type = if path.join("lua").exists() || path.join("plugin").exists() {
            PluginType::Neovim
        } else if path.join("init.lua").exists() {
            PluginType::Lua
        } else {
            return Err(FKVimError::PluginError(format!(
                "无法确定插件类型: {}", name
            )));
        };
        
        // 创建元数据
        let metadata = PluginMetadata {
            name: name.to_string(),
            version: "0.1.0".to_string(), // 默认版本
            author: "Unknown".to_string(),
            description: "".to_string(),
            plugin_type,
            path: path.to_path_buf(),
            source,
            lazy,
            dependencies: Vec::new(),
        };
        
        Ok(metadata)
    }
    
    /// 加载插件
    pub fn load_plugins(&mut self, lua_env: &mut lua::LuaEnv) -> Result<()> {
        // 加载 Lua 插件
        self.load_lua_plugins(lua_env)?;
        
        // 加载 Neovim 兼容插件
        if self.config.neovim_compat.enabled {
            self.load_neovim_plugins(lua_env)?;
        }
        
        Ok(())
    }
    
    /// 加载 Lua 插件
    fn load_lua_plugins(&mut self, lua_env: &mut lua::LuaEnv) -> Result<()> {
        let plugin_dir = &self.plugin_dir;
        if !plugin_dir.exists() {
            return Ok(());
        }
        
        // 扫描并加载插件
        for entry in std::fs::read_dir(plugin_dir)
            .map_err(|e| FKVimError::PluginError(format!("无法读取插件目录: {}", e)))? {
                
            let entry = entry.map_err(|e| {
                FKVimError::PluginError(format!("无法读取插件目录条目: {}", e))
            })?;
            
            let path = entry.path();
            if path.is_dir() {
                let init_lua = path.join("init.lua");
                if init_lua.exists() {
                    // 如果插件还没有加载过
                    if !self.plugins.iter().any(|p| p.path == path) {
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        
                        // 简单的元数据提取
                        let metadata = PluginMetadata {
                            name: name.clone(),
                            version: "0.1.0".to_string(),
                            author: "Unknown".to_string(),
                            description: "".to_string(),
                            plugin_type: PluginType::Lua,
                            path: path.clone(),
                            source: PluginSource::Local(path.clone()),
                            lazy: false,
                            dependencies: Vec::new(),
                        };
                        
                        self.plugins.push(metadata);
                    }
                    
                    // 加载插件
                    lua_env.load_plugin(&path)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// 加载 Neovim 兼容插件
    fn load_neovim_plugins(&mut self, lua_env: &mut lua::LuaEnv) -> Result<()> {
        if let Some(dirs) = &self.nvim_plugin_dirs {
            // 加载 start 目录下的插件
            if dirs.start.exists() {
                for entry in std::fs::read_dir(&dirs.start)
                    .map_err(|e| FKVimError::PluginError(format!("无法读取 Neovim start 插件目录: {}", e)))? {
                        
                    let entry = entry.map_err(|e| {
                        FKVimError::PluginError(format!("无法读取 Neovim start 插件目录条目: {}", e))
                    })?;
                    
                    let path = entry.path();
                    if path.is_dir() {
                        // 加载 Neovim 插件
                        lua_env.load_plugin(&path)?;
                        
                        // 如果插件还没有加载过，添加到列表
                        if !self.plugins.iter().any(|p| p.path == path) {
                            let name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let metadata = PluginMetadata {
                                name,
                                version: "0.1.0".to_string(),
                                author: "Unknown".to_string(),
                                description: "Neovim plugin".to_string(),
                                plugin_type: PluginType::Neovim,
                                path: path.clone(),
                                source: PluginSource::Local(path.clone()),
                                lazy: false,
                                dependencies: Vec::new(),
                            };
                            
                            self.plugins.push(metadata);
                        }
                    }
                }
            }
        } else if let Some(neovim_plugin_dir) = &self.config.neovim_compat.plugin_dir {
            // 传统方式：直接扫描 Neovim 插件目录
            if !neovim_plugin_dir.exists() {
                return Ok(());
            }
            
            for plugin_dir in find_nvim_plugin_dirs(neovim_plugin_dir, "start") {
                for entry in std::fs::read_dir(&plugin_dir)
                    .map_err(|e| FKVimError::PluginError(format!("无法读取 Neovim 插件目录: {}", e)))? {
                        
                    let entry = entry.map_err(|e| {
                        FKVimError::PluginError(format!("无法读取 Neovim 插件目录条目: {}", e))
                    })?;
                    
                    let path = entry.path();
                    if path.is_dir() {
                        // 加载 Neovim 插件
                        lua_env.load_plugin(&path)?;
                        
                        // 如果插件还没有加载过，添加到列表
                        if !self.plugins.iter().any(|p| p.path == path) {
                            let name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let metadata = PluginMetadata {
                                name,
                                version: "0.1.0".to_string(),
                                author: "Unknown".to_string(),
                                description: "Neovim plugin".to_string(),
                                plugin_type: PluginType::Neovim,
                                path: path.clone(),
                                source: PluginSource::Local(path.clone()),
                                lazy: false,
                                dependencies: Vec::new(),
                            };
                            
                            self.plugins.push(metadata);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 按需加载懒加载的 Neovim 插件
    pub fn load_lazy_plugin(&mut self, name: &str, lua_env: &mut lua::LuaEnv) -> Result<bool> {
        if let Some(dirs) = &self.nvim_plugin_dirs {
            let opt_plugin_path = dirs.opt.join(name);
            if opt_plugin_path.exists() {
                // 加载 Neovim 插件
                lua_env.load_plugin(&opt_plugin_path)?;
                
                // 如果插件还没有加载过，添加到列表
                if !self.plugins.iter().any(|p| p.path == opt_plugin_path) {
                    let metadata = PluginMetadata {
                        name: name.to_string(),
                        version: "0.1.0".to_string(),
                        author: "Unknown".to_string(),
                        description: "Lazy-loaded Neovim plugin".to_string(),
                        plugin_type: PluginType::Neovim,
                        path: opt_plugin_path.clone(),
                        source: PluginSource::Local(opt_plugin_path.clone()),
                        lazy: true,
                        dependencies: Vec::new(),
                    };
                    
                    self.plugins.push(metadata);
                }
                
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// 获取所有插件列表
    pub fn get_plugins(&self) -> &[PluginMetadata] {
        &self.plugins
    }
    
    /// 判断是否正在加载插件
    pub fn is_loading(&self) -> bool {
        // 简单实现，实际可能需要更复杂的状态跟踪
        false
    }
    
    /// 获取已加载的插件数量
    pub fn plugin_count(&self) -> usize {
        // 简单实现，实际需要根据插件管理器的实现返回正确的数量
        self.plugins.len()
    }
}

/// 查找 Neovim 插件目录
fn find_nvim_plugin_dirs(root: &Path, subdir: &str) -> Vec<PathBuf> {
    let pack_dir = root.join("pack");
    if !pack_dir.exists() {
        return Vec::new();
    }
    
    let mut result = Vec::new();
    
    // 遍历 pack/* 下的所有目录
    if let Ok(entries) = std::fs::read_dir(&pack_dir) {
        for entry in entries.filter_map(|res| res.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // 检查是否有 start 或 opt 子目录
                let target_dir = path.join(subdir);
                if target_dir.exists() {
                    result.push(target_dir);
                }
            }
        }
    }
    
    result
}