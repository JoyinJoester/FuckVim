use std::collections::HashMap;
use crate::editor::{Editor, EditorMode};
use crate::error::{Result};

/// 按键处理器
pub struct KeyHandler {
    /// 编辑器实例
    editor: *mut Editor,
    
    /// 正常模式下的键映射
    normal_mappings: HashMap<String, String>,
    
    /// 插入模式下的键映射
    insert_mappings: HashMap<String, String>,
    
    /// 可视模式下的键映射
    visual_mappings: HashMap<String, String>,
    
    /// 命令模式下的键映射
    command_mappings: HashMap<String, String>,
    
    /// 当前命令缓冲区
    command_buffer: String,
}

/// 输入动作类型
pub enum InputAction {
    /// 插入文本
    Insert(String),
    
    /// 删除文本
    Delete(usize, usize, usize, usize),
    
    /// 移动光标
    MoveCursor(isize, isize),
    
    /// 执行命令
    ExecuteCommand(String),
    
    /// 切换模式
    SwitchMode(EditorMode),
    
    /// 无操作
    None,
}

impl KeyHandler {
    /// 创建按键处理器
    pub fn new(editor: &mut Editor) -> Self {
        // 从编辑器配置中加载按键映射
        let mut normal_mappings = HashMap::new();
        let mut insert_mappings = HashMap::new();
        let visual_mappings = HashMap::new();
        let command_mappings = HashMap::new();
        
        // 添加一些默认映射
        normal_mappings.insert("<C-s>".to_string(), "w".to_string());
        normal_mappings.insert("<C-q>".to_string(), "q".to_string());
        insert_mappings.insert("<C-s>".to_string(), "<Esc>:w<CR>i".to_string());
        
        Self {
            editor: editor as *mut Editor,
            normal_mappings,
            insert_mappings,
            visual_mappings,
            command_mappings,
            command_buffer: String::new(),
        }
    }
    
    /// 处理按键输入
    pub fn handle_key(&mut self, key: &str) -> Result<InputAction> {
        let editor = unsafe { &mut *self.editor };
        
        match editor.mode {
            EditorMode::Normal => self.handle_normal_key(key),
            EditorMode::Insert => self.handle_insert_key(key),
            EditorMode::Visual => self.handle_visual_key(key),
            EditorMode::Command => self.handle_command_key(key),
            EditorMode::Replace => self.handle_replace_key(key),
            EditorMode::Terminal => self.handle_terminal_key(key),
        }
    }
    
    /// 处理正常模式下的按键
    fn handle_normal_key(&mut self, key: &str) -> Result<InputAction> {
        // 首先检查是否有按键映射
        let mapped_action = if let Some(mapped) = self.normal_mappings.get(key).cloned() {
            Some(self.handle_mapped_keys(&mapped)?)
        } else {
            None
        };
        
        if let Some(action) = mapped_action {
            return Ok(action);
        }
        
        // 处理不同的按键
        match key {
            "i" | "<Insert>" => Ok(InputAction::SwitchMode(EditorMode::Insert)),
            "I" => {
                // 移动到行首并进入插入模式
                Ok(InputAction::SwitchMode(EditorMode::Insert))
            },
            "a" => {
                // 光标向右移动一格并进入插入模式
                Ok(InputAction::SwitchMode(EditorMode::Insert))
            },
            "A" => {
                // 移动到行尾并进入插入模式
                Ok(InputAction::SwitchMode(EditorMode::Insert))
            },
            "v" => Ok(InputAction::SwitchMode(EditorMode::Visual)),
            ":" => {
                // 切换到命令模式并设置命令行状态
                let editor = unsafe { &mut *self.editor };
                editor.switch_to_command_mode();
                Ok(InputAction::SwitchMode(EditorMode::Command))
            },
            "R" => Ok(InputAction::SwitchMode(EditorMode::Replace)),
            
            // 光标移动
            "h" | "<Left>" => Ok(InputAction::MoveCursor(-1, 0)),
            "j" | "<Down>" => Ok(InputAction::MoveCursor(0, 1)),
            "k" | "<Up>" => Ok(InputAction::MoveCursor(0, -1)),
            "l" | "<Right>" => Ok(InputAction::MoveCursor(1, 0)),
            
            // 行首、行尾移动
            "0" | "<Home>" => Ok(InputAction::ExecuteCommand("0".to_string())),
            "$" | "<End>" => Ok(InputAction::ExecuteCommand("$".to_string())),
            "^" => Ok(InputAction::ExecuteCommand("^".to_string())),
            
            // 翻页
            "<PageUp>" | "<C-b>" => Ok(InputAction::MoveCursor(0, -20)),
            "<PageDown>" | "<C-f>" => Ok(InputAction::MoveCursor(0, 20)),
            "<C-u>" => Ok(InputAction::MoveCursor(0, -10)), // 向上半页
            "<C-d>" => Ok(InputAction::MoveCursor(0, 10)),  // 向下半页
            
            // 文件内导航
            "gg" => Ok(InputAction::ExecuteCommand("gg".to_string())), // 文件开头
            "G" => Ok(InputAction::ExecuteCommand("G".to_string())),   // 文件结尾
            
            // 文本操作
            "x" => {
                // 删除当前字符
                Ok(InputAction::Delete(0, 0, 0, usize::MAX))
            },
            "dd" => {
                // 删除当前行
                Ok(InputAction::ExecuteCommand("dd".to_string()))
            },
            "yy" => {
                // 复制当前行
                Ok(InputAction::ExecuteCommand("yy".to_string()))
            },
            "p" => {
                // 粘贴
                Ok(InputAction::ExecuteCommand("p".to_string()))
            },
            "u" => {
                // 撤销
                Ok(InputAction::ExecuteCommand("u".to_string()))
            },
            "<C-r>" => {
                // 重做
                Ok(InputAction::ExecuteCommand("redo".to_string()))
            },
            
            // 窗口操作
            "<C-w>h" | "<C-w><Left>" => Ok(InputAction::ExecuteCommand("win h".to_string())),
            "<C-w>j" | "<C-w><Down>" => Ok(InputAction::ExecuteCommand("win j".to_string())),
            "<C-w>k" | "<C-w><Up>" => Ok(InputAction::ExecuteCommand("win k".to_string())),
            "<C-w>l" | "<C-w><Right>" => Ok(InputAction::ExecuteCommand("win l".to_string())),
            "<C-w>w" => Ok(InputAction::ExecuteCommand("win w".to_string())),
            "<C-w>W" => Ok(InputAction::ExecuteCommand("win W".to_string())),
            "<C-w>s" => Ok(InputAction::ExecuteCommand("split".to_string())),
            "<C-w>v" => Ok(InputAction::ExecuteCommand("vsplit".to_string())),
            "<C-w>c" => Ok(InputAction::ExecuteCommand("close".to_string())),
            "<C-w>o" => Ok(InputAction::ExecuteCommand("only".to_string())),
            
            // 查找操作
            "/" => Ok(InputAction::ExecuteCommand("search".to_string())),
            "n" => Ok(InputAction::ExecuteCommand("find_next".to_string())),
            "N" => Ok(InputAction::ExecuteCommand("find_prev".to_string())),
            
            // 其他命令
            _ => Ok(InputAction::None),
        }
    }
    
