use std::io::{Write, BufReader, Read};
use std::process::{Command, Stdio, Child};
use std::sync::{Arc, Mutex};
use crossterm::event::{KeyCode, KeyEvent};
use std::thread;
use std::collections::HashMap;

use crate::error::{Result, FKVimError};

/// 表示单个终端会话
pub struct TerminalSession {
    /// 终端进程
    process: Option<Child>,
    /// 终端输出内容
    output: Vec<String>,
    /// 用户输入的命令
    input_buffer: String,
    /// 输入行中的光标位置
    cursor_pos: usize,
    /// 终端当前路径
    current_dir: String,
    /// 终端输出历史记录的滚动位置
    pub scroll: usize,
    /// 终端历史记录的最大行数
    max_history: usize,
    /// 会话名称
    pub name: String,
}

impl Clone for TerminalSession {
    fn clone(&self) -> Self {
        TerminalSession {
            process: None, // 不复制进程句柄
            output: self.output.clone(),
            input_buffer: self.input_buffer.clone(),
            cursor_pos: self.cursor_pos,
            current_dir: self.current_dir.clone(),
            scroll: self.scroll,
            max_history: self.max_history,
            name: self.name.clone(),
        }
    }
}

impl TerminalSession {
    /// 创建一个新的终端会话
    pub fn new(name: String, dir: Option<String>) -> Self {
        let current_dir = if let Some(d) = dir {
            d
        } else if let Ok(dir) = std::env::current_dir() {
            dir.to_string_lossy().to_string()
        } else {
            ".".to_string()
        };
        
        Self {
            process: None,
            output: Vec::new(),
            input_buffer: String::new(),
            cursor_pos: 0,
            current_dir,
            scroll: 0,
            max_history: 1000,
            name,
        }
    }

