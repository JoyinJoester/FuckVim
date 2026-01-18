mod keybindings;
pub mod lua_config;

use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use crate::error::{Result, FKVimError};

pub use lua_config::LuaConfig;

/// 编辑器的全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 配置文件目录
    pub config_dir: PathBuf,
    
    /// 插件目录
    pub plugin_dir: PathBuf,
    
    /// 编辑器主题
    pub theme: String,
    
    /// 默认缩进宽度
    pub tab_width: usize,
    
    /// 是否使用空格代替制表符
    pub use_spaces: bool,
    
    /// 显示行号
    pub show_line_numbers: bool,
    
    /// 语法高亮
    pub syntax_highlight: bool,
    
    /// 自动缩进
    pub auto_indent: bool,
    
    /// 自动保存（秒数，0表示禁用）
    pub auto_save: u64,
    
    /// 兼容模式设置
    pub neovim_compat: NeovimCompatConfig,
    
    /// 按键映射
    pub keymaps: HashMap<String, HashMap<String, String>>,
}

/// Neovim 兼容性配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeovimCompatConfig {
    /// 是否启用 Neovim 兼容模式
    pub enabled: bool,
    
    /// Neovim 插件目录
    pub plugin_dir: Option<PathBuf>,
    
    /// 加载 Neovim 运行时文件
    pub load_runtime: bool,
    
    /// 是否支持 VimScript
    pub support_vimscript: bool,
    
    /// 是否启用 Neovim 的包管理器兼容模式
    pub package_manager: NeovimPackageManagerType,
    
    /// 自动检测并安装缺失的插件依赖
    pub auto_install_dependencies: bool,
}

/// Neovim 包管理器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NeovimPackageManagerType {
    /// 不使用包管理器
    None,
    
    /// 使用类似 packer.nvim 的包管理器
    Packer,
    
    /// 使用类似 lazy.nvim 的包管理器
    Lazy,
    
    /// 使用类似 vim-plug 的包管理器
    VimPlug,
}

impl Default for NeovimCompatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            plugin_dir: None,
            load_runtime: true,
            support_vimscript: false,
            package_manager: NeovimPackageManagerType::Lazy,
            auto_install_dependencies: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let config_dir = get_default_config_dir();
        let plugin_dir = config_dir.join("plugins");
        
        // 默认按键映射
        let mut keymaps = HashMap::new();
        let mut normal_maps = HashMap::new();
        let mut insert_maps = HashMap::new();
        
        normal_maps.insert("<C-s>".to_string(), "w".to_string());
        normal_maps.insert("<C-q>".to_string(), "q".to_string());
        
        insert_maps.insert("<C-s>".to_string(), "w".to_string());
        
        keymaps.insert("normal".to_string(), normal_maps);
        keymaps.insert("insert".to_string(), insert_maps);
        
        Self {
            config_dir,
            plugin_dir,
            theme: "default".to_string(),
            tab_width: 4,
            use_spaces: true,
            show_line_numbers: true,
            syntax_highlight: true,
            auto_indent: true,
            auto_save: 0,
            neovim_compat: NeovimCompatConfig::default(),
            keymaps,
        }
    }
}

/// 获取默认的配置目录
fn get_default_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "fkvim", "fkvim") {
        let config_dir = proj_dirs.config_dir().to_path_buf();
        if !config_dir.exists() {
            let _ = fs::create_dir_all(&config_dir);
        }
        config_dir
    } else {
        // 如果无法获取标准配置目录，则使用当前目录
        PathBuf::from("./.config/fkvim")
    }
}

/// 加载用户配置
pub fn load_config() -> Result<Config> {
    let config_dir = get_default_config_dir();
    let config_file = config_dir.join("config.lua");
    
    // 如果配置文件存在，则加载
    if config_file.exists() {
        let lua_config = lua_config::load_lua_config(&config_file)?;
        Ok(lua_config.to_config()?)
    } else {
        // 创建默认配置文件
        let default_config = Config::default();
        create_default_config_file(&config_file)?;
        Ok(default_config)
    }
}