    /// 处理插入模式下的按键
    fn handle_insert_key(&mut self, key: &str) -> Result<InputAction> {
        // 首先检查是否有按键映射
        let mapped_action = if let Some(mapped) = self.insert_mappings.get(key).cloned() {
            Some(self.handle_mapped_keys(&mapped)?)
        } else {
            None
        };
        
        if let Some(action) = mapped_action {
            return Ok(action);
        }
        
        // 对于特殊按键的单独处理
        match key {
            "<Esc>" => Ok(InputAction::SwitchMode(EditorMode::Normal)),
            "<Insert>" => Ok(InputAction::SwitchMode(EditorMode::Replace)),
            "<CR>" => {
                // 回车键，插入换行符
                Ok(InputAction::Insert("\n".to_string()))
            },
            "<BS>" => {
                // 退格键，删除前一个字符
                Ok(InputAction::Delete(0, 0, 0, 1))
            },
            "<Tab>" => {
                let editor = unsafe { &*self.editor };
                if editor.config.use_spaces {
                    let spaces = " ".repeat(editor.config.tab_width);
                    Ok(InputAction::Insert(spaces))
                } else {
                    Ok(InputAction::Insert("\t".to_string()))
                }
            },
            // 方向键支持
            "<Left>" => Ok(InputAction::MoveCursor(-1, 0)),
            "<Right>" => Ok(InputAction::MoveCursor(1, 0)),
            "<Up>" => Ok(InputAction::MoveCursor(0, -1)),
            "<Down>" => Ok(InputAction::MoveCursor(0, 1)),
            
            // 行首、行尾支持
            "<Home>" => Ok(InputAction::ExecuteCommand("0".to_string())),
            "<End>" => Ok(InputAction::ExecuteCommand("$".to_string())),
            
            // 翻页支持
            "<PageUp>" => Ok(InputAction::MoveCursor(0, -20)),
            "<PageDown>" => Ok(InputAction::MoveCursor(0, 20)),
            
            // 保存快捷键
            "<C-s>" => Ok(InputAction::ExecuteCommand("w".to_string())),
            
            // 其他按键作为文本输入 - 直接执行None，由上层UI处理
            _ => {
                // 检查是否是单个字符的普通键
                if key.len() == 1 {
                    // 返回None，让上层UI部分处理
                    Ok(InputAction::None)
                } else {
                    // 无法识别的按键，记录日志但不做任何操作
                    eprintln!("未处理的按键: '{}'", key);
                    Ok(InputAction::None)
                }
            }
        }
    }
    