    /// 启动终端进程
    pub fn start(&mut self) -> Result<()> {
        // 获取默认shell
        let shell = if cfg!(target_os = "windows") {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
        };
        
        // 添加启动提示
        self.output.push(format!("正在启动终端: {}", shell));
        self.output.push(format!("工作目录: {}", self.current_dir));
        
        // 确保工作目录存在，如果不存在则使用当前目录
        let work_dir = std::path::Path::new(&self.current_dir);
        if !work_dir.exists() || !work_dir.is_dir() {
            // 工作目录不存在或不是目录，使用当前目录
            if let Ok(current_dir) = std::env::current_dir() {
                self.current_dir = current_dir.to_string_lossy().to_string();
                self.output.push(format!("指定的工作目录不存在，使用当前目录: {}", self.current_dir));
            } else {
                // 如果无法获取当前目录，使用系统临时目录
                if let Some(temp_dir) = std::env::temp_dir().to_str() {
                    self.current_dir = temp_dir.to_string();
                    self.output.push(format!("无法获取当前目录，使用临时目录: {}", self.current_dir));
                }
            }
        }
        
        // 创建子进程
        let mut command = Command::new(&shell);
        
        // 设置工作目录
        command.current_dir(&self.current_dir);
        
        // 设置标准输入输出
        command.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // 设置环境变量
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        
        // 传递当前环境变量
        for (key, value) in std::env::vars() {
            if key != "TERM" && key != "COLORTERM" { // 避免覆盖我们设置的TERM相关变量
                command.env(key, value);
            }
        }
        
        // 在Linux/macOS上，添加-l参数使bash作为登录shell启动
        if !cfg!(target_os = "windows") {
            command.arg("-l");
        }
        
        // 启动进程
        match command.spawn() {
            Ok(mut child) => {
                // 获取标准输出和标准错误
                let stdout = match child.stdout.take() {
                    Some(stdout) => stdout,
                    None => {
                        self.output.push("无法获取标准输出".to_string());
                        return Err(FKVimError::TerminalError("无法获取标准输出".to_string()));
                    }
                };
                
                let stderr = match child.stderr.take() {
                    Some(stderr) => stderr,
                    None => {
                        self.output.push("无法获取标准错误".to_string());
                        return Err(FKVimError::TerminalError("无法获取标准错误".to_string()));
                    }
                };
                
                // 创建一个线程安全的输出缓冲区
                let output = Arc::new(Mutex::new(self.output.clone()));
                let output_clone = output.clone();
                let max_history = self.max_history;
                
                // 使用标准库的线程处理标准输出 - 使用字节级读取而不是行缓冲
                let stdout_thread = thread::spawn(move || {
                    let mut reader = BufReader::new(stdout);
                    let mut buffer = [0; 4096]; // 增大缓冲区以处理更多数据
                    let mut line_buffer = String::new();
                    
                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                // 将读取的字节转换为字符串
                                let chunk = String::from_utf8_lossy(&buffer[0..n]).to_string();
                                // 处理每个字符
                                for c in chunk.chars() {
                                    if c == '\n' {
                                        // 行结束，添加到输出
                                        if let Ok(mut output) = output.lock() {
                                            output.push(line_buffer.clone());
                                            // 保持输出历史在合理范围内
                                            if output.len() > max_history {
                                                output.remove(0);
                                            }
                                        }
                                        line_buffer.clear();
                                    } else if c == '\r' {
                                        // 忽略回车符
                                    } else {
                                        line_buffer.push(c);
                                    }
                                }
                            },
                            Err(_) => break, // 读取错误
                        }
                        
                        // 即使没有换行符，也要定期更新输出
                        if !line_buffer.is_empty() {
                            if let Ok(mut output) = output.lock() {
                                // 如果输出不为空且最后一行不是当前行缓冲区，则更新最后一行
                                if !output.is_empty() {
                                    let last_index = output.len() - 1;
                                    output[last_index] = line_buffer.clone();
                                } else {
                                    output.push(line_buffer.clone());
                                }
                                
                                // 保持输出历史在合理范围内
                                if output.len() > max_history {
                                    output.remove(0);
                                }
                            }
                        }
                        
                        // 短暂休眠以避免CPU占用过高
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    
                    // 确保最后一行也被添加（如果没有以换行符结束）
                    if !line_buffer.is_empty() {
                        if let Ok(mut output) = output.lock() {
                            output.push(line_buffer);
                            if output.len() > max_history {
                                output.remove(0);
                            }
                        }
                    }
                });
                
                // 使用标准库的线程处理标准错误 - 使用字节级读取
                let stderr_thread = thread::spawn(move || {
                    let mut reader = BufReader::new(stderr);
                    let mut buffer = [0; 4096]; // 增大缓冲区以处理更多数据
                    let mut line_buffer = String::new();
                    
                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                // 将读取的字节转换为字符串
                                let chunk = String::from_utf8_lossy(&buffer[0..n]).to_string();
                                // 处理每个字符
                                for c in chunk.chars() {
                                    if c == '\n' {
                                        // 行结束，添加到输出
                                        if let Ok(mut output) = output_clone.lock() {
                                            output.push(line_buffer.clone());
                                            // 保持输出历史在合理范围内
                                            if output.len() > max_history {
                                                output.remove(0);
                                            }
                                        }
                                        line_buffer.clear();
                                    } else if c == '\r' {
                                        // 忽略回车符
                                    } else {
                                        line_buffer.push(c);
                                    }
                                }
                            },
                            Err(_) => break, // 读取错误
                        }
                        
                        // 即使没有换行符，也要定期更新输出
                        if !line_buffer.is_empty() {
                            if let Ok(mut output) = output_clone.lock() {
                                // 如果输出不为空且最后一行不是当前行缓冲区，则更新最后一行
                                if !output.is_empty() {
                                    let last_index = output.len() - 1;
                                    output[last_index] = line_buffer.clone();
                                } else {
                                    output.push(line_buffer.clone());
                                }
                                
                                // 保持输出历史在合理范围内
                                if output.len() > max_history {
                                    output.remove(0);
                                }
                            }
                        }
                        
                        // 短暂休眠以避免CPU占用过高
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    
                    // 确保最后一行也被添加（如果没有以换行符结束）
                    if !line_buffer.is_empty() {
                        if let Ok(mut output) = output_clone.lock() {
                            output.push(line_buffer);
                            if output.len() > max_history {
                                output.remove(0);
                            }
                        }
                    }
                });
                
                // 保存进程
                self.process = Some(child);
                
                // 添加成功启动提示
                self.output.push("终端已启动，可以输入命令了".to_string());
                
                Ok(())
            },
            Err(e) => {
                self.output.push(format!("启动终端失败: {}", e));
                Err(FKVimError::IoError(e))
            }
        }
    }
    
    /// 发送命令到终端
    pub fn send_command(&mut self, cmd: &str) -> Result<()> {
        // 如果进程不存在，尝试启动
        if self.process.is_none() {
            self.start()?;
        }
        
        // 发送命令到终端进程
        if let Some(ref mut child) = self.process {
            if let Some(stdin) = child.stdin.as_mut() {
                // 添加换行符确保命令被执行
                let cmd_with_newline = format!("{}\n", cmd);
                
                // 写入命令到标准输入
                stdin.write_all(cmd_with_newline.as_bytes())?;
                stdin.flush()?;
                
                // 不再重复添加到输出历史，因为在handle_key中已经添加过了
                
                return Ok(());
            }
        }
        
        // 如果无法发送命令，添加错误信息
        self.output.push("无法发送命令到终端进程".to_string());
        Err(FKVimError::TerminalError("无法发送命令到终端进程".to_string()))
    }
    
    /// 发送文本到终端
    pub fn send_text(&mut self, text: &str) -> Result<()> {
        // 直接发送命令到终端进程
        self.send_command(text)
    }
    
    /// 处理终端的键盘输入
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        let mut handled = false;
        
        match key.code {
            KeyCode::Enter => {
                // 执行命令
                let cmd = self.input_buffer.clone();
                self.output.push(format!("> {}", cmd));
                self.input_buffer.clear();
                self.cursor_pos = 0;
                
                // 发送命令到终端进程
                self.send_command(&cmd)?;
                handled = true;
            },
            KeyCode::Backspace => {
                // 删除光标前的字符
                if self.cursor_pos > 0 {
                    self.input_buffer.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
                handled = true;
            },
            KeyCode::Delete => {
                // 删除光标处的字符
                if self.cursor_pos < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_pos);
                }
                handled = true;
            },
            KeyCode::Left => {
                // 光标左移
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                handled = true;
            },
            KeyCode::Right => {
                // 光标右移
                if self.cursor_pos < self.input_buffer.len() {
                    self.cursor_pos += 1;
                }
                handled = true;
            },
            KeyCode::Up => {
                // 滚动终端历史向上
                if self.scroll < self.output.len() {
                    self.scroll += 1;
                }
                handled = true;
            },
            KeyCode::Down => {
                // 滚动终端历史向下
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
                handled = true;
            },
            KeyCode::Home => {
                // 光标移动到行首
                self.cursor_pos = 0;
                handled = true;
            },
            KeyCode::End => {
                // 光标移动到行尾
                self.cursor_pos = self.input_buffer.len();
                handled = true;
            },
            KeyCode::PageUp => {
                // 向上翻页
                let page_size = 10;
                if self.scroll + page_size < self.output.len() {
                    self.scroll += page_size;
                } else {
                    self.scroll = self.output.len();
                }
                handled = true;
            },
            KeyCode::PageDown => {
                // 向下翻页
                let page_size = 10;
                if self.scroll > page_size {
                    self.scroll -= page_size;
                } else {
                    self.scroll = 0;
                }
                handled = true;
            },
            KeyCode::Tab => {
                // 插入Tab字符
                self.input_buffer.insert_str(self.cursor_pos, "    ");
                self.cursor_pos += 4;
                handled = true;
            },
            KeyCode::Char(c) => {
                // 插入字符
                self.input_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                handled = true;
            },
            _ => {}
        }
        
        Ok(handled)
    }
    
    /// 处理从子进程接收的输出
    pub fn process_output(&mut self, line: String) {
        self.output.push(line);
        if self.output.len() > self.max_history {
            self.output.remove(0);
        }
    }
    
    /// 获取可见行
    pub fn visible_lines(&self, height: u16) -> Vec<&String> {
        let start = self.scroll;
        let end = (start + height as usize).min(self.output.len());
        self.output[start..end].iter().collect()
    }
    
    /// 关闭终端会话
    pub fn close(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            child.kill().map_err(|e| FKVimError::IoError(e))?;
        }
        
        self.output.clear();
        self.input_buffer.clear();
        
        Ok(())
    }
    
    /// 获取光标位置
    pub fn get_cursor_position(&self) -> (usize, usize) {
        // 计算光标在输入行的位置，包括提示符"> "的长度
        let prompt_len = 2; // 提示符"> "的长度
        let cursor_x = prompt_len + self.cursor_pos; 
        
        // 计算光标的Y位置 - 应该在最后一行
        // 如果有滚动，需要考虑滚动的影响
        let visible_lines = self.output.len().saturating_sub(self.scroll);
        
        (cursor_x, visible_lines)
    }
    
    /// 获取可见行
    pub fn get_visible_lines(&self, visible_height: usize) -> Vec<String> {
        if self.output.is_empty() {
            return vec!["终端已启动，等待输入...".to_string(), format!("> {}", self.input_buffer)];
        }
        
        // 计算可见范围，考虑滚动位置
        let start = if self.scroll < self.output.len() {
            self.scroll
        } else {
            0
        };
        
        let end = (start + visible_height - 1).min(self.output.len());
        
        let mut result = if start < end {
            self.output[start..end].to_vec()
        } else {
            Vec::new()
        };
        
        // 添加当前的输入行
        result.push(format!("> {}", self.input_buffer));
        
        result
    }

    /// 向上滚动终端
    pub fn scroll_up(&mut self, lines: usize) {
        if self.scroll + lines < self.output.len() {
            self.scroll += lines;
        } else {
            self.scroll = self.output.len() - 1;
        }
    }

    /// 向下滚动终端
    pub fn scroll_down(&mut self, lines: usize) {
        if self.scroll >= lines {
            self.scroll -= lines;
        } else {
            self.scroll = 0;
        }
    }

    /// 清空终端内容
    pub fn clear(&mut self) {
        self.output.clear();
        self.scroll = 0;
    }

    /// 重启终端
    pub async fn restart(&mut self) -> Result<()> {
        // 如果存在进程，先关闭它
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
        
        // 清空输出和输入
        self.output.clear();
        self.input_buffer.clear();
        
        // 重新启动终端
        self.start()?;
        
        Ok(())
    }

    /// 同步终端输出
    pub fn sync_output(&mut self) -> Result<()> {
        // 如果进程不存在，不需要同步
        if self.process.is_none() {
            return Ok(());
        }
        
        // 检查子进程是否还在运行
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // 进程已结束
                    self.output.push(format!("进程已退出，退出码: {:?}", status.code()));
                    self.process = None;
                },
                Ok(None) => {
                    // 进程仍在运行，不做任何事
                    // 此处可以添加额外的输出同步逻辑，但由于我们已经在线程中处理了输出，
                    // 所以这里不需要额外的操作
                },
                Err(e) => {
                    // 检查进程状态出错
                    self.output.push(format!("检查进程状态出错: {}", e));
                    self.process = None;
                }
            }
        }
        
        Ok(())
    }
}

