// filepath: /home/joyin/桌面/fkvim/src/ui/components/terminal.rs
use std::io::Stdout;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};
use crate::terminal::{Terminal, TerminalLayout, TerminalSession};

/// 终端UI组件，负责渲染终端标签页和分屏
pub struct TerminalComponent {}

impl TerminalComponent {
    /// 创建一个新的终端组件
    pub fn new() -> Self {
        TerminalComponent {}
    }
    
    /// 渲染终端UI
    pub fn render(&self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect, terminal: &Terminal) {
        if !terminal.is_visible() {
            return;
        }
        
        // 创建标签页和终端区域
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // 标签页高度
                Constraint::Min(3),    // 终端内容区域
            ])
            .split(area);
        
        // 渲染标签页
        self.render_tabs(f, chunks[0], terminal);
        
        // 根据当前布局渲染终端会话
        match terminal.layout {
            TerminalLayout::Single => {
                self.render_single_terminal(f, chunks[1], terminal);
            },
            TerminalLayout::Horizontal => {
                self.render_horizontal_split(f, chunks[1], terminal);
            },
            TerminalLayout::Vertical => {
                self.render_vertical_split(f, chunks[1], terminal);
            },
            TerminalLayout::Grid => {
                self.render_grid_split(f, chunks[1], terminal);
            },
        }
    }
    
    /// 渲染终端标签页
    fn render_tabs(&self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect, terminal: &Terminal) {
        let tab_names = terminal.get_tab_names();
        
        let tabs = Tabs::new(
            tab_names.iter().map(|name| {
                Spans::from(vec![Span::styled(
                    format!(" {} ", name),
                    Style::default().fg(Color::White),
                )])
            }).collect()
        )
        .block(Block::default().borders(Borders::ALL).title("终端标签页"))
        .select(terminal.active_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        );
        
        f.render_widget(tabs, area);
    }
    
    /// 渲染单个终端
    fn render_single_terminal(&self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect, terminal: &Terminal) {
        if let Some(session) = terminal.get_layout_sessions().first() {
            self.render_terminal_session(f, area, session, terminal);
        }
    }
    
    /// 渲染水平分割（上下布局）
    fn render_horizontal_split(&self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect, terminal: &Terminal) {
        let sessions = terminal.get_layout_sessions();
        if sessions.len() < 2 {
            // 如果会话不足，按单个终端渲染
            self.render_single_terminal(f, area, terminal);
            return;
        }
        
        // 上下分割区域
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(area);
        
        // 渲染两个终端会话
        self.render_terminal_session(f, chunks[0], sessions[0], terminal);
        self.render_terminal_session(f, chunks[1], sessions[1], terminal);
    }
    
    /// 渲染垂直分割（左右布局）
    fn render_vertical_split(&self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect, terminal: &Terminal) {
        let sessions = terminal.get_layout_sessions();
        if sessions.len() < 2 {
            // 如果会话不足，按单个终端渲染
            self.render_single_terminal(f, area, terminal);
            return;
        }
        
        // 左右分割区域
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(area);
        
        // 渲染两个终端会话
        self.render_terminal_session(f, chunks[0], sessions[0], terminal);
        self.render_terminal_session(f, chunks[1], sessions[1], terminal);
    }
    
    /// 渲染网格分割（四象限）
    fn render_grid_split(&self, f: &mut Frame<CrosstermBackend<Stdout>>, area: Rect, terminal: &Terminal) {
        let sessions = terminal.get_layout_sessions();
        if sessions.len() < 4 {
            // 如果会话不足4个，根据实际数量选择其他布局
            match sessions.len() {
                0 => { return; },
                1 => {
                    self.render_single_terminal(f, area, terminal);
                    return;
                },
                2 | 3 => {
                    // 对于2或3个会话，使用上下分割后，下半部分再左右分割
                    let vertical_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Ratio(1, 2),
                            Constraint::Ratio(1, 2),
                        ])
                        .split(area);
                    
                    // 上半部分放第一个会话
                    self.render_terminal_session(f, vertical_chunks[0], sessions[0], terminal);
                    
                    // 下半部分水平分割
                    let horizontal_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Ratio(1, 2),
                            Constraint::Ratio(1, 2),
                        ])
                        .split(vertical_chunks[1]);
                    
                    // 下半部分左侧放第二个会话
                    self.render_terminal_session(f, horizontal_chunks[0], sessions[1], terminal);
                    
                    // 如果有第三个会话，放在下半部分右侧
                    if sessions.len() >= 3 {
                        self.render_terminal_session(f, horizontal_chunks[1], sessions[2], terminal);
                    }
                    
                    return;
                },
                _ => {}
            }
        }
        
        // 创建2x2网格布局
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(area);
        
        let left_vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(horizontal_chunks[0]);
        
        let right_vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(horizontal_chunks[1]);
        
        // 渲染四个象限的终端
        self.render_terminal_session(f, left_vertical_chunks[0], sessions[0], terminal);
        self.render_terminal_session(f, left_vertical_chunks[1], sessions[1], terminal);
        self.render_terminal_session(f, right_vertical_chunks[0], sessions[2], terminal);
        self.render_terminal_session(f, right_vertical_chunks[1], sessions[3], terminal);
    }
    
    /// 渲染单个终端会话
    fn render_terminal_session(
        &self,
        f: &mut Frame<CrosstermBackend<Stdout>>,
        area: Rect,
        session: &TerminalSession,
        terminal: &Terminal
    ) {
        // 获取会话ID，用于确定是否是活动会话
        let session_id = format!("{}:{}", terminal.get_current_tab_name().unwrap_or_default(), session.name);
        let is_active = terminal.is_active_session(&session_id);
        
        // 创建边框样式，活动会话高亮显示
        let block = Block::default()
            .title(format!(" {} ", session.name))
            .borders(Borders::ALL)
            .border_style(
                if is_active {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                }
            );
        
        // 获取终端内容区域
        let inner_area = block.inner(area);
        
        // 获取终端可见内容
        let content = session.get_visible_lines(inner_area.height as usize);
        
        // 创建段落组件来显示终端内容
        let paragraph = Paragraph::new(
            content.iter().map(|line| {
                Spans::from(Span::styled(line, Style::default().fg(Color::White)))
            }).collect::<Vec<Spans>>()
        )
        .block(block)
        .style(Style::default().fg(Color::White).bg(Color::Black));
        
        // 渲染终端内容
        f.render_widget(paragraph, area);
        
        // 如果是活动会话，还需要渲染光标
        if is_active {
            // 获取光标位置
            let (cursor_x, cursor_y) = session.get_cursor_position();
            let cursor_x = cursor_x as u16 + inner_area.x;
            let cursor_y = cursor_y as u16 + inner_area.y;
            
            // 确保光标在可见区域内
            if cursor_x >= inner_area.x && cursor_x < inner_area.x + inner_area.width &&
               cursor_y >= inner_area.y && cursor_y < inner_area.y + inner_area.height {
                // 设置光标位置
                f.set_cursor(cursor_x, cursor_y);
            }
        }
    }
}