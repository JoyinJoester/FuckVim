use std::collections::HashMap;
use std::path::{Path};
use std::time::{Instant};

// 修正导入
use crate::config::Config;
use crate::buffer::Buffer;
use crate::error::{Result, FKVimError};
use crate::highlight::Highlighter;
use crate::plugin::{PluginManager, PluginSource};
use crate::plugin::lua::LuaEnv;
use crate::plugin::nvim_compat::NeovimCompat;
use crate::plugin::package_manager::PackageManager;
use crate::file_browser::FileBrowser;

pub mod window;
pub mod status;
pub use status::{Status, StatusMessageType};

pub use window::{Window, WindowId, Tab, TabId, Layout, Rect, Split, TabManager};

/// 编辑器状态
pub struct Editor {
    /// 全局配置
    pub config: Config,
    
    /// Lua 环境
    pub lua_env: LuaEnv,
    
    /// 插件管理器
    pub plugin_manager: PluginManager,
    
    /// Neovim 兼容层
    pub neovim_compat: Option<NeovimCompat>,
    
    /// 插件包管理器
    pub package_manager: Option<PackageManager>,
    
    /// 缓冲区列表
    pub buffers: Vec<Buffer>,
    
    /// 当前活动缓冲区的索引
    pub current_buffer: usize,
    
    /// 标签和窗口管理器
    pub tab_manager: TabManager,
    
    /// 当前模式
    pub mode: EditorMode,
    
    /// 命令历史
    pub command_history: Vec<String>,
    
    /// 编辑器状态
    pub status: EditorStatus,
    
    /// 按键映射
    pub keymaps: HashMap<String, HashMap<String, String>>,
    
    /// 集成终端
    pub terminal: crate::terminal::Terminal,
    
    /// 状态消息
    pub status_message: Option<StatusMessage>,
    
    /// 语法高亮处理器
    pub highlighter: Highlighter,
    
    /// 文件浏览器
    pub file_browser: Option<FileBrowser>,
    
    /// 是否处于文件浏览器模式
    pub in_file_browser: bool,
    
    /// 显示区域宽度
    pub screen_width: usize,
    
    /// 显示区域高度
    pub screen_height: usize,
    
    /// 光标行
    pub cursor_line: usize,
    
    /// 光标列
    pub cursor_col: usize,

    /// 终端是否已初始化
    pub terminal_initialized: bool,

    /// 终端是否可见
    pub terminal_visible: bool,
    
    /// 终端高度
    pub terminal_height: u16,
    
    /// 命令行状态
    pub command_line: CommandLine,

    /// 重复次数
    pub repeat_count: usize,

    /// 最后一次执行的命令
    pub last_command: String,

    /// 帮助系统
    pub help_system: crate::command::help::HelpSystem,
}

/// 编辑器模式
#[derive(Debug, Clone, PartialEq)]
pub enum EditorMode {
    Normal,
    Insert,
    Visual,
    Command,
    Replace,
    Terminal,
}

/// 编辑器状态
#[derive(Debug, Clone, PartialEq)]
pub enum EditorStatus {
    Running,
    Exiting,
    Error(String),
}

/// 状态消息
pub struct StatusMessage {
    /// 消息内容
    pub content: String,
    
    /// 消息类型
    pub msg_type: StatusMessageType,
    
    /// 时间戳
    pub timestamp: Instant,
}

/// 搜索选项
pub struct SearchOptions {
    /// 是否区分大小写
    pub case_sensitive: bool,
    
    /// 是否使用正则表达式
    pub use_regex: bool,
    
    /// 是否全词匹配
    pub whole_word: bool,
    
    /// 是否在选择范围内搜索
    pub in_selection: bool,
}

/// 命令行模式
#[derive(Debug, Clone, PartialEq)]
pub enum CommandLineMode {
    /// 普通模式（不显示命令行）
    Normal,
    
    /// 命令模式（:命令）
    Command,
    
    /// 搜索模式（/搜索）
    Search,
    
    /// 替换确认模式
    ReplaceConfirm,
}

/// 命令行状态
pub struct CommandLine {
    /// 命令行内容
    pub content: String,
    
    /// 命令行模式
    pub mode: CommandLineMode,
    
    /// 光标位置
    pub cursor_pos: usize,
}

/// 临时编辑器引用，用于文件浏览器操作
pub struct EditorRef<'a> {
    pub config: &'a Config,
    pub status_message: &'a mut Option<StatusMessage>,
    pub open_file_fn: Box<dyn Fn(&Path) -> Result<usize> + 'a>,
    pub close_file_browser_fn: Box<dyn Fn() -> () + 'a>,
}

impl<'a> EditorRef<'a> {
    pub fn open_file(&self, path: &Path) -> Result<usize> {
        (self.open_file_fn)(path)
    }
    
    pub fn close_file_browser(&self) {
        (self.close_file_browser_fn)()
    }
    
    pub fn set_status_message(&mut self, content: impl Into<String>, msg_type: StatusMessageType) {
        *self.status_message = Some(StatusMessage {
            content: content.into(),
            msg_type,
            timestamp: std::time::Instant::now(),
        });
    }
}