/// 终端分屏布局类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalLayout {
    /// 单个全宽终端
    Single,
    /// 水平分割（上下布局）
    Horizontal,
    /// 垂直分割（左右布局）
    Vertical,
    /// 四象限分割
    Grid,
}

/// 表示集成终端的状态
pub struct Terminal {
    /// 终端会话映射表
    sessions: HashMap<String, TerminalSession>,
    /// 分屏布局中的会话ID列表
    layout_sessions: Vec<String>,
    /// 终端是否可见
    pub visible: bool,
    /// 终端高度
    pub height: Option<u16>,
    /// 当前活动的会话ID
    active_session: Option<String>,
    /// 终端分屏布局
    pub layout: TerminalLayout,
    /// 终端标签页列表
    tabs: Vec<String>,
    /// 当前活动的标签页索引
    active_tab: usize,
}

impl Terminal {
    /// 创建一个新的终端实例
    pub fn new() -> Self {
        let mut terminal = Terminal {
            sessions: HashMap::new(),
            layout_sessions: Vec::new(),
            visible: false,
            height: Some(10), // 默认高度
            active_session: None,
            layout: TerminalLayout::Single,
            tabs: Vec::new(),
            active_tab: 0,
        };
        
        // 创建默认标签页和会话
        terminal.create_new_tab("Terminal 1".to_string());
        
        terminal
    }
    
