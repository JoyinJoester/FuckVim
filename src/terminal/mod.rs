use std::io::{Write, BufRead, BufReader};
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
        TerminalSession {
            process: None,
            output: Vec::new(),
            input_buffer: String::new(),
            current_dir: dir.unwrap_or_else(|| std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()),
            scroll: 0,
            max_history: 1000,
            name,
        }
    }

    /// 启动终端进程
    pub fn start(&mut self) -> Result<()> {
        let shell = if cfg!(target_os = "windows") {
            "cmd.exe"
        } else {
            "bash"
        };
        
        // 创建子进程
        let mut child = Command::new(shell)
            .current_dir(&self.current_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| FKVimError::IoError(e))?;
        
        // 获取标准输出和标准错误
        let stdout = child.stdout.take()
            .ok_or_else(|| FKVimError::TerminalError("无法获取标准输出".to_string()))?;
        
        let stderr = child.stderr.take()
            .ok_or_else(|| FKVimError::TerminalError("无法获取标准错误".to_string()))?;
        
        // 创建一个线程安全的输出缓冲区
        let output = Arc::new(Mutex::new(self.output.clone()));
        let output_clone = output.clone();
        let max_history = self.max_history;
        
        // 使用标准库的线程处理标准输出
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // 添加到输出缓冲区
                    if let Ok(mut output) = output.lock() {
                        output.push(line.clone());
                        // 保持输出历史在合理范围内
                        if output.len() > max_history {
                            output.remove(0);
                        }
                    }
                    println!("{}", line); // 调试输出
                }
            }
        });
        
        // 使用标准库的线程处理标准错误
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // 添加到输出缓冲区
                    let error_line = format!("错误: {}", line);
                    if let Ok(mut output) = output_clone.lock() {
                        output.push(error_line.clone());
                        // 保持输出历史在合理范围内
                        if output.len() > max_history {
                            output.remove(0);
                        }
                    }
                    eprintln!("{}", error_line); // 调试输出
                }
            }
        });
        
        // 保存进程
        self.process = Some(child);
        
        Ok(())
    }
    
    /// 发送命令到终端
    pub fn send_command(&mut self, height: u16) -> Result<()> {
        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                writeln!(stdin, "{}", self.input_buffer)
                    .map_err(|e| FKVimError::IoError(e))?;
                
                // 添加命令到输出历史
                self.output.push(format!("> {}", self.input_buffer));
                if self.output.len() > self.max_history {
                    self.output.remove(0);
                }
                
                // 清空输入缓冲
                self.input_buffer.clear();
                
                // 自动滚动到底部
                self.scroll = self.output.len().saturating_sub(height as usize);
            }
        }
        
        Ok(())
    }
    
    /// 处理终端的键盘输入
    pub fn handle_key(&mut self, key: KeyEvent, height: u16) -> Result<bool> {
        match key.code {
            KeyCode::Enter => {
                self.send_command(height)?;
                Ok(true)
            },
            KeyCode::Backspace => {
                self.input_buffer.pop();
                Ok(true)
            },
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                Ok(true)
            },
            KeyCode::Esc => {
                Ok(false) // 返回false表示退出终端模式
            },
            KeyCode::Up => {
                if self.scroll > 0 {
                    self.scroll -= 1;
                }
                Ok(true)
            },
            KeyCode::Down => {
                if self.scroll < self.output.len().saturating_sub(height as usize) {
                    self.scroll += 1;
                }
                Ok(true)
            },
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(height as usize);
                Ok(true)
            },
            KeyCode::PageDown => {
                let max_scroll = self.output.len().saturating_sub(height as usize);
                self.scroll = (self.scroll + height as usize).min(max_scroll);
                Ok(true)
            },
            _ => Ok(true),
        }
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
        // 计算光标在输入行的位置
        let cursor_x = self.input_buffer.len();
        // 光标总是在最后一行
        let cursor_y = if self.output.is_empty() { 0 } else { self.output.len() - self.scroll };
        (cursor_x, cursor_y)
    }
    
    /// 获取可见行
    pub fn get_visible_lines(&self, visible_height: usize) -> Vec<String> {
        let end = self.output.len();
        if end == 0 {
            return vec!["终端已启动，等待输入...".to_string()];
        }
        
        let start = if end > visible_height {
            end - visible_height
        } else {
             0
        };
        
        let mut result = self.output[start..end].to_vec();
        
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
        if let Some(active_tab) = self.tabs.get(self.active_tab) {
            if let Some(session) = self.sessions.get_mut(active_tab) {
                session.start()?;
            }
        }
        
        Ok(())
    }
    
    /// 发送文本到终端
    pub fn send_text(&mut self, text: &str) -> Result<()> {
        let height = self.height.unwrap_or(10);
        if let Some(session) = self.get_active_session_mut() {
            // 由于 TerminalSession 没有 send_text 方法，我们使用 send_command 方法
            session.input_buffer = text.to_string();
            return session.send_command(height);
        }
        
        Err(FKVimError::TerminalError("没有活动的终端会话".to_string()))
    }

    /// 发送命令到终端
    pub fn send_command(&mut self, height: u16) -> Result<()> {
        if let Some(session) = self.get_active_session_mut() {
            return session.send_command(height);
        }
        
        Err(FKVimError::TerminalError("没有活动的终端会话".to_string()))
    }
    
    /// 处理终端的键盘输入
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if let Some(active_id) = &self.active_session {
            if let Some(session) = self.sessions.get_mut(active_id) {
                return session.handle_key(key, self.height.unwrap_or(10));
            }
        }
        
        Ok(false)
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

    /// 初始化终端
    pub fn init(&mut self) -> Result<()> {
        // 初始化终端设置
        if self.tabs.is_empty() {
            self.tabs.push("默认".to_string());
        }
        
        // 确保有默认会话
        if self.sessions.is_empty() {
            let session_id = format!("{}:0", self.tabs[0]);
            let session = TerminalSession::new(self.tabs[0].clone(), None);
            self.sessions.insert(session_id.clone(), session);
            self.layout_sessions = vec![session_id.clone()];
            self.active_session = Some(session_id);
        }
        
        if self.active_session.is_none() && !self.layout_sessions.is_empty() {
            self.active_session = Some(self.layout_sessions[0].clone());
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
}