impl Editor {
    /// 向左移动光标
    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            
            // 更新当前窗口的光标位置并确保可见
            if let Ok(tab) = self.tab_manager.current_tab_mut() {
                if let Ok(window) = tab.active_window_mut() {
                    window.update_cursor(self.cursor_line, self.cursor_col);
                }
            }
        }
    }
    
    /// 向右移动光标
    pub fn move_cursor_right(&mut self) -> Result<()> {
        if let Ok(buffer) = self.current_buffer() {
            if let Some(line) = buffer.text.get_line(self.cursor_line) {
                if self.cursor_col < line.len_chars() {
                    self.cursor_col += 1;
                    
                    // 更新当前窗口的光标位置并确保可见
                    if let Ok(tab) = self.tab_manager.current_tab_mut() {
                        if let Ok(window) = tab.active_window_mut() {
                            window.update_cursor(self.cursor_line, self.cursor_col);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    /// 向上移动光标
    pub fn move_cursor_up(&mut self) -> Result<()> {
        if self.cursor_line > 0 {
            let new_line = self.cursor_line - 1;
            let mut max_col = 0;
            
            // 获取新行的最大列
            if let Ok(buffer) = self.current_buffer() {
                if let Some(line) = buffer.text.get_line(new_line) {
                    max_col = line.len_chars();
                }
            }
            
            self.cursor_line = new_line;
            self.cursor_col = self.cursor_col.min(max_col);
            
            // 更新当前窗口的光标位置并确保可见
            if let Ok(tab) = self.tab_manager.current_tab_mut() {
                if let Ok(window) = tab.active_window_mut() {
                    window.update_cursor(self.cursor_line, self.cursor_col);
                }
            }
        }
        Ok(())
    }
    
    /// 向下移动光标
    pub fn move_cursor_down(&mut self) -> Result<()> {
        let mut should_move = false;
        let mut new_line = self.cursor_line;
        let mut max_col = 0;
        
        if let Ok(buffer) = self.current_buffer() {
            if self.cursor_line < buffer.text.len_lines() - 1 {
                new_line = self.cursor_line + 1;
                should_move = true;
                
                if let Some(line) = buffer.text.get_line(new_line) {
                    max_col = line.len_chars();
                }
            }
        }
        
        if should_move {
            self.cursor_line = new_line;
            self.cursor_col = self.cursor_col.min(max_col);
            
            // 更新当前窗口的光标位置并确保可见
            if let Ok(tab) = self.tab_manager.current_tab_mut() {
                if let Ok(window) = tab.active_window_mut() {
                    window.update_cursor(self.cursor_line, self.cursor_col);
                }
            }
        }
        
        Ok(())
    }
    
    /// 移动光标到行首
    fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }
    
    /// 移动光标到行尾
    fn move_cursor_end(&mut self) -> Result<()> {
        if let Ok(buffer) = self.current_buffer() {
            if let Some(line) = buffer.text.get_line(self.cursor_line) {
                self.cursor_col = line.len_chars();
            }
        }
        Ok(())
    }
    
    /// 向上翻页
    fn page_up(&mut self) -> Result<()> {
        let page_size = 10; // 或者根据窗口大小决定
        if self.cursor_line >= page_size {
            self.cursor_line -= page_size;
        } else {
            self.cursor_line = 0;
        }
        // 确保光标在新行的合法位置
        if let Ok(buffer) = self.current_buffer() {
            if let Some(line) = buffer.text.get_line(self.cursor_line) {
                self.cursor_col = self.cursor_col.min(line.len_chars());
            }
        }
        Ok(())
    }
    
    /// 向下翻页
    fn page_down(&mut self) -> Result<()> {
        let page_size = 10; // 或者根据窗口大小决定
        let mut new_line = self.cursor_line;
        let mut max_col = 0;
        
        if let Ok(buffer) = self.current_buffer() {
            if self.cursor_line + page_size < buffer.text.len_lines() {
                new_line = self.cursor_line + page_size;
            } else {
                new_line = buffer.text.len_lines() - 1;
            }
            
            if let Some(line) = buffer.text.get_line(new_line) {
                max_col = line.len_chars();
            }
        }
        
        self.cursor_line = new_line;
        self.cursor_col = self.cursor_col.min(max_col);
        
        Ok(())
    }

    /// 设置状态消息
    pub fn set_status_message(&mut self, content: impl Into<String>, msg_type: StatusMessageType) {
        self.status_message = Some(StatusMessage {
            content: content.into(),
            msg_type,
            timestamp: std::time::Instant::now(),
        });
    }
    
    /// 处理命令行模式切换
    pub fn switch_to_command_mode(&mut self) {
        self.mode = EditorMode::Command;
        self.command_line.mode = CommandLineMode::Command;
        self.command_line.content.clear();
        self.command_line.cursor_pos = 0;
        
        // 清除可能存在的状态消息，以使命令行输入更清晰
        self.status_message = None;
    }

    /// 创建一个新的编辑器实例
    pub fn new(config: Config, lua_env: LuaEnv) -> Result<Self> {
        // 创建插件管理器
        let plugin_manager = PluginManager::new(config.clone());
        
        // 创建 Neovim 兼容层
        let neovim_compat = if config.neovim_compat.enabled {
            Some(NeovimCompat::new(config.clone()))
        } else {
            None
        };
        
        // 创建初始缓冲区
        let buffers = vec![Buffer::new()];
        
        // 设置默认按键映射
        let keymaps = config.keymaps.clone();
        
        // 创建语法高亮处理器
        let highlighter = Highlighter::new();
        
        // 创建标签管理器
        let tab_manager = TabManager::new();
        
        // 创建帮助系统
        let help_system = crate::command::help::HelpSystem::new();
        
        // 返回编辑器实例
        let mut editor = Self {
            config,
            lua_env,
            plugin_manager,
            neovim_compat,
            package_manager: None, // 暂时为空，稍后初始化
            buffers,
            current_buffer: 0,
            tab_manager,
            mode: EditorMode::Normal,
            command_history: Vec::new(),
            status: EditorStatus::Running,
            keymaps,
            highlighter,
            status_message: None,
            file_browser: None,
            in_file_browser: false,
            screen_width: 80,  // 默认宽度
            screen_height: 24, // 默认高度
            terminal: crate::terminal::Terminal::new(), // 初始化终端
            cursor_line: 0,
            cursor_col: 0,
            terminal_initialized: false,
            terminal_visible: false,
            terminal_height: 10,
            command_line: CommandLine {
                content: String::new(),
                mode: CommandLineMode::Normal,
                cursor_pos: 0,
            },
            repeat_count: 0,
            last_command: String::new(),
            help_system: help_system,
        };
        
        // 使用更新后的编辑器实例初始化系统
        editor.init()?;
        
        Ok(editor)
    }
    
    /// 初始化编辑器
    pub fn init(&mut self) -> Result<()> {
        // 初始化 Neovim 兼容层
        if let Some(nvim_compat) = &mut self.neovim_compat {
            nvim_compat.init(&mut self.lua_env)?;
            
            // 找到 Neovim 插件并注册到插件管理器
            for plugin_path in nvim_compat.find_neovim_plugins() {
                let plugin_name = plugin_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let source = PluginSource::Local(plugin_path.clone());
                self.plugin_manager.register_plugin(&plugin_name, source, false)?;
            }
        }
        
        // 从配置中初始化插件包管理器
        self.init_package_manager()?;
        
        // 加载插件
        self.plugin_manager.load_plugins(&mut self.lua_env)?;
        
        // 如果有包管理器，加载包管理器管理的插件
        if let Some(pkg_manager) = &self.package_manager {
            pkg_manager.load_plugins(&mut self.plugin_manager, &mut self.lua_env)?;
        }
        
        // 设置状态消息
        self.set_status_message("编辑器初始化完成".to_string(), StatusMessageType::Info);
        
        Ok(())
    }
    
    /// 初始化插件包管理器
    fn init_package_manager(&mut self) -> Result<()> {
        if !matches!(self.config.neovim_compat.package_manager, crate::config::NeovimPackageManagerType::None) {
            // 获取 Lua 配置用于提取插件定义
            let config_dir = self.config.config_dir.join("config.lua");
            if config_dir.exists() {
                let lua_config = crate::config::lua_config::load_lua_config(&config_dir)?;
                
                // 创建包管理器并初始化
                let mut pkg_manager = PackageManager::load_from_lua_config(
                    self.config.clone(), 
                    &lua_config
                );
                
                // 初始化包管理器，扫描插件并安装缺失的插件
                pkg_manager.init()?;
                
                // 保存到编辑器
                self.package_manager = Some(pkg_manager);
            }
        }
        
        Ok(())
    }
    
    /// 加载懒加载插件
    pub fn load_lazy_plugin(&mut self, name: &str) -> Result<bool> {
        // 首先尝试通过包管理器加载
        if let Some(pkg_manager) = &self.package_manager {
            if pkg_manager.load_lazy_plugin(name, &mut self.plugin_manager, &mut self.lua_env)? {
                return Ok(true);
            }
        }
        
        // 否则尝试通过插件管理器加载
        self.plugin_manager.load_lazy_plugin(name, &mut self.lua_env)
    }
    
    /// 获取当前缓冲区
    pub fn current_buffer(&self) -> Result<&Buffer> {
        self.buffers.get(self.current_buffer)
            .ok_or_else(|| FKVimError::EditorError("无效的缓冲区索引".to_string()))
    }
    
    /// 获取当前缓冲区的可变引用
    pub fn current_buffer_mut(&mut self) -> Result<&mut Buffer> {
        self.buffers.get_mut(self.current_buffer)
            .ok_or_else(|| FKVimError::EditorError("无效的缓冲区索引".to_string()))
    }
    
    /// 打开文件
    pub fn open_file(&mut self, path: &Path) -> Result<usize> {
        // 检查是否已经打开
        for (idx, buffer) in self.buffers.iter().enumerate() {
            if let Some(file_path) = &buffer.file_path {
                if file_path == path {
                    self.current_buffer = idx;
                    
                    // 提前准备标题
                    let title = path.file_name()
                        .and_then(|f| f.to_str())
                        .map(|s| s.to_string());
                    
                    // 更新标签页标题
                    if let Some(title_str) = title {
                        if let Ok(tab) = self.tab_manager.current_tab_mut() {
                            tab.set_title(title_str);
                        }
                    }
                    
                    self.set_status_message(format!("切换到文件: {}", path.display()), StatusMessageType::Info);
                    return Ok(idx);
                }
            }
        }
        
        // 创建新缓冲区
        let buffer = Buffer::from_file(path)?;
        self.buffers.push(buffer);
        let buffer_idx = self.buffers.len() - 1;
        self.current_buffer = buffer_idx;
        
        // 提前准备标题
        let title = path.file_name()
            .and_then(|f| f.to_str())
            .map(|s| s.to_string());
        
        // 更新标签页标题
        if let Some(title_str) = title {
            if let Ok(tab) = self.tab_manager.current_tab_mut() {
                tab.set_title(title_str);
            }
        }
        
        // 确保在窗口中加载这个缓冲区
        if self.tab_manager.is_empty() {
            self.new_tab()?;
        }
        
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(window_id) = tab.active_window_id() {
                if let Some(window) = tab.get_window_mut(window_id) {
                    window.set_buffer(buffer_idx);
                }
            }
        }
        
        // 显示打开文件的状态消息
        self.set_status_message(format!("已打开: {}", path.display()), StatusMessageType::Info);
        
        Ok(buffer_idx)
    }
    
    /// 保存当前文件
    pub fn save_current_file(&mut self) -> Result<()> {
        match self.current_buffer_mut()?.save() {
            Ok(_) => {
                if let Some(path) = &self.current_buffer()?.file_path {
                    self.set_status_message(format!("已保存 {}", path.display()), StatusMessageType::Info);
                }
                Ok(())
            },
            Err(e) => {
                self.set_status_message(format!("保存失败: {}", e), StatusMessageType::Error);
                Err(e)
            }
        }
    }
    
    /// 保存当前文件到指定路径
    pub fn save_current_file_as(&mut self, path: &Path) -> Result<()> {
        match self.current_buffer_mut()?.save_as(path) {
            Ok(_) => {
                self.set_status_message(format!("已保存 {}", path.display()), StatusMessageType::Info);
                Ok(())
            },
            Err(e) => {
                self.set_status_message(format!("保存失败: {}", e), StatusMessageType::Error);
                Err(e)
            }
        }
    }
    
    /// 切换编辑器模式
    pub fn set_mode(&mut self, mode: EditorMode) {
        self.mode = mode;
    }
    
    /// 执行命令
    pub fn execute_command(&mut self, command: &str) -> Result<()> {
        // 记录到历史
        self.command_history.push(command.to_string());
        
        // 解析命令
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }
        
        match parts[0] {
            "q" | "quit" => {
                self.status = EditorStatus::Exiting;
            },
            "w" | "write" => {
                if parts.len() > 1 {
                    let path = Path::new(parts[1]);
                    self.save_current_file_as(path)?;
                } else {
                    self.save_current_file()?;
                }
            },
            "wq" => {
                // 保存并退出
                if parts.len() > 1 {
                    let path = Path::new(parts[1]);
                    self.save_current_file_as(path)?;
                } else {
                    self.save_current_file()?;
                }
                self.status = EditorStatus::Exiting;
            },
            "x" => {
                // 与 wq 相同，保存并退出
                if parts.len() > 1 {
                    let path = Path::new(parts[1]);
                    self.save_current_file_as(path)?;
                } else {
                    self.save_current_file()?;
                }
                self.status = EditorStatus::Exiting;
            },
            "help" => {
                // 显示帮助信息
                self.show_help()?;
            },
            "e" | "edit" => {
                if parts.len() > 1 {
                    let path = Path::new(parts[1]);
                    let buffer_idx = self.open_file(path)?;
                    // 在当前窗口中加载新打开的缓冲区
                    self.load_buffer_in_current_window(buffer_idx)?;
                }
            },
            "tabnew" | "tabe" => {
                // 创建新标签页
                self.new_tab()?;
            },
            "tabnext" | "tabn" => {
                // 切换到下一个标签页
                self.next_tab()?;
            },
            "tabprevious" | "tabp" => {
                // 切换到上一个标签页
                self.prev_tab()?;
            },
            "tabclose" | "tabc" => {
                // 关闭当前标签页
                self.close_current_tab()?;
            },
            "split" | "sp" => {
                // 水平分割窗口
                self.split_window_horizontal()?;
            },
            "vsplit" | "vs" => {
                // 垂直分割窗口
                self.split_window_vertical()?;
            },
            "close" | "clo" => {
                // 关闭当前窗口
                self.close_current_window()?;
            },
            "winc" | "wincmd" => {
                if parts.len() > 1 {
                    match parts[1] {
                        "h" => {
                            // 向左切换窗口
                            self.focus_left_window()?;
                        },
                        "j" => {
                            // 向下切换窗口
                            self.focus_down_window()?;
                        },
                        "k" => {
                            // 向上切换窗口
                            self.focus_up_window()?;
                        },
                        "l" => {
                            // 向右切换窗口
                            self.focus_right_window()?;
                        },
                        "w" => {
                            // 切换到下一个窗口
                            self.next_window()?;
                        },
                        "W" => {
                            // 切换到上一个窗口
                            self.prev_window()?;
                        },
                        _ => {
                            return Err(FKVimError::CommandError(format!("未知的窗口命令: {}", parts[1])));
                        }
                    }
                } else {
                    return Err(FKVimError::CommandError("窗口命令需要参数".to_string()));
                }
            },
            "buffer" | "b" => {
                if parts.len() > 1 {
                    // 解析缓冲区索引
                    if let Ok(buffer_idx) = parts[1].parse::<usize>() {
                        // 在当前窗口中加载指定缓冲区
                        self.load_buffer_in_current_window(buffer_idx - 1)?;
                    } else {
                        return Err(FKVimError::CommandError(format!("无效的缓冲区索引: {}", parts[1])));
                    }
                } else {
                    // 显示缓冲区列表
                    let buffer_list = self.format_buffer_list();
                    self.set_status_message(buffer_list, StatusMessageType::Info);
                }
            },
            "bnext" | "bn" => {
                // 切换到下一个缓冲区
                let next_idx = (self.current_buffer + 1) % self.buffers.len();
                self.load_buffer_in_current_window(next_idx)?;
            },
            "bprevious" | "bp" => {
                // 切换到上一个缓冲区
                let prev_idx = if self.current_buffer == 0 {
                    self.buffers.len() - 1
                } else {
                    self.current_buffer - 1
                };
                self.load_buffer_in_current_window(prev_idx)?;
            },
            "lua" => {
                if parts.len() > 1 {
                    let lua_code = &command[4..]; 
                    self.lua_env.execute(lua_code)?;
                }
            },
            "browse" | "explorer" => {
                self.show_file_browser()?;
            },
            "find" | "search" => {
                if parts.len() > 1 {
                    let query = &command[parts[0].len() + 1..]; // 跳过命令名和空格
                    self.search_text(query, false)?;
                } else {
                    return Err(FKVimError::CommandError("请指定搜索文本".to_string()));
                }
            },
            "findcase" | "searchcase" => {
                if parts.len() > 1 {
                    let query = &command[parts[0].len() + 1..]; // 跳过命令名和空格
                    self.search_text(query, true)?;
                } else {
                    return Err(FKVimError::CommandError("请指定搜索文本".to_string()));
                }
            },
            "toggleterm" => {
                self.toggle_terminal()?;
            },
            "focusterm" => {
                self.focus_terminal()?;
            },
            "exitterm" => {
                self.exit_terminal_focus()?;
            },
            "sendterm" => {
                if parts.len() > 1 {
                    let cmd = &command[parts[0].len() + 1..]; // 跳过命令名和空格
                    self.send_to_terminal(cmd)?;
                } else {
                    return Err(FKVimError::CommandError("请指定要发送的命令".to_string()));
                }
            },
            "clearterm" => {
                self.clear_terminal()?;
            },
            "restartterm" | "restart_terminal" => {
                self.restart_terminal()?;
            },
            _ => {
                // 尝试通过 Lua 执行命令
                if let Err(_) = self.lua_env.execute_command(command) {
                    // 使用统一的错误格式，同时保持Vim风格的错误码
                    return Err(FKVimError::CommandError(format!("E492: 不是编辑器命令: {}", command)));
                }
            }
        }
        
        Ok(())
    }

    /// 在当前窗口中加载缓冲区
    pub fn load_buffer_in_current_window(&mut self, buffer_idx: usize) -> Result<()> {
        if buffer_idx >= self.buffers.len() {
            return Err(FKVimError::EditorError(format!("无效的缓冲区索引: {}", buffer_idx)));
        }
        
        // 设置当前缓冲区索引
        self.current_buffer = buffer_idx;
        
        // 提前准备状态消息和标题
        let (status_message, title) = if let Some(path) = &self.buffers[buffer_idx].file_path {
            let status = format!("已加载文件: {}", path.display());
            let title = path.file_name()
                .and_then(|f| f.to_str())
                .map(|s| s.to_string());
            (Some(status), title)
        } else {
            (None, None)
        };
        
        // 获取当前标签页和窗口
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(window_id) = tab.active_window_id() {
                if let Some(window) = tab.get_window_mut(window_id) {
                    window.set_buffer(buffer_idx);
                    
                    // 如果有标题，更新标签页标题
                    if let Some(title_str) = &title {
                        tab.set_title(title_str.clone());
                    }
                    
                    // 这里已经对 tab 完成了操作，可以安全地释放借用
                }
            }
        }
        
        // 现在可以安全地设置状态消息
        if let Some(msg) = status_message {
            self.set_status_message(msg, StatusMessageType::Info);
        }
        
        // 如果没有活动窗口，则创建一个
        if self.tab_manager.is_empty() {
            self.new_tab()?;
            
            // 再次尝试设置缓冲区
            if let Ok(tab) = self.tab_manager.current_tab_mut() {
                if let Some(window_id) = tab.active_window_id() {
                    if let Some(window) = tab.get_window_mut(window_id) {
                        window.set_buffer(buffer_idx);
                        
                        // 如果有标题，更新标签页标题
                        if let Some(title_str) = &title {
                            tab.set_title(title_str.clone());
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 创建新标签页
    pub fn new_tab(&mut self) -> Result<TabId> {
        let tab_id = self.tab_manager.new_tab("New Tab".to_string())?;
        let tab_id = TabId(tab_id);
        
        // 在新标签页中创建一个窗口
        if let Ok(tab) = self.tab_manager.get_tab_mut(tab_id) {
            let window_id = WindowId(0); // 暂时使用一个假ID，实际应由Tab生成
            let window = Window::new(window_id, self.current_buffer);
            let window_id = tab.add_window(window);
            tab.set_active_window(window_id)?;
        }
        
        Ok(tab_id)
    }
    
    /// 显示文件浏览器
    pub fn show_file_browser(&mut self) -> Result<()> {
        if self.file_browser.is_none() {
            let current_dir = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            
            self.file_browser = Some(FileBrowser::new(Some(&current_dir))?);
        }
        
        self.in_file_browser = true;
        Ok(())
    }
    
    /// 水平分割窗口
    pub fn split_window_horizontal(&mut self) -> Result<WindowId> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(active_window_id) = tab.active_window_id() {
                if let Some(active_window) = tab.get_window(active_window_id) {
                    let buffer_id = active_window.buffer_id();
                    let new_window_id = WindowId(active_window_id.0 + 1); // 临时ID
                    let new_window = Window::new(new_window_id, buffer_id);
                    let new_window_id = tab.add_window(new_window);
                    tab.split(active_window_id, new_window_id, Split::Horizontal)?;
                    return Ok(new_window_id);
                }
            }
        }
        
        Err(FKVimError::EditorError("无法水平分割窗口".to_string()))
    }
    
    /// 垂直分割窗口
    pub fn split_window_vertical(&mut self) -> Result<WindowId> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(active_window_id) = tab.active_window_id() {
                if let Some(active_window) = tab.get_window(active_window_id) {
                    let buffer_id = active_window.buffer_id();
                    let new_window_id = WindowId(active_window_id.0 + 1); // 临时ID
                    let new_window = Window::new(new_window_id, buffer_id);
                    let new_window_id = tab.add_window(new_window);
                    tab.split(active_window_id, new_window_id, Split::Vertical)?;
                    return Ok(new_window_id);
                }
            }
        }
        
        Err(FKVimError::EditorError("无法垂直分割窗口".to_string()))
    }
    
    /// 关闭当前窗口
    pub fn close_current_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(window_id) = tab.active_window_id() {
                tab.remove_window(window_id)?;
                return Ok(());
            }
        }
        
        Err(FKVimError::EditorError("无法关闭当前窗口".to_string()))
    }
    
    /// 焦点移动到左侧窗口
    pub fn focus_left_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            tab.focus_left()
        } else {
            Err(FKVimError::EditorError("无法切换到左侧窗口".to_string()))
        }
    }
    
    /// 焦点移动到下方窗口
    pub fn focus_down_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            tab.focus_down()
        } else {
            Err(FKVimError::EditorError("无法切换到下方窗口".to_string()))
        }
    }
    
    /// 焦点移动到上方窗口
    pub fn focus_up_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            tab.focus_up()
        } else {
            Err(FKVimError::EditorError("无法切换到上方窗口".to_string()))
        }
    }
    
    /// 焦点移动到右侧窗口
    pub fn focus_right_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            tab.focus_right()
        } else {
            Err(FKVimError::EditorError("无法切换到右侧窗口".to_string()))
        }
    }
    
    /// 切换到下一个窗口
    pub fn next_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            tab.next_window()
        } else {
            Err(FKVimError::EditorError("无法切换到下一个窗口".to_string()))
        }
    }
    
    /// 切换到上一个窗口
    pub fn prev_window(&mut self) -> Result<()> {
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            tab.prev_window()
        } else {
            Err(FKVimError::EditorError("无法切换到上一个窗口".to_string()))
        }
    }
    
    /// 格式化缓冲区列表
    pub fn format_buffer_list(&self) -> String {
        let mut result = String::new();
        
        for (idx, buffer) in self.buffers.iter().enumerate() {
            let is_current = idx == self.current_buffer;
            let name = if let Some(path) = &buffer.file_path {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("[未命名]")
            } else {
                "[未命名]"
            };
            
            let modified = if buffer.modified { "[+]" } else { "" };
            let current = if is_current { "*" } else { " " };
            
            result.push_str(&format!("{}{} {:2}: {}{}\n", current, modified, idx + 1, name, if idx == self.buffers.len() - 1 { "" } else { "," }));
        }
        
        result
    }
    
    /// 搜索文本
    pub fn search_text(&mut self, query: &str, case_sensitive: bool) -> Result<()> {
        if query.is_empty() {
            return Err(FKVimError::EditorError("搜索文本不能为空".to_string()));
        }
        
        // 简单实现，未考虑复杂的正则表达式搜索
        let buffer = self.current_buffer()?;
        
        // 转换查询和文本以处理大小写不敏感搜索
        let search_query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        
        // 从当前光标位置开始搜索
        let mut found = false;
        
        for line_idx in self.cursor_line..buffer.text.len_lines() {
            if let Some(line) = buffer.text.get_line(line_idx) {
                let line_str = line.to_string();
                let line_compare = if case_sensitive {
                    line_str.clone()
                } else {
                    line_str.to_lowercase()
                };
                
                let start_col = if line_idx == self.cursor_line {
                    self.cursor_col + 1 // 从当前光标位置之后开始
                } else {
                    0
                };
                
                if start_col < line_compare.len() {
                    if let Some(col_idx) = line_compare[start_col..].find(&search_query) {
                        let real_col_idx = start_col + col_idx;
                        self.cursor_line = line_idx;
                        self.cursor_col = real_col_idx;
                        found = true;
                        break;
                    }
                }
            }
        }
        
        if !found {
            self.set_status_message("未找到匹配项", StatusMessageType::Info);
        }
        
        Ok(())
    }

    /// 切换到下一个标签页
    pub fn next_tab(&mut self) -> Result<()> {
        self.tab_manager.next_tab()
    }

    /// 切换到上一个标签页
    pub fn prev_tab(&mut self) -> Result<()> {
        self.tab_manager.prev_tab()
    }

    /// 关闭当前标签页
    pub fn close_current_tab(&mut self) -> Result<()> {
        self.tab_manager.close_current_tab()
    }

    /// 关闭当前缓冲区
    pub fn close_current_buffer(&mut self) -> Result<()> {
        if self.buffers.len() <= 1 {
            return Err(FKVimError::EditorError("不能关闭最后一个缓冲区".to_string()));
        }

        let current_buffer_idx = self.current_buffer;

        // 检查缓冲区是否有未保存的更改
        let buffer = &self.buffers[current_buffer_idx];
        if buffer.modified {
            return Err(FKVimError::EditorError("缓冲区有未保存的更改".to_string()));
        }

        // 移除缓冲区
        self.buffers.remove(current_buffer_idx);

        // 更新所有窗口中的缓冲区ID
        for tab_id in self.tab_manager.get_tab_ids() {
            if let Ok(tab) = self.tab_manager.get_tab_mut(tab_id) {
                for window_id in tab.get_window_ids() {
                    if let Some(window) = tab.get_window_mut(window_id) {
                        let buffer_id = window.buffer_id();
                        if buffer_id == current_buffer_idx {
                            // 如果窗口使用的是被删除的缓冲区，设置为第一个缓冲区
                            window.set_buffer(0);
                        } else if buffer_id > current_buffer_idx {
                            // 如果窗口使用的是更高索引的缓冲区，减少索引
                            window.set_buffer(buffer_id - 1);
                        }
                    }
                }
            }
        }

        // 更新当前缓冲区索引
        if self.current_buffer >= self.buffers.len() {
            self.current_buffer = self.buffers.len() - 1;
        }

        Ok(())
    }

    /// 关闭所有缓冲区
    pub fn close_all_buffers(&mut self) -> Result<()> {
        // 更新所有窗口的缓冲区ID为0
        for tab_id in self.tab_manager.get_tab_ids() {
            if let Ok(tab) = self.tab_manager.get_tab_mut(tab_id) {
                for window_id in tab.get_window_ids() {
                    if let Some(window) = tab.get_window_mut(window_id) {
                        window.set_buffer(0);
                    }
                }
            }
        }

        Ok(())
    }

    /// 切换到上一个缓冲区
    pub fn previous_buffer(&mut self) -> Result<()> {
        if self.buffers.is_empty() {
            return Err(FKVimError::EditorError("没有可用的缓冲区".to_string()));
        }
        
        let prev_buffer = if self.current_buffer > 0 {
            self.current_buffer - 1
        } else {
            self.buffers.len() - 1 // 循环到最后一个缓冲区
        };
        
        self.switch_to_buffer(prev_buffer)
    }

    /// 切换到指定缓冲区
    pub fn switch_to_buffer(&mut self, idx: usize) -> Result<()> {
        if idx >= self.buffers.len() {
            return Err(FKVimError::EditorError(format!("无效的缓冲区索引: {}", idx)));
        }
        
        // 获取当前标签页和窗口
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(window_id) = tab.active_window_id() {
                if let Some(window) = tab.get_window_mut(window_id) {
                    window.set_buffer(idx);
                    self.current_buffer = idx;
                    return Ok(());
                }
            }
        }
        
        self.current_buffer = idx;
        Ok(())
    }
    
    /// 创建新缓冲区
    pub fn new_buffer(&mut self) -> Result<usize> {
        let new_buffer = Buffer::new();
        self.buffers.push(new_buffer);
        let new_buffer_idx = self.buffers.len() - 1;
        
        // 如果有活动窗口，将新缓冲区设置为活动窗口的缓冲区
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(window_id) = tab.active_window_id() {
                if let Some(window) = tab.get_window_mut(window_id) {
                    window.set_buffer(new_buffer_idx);
                }
            }
        }
        
        self.current_buffer = new_buffer_idx;
        Ok(new_buffer_idx)
    }

    /// 切换到下一个缓冲区
    pub fn next_buffer(&mut self) -> Result<()> {
        if self.buffers.is_empty() {
            return Err(FKVimError::EditorError("没有可用的缓冲区".to_string()));
        }
        
        let next_buffer = if self.current_buffer + 1 < self.buffers.len() {
            self.current_buffer + 1
        } else {
            0 // 循环到第一个缓冲区
        };
        
        self.switch_to_buffer(next_buffer)
    }
    
    /// 重新加载当前文件
    pub fn reload_current_file(&mut self) -> Result<()> {
        let buffer = self.current_buffer_mut()?;
        
        if let Some(path) = &buffer.file_path {
            let path_clone = path.clone();
            *buffer = Buffer::from_file(&path_clone)?;
            self.set_status_message(format!("已重新加载 {}", path_clone.display()), StatusMessageType::Info);
        } else {
            return Err(FKVimError::EditorError("当前缓冲区没有关联文件".to_string()));
        }
        
        Ok(())
    }

    /// 处理终端相关功能
    
    /// 切换终端可见性
    pub fn toggle_terminal(&mut self) -> Result<()> {
        self.terminal_visible = !self.terminal_visible;
        
        if self.terminal_visible && !self.terminal_initialized {
            self.terminal.init()?;
            self.terminal_initialized = true;
        }
        
        Ok(())
    }
    
    /// 切换到终端模式
    pub fn focus_terminal(&mut self) -> Result<()> {
        if !self.terminal_visible {
            self.toggle_terminal()?;
        }
        
        self.terminal.focus();
        self.mode = EditorMode::Terminal;
        Ok(())
    }
    
    /// 退出终端焦点
    pub fn exit_terminal_focus(&mut self) -> Result<()> {
        self.terminal.unfocus();
        self.mode = EditorMode::Normal;
        Ok(())
    }
    
    /// 向终端发送命令
    pub fn send_to_terminal(&mut self, cmd: &str) -> Result<()> {
        if !self.terminal_initialized {
            self.terminal.init()?;
            self.terminal_initialized = true;
        }
        
        self.terminal.send_text(cmd)?;
        
        if !self.terminal_visible {
            self.toggle_terminal()?;
        }
        
        Ok(())
    }
    
    /// 清除终端
    pub fn clear_terminal(&mut self) -> Result<()> {
        if self.terminal_initialized {
            self.terminal.clear();
        }
        
        Ok(())
    }
    
    /// 重启终端
    pub fn restart_terminal(&mut self) -> Result<()> {
        if self.terminal_initialized {
            self.terminal.restart()?;
        }
        
        Ok(())
    }

    /// 显示帮助信息
    pub fn show_help(&mut self) -> Result<()> {
        // 创建帮助内容
        let help_content = self.generate_help_content();
        
        // 创建新缓冲区用于帮助内容
        let mut help_buffer = Buffer::new();
        help_buffer.set_content(&help_content);
        help_buffer.file_path = Some(std::path::PathBuf::from("[帮助]")); // 虚拟路径
        help_buffer.read_only = true; // 设为只读
        
        // 添加到缓冲区列表
        self.buffers.push(help_buffer);
        let help_buffer_idx = self.buffers.len() - 1;
        
        // 垂直分割窗口
        let new_window_id = self.split_window_vertical()?;
        
        // 在新窗口中加载帮助缓冲区
        if let Ok(tab) = self.tab_manager.current_tab_mut() {
            if let Some(window) = tab.get_window_mut(new_window_id) {
                window.set_buffer(help_buffer_idx);
            }
        }
        
        // 设置状态消息
        self.set_status_message("帮助文档已打开", StatusMessageType::Info);
        
        Ok(())
    }

    /// 生成帮助内容
    fn generate_help_content(&self) -> String {
        let mut content = String::new();
        
        // 添加标题
        content.push_str("FKVim 帮助文档\n");
        content.push_str("=============\n\n");
        
        // 基本命令
        content.push_str("基本命令:\n");
        content.push_str("---------\n");
        content.push_str(":q                  退出编辑器\n");
        content.push_str(":w                  保存当前文件\n");
        content.push_str(":wq, :x             保存并退出\n");
        content.push_str(":e <文件>           编辑指定文件\n");
        content.push_str(":help               显示此帮助\n\n");
        
        // 窗口管理
        content.push_str("窗口管理:\n");
        content.push_str("---------\n");
        content.push_str(":split, :sp         水平分割窗口\n");
        content.push_str(":vsplit, :vs        垂直分割窗口\n");
        content.push_str(":close, :clo        关闭当前窗口\n");
        content.push_str(":wincmd h           切换到左侧窗口\n");
        content.push_str(":wincmd j           切换到下方窗口\n");
        content.push_str(":wincmd k           切换到上方窗口\n");
        content.push_str(":wincmd l           切换到右侧窗口\n\n");
        
        // 标签页管理
        content.push_str("标签页管理:\n");
        content.push_str("-----------\n");
        content.push_str(":tabnew, :tabe      新建标签页\n");
        content.push_str(":tabnext, :tabn     切换到下一个标签页\n");
        content.push_str(":tabprevious, :tabp 切换到上一个标签页\n");
        content.push_str(":tabclose, :tabc    关闭当前标签页\n\n");
        
        // 缓冲区管理
        content.push_str("缓冲区管理:\n");
        content.push_str("-----------\n");
        content.push_str(":buffer, :b <编号>  切换到指定缓冲区\n");
        content.push_str(":bnext, :bn         切换到下一个缓冲区\n");
        content.push_str(":bprevious, :bp     切换到上一个缓冲区\n\n");
        
        // 终端集成
        content.push_str("终端集成:\n");
        content.push_str("---------\n");
        content.push_str(":toggleterm         切换终端可见性\n");
        content.push_str(":focusterm          聚焦到终端\n");
        content.push_str(":exitterm           退出终端模式\n");
        content.push_str(":sendterm <命令>    向终端发送命令\n");
        content.push_str(":clearterm          清空终端\n");
        content.push_str(":restartterm        重启终端\n\n");
        
        // 文件浏览
        content.push_str("文件浏览:\n");
        content.push_str("---------\n");
        content.push_str(":browse, :explorer  打开文件浏览器\n\n");
        
        // 搜索
        content.push_str("搜索:\n");
        content.push_str("-----\n");
        content.push_str(":find, :search <文本>     搜索文本（不区分大小写）\n");
        content.push_str(":findcase, :searchcase <文本>  搜索文本（区分大小写）\n\n");
        
        // 普通模式快捷键
        content.push_str("普通模式快捷键:\n");
        content.push_str("-------------\n");
        content.push_str("h, j, k, l          左、下、上、右移动\n");
        content.push_str("i                    进入插入模式\n");
        content.push_str("a                    在光标后进入插入模式\n");
        content.push_str("o                    在下方新行进入插入模式\n");
        content.push_str("O                    在上方新行进入插入模式\n");
        content.push_str("x                    删除字符\n");
        content.push_str("dd                   删除行\n");
        content.push_str("yy                   复制行\n");
        content.push_str("p                    粘贴\n");
        content.push_str("u                    撤销\n");
        content.push_str("Ctrl+r               重做\n\n");
        
        // 插入模式快捷键
        content.push_str("插入模式快捷键:\n");
        content.push_str("-------------\n");
        content.push_str("Esc                  返回普通模式\n");
        content.push_str("Ctrl+s               保存文件\n\n");
        
        // 底部提示
        content.push_str("\n按 q 关闭此帮助窗口\n");
        
        content
    }
}