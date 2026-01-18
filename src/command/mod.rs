use crate::editor::Editor;
use crate::error::{Result, FKVimError};
use crate::editor::status::StatusMessageType;
use crate::editor::StatusMessage;
use std::collections::HashMap;
use std::path::Path;

pub mod help;

/// 自定义命令定义
#[derive(Clone)]
pub struct UserCommand {
    /// 命令名称
    pub name: String,
    
    /// 命令说明
    pub description: Option<String>,
    
    /// 命令类型（Lua脚本或回调函数）
    pub command_type: UserCommandType,
}

/// 自定义命令类型
#[derive(Clone)]
pub enum UserCommandType {
    /// Lua脚本命令
    Lua(String),
    
    /// 内部命令别名
    Alias(String),
}

/// 命令类型
pub enum CommandType {
    /// 内置命令
    Builtin(BuiltinCommand),
    
    /// Lua 脚本命令
    Lua(String),
    
    /// 用户定义命令
    UserDefined(String),
}

/// 内置命令
pub enum BuiltinCommand {
    /// 退出编辑器
    Quit,
    
    /// 保存当前文件
    Write(Option<String>), // 可选的文件路径
    
    /// 保存并退出
    WriteQuit(Option<String>), // 可选的文件路径
    
    /// 打开文件
    Edit(String), // 文件路径
    
    /// 设置选项
    Set(String, String), // 选项名, 选项值
    
    /// 显示当前设置
    ShowOption(Option<String>), // 可选的选项名
    
    /// 切换到缓冲区
    Buffer(usize), // 缓冲区索引
    
    /// 显示所有缓冲区
    Buffers,
    
    /// 新建缓冲区
    New,
    
    /// 关闭当前缓冲区
    Close,
    
    /// 关闭所有缓冲区
    CloseAll,
    
    /// 下一个缓冲区
    Next,
    
    /// 上一个缓冲区
    Previous,
    
    /// 重新加载当前文件
    Reload,
    
    /// 显示帮助
    Help(Option<String>), // 可选的帮助主题
}

/// 搜索标志位
#[derive(Debug, Clone, Default)]
pub struct SearchFlags {
    /// 是否区分大小写
    pub case_sensitive: bool,
    
    /// 是否使用正则表达式
    pub use_regex: bool,
    
    /// 是否全词匹配
    pub whole_word: bool,
    
    /// 是否在选择范围内搜索
    pub in_selection: bool,
}

impl std::fmt::Display for SearchFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = String::new();
        if self.case_sensitive {
            flags.push('c');
        } else {
            flags.push('i'); // 不区分大小写
        }
        if self.use_regex {
            flags.push('r');
        }
        if self.whole_word {
            flags.push('w');
        }
        if self.in_selection {
            flags.push('s');
        }
        write!(f, "{}", flags)
    }
}

/// 替换命令标志位
#[derive(Debug, Clone, Default)]
pub struct SubstituteFlags {
    /// 是否区分大小写
    pub case_sensitive: bool,
    
    /// 是否替换所有匹配
    pub global: bool,
    
    /// 是否确认每次替换
    pub confirm: bool,
    
    /// 是否使用正则表达式
    pub use_regex: bool,
}

impl std::fmt::Display for SubstituteFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = String::new();
        if !self.case_sensitive {
            flags.push('i'); // 不区分大小写
        }
        if self.global {
            flags.push('g');
        }
        if self.confirm {
            flags.push('c');
        }
        if self.use_regex {
            flags.push('r');
        }
        write!(f, "{}", flags)
    }
}

/// 命令解析器
pub struct CommandParser {
    command_manager: CommandManager,
}

impl CommandParser {
    /// 创建命令解析器
    pub fn new() -> Self {
        Self {
            command_manager: CommandManager::new(),
        }
    }

    /// 获取命令管理器的不可变引用
    pub fn command_manager(&self) -> &CommandManager {
        &self.command_manager
    }
    
    /// 获取命令管理器的可变引用
    pub fn command_manager_mut(&mut self) -> &mut CommandManager {
        &mut self.command_manager
    }