    /// 处理键盘输入
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if let Some(session) = self.get_active_session_mut() {
            return session.handle_key(key);
        }
        
        Ok(false)
    }

    /// 创建一个新的标签页
    pub fn create_new_tab(&mut self, name: String) -> String {
        let tab_name = if self.tabs.contains(&name) {
            format!("{} ({})", name, self.tabs.len() + 1)
        } else {
            name.clone()
        };
        
        self.tabs.push(tab_name.clone());
        self.active_tab = self.tabs.len() - 1;
        
        // 为新标签页创建一个默认会话
        let session_id = format!("{}:0", tab_name);
        let session = TerminalSession::new(tab_name.clone(), None);
        self.sessions.insert(session_id.clone(), session);
        
        // 更新布局会话列表
        self.layout_sessions = vec![session_id.clone()];
        self.active_session = Some(session_id.clone());
        
        tab_name
    }

    /// 切换到指定标签页
    pub fn switch_tab(&mut self, index: usize) -> Result<()> {
        if index < self.tabs.len() {
            self.active_tab = index;
            
            // 更新活动会话和布局会话列表
            let tab_name = &self.tabs[index];
            let session_ids: Vec<String> = self.sessions.keys()
                .filter(|id| id.starts_with(&format!("{}:", tab_name)))
                .cloned()
                .collect();
            
            if !session_ids.is_empty() {
                self.layout_sessions = session_ids;
                self.active_session = Some(self.layout_sessions[0].clone());
            } else {
                // 如果没有找到会话，创建一个新会话
                let session_id = format!("{}:0", tab_name);
                let session = TerminalSession::new(tab_name.clone(), None);
                self.sessions.insert(session_id.clone(), session);
                self.layout_sessions = vec![session_id.clone()];
                self.active_session = Some(session_id);
            }
            
            Ok(())
        } else {
            Err(FKVimError::Generic(format!("标签页索引 {} 超出范围", index)))
        }
    }

    /// 关闭当前标签页
    pub fn close_current_tab(&mut self) -> Result<()> {
        if self.tabs.len() <= 1 {
            return Err(FKVimError::Generic("不能关闭最后一个标签页".to_string()));
        }
        
        let tab_name = &self.tabs[self.active_tab];
        
        // 关闭并删除此标签页的所有会话
        let session_ids: Vec<String> = self.sessions.keys()
            .filter(|id| id.starts_with(&format!("{}:", tab_name)))
            .cloned()
            .collect();
        
        for id in session_ids {
            if let Some(mut session) = self.sessions.remove(&id) {
                let _ = session.close();
            }
        }
        
        // 删除标签页
        self.tabs.remove(self.active_tab);
        
        // 更新活动标签页索引
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        
        // 切换到新的活动标签页
        self.switch_tab(self.active_tab)
    }

    /// 重命名当前标签页
    pub fn rename_current_tab(&mut self, new_name: String) -> Result<()> {
        if self.tabs.is_empty() {
            return Err(FKVimError::Generic("没有活动的标签页".to_string()));
        }
        
        let old_name = self.tabs[self.active_tab].clone();
        self.tabs[self.active_tab] = new_name.clone();
        
        // 更新会话的标签页前缀
        let old_sessions: Vec<(String, TerminalSession)> = self.sessions.iter()
            .filter_map(|(id, session)| {
                if id.starts_with(&format!("{}:", old_name)) {
                    let suffix = id.split(':').nth(1).unwrap_or("0");
                    let new_id = format!("{}:{}", new_name, suffix);
                    Some((new_id, session.clone()))
                } else {
                    None
                }
            })
            .collect();
        
        // 删除旧会话并添加新会话
        for (old_id, _) in old_sessions.iter() {
            if old_id.starts_with(&format!("{}:", old_name)) {
                self.sessions.remove(old_id);
            }
        }
        
        for (new_id, session) in old_sessions {
            self.sessions.insert(new_id, session);
        }
        
        // 更新布局会话列表和活动会话
        self.layout_sessions = self.layout_sessions.iter()
            .map(|id| {
                if id.starts_with(&format!("{}:", old_name)) {
                    let suffix = id.split(':').nth(1).unwrap_or("0");
                    format!("{}:{}", new_name, suffix)
                } else {
                    id.clone()
                }
            })
            .collect();
        
        if let Some(active_id) = &self.active_session {
            if active_id.starts_with(&format!("{}:", old_name)) {
                let suffix = active_id.split(':').nth(1).unwrap_or("0");
                self.active_session = Some(format!("{}:{}", new_name, suffix));
            }
        }
        
        Ok(())
    }

    /// 设置终端分屏布局
    pub fn set_layout(&mut self, layout: TerminalLayout) -> Result<()> {
        self.layout = layout;
        
        // 确保有足够的会话用于布局
        let required_sessions = match layout {
            TerminalLayout::Single => 1,
            TerminalLayout::Horizontal | TerminalLayout::Vertical => 2,
            TerminalLayout::Grid => 4,
        };
        
        let tab_name = &self.tabs[self.active_tab];
        let current_sessions = self.layout_sessions.len();
        
        // 如果需要更多会话，创建它们
        if current_sessions < required_sessions {
            for i in current_sessions..required_sessions {
                let session_id = format!("{}:{}", tab_name, i);
                if !self.sessions.contains_key(&session_id) {
                    let session = TerminalSession::new(format!("{} #{}", tab_name, i+1), None);
                    self.sessions.insert(session_id.clone(), session);
                }
                
                if !self.layout_sessions.contains(&session_id) {
                    self.layout_sessions.push(session_id);
                }
            }
        }
        
        // 启动所有分屏会话
        for session_id in &self.layout_sessions[0..required_sessions] {
            if let Some(session) = self.sessions.get_mut(session_id) {
                if session.process.is_none() {
                    let _ = session.start(); // 注意：这里应该处理异步启动，但为简化暂时忽略
                }
            }
        }
        
        Ok(())
    }

    /// 切换到下一个布局中的会话
    pub fn next_session(&mut self) -> Result<()> {
        if self.layout_sessions.is_empty() {
            return Err(FKVimError::Generic("没有可用的会话".to_string()));
        }
        
        if let Some(active_id) = &self.active_session {
            if let Some(pos) = self.layout_sessions.iter().position(|id| id == active_id) {
                let next_pos = (pos + 1) % self.layout_sessions.len();
                self.active_session = Some(self.layout_sessions[next_pos].clone());
            }
        } else {
            self.active_session = Some(self.layout_sessions[0].clone());
        }
        
        Ok(())
    }

    /// 切换到上一个布局中的会话
    pub fn prev_session(&mut self) -> Result<()> {
        if self.layout_sessions.is_empty() {
            return Err(FKVimError::Generic("没有可用的会话".to_string()));
        }
        
        if let Some(active_id) = &self.active_session {
            if let Some(pos) = self.layout_sessions.iter().position(|id| id == active_id) {
                let next_pos = if pos == 0 {
                    self.layout_sessions.len() - 1
                } else {
                    pos - 1
                };
                self.active_session = Some(self.layout_sessions[next_pos].clone());
            }
        } else {
            self.active_session = Some(self.layout_sessions[0].clone());
        }
        
        Ok(())
    }

    /// 启动终端进程
    pub fn start(&mut self) -> Result<()> {
        // 获取活动会话并启动
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                if session.process.is_none() {
                    session.start()?;
                }
            }
        }
        
        Ok(())
    }
    
    /// 获取可见行
    pub fn visible_lines(&self) -> Vec<&String> {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get(active_id) {
                return session.visible_lines(self.height.unwrap_or(10));
            }
        }
        
        vec![]
    }
    
    /// 调整终端高度
    pub fn resize(&mut self, height: u16) {
        self.height = Some(height);
    }
    
    /// 切换终端可见性
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }
    
    /// 关闭终端
    pub fn close(&mut self) -> Result<()> {
        for (_, session) in self.sessions.iter_mut() {
            session.close()?;
        }
        
        self.sessions.clear();
        self.layout_sessions.clear();
        self.active_session = None;
        self.visible = false;
        
        // 重新创建默认会话
        let tab_name = self.tabs[self.active_tab].clone();
        let session_id = format!("{}:0", tab_name);
        let session = TerminalSession::new(tab_name, None);
        self.sessions.insert(session_id.clone(), session);
        self.layout_sessions = vec![session_id.clone()];
        self.active_session = Some(session_id);
        
        Ok(())
    }

    /// 终端是否可见
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    
    /// 终端是否有焦点
    pub fn has_focus(&self) -> bool {
        self.visible && self.active_session.is_some()
    }
    
    /// 获取光标位置
    pub fn get_cursor_position(&self) -> (usize, usize) {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get(active_id) {
                return session.get_cursor_position();
            }
        }
        
        (0, 0)
    }
    
    /// 获取可见行
    pub fn get_visible_lines(&self, visible_height: usize) -> Vec<String> {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get(active_id) {
                return session.get_visible_lines(visible_height);
            }
        }
        
        vec!["终端未启动或无活动会话".to_string()]
    }
    
    /// 设置终端的高度
    pub fn set_height(&mut self, height: Option<u16>) {
        self.height = height;
    }

    /// 向上滚动终端
    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                session.scroll_up(lines);
            }
        }
    }

    /// 向下滚动终端
    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                session.scroll_down(lines);
            }
        }
    }

    /// 清空终端内容
    pub fn clear(&mut self) {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                session.clear();
            }
        }
    }

    /// 重启终端
    pub fn restart(&mut self) -> Result<()> {
        // 重启当前会话
        if let Some(active_tab) = self.tabs.get(self.active_tab).cloned() {
            if let Some(session) = self.sessions.get_mut(&active_tab) {
                // 如果存在进程，先关闭它
                if let Some(mut process) = session.process.take() {
                    let _ = process.kill();
                }
                
                // 清空输出和输入
                session.output.clear();
                session.input_buffer.clear();
                
                // 重新启动终端
                session.start()?;
            }
        }
        
        Ok(())
    }

    /// 获取当前标签页名称
    pub fn get_current_tab_name(&self) -> Option<String> {
        if self.active_tab < self.tabs.len() {
            Some(self.tabs[self.active_tab].clone())
        } else {
            None
        }
    }

    /// 获取所有标签页名称
    pub fn get_tab_names(&self) -> Vec<String> {
        self.tabs.clone()
    }

    /// 获取标签页数量
    pub fn get_tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// 获取当前活动会话
    pub fn get_active_session(&self) -> Option<&TerminalSession> {
        if let Some(active_id) = &self.active_session {
            self.sessions.get(active_id)
        } else {
            None
        }
    }

    /// 获取当前活动会话（可变引用）
    pub fn get_active_session_mut(&mut self) -> Option<&mut TerminalSession> {
        if let Some(active_id) = &self.active_session {
            let id = active_id.clone();
            self.sessions.get_mut(&id)
        } else {
            None
        }
    }

    /// 获取所有布局会话
    pub fn get_layout_sessions(&self) -> Vec<&TerminalSession> {
        self.layout_sessions.iter()
            .filter_map(|id| self.sessions.get(id))
            .collect()
    }

    /// 检查会话是否是活动会话
    pub fn is_active_session(&self, session_id: &str) -> bool {
        if let Some(active_id) = &self.active_session {
            active_id == session_id
        } else {
            false
        }
    }

    /// 初始化终端，并指定工作目录
    pub fn init_with_dir(&mut self, dir: Option<std::path::PathBuf>) -> Result<()> {
        // 初始化终端设置
        if self.tabs.is_empty() {
            self.tabs.push("默认".to_string());
        }
        
        // 将目录转换为字符串，确保目录存在
        let dir_str = if let Some(path) = dir {
            if path.exists() && path.is_dir() {
                path.to_str().map(|s| s.to_string())
            } else {
                // 目录不存在，尝试使用当前目录
                if let Ok(current_dir) = std::env::current_dir() {
                    current_dir.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            }
        } else {
            // 没有指定目录，使用当前目录
            if let Ok(current_dir) = std::env::current_dir() {
                current_dir.to_str().map(|s| s.to_string())
            } else {
                None
            }
        };
        
        // 确保有默认会话
        if self.sessions.is_empty() {
            let session_id = format!("{}:0", self.tabs[0]);
            let session = TerminalSession::new(self.tabs[0].clone(), dir_str);
            self.sessions.insert(session_id.clone(), session);
            self.layout_sessions = vec![session_id.clone()];
            self.active_session = Some(session_id);
        } else if let Some(active_id) = &self.active_session {
            // 更新现有会话的工作目录
            if let Some(session) = self.sessions.get_mut(active_id) {
                if let Some(dir) = dir_str {
                    session.current_dir = dir;
                }
            }
        }
        
        if self.active_session.is_none() && !self.layout_sessions.is_empty() {
            self.active_session = Some(self.layout_sessions[0].clone());
        }
        
        // 启动活动会话的终端进程
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                match session.start() {
                    Ok(_) => (),
                    Err(e) => {
                        // 记录错误但继续运行
                        session.output.push(format!("终端启动失败: {}", e));
                        return Err(e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// 将焦点设置到终端
    pub fn focus(&mut self) {
        self.visible = true;
    }
    
    /// 取消终端焦点
    pub fn unfocus(&mut self) {
        self.visible = false;
    }
    
    /// 切换到下一个标签页
    pub fn next_tab(&mut self) -> Result<()> {
        if self.tabs.is_empty() {
            return Ok(());
        }
        
        let next_tab = if self.active_tab + 1 >= self.tabs.len() {
            0
        } else {
            self.active_tab + 1
        };
        
        self.active_tab = next_tab;
        
        // 更新活动会话
        let tab_name = &self.tabs[self.active_tab];
        let session_id = format!("{}:0", tab_name);
        
        // 确保会话存在
        if !self.sessions.contains_key(&session_id) {
            let session = TerminalSession::new(tab_name.clone(), None);
            self.sessions.insert(session_id.clone(), session);
            self.layout_sessions = vec![session_id.clone()];
        }
        
        self.active_session = Some(session_id);
        
        Ok(())
    }
    
    /// 切换到上一个标签页
    pub fn prev_tab(&mut self) -> Result<()> {
        if self.tabs.is_empty() {
            return Ok(());
        }
        
        let prev_tab = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        
        self.active_tab = prev_tab;
        
        // 更新活动会话
        let tab_name = &self.tabs[self.active_tab];
        let session_id = format!("{}:0", tab_name);
        
        // 确保会话存在
        if !self.sessions.contains_key(&session_id) {
            let session = TerminalSession::new(tab_name.clone(), None);
            self.sessions.insert(session_id.clone(), session);
            self.layout_sessions = vec![session_id.clone()];
        }
        
        self.active_session = Some(session_id);
        
        Ok(())
    }
    
    /// 切换到下一个分割窗口
    pub fn next_split(&mut self) -> Result<()> {
        if self.layout_sessions.is_empty() {
            return Ok(());
        }
        
        // 找到当前会话的索引
        let current_index = if let Some(active_id) = &self.active_session {
            self.layout_sessions.iter().position(|id| id == active_id).unwrap_or(0)
        } else {
            0
        };
        
        // 计算下一个会话索引
        let next_index = if current_index + 1 >= self.layout_sessions.len() {
            0
        } else {
            current_index + 1
        };
        
        // 更新活动会话
        self.active_session = Some(self.layout_sessions[next_index].clone());
        
        Ok(())
    }
    
    /// 创建新的标签页
    pub fn create_tab(&mut self, name: String) -> Result<()> {
        // 检查名称是否已存在
        if self.tabs.contains(&name) {
            return Err(FKVimError::TerminalError(format!("标签页 {} 已存在", name)));
        }
        
        // 添加新标签页
        self.tabs.push(name.clone());
        
        // 创建默认会话
        let session_id = format!("{}:0", name);
        let session = TerminalSession::new(name.clone(), None);
        self.sessions.insert(session_id.clone(), session);
        
        // 切换到新标签页
        self.active_tab = self.tabs.len() - 1;
        self.layout_sessions = vec![session_id.clone()];
        self.active_session = Some(session_id);
        
        Ok(())
    }

    /// 初始化终端
    pub fn init(&mut self) -> Result<()> {
        self.init_with_dir(None)
    }

    /// 同步终端输出
    pub fn sync_output(&mut self) -> Result<()> {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                return session.sync_output();
            }
        }
        
        Ok(())
    }

    /// 发送文本到终端
    pub fn send_text(&mut self, text: &str) -> Result<()> {
        if let Some(session) = self.get_active_session_mut() {
            // 直接在会话中发送命令
            return session.send_text(text);
        } else if !self.sessions.is_empty() {
            // 如果没有活动会话但有会话存在，使用第一个会话
            let first_key = self.sessions.keys().next().unwrap().clone();
            self.active_session = Some(first_key.clone());
            if let Some(session) = self.sessions.get_mut(&first_key) {
                return session.send_text(text);
            }
        } else {
            // 如果没有会话，创建一个新会话
            self.add_session("默认")?;
            if let Some(active_id) = &self.active_session {
                if let Some(session) = self.sessions.get_mut(active_id) {
                    return session.send_text(text);
                }
            }
        }
        
        Err(FKVimError::TerminalError("无法发送文本到终端".to_string()))
    }

    /// 添加一个新的终端会话
    pub fn add_session(&mut self, name: &str) -> Result<String> {
        let session_id = format!("{}:{}", name, self.sessions.len());
        let session = TerminalSession::new(name.to_string(), None);
        self.sessions.insert(session_id.clone(), session);
        
        if self.active_session.is_none() {
            self.active_session = Some(session_id.clone());
            self.layout_sessions.push(session_id.clone());
        }
        
        Ok(session_id)
    }
}