/// 创建默认配置文件
fn create_default_config_file(config_file: &Path) -> Result<()> {
    let parent = config_file.parent().ok_or_else(|| {
        FKVimError::ConfigError("无法获取配置文件的父目录".to_string())
    })?;
    
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| {
            FKVimError::ConfigError(format!("无法创建配置目录: {}", e))
        })?;
    }
    
    let default_config = r#"-- FKVim 默认配置文件
-- 可以在此编辑您的编辑器配置

local config = {}

-- 基本设置
config.theme = "default"
config.tab_width = 4
config.use_spaces = true
config.show_line_numbers = true
config.syntax_highlight = true
config.auto_indent = true
config.auto_save = 0  -- 0表示禁用自动保存

-- Neovim 兼容性设置
config.neovim_compat = {
  enabled = true,
  load_runtime = true,
  support_vimscript = false,
  package_manager = "lazy",  -- 可选值: "none", "packer", "lazy", "vim-plug"
  auto_install_dependencies = true,
  -- plugin_dir = "~/.local/share/nvim/site/pack",  -- 可选：指定 Neovim 插件目录
}

-- 插件列表 (当使用 lazy 包管理器时)
config.plugins = {
  -- 示例插件配置
  -- { 
  --   "nvim-telescope/telescope.nvim",  -- 插件 GitHub 仓库
  --   dependencies = { "nvim-lua/plenary.nvim" },  -- 依赖项
  --   lazy = false,  -- 是否懒加载
  --   config = function()  -- 插件配置函数
  --     -- 这里可以放置插件的配置代码
  --     require("telescope").setup({})
  --   end
  -- },
  
  -- 更多插件...
}

-- 按键映射
config.mappings = {
  normal = {
    ["<C-s>"] = "save_buffer",
    ["<C-q>"] = "quit",
  },
  insert = {
    ["<C-s>"] = "save_buffer",
  },
}

-- 自定义命令
config.commands = {
  save_all = function()
    vim.command("wall")
  end
}

return config
"#;

    fs::write(config_file, default_config).map_err(|e| {
        FKVimError::ConfigError(format!("无法写入默认配置文件: {}", e))
    })?;
    
    Ok(())
}

impl Config {
    /// 获取指定选项的值
    pub fn get_option(&self, option: &str) -> Option<String> {
        match option {
            "theme" => Some(self.theme.clone()),
            "tab_width" => Some(self.tab_width.to_string()),
            "use_spaces" => Some(self.use_spaces.to_string()),
            "show_line_numbers" => Some(self.show_line_numbers.to_string()),
            "syntax_highlight" => Some(self.syntax_highlight.to_string()),
            "auto_indent" => Some(self.auto_indent.to_string()),
            "auto_save" => Some(self.auto_save.to_string()),
            "neovim_compat.enabled" => Some(self.neovim_compat.enabled.to_string()),
            "neovim_compat.load_runtime" => Some(self.neovim_compat.load_runtime.to_string()),
            "neovim_compat.support_vimscript" => Some(self.neovim_compat.support_vimscript.to_string()),
            "neovim_compat.auto_install_dependencies" => Some(self.neovim_compat.auto_install_dependencies.to_string()),
            _ => None,
        }
    }
    
    /// 获取所有选项的键值对
    pub fn get_all_options(&self) -> Vec<(String, String)> {
        let mut options = Vec::new();
        options.push(("theme".to_string(), self.theme.clone()));
        options.push(("tab_width".to_string(), self.tab_width.to_string()));
        options.push(("use_spaces".to_string(), self.use_spaces.to_string()));
        options.push(("show_line_numbers".to_string(), self.show_line_numbers.to_string()));
        options.push(("syntax_highlight".to_string(), self.syntax_highlight.to_string()));
        options.push(("auto_indent".to_string(), self.auto_indent.to_string()));
        options.push(("auto_save".to_string(), self.auto_save.to_string()));
        options.push(("neovim_compat.enabled".to_string(), self.neovim_compat.enabled.to_string()));
        options.push(("neovim_compat.load_runtime".to_string(), self.neovim_compat.load_runtime.to_string()));
        options.push(("neovim_compat.support_vimscript".to_string(), self.neovim_compat.support_vimscript.to_string()));
        options.push(("neovim_compat.auto_install_dependencies".to_string(), self.neovim_compat.auto_install_dependencies.to_string()));
        
        options
    }
}