    /// 解析命令
    pub fn parse(&self, command_str: &str) -> Result<CommandType> {
        // 移除开头的冒号，并拆分命令和参数
        let command_str = command_str.trim_start_matches(':').trim();
        if command_str.is_empty() {
            return Err(FKVimError::CommandError("命令为空".to_string()));
        }

        // 拆分命令和参数
        let parts: Vec<&str> = command_str.splitn(2, ' ').collect();
        let cmd = parts[0];
        
        // 1. 检查是否为内置命令
        if let Some(builtin) = self.parse_builtin_command(cmd, parts.get(1).map(|s| *s).unwrap_or("")) {
            return Ok(CommandType::Builtin(builtin));
        }
        
        // 2. 检查是否为用户自定义命令
        if self.command_manager.has_command(cmd) {
            let args = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
            return Ok(CommandType::UserDefined(format!("{} {}", cmd, args).trim().to_string()));
        }
        
        // 3. 尝试进行模糊匹配
        let matches = self.command_manager.fuzzy_match(cmd);
        if matches.len() == 1 {
            // 找到唯一匹配的命令
            let matched_cmd = &matches[0].name;
            let args = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
            return Ok(CommandType::UserDefined(format!("{} {}", matched_cmd, args).trim().to_string()));
        } else if matches.len() > 1 {
            // 找到多个匹配项，返回错误
            let matches_str: Vec<String> = matches.iter().map(|cmd| cmd.name.clone()).collect();
            return Err(FKVimError::CommandError(
                format!("命令 '{}' 有多个匹配项: {}", cmd, matches_str.join(", "))
            ));
        }
        
        // 4. 假设为Lua命令
        if cmd.starts_with("lua") {
            let lua_code = command_str.trim_start_matches("lua").trim();
            return Ok(CommandType::Lua(lua_code.to_string()));
        }
        
        // 5. 都不匹配，返回为普通的用户命令
        Ok(CommandType::UserDefined(command_str.to_string()))
    }

    /// 获取命令补全列表
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        let partial = partial.trim_start_matches(':').trim();
        
        // 获取内置命令名称列表
        let builtin_cmds = vec![
            "q", "quit", "w", "write", "wq", "e", "edit", "source", 
            "split", "vsplit", "tabopen", "tabnew", "bd", "buffer", "buffers"
        ];
        
        let mut completions: Vec<String> = Vec::new();
        
        // 添加匹配的内置命令
        for cmd in builtin_cmds {
            if cmd.starts_with(partial) {
                completions.push(cmd.to_string());
            }
        }
        
        // 添加匹配的用户自定义命令
        let user_completions = self.command_manager.get_completion_list(partial);
        completions.extend(user_completions);
        
