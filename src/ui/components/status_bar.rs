use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::editor::{Editor, EditorMode, EditorStatus, StatusMessageType};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Local};

/// 绘制增强的状态栏
pub fn draw_status_bar<B: Backend>(
    f: &mut Frame<B>,
    editor: &Editor,
    area: Rect,
) {
    // 分割状态栏为左中右三部分
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    // 获取当前缓冲区信息
    let buffer = match editor.current_buffer() {
        Ok(buf) => buf,
        Err(_) => return,
    };

    // 左侧 - 文件名、修改状态和额外信息
    let file_name = buffer
        .file_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("[未命名]");
    let modified_indicator = if buffer.modified { "[+]" } else { "" };
    
    // 添加文件编码和换行符类型信息
    let encoding = buffer.encoding.as_deref().unwrap_or("UTF-8");
    let line_ending = match buffer.line_ending {
        Some(ending) => match ending {
            "\r\n" => "CRLF",
            "\n" => "LF", 
            "\r" => "CR",
            _ => "?",
        },
        None => "LF", // 默认为LF
    };
    
    // 缩进设置
    let indent_info = if buffer.use_tabs {
        format!("Tab:{}", buffer.tab_size)
    } else {
        format!("Spaces:{}", buffer.tab_size)
    };
    
    let left_text = Spans::from(vec![
        Span::styled(
            format!(" {} {} | {}  {} | {}", file_name, modified_indicator, encoding, line_ending, indent_info),
            Style::default().fg(Color::White)
        )
    ]);

    // 中间 - 光标位置、行列信息和文件浏览进度
    let cursor_y = editor.windows[0].cursor_y + 1; // 1-based
    let cursor_x = editor.windows[0].cursor_x + 1; // 1-based
    let line_count = buffer.get_lines().len();
    let percentage = if line_count > 0 {
        (cursor_y as f32 / line_count as f32 * 100.0) as u16
    } else {
        0
    };
    
    // 添加语法高亮状态
    let syntax_status = if buffer.syntax_enabled {
        match &buffer.language {
            Some(lang) => format!("语法: {}", lang),
            None => "语法: 自动".to_string(),
        }
    } else {
        "语法: 关闭".to_string()
    };
    
    // 添加"是否继续迭代？"的提示信息
    let iteration_prompt = "是否继续迭代？";
    
    let middle_text = Spans::from(vec![
        Span::styled(
            format!("行 {}/{} 列 {} ({}%) | {} | {}", cursor_y, line_count, cursor_x, percentage, syntax_status, iteration_prompt),
            Style::default().fg(Color::White)
        )
    ]);

    // 右侧 - 显示当前模式、文件类型和时间
    let mode_str = match editor.mode {
        EditorMode::Normal => "NORMAL",
        EditorMode::Insert => "INSERT",
        EditorMode::Visual => "VISUAL",
        EditorMode::Command => "COMMAND",
        _ => "UNKNOWN",
    };
    
    let file_type = buffer
        .file_path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|s| s.to_str())
        .unwrap_or("txt");
    
    // 添加当前时间
    let now: DateTime<Local> = Local::now();
    let time_str = now.format("%H:%M").to_string();
    
    // Git状态信息（简单显示）
    let git_status = if let Some(git_info) = &buffer.git_status {
        match git_info.as_str() {
            "modified" => "M",
            "added" => "A",
            "deleted" => "D",
            "renamed" => "R",
            "untracked" => "?",
            _ => git_info,
        }
    } else {
        ""
    };
    
    let git_indicator = if !git_status.is_empty() {
        format!(" [{}]", git_status)
    } else {
        "".to_string()
    };
    
    let right_text = Spans::from(vec![
        Span::styled(
            format!("{}{} | {} | {}", mode_str, git_indicator, file_type.to_uppercase(), time_str),
            Style::default()
                .fg(Color::Black)
                .bg(match editor.mode {
                    EditorMode::Normal => Color::Green,
                    EditorMode::Insert => Color::Blue,
                    EditorMode::Visual => Color::Yellow,
                    EditorMode::Command => Color::Magenta,
                    _ => Color::Gray,
                })
                .add_modifier(Modifier::BOLD)
        )
    ]);

    // 渲染三部分状态栏
    f.render_widget(Paragraph::new(left_text), horizontal_chunks[0]);
    f.render_widget(Paragraph::new(middle_text), horizontal_chunks[1]);
    
    // 右侧状态靠右对齐
    let right_aligned = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(right_text.width() as u16),
        ])
        .split(horizontal_chunks[2]);
    
    f.render_widget(Paragraph::new(right_text), right_aligned[1]);
}