    /// 处理可视模式下的按键
    fn handle_visual_key(&mut self, key: &str) -> Result<InputAction> {
        // 首先检查是否有按键映射
        let mapped_action = if let Some(mapped) = self.visual_mappings.get(key).cloned() {
            Some(self.handle_mapped_keys(&mapped)?)
        } else {
            None
        };
        
        if let Some(action) = mapped_action {
            return Ok(action);
        }
        
        match key {
            "<Esc>" => Ok(InputAction::SwitchMode(EditorMode::Normal)),
            // 光标移动
            "h" | "<Left>" => Ok(InputAction::MoveCursor(-1, 0)),
            "j" | "<Down>" => Ok(InputAction::MoveCursor(0, 1)),
            "k" | "<Up>" => Ok(InputAction::MoveCursor(0, -1)),
            "l" | "<Right>" => Ok(InputAction::MoveCursor(1, 0)),
            
            // 文本操作
            "d" => {
                // 删除选中内容
                Ok(InputAction::ExecuteCommand("d".to_string()))
            },
            "y" => {
                // 复制选中内容
                Ok(InputAction::ExecuteCommand("y".to_string()))
            },
            
            _ => Ok(InputAction::None),
        }
    }
    
    /// 处理命令模式下的按键
    fn handle_command_key(&mut self, key: &str) -> Result<InputAction> {
        let editor = unsafe { &mut *self.editor };
        
        match key {
            "<Esc>" => {
                self.command_buffer.clear();
                editor.command_line.content.clear();
                editor.command_line.mode = crate::editor::CommandLineMode::Normal;
                Ok(InputAction::SwitchMode(EditorMode::Normal))
            },
            "<CR>" => {
                let cmd = self.command_buffer.clone();
                self.command_buffer.clear();
                editor.command_line.content.clear();
                editor.command_line.mode = crate::editor::CommandLineMode::Normal;
                if !cmd.is_empty() {
                    Ok(InputAction::ExecuteCommand(cmd))
                } else {
                    Ok(InputAction::SwitchMode(EditorMode::Normal))
                }
            },
            "<BS>" => {
                if !self.command_buffer.is_empty() {
                    self.command_buffer.pop();
                    
                    // 同时更新编辑器的命令行内容
                    if !editor.command_line.content.is_empty() {
                        editor.command_line.content.pop();
                        // 更新光标位置
                        if editor.command_line.cursor_pos > 0 {
                            editor.command_line.cursor_pos -= 1;
                        }
                    }
                }
                Ok(InputAction::None)
            },
            _ => {
                if key.len() == 1 {
                    self.command_buffer.push_str(key);
                    
                    // 同时更新编辑器的命令行内容
                    editor.command_line.content.push_str(key);
                    // 更新光标位置
                    editor.command_line.cursor_pos += 1;
                }
                Ok(InputAction::None)
            }
        }
    }
    
    /// 处理替换模式下的按键
    fn handle_replace_key(&mut self, key: &str) -> Result<InputAction> {
        match key {
            "<Esc>" => Ok(InputAction::SwitchMode(EditorMode::Normal)),
            "<Insert>" => Ok(InputAction::SwitchMode(EditorMode::Insert)),
            // 其他按键作为替换输入
            _ => {
                if key.len() == 1 {
                    // 替换当前字符
                    Ok(InputAction::Insert(key.to_string()))
                } else {
                    Ok(InputAction::None)
                }
            }
        }
    }
    
    /// 处理终端模式下的按键
    fn handle_terminal_key(&mut self, key: &str) -> Result<InputAction> {
        match key {
            "<C-\\><C-n>" | "<Esc>" => Ok(InputAction::SwitchMode(EditorMode::Normal)),
            _ => {
                // 在终端模式下，所有按键都直接传递给终端
                Ok(InputAction::Insert(key.to_string()))
            }
        }
    }
    
    /// 处理映射的按键序列
    fn handle_mapped_keys(&mut self, keys: &str) -> Result<InputAction> {
        // 简单实现：只执行第一个键
        if !keys.is_empty() {
            let first_key = &keys[0..1];
            // 根据键值返回对应的动作，而不是递归调用 handle_key
            match first_key {
                "i" => Ok(InputAction::SwitchMode(EditorMode::Insert)),
                ":" => Ok(InputAction::SwitchMode(EditorMode::Command)),
                // 可以添加更多常见映射动作的处理
                _ => Ok(InputAction::None)
            }
        } else {
            Ok(InputAction::None)
        }
    }
    
    /// 获取当前命令缓冲区
    pub fn get_command_buffer(&self) -> &str {
        &self.command_buffer
    }
    
    /// 设置按键映射
    pub fn set_mapping(&mut self, mode: &str, key: String, command: String) {
        match mode {
            "normal" => { self.normal_mappings.insert(key, command); },
            "insert" => { self.insert_mappings.insert(key, command); },
            "visual" => { self.visual_mappings.insert(key, command); },
            "command" => { self.command_mappings.insert(key, command); },
            _ => {}
        }
    }
}