        completions
    }

    fn parse_builtin_command(&self, cmd: &str, args: &str) -> Option<BuiltinCommand> {
        match cmd {
            "q" | "quit" => Some(BuiltinCommand::Quit),
            "w" | "write" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Write(None))
                } else {
                    Some(BuiltinCommand::Write(Some(args.to_string())))
                }
            },
            "wq" | "x" | "exit" => {
                if args.is_empty() {
                    Some(BuiltinCommand::WriteQuit(None))
                } else {
                    Some(BuiltinCommand::WriteQuit(Some(args.to_string())))
                }
            },
            "e" | "edit" | "open" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "set" | "s" => {
                let parts: Vec<&str> = args.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some(BuiltinCommand::Set(parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            },
            "help" | "h" | "?" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Help(None))
                } else {
                    Some(BuiltinCommand::Help(Some(args.to_string())))
                }
            },
            "lua" | "l" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "plugin" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "sp" | "split" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Edit(args.to_string()))
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "vs" | "vsplit" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Edit(args.to_string()))
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "tabnew" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Edit(args.to_string()))
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "bn" | "bnext" => Some(BuiltinCommand::Next),
            "bp" | "bprevious" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Previous)
                } else {
                    None
                }
            },
            "bd" | "bdelete" => {
                if args.is_empty() {
                    Some(BuiltinCommand::Close)
                } else {
                    match args.parse::<usize>() {
                        Ok(num) => Some(BuiltinCommand::Buffer(num)),
                        Err(_) => None,
                    }
                }
            },
            "cd" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "ls" | "buffers" => Some(BuiltinCommand::Buffers),
            
            // 新增搜索命令
            "find" | "search" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "findcase" | "searchcase" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "advfind" | "advsearch" => {
                if args.is_empty() {
                    None
                } else {
                    // 解析高级搜索参数，格式: pattern [-ciwr]
                    // c: 区分大小写, i: 不区分大小写, w: 全词匹配, r: 正则表达式
                    let parts: Vec<&str> = args.splitn(2, " -").collect();
                    let pattern = parts[0].trim().to_string();
                    
                    let mut flags = SearchFlags::default();
                    
                    if parts.len() > 1 {
                        let options = parts[1].trim();
                        flags.case_sensitive = options.contains('c');
                        flags.use_regex = options.contains('r');
                        flags.whole_word = options.contains('w');
                        
                        // 如果既有c又有i，以i为准
                        if options.contains('i') {
                            flags.case_sensitive = false;
                        }
                    }
                    
                    Some(BuiltinCommand::Edit(format!("{} {}", pattern, flags.to_string())))
                }
            },
            
            // 替换命令
            "replace" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "replaceall" => {
                if args.is_empty() {
                    None
                } else {
                    Some(BuiltinCommand::Edit(args.to_string()))
                }
            },
            "substitute" => {
                if args.is_empty() {
                    None
                } else {
                    // 解析替换命令，格式: /pattern/replacement/[flags]
                    // flags: g-全局替换, c-确认替换, i-不区分大小写, r-正则表达式
                    
                    // 找出分隔符
                    let delimiter = args.chars().next().unwrap_or('/');
                    let parts: Vec<&str> = args[1..].split(delimiter).collect();
                    
                    if parts.len() >= 2 {
                        let pattern = parts[0].to_string();
                        let replacement = parts[1].to_string();
                        
                        let mut flags = SubstituteFlags::default();
                        
                        if parts.len() >= 3 {
                            let flag_str = parts[2];
                            flags.global = flag_str.contains('g');
                            flags.confirm = flag_str.contains('c');
                            flags.case_sensitive = !flag_str.contains('i');
                            flags.use_regex = flag_str.contains('r');
                        }
                        
                        Some(BuiltinCommand::Edit(format!("{} {} {}", pattern, replacement, flags.to_string())))
                    } else {
                        None
                    }
                }
            },
            
            // 切换搜索高亮
            "nohlsearch" | "nohl" => {
                Some(BuiltinCommand::Edit("nohlsearch".to_string()))
            },
            
            "terminal" | "term" => {
                let term_parts: Vec<&str> = args.splitn(2, ' ').collect();
                let term_cmd = term_parts.get(0).map_or("", |s| *s);
                let term_args = term_parts.get(1).map_or("", |s| *s);
                
                match term_cmd {
                    "open" => Some(BuiltinCommand::Edit(format!("terminal open {}", term_args))),
                    "close" => Some(BuiltinCommand::Edit(format!("terminal close {}", term_args))),
                    "toggle" => Some(BuiltinCommand::Edit(format!("terminal toggle {}", term_args))),
                    "focus" => Some(BuiltinCommand::Edit(format!("terminal focus {}", term_args))),
                    "new" => {
                        if term_args.is_empty() {
                            Some(BuiltinCommand::Edit("terminal new".to_string()))
                        } else {
                            Some(BuiltinCommand::Edit(format!("terminal new {}", term_args)))
                        }
                    },
                    "height" => {
                        if term_args.is_empty() {
                            None
                        } else {
                            match term_args.parse::<u16>() {
                                Ok(height) => Some(BuiltinCommand::Edit(format!("terminal height {}", height))),
                                Err(_) => None,
                            }
                        }
                    },
                    "exec" | "execute" => {
                        if term_args.is_empty() {
                            None
                        } else {
                            Some(BuiltinCommand::Edit(format!("terminal execute {}", term_args)))
                        }
                    },
                    "next" => Some(BuiltinCommand::Next),
                    "prev" => Some(BuiltinCommand::Previous),
                    "layout" => {
                        if term_args.is_empty() {
                            None
                        } else {
                            Some(BuiltinCommand::Edit(format!("terminal layout {}", term_args)))
                        }
                    },
                    "rename" => {
                        if term_args.is_empty() {
                            None
                        } else {
                            Some(BuiltinCommand::Edit(format!("terminal rename {}", term_args)))
                        }
                    },
                    _ => {
                        return Some(BuiltinCommand::Edit(format!("terminal {}", term_args)));
                    }
                }
            },
            _ => None,
        }
    }
}

/// 命令管理器
pub struct CommandManager {
    /// 用户自定义命令
    user_commands: HashMap<String, UserCommand>,
}

impl CommandManager {
    /// 创建命令管理器
    pub fn new() -> Self {
        Self {
            user_commands: HashMap::new(),
        }
    }
    
    /// 注册用户自定义命令
    pub fn register_command(&mut self, name: &str, command: UserCommand) -> Result<()> {
        if self.user_commands.contains_key(name) {
            return Err(FKVimError::CommandError(format!("命令 '{}' 已经存在", name)));
        }
        
        self.user_commands.insert(name.to_string(), command);
        Ok(())
    }
    
    /// 获取用户自定义命令
    pub fn get_command(&self, name: &str) -> Option<&UserCommand> {
        self.user_commands.get(name)
    }
    
    /// 检查命令是否存在
    pub fn has_command(&self, name: &str) -> bool {
        self.user_commands.contains_key(name)
    }
    
    /// 删除用户自定义命令
    pub fn unregister_command(&mut self, name: &str) -> Result<()> {
        if !self.user_commands.contains_key(name) {
            return Err(FKVimError::CommandError(format!("命令 '{}' 不存在", name)));
        }
        
        self.user_commands.remove(name);
        Ok(())
    }
    
    /// 列出所有用户自定义命令
    pub fn list_commands(&self) -> Vec<&UserCommand> {
        self.user_commands.values().collect()
    }
    
    /// 命令模糊匹配
    pub fn fuzzy_match(&self, partial_name: &str) -> Vec<&UserCommand> {
        if partial_name.is_empty() {
            return self.list_commands();
        }
        
        let partial_lower = partial_name.to_lowercase();
        self.user_commands
            .values()
            .filter(|cmd| cmd.name.to_lowercase().contains(&partial_lower))
            .collect()
    }
    
    /// 获取命令补全列表
    pub fn get_completion_list(&self, partial_name: &str) -> Vec<String> {
        if partial_name.is_empty() {
            return self.user_commands.keys().cloned().collect();
        }
        
        let partial_lower = partial_name.to_lowercase();
        self.user_commands
            .keys()
            .filter(|name| name.to_lowercase().starts_with(&partial_lower))
            .cloned()
            .collect()
    }

    /// 注册终端相关命令
    pub fn register_terminal_commands(&mut self) -> Result<()> {
        // 定义终端切换命令
        let toggle_terminal = UserCommand {
            name: CMD_TOGGLE_TERMINAL.to_string(),
            description: Some("切换终端显示状态".to_string()),
            command_type: UserCommandType::Alias("ToggleTerminal".to_string()),
        };
        self.register_command(CMD_TOGGLE_TERMINAL, toggle_terminal)?;
        
        // 定义终端焦点命令
        let focus_terminal = UserCommand {
            name: CMD_FOCUS_TERMINAL.to_string(),
            description: Some("将焦点切换到终端".to_string()),
            command_type: UserCommandType::Alias("FocusTerminal".to_string()),
        };
        self.register_command(CMD_FOCUS_TERMINAL, focus_terminal)?;
        
        // 定义退出终端焦点命令
        let exit_terminal_focus = UserCommand {
            name: CMD_EXIT_TERMINAL_FOCUS.to_string(),
            description: Some("退出终端焦点模式".to_string()),
            command_type: UserCommandType::Alias("ExitTerminalFocus".to_string()),
        };
        self.register_command(CMD_EXIT_TERMINAL_FOCUS, exit_terminal_focus)?;
        
        // 定义清空终端命令
        let clear_terminal = UserCommand {
            name: CMD_CLEAR_TERMINAL.to_string(),
            description: Some("清空终端内容".to_string()),
            command_type: UserCommandType::Alias("ClearTerminal".to_string()),
        };
        self.register_command(CMD_CLEAR_TERMINAL, clear_terminal)?;
        
        // 定义重启终端命令
        let restart_terminal = UserCommand {
            name: CMD_RESTART_TERMINAL.to_string(),
            description: Some("重启终端会话".to_string()),
            command_type: UserCommandType::Alias("RestartTerminal".to_string()),
        };
        self.register_command(CMD_RESTART_TERMINAL, restart_terminal)?;
        
        // 定义发送命令到终端
        let send_to_terminal = UserCommand {
            name: CMD_SEND_TO_TERMINAL.to_string(),
            description: Some("发送命令到终端".to_string()),
            command_type: UserCommandType::Alias("SendToTerminal".to_string()),
        };
        self.register_command(CMD_SEND_TO_TERMINAL, send_to_terminal)?;
        
        Ok(())
    }
}

/// 命令执行器
pub struct CommandExecutor {
    /// 编辑器实例
    editor: *mut Editor,
}

impl CommandExecutor {
    /// 创建命令执行器
    pub fn new(editor: &mut Editor) -> Self {
        Self {
            editor: editor as *mut Editor,
        }
    }
    
    /// 执行命令
    pub fn execute(&self, cmd_type: CommandType) -> Result<()> {
        let editor = unsafe { &mut *self.editor };
        
        match cmd_type {
            CommandType::Builtin(builtin) => self.execute_builtin(editor, builtin),
            CommandType::Lua(lua_code) => {
                editor.lua_env.execute(&lua_code)
            },
            CommandType::UserDefined(cmd) => {
                editor.lua_env.execute_command(&cmd)
            },
        }
    }
    
    /// 执行内置命令
    fn execute_builtin(&self, editor: &mut Editor, cmd: BuiltinCommand) -> Result<()> {
        match cmd {
            BuiltinCommand::Quit => {
                // 设置编辑器状态为退出
                editor.status = crate::editor::EditorStatus::Exiting;
                Ok(())
            },
            BuiltinCommand::Write(path) => {
                if let Some(path) = path {
                    editor.save_current_file_as(Path::new(&path))?
                } else {
                    editor.save_current_file()?
                }
                Ok(())
            },
            BuiltinCommand::WriteQuit(path) => {
                // 首先尝试保存
                if let Some(path) = path {
                    editor.save_current_file_as(Path::new(&path))?;
                } else {
                    editor.save_current_file()?;
                }
                
                // 然后设置编辑器状态为退出
                editor.status = crate::editor::EditorStatus::Exiting;
                Ok(())
            },
            BuiltinCommand::Edit(path) => {
                // 打开文件
                editor.open_file(Path::new(&path))?;
                Ok(())
            },
            BuiltinCommand::Set(option, value) => {
                // 设置选项
                editor.lua_env.set_config(&option, &value)?;
                Ok(())
            },
            BuiltinCommand::ShowOption(option) => {
                // 显示选项
                if let Some(option) = option {
                    // 显示特定选项
                    editor.status_message = Some(StatusMessage {
                        content: format!("{} = {}", option, editor.config.get_option(&option).unwrap_or("未设置".to_string())),
                        msg_type: StatusMessageType::Info,
                        timestamp: std::time::Instant::now(),
                    });
                } else {
                    // 显示所有选项
                    let mut options = Vec::new();
                    for (key, value) in editor.config.get_all_options() {
                        options.push(format!("{} = {}", key, value));
                    }
                    
                    editor.status_message = Some(StatusMessage {
                        content: options.join("\n"),
                        msg_type: StatusMessageType::Info,
                        timestamp: std::time::Instant::now(),
                    });
                }
                Ok(())
            },
            BuiltinCommand::Buffer(idx) => {
                // 切换到指定缓冲区
                editor.switch_to_buffer(idx)?;
                Ok(())
            },
            BuiltinCommand::Buffers => {
                // 显示所有缓冲区
                let mut buffer_list = Vec::new();
                for (i, buffer) in editor.buffers.iter().enumerate() {
                    let modified = if buffer.modified { "[+]" } else { "" };
                    let active = if i == editor.current_buffer { "*" } else { " " };
                    let path = buffer.file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| String::from("未命名"));
                    buffer_list.push(format!("{} {:2} {}{}", active, i, path, modified));
                }
                
                editor.status_message = Some(StatusMessage {
                    content: buffer_list.join("\n"),
                    msg_type: StatusMessageType::Info,
                    timestamp: std::time::Instant::now(),
                });
                Ok(())
            },
            BuiltinCommand::New => {
                // 创建新缓冲区
                editor.new_buffer()?;
                Ok(())
            },
            BuiltinCommand::Close => {
                // 关闭当前缓冲区
                editor.close_current_buffer()?;
                Ok(())
            },
            BuiltinCommand::CloseAll => {
                // 关闭所有缓冲区
                editor.close_all_buffers()?;
                Ok(())
            },
            BuiltinCommand::Next => {
                // 切换到下一个缓冲区
                editor.next_buffer()?;
                Ok(())
            },
            BuiltinCommand::Previous => {
                // 切换到上一个缓冲区
                editor.previous_buffer()?;
                Ok(())
            },
            BuiltinCommand::Reload => {
                // 重新加载当前文件
                editor.reload_current_file()?;
                Ok(())
            },
            BuiltinCommand::Help(topic) => {
                // 显示帮助信息
                let help_content = if let Some(topic) = topic {
                    // 查找特定主题的帮助内容
                    editor.help_system.get_topic_help(&topic)
                } else {
                    // 显示通用帮助信息
                    editor.help_system.get_general_help()
                };
                
                editor.status_message = Some(StatusMessage {
                    content: help_content,
                    msg_type: StatusMessageType::Info,
                    timestamp: std::time::Instant::now(),
                });
                Ok(())
            }
        }
    }
}

// 添加终端相关命令
pub const CMD_TOGGLE_TERMINAL: &str = "toggle_terminal";
pub const CMD_FOCUS_TERMINAL: &str = "focus_terminal";
pub const CMD_EXIT_TERMINAL_FOCUS: &str = "exit_terminal_focus";
pub const CMD_CLEAR_TERMINAL: &str = "clear_terminal";
pub const CMD_RESTART_TERMINAL: &str = "restart_terminal";
pub const CMD_SEND_TO_TERMINAL: &str = "send_to_terminal";