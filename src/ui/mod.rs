use std::io;
use std::time::{Duration, Instant};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect, Alignment},
    style::{Color, Style, Modifier},
    text::{Span, Text, Line},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, ListState},
    Frame, Terminal,
};
use crate::editor::{Editor, EditorMode, EditorStatus, StatusMessageType, CommandLineMode};
use crate::highlight::{HighlightSpan, HighlightStyle};
use crate::buffer::Buffer;
use crate::error::{Result};
use crate::file_browser::{FileBrowser};
use std::fs;
use chrono;

/// 启动UI
pub fn start(editor: &mut Editor) -> Result<()> {
    // 设置终端
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // 运行应用程序
    let res = run_app(&mut terminal, editor);
    
    // 恢复终端
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    // 检查结果
    if let Err(err) = res {
        println!("Error: {:?}", err);
    }
    
    Ok(())
}

/// 运行应用程序
fn run_app<B: Backend>(terminal: &mut Terminal<B>, editor: &mut Editor) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250); // 250ms刷新率
    
    // 初始时处于普通模式而不是插入模式
    editor.set_mode(EditorMode::Normal);
    
    loop {
        // 绘制UI
        terminal.draw(|f| ui(f, editor))?;
        
        // 处理事件
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // 只处理按下事件，忽略释放事件，避免重复处理
                if let crossterm::event::KeyEventKind::Release = key.kind {
                    continue;
                }
                
                // 记录按键用于调试，但不显示在状态栏
                // 只在调试构建中记录
                #[cfg(debug_assertions)]
                log::debug!("处理按键: {}", key_event_to_str(key));
                
                // 按以下优先级处理按键：
                // 1. 终端模式
                // 2. 全局特殊键（Esc, Ctrl+C 等）
                // 3. 模式特定处理（命令模式、普通模式下的特殊键等）
                // 4. 一般按键处理（通过 KeyHandler）
                
                // 1. 终端模式处理
                if editor.terminal_visible && editor.terminal.has_focus() {
                    match key.code {
                        KeyCode::Esc => editor.terminal.toggle_visibility(),
                        _ => { let _ = editor.terminal.handle_key(key)?; }
                    }
                    continue;
                }
                
                // Ctrl+T 切换终端可见性
                if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    editor.terminal.toggle_visibility();
                    continue;
                }
                
                // 2. 全局特殊键处理
                match key.code {
                    KeyCode::Esc => {
                        editor.set_mode(EditorMode::Normal);
                        editor.command_line.mode = CommandLineMode::Normal;
                        continue;
                    },
                    _ => {} // 继续其他处理
                }
                
                // 3. 模式特定处理
                let mode_handled = match editor.mode {
                    EditorMode::Normal => {
                        match key.code {
                            KeyCode::Char('i') => {
                                editor.set_mode(EditorMode::Insert);
                                true
                            },
                            KeyCode::Char(':') => {
                                editor.switch_to_command_mode();
                                true
                            },
                            _ => false
                        }
                    },
                    EditorMode::Insert => {
                        // 插入模式下直接处理所有按键
                        match key.code {
                            // 处理普通字符输入
                            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) 
                                            && !key.modifiers.contains(KeyModifiers::ALT) => {
                                let cursor_line = editor.cursor_line;
                                let cursor_col = editor.cursor_col;
                                
                                if let Ok(buffer) = editor.current_buffer_mut() {
                                    buffer.insert_at(cursor_line, cursor_col, &c.to_string());
                                    buffer.modified = true;
                                    editor.cursor_col += 1;
                                }
                                true // 表示已处理
                            },
                            // 处理回车键
                            KeyCode::Enter => {
                                let cursor_line = editor.cursor_line;
                                let cursor_col = editor.cursor_col;
                                
                                if let Ok(buffer) = editor.current_buffer_mut() {
                                    buffer.insert_at(cursor_line, cursor_col, "\n");
                                    buffer.modified = true;
                                    editor.cursor_line += 1;
                                    editor.cursor_col = 0;
                                }
                                true // 表示已处理
                            },
                            // 处理退格键
                            KeyCode::Backspace => {
                                let cursor_line = editor.cursor_line;
                                let cursor_col = editor.cursor_col;
                                
                                if let Ok(buffer) = editor.current_buffer_mut() {
                                    if cursor_col > 0 {
                                        if buffer.delete_at(cursor_line, cursor_col - 1, 1) {
                                            editor.cursor_col -= 1;
                                        }
                                    } else if cursor_line > 0 {
                                        // 如果光标在行首，删除换行符（合并行）
                                        let prev_line = cursor_line - 1;
                                        let prev_line_len = buffer.get_line(prev_line)
                                            .map(|line| line.len())
                                            .unwrap_or(0);
                                            
                                        if buffer.delete(cursor_line - 1, prev_line_len, cursor_line, 0).is_ok() {
                                            editor.cursor_line = prev_line;
                                            editor.cursor_col = prev_line_len;
                                        }
                                    }
                                }
                                true // 表示已处理
                            },
                            // 处理制表符
                            KeyCode::Tab => {
                                let cursor_line = editor.cursor_line;
                                let cursor_col = editor.cursor_col;
                                
                                // 先获取配置值，避免可变借用冲突
                                let use_spaces = editor.config.use_spaces;
                                let tab_width = editor.config.tab_width;
                                
                                if let Ok(buffer) = editor.current_buffer_mut() {
                                    let tab_text = if use_spaces {
                                        " ".repeat(tab_width)
                                    } else {
                                        "\t".to_string()
                                    };
                                    
                                    buffer.insert_at(cursor_line, cursor_col, &tab_text);
                                    buffer.modified = true;
                                    editor.cursor_col += tab_text.len();
                                }
                                true // 表示已处理
                            },
                            // 方向键处理
                            KeyCode::Left => {
                                let _ = editor.move_cursor_left();
                                true // 表示已处理
                            },
                            KeyCode::Right => {
                                let _ = editor.move_cursor_right();
                                true // 表示已处理
                            },
                            KeyCode::Up => {
                                let _ = editor.move_cursor_up();
                                true // 表示已处理
                            },
                            KeyCode::Down => {
                                let _ = editor.move_cursor_down();
                                true // 表示已处理
                            },
                            // 其他特殊键继续处理
                            _ => false
                        }
                    },
                    EditorMode::Command => {
                        match key.code {
                            KeyCode::Char(c) => {
                                editor.command_line.content.push(c);
                                editor.command_line.cursor_pos += 1;
                                true
                            },
                            KeyCode::Backspace => {
                                if !editor.command_line.content.is_empty() && editor.command_line.cursor_pos > 0 {
                                    editor.command_line.content.remove(editor.command_line.cursor_pos - 1);
                                    editor.command_line.cursor_pos -= 1;
                                }
                                true
                            },
                            KeyCode::Enter => {
                                let cmd = editor.command_line.content.clone();
                                editor.command_line.content.clear();
                                editor.command_line.cursor_pos = 0;
                                editor.command_line.mode = CommandLineMode::Normal;
                                editor.set_mode(EditorMode::Normal);
                                if !cmd.is_empty() {
                                    let _ = editor.execute_command(&cmd);
                                }
                                true
                            },
                            _ => false
                        }
                    },
                    _ => false
                };
                
                // 如果已由模式特定代码处理，跳过常规处理
                if mode_handled {
                    continue;
                }
                
                // 4. 常规按键处理
                let key_str = key_event_to_str(key);
                let mut key_handler = crate::input::KeyHandler::new(editor);
                
                match key_handler.handle_key(&key_str) {
                    Ok(action) => {
                        // 处理返回的动作
                        match action {
                            crate::input::InputAction::MoveCursor(dx, dy) => {
                                // 处理光标移动
                                if dx < 0 {
                                    for _ in 0..dx.abs() as usize {
                                        let _ = editor.move_cursor_left();
                                    }
                                } else if dx > 0 {
                                    for _ in 0..dx as usize {
                                        let _ = editor.move_cursor_right();
                                    }
                                }
                                
                                if dy < 0 {
                                    for _ in 0..dy.abs() as usize {
                                        let _ = editor.move_cursor_up();
                                    }
                                } else if dy > 0 {
                                    for _ in 0..dy as usize {
                                        let _ = editor.move_cursor_down();
                                    }
                                }
                            },
                            crate::input::InputAction::Insert(text) => {
                                // 处理文本插入
                                let cursor_line = editor.cursor_line;
                                let cursor_col = editor.cursor_col;
                                
                                if let Ok(buffer) = editor.current_buffer_mut() {
                                    buffer.insert_at(cursor_line, cursor_col, &text);
                                    
                                    // 设置缓冲区的修改状态
                                    buffer.modified = true;
                                    
                                    // 向后移动光标位置（仅临时保存，借用结束后更新）
                                    let mut new_line = cursor_line;
                                    let mut new_col = cursor_col;
                                    
                                    // 特殊处理换行符
                                    if text == "\n" {
                                        // 移动到下一行的开头
                                        new_line += 1;
                                        new_col = 0;
                                    } else {
                                        // 普通文本，光标向右移动
                                        new_col += text.len();
                                    }
                                    
                                    // 借用结束后更新编辑器的光标位置
                                    editor.cursor_line = new_line;
                                    editor.cursor_col = new_col;
                                }
                            },
                            crate::input::InputAction::Delete(start_line, start_col, end_line, end_col) => {
                                // 处理删除操作
                                let cursor_line = editor.cursor_line;
                                let cursor_col = editor.cursor_col;
                                
                                if let Ok(buffer) = editor.current_buffer_mut() {
                                    if start_line == 0 && start_col == 0 && end_line == 0 && end_col == 1 {
                                        // 处理退格键 - 删除光标前的字符
                                        if cursor_col > 0 {
                                            if buffer.delete_at(cursor_line, cursor_col - 1, 1) {
                                                editor.cursor_col -= 1;
                                            }
                                        } else if cursor_line > 0 {
                                            // 如果光标在行首，删除换行符（合并行）
                                            let prev_line = cursor_line - 1;
                                            let prev_line_len = buffer.get_line(prev_line)
                                                .map(|line| line.len())
                                                .unwrap_or(0);
                                                
                                            if buffer.delete(cursor_line - 1, prev_line_len, cursor_line, 0).is_ok() {
                                                editor.cursor_line = prev_line;
                                                editor.cursor_col = prev_line_len;
                                            }
                                        }
                                    } else {
                                        // 处理一般的删除操作
                                        let actual_start_line = if start_line == usize::MAX { cursor_line } else { start_line };
                                        let actual_start_col = if start_col == usize::MAX { cursor_col } else { start_col };
                                        let actual_end_line = if end_line == usize::MAX { cursor_line } else { end_line };
                                        let actual_end_col = if end_col == usize::MAX { cursor_col + 1 } else { end_col };
                                        
                                        if buffer.delete(actual_start_line, actual_start_col, actual_end_line, actual_end_col).is_ok() {
                                            editor.cursor_col = actual_start_col;
                                        }
                                    }
                                }
                            },
                            crate::input::InputAction::ExecuteCommand(cmd) => {
                                // 执行命令
                                let _ = editor.execute_command(&cmd);
                            },
                            crate::input::InputAction::SwitchMode(mode) => {
                                // 切换模式
                                editor.set_mode(mode);
                            },
                            crate::input::InputAction::None => {
                                // 无操作
                            }
                        }
                    },
                    Err(_) => {
                        // 如果处理出错，记录错误但不退出
                        editor.set_status_message("按键处理错误", StatusMessageType::Error);
                    }
                }
            }
        }
        
        // 检查是否需要更新
        if last_tick.elapsed() >= tick_rate {
            // 更新编辑器状态
            last_tick = Instant::now();
        }
        
        // 检查退出状态
        if editor.status == crate::editor::EditorStatus::Exiting {
            return Ok(());
        }
    }
}

/// 处理键盘事件
fn handle_key_event(editor: &mut Editor, key: KeyEvent) -> Result<()> {
    // 通用热键
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            editor.status = EditorStatus::Exiting;
            return Ok(());
        },
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
            return editor.save_current_file();
        },
        _ => {}
    }
    
    // 将键盘事件转换为编辑器可理解的键码
    let key_str = key_event_to_str(key);
    
    // 处理按键，使用输入处理器
    let mut key_handler = crate::input::KeyHandler::new(editor);
    key_handler.handle_key(&key_str)?;
    Ok(())
}

/// 将键盘事件转换为字符串表示
fn key_event_to_str(key: KeyEvent) -> String {
    // 添加调试信息
    let result = match key.code {
        KeyCode::Esc => "<Esc>".to_string(),
        KeyCode::Enter => "<CR>".to_string(),
        KeyCode::Tab => "<Tab>".to_string(),
        KeyCode::Backspace => "<BS>".to_string(),
        KeyCode::Delete => "<Del>".to_string(),
        KeyCode::Left => "<Left>".to_string(),
        KeyCode::Right => "<Right>".to_string(),
        KeyCode::Up => "<Up>".to_string(),
        KeyCode::Down => "<Down>".to_string(),
        KeyCode::Home => "<Home>".to_string(),
        KeyCode::End => "<End>".to_string(),
        KeyCode::PageUp => "<PageUp>".to_string(),
        KeyCode::PageDown => "<PageDown>".to_string(),
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                format!("<C-{}>", c)
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                format!("<A-{}>", c)
            } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                if c.is_ascii_lowercase() {
                    c.to_uppercase().to_string()
                } else {
                    format!("<S-{}>", c)
                }
            } else {
                c.to_string()
            }
        },
        KeyCode::F(n) => format!("<F{}>", n),
        _ => String::new(),
    };
    
    // 只在调试模式下输出详细信息
    #[cfg(debug_assertions)]
    {
        // 加入事件类型信息
        let kind_str = match key.kind {
            crossterm::event::KeyEventKind::Press => "按下",
            crossterm::event::KeyEventKind::Release => "释放",
            crossterm::event::KeyEventKind::Repeat => "重复",
            _ => "未知",
        };
        
        // 使用log::debug!代替eprintln!，避免干扰UI
        log::debug!("键盘事件: {:?} - 类型: {} - 转换为: {}", key.code, kind_str, result);
    }
    
    result
}

/// UI 逻辑
fn ui(f: &mut Frame, editor: &Editor) {
    let area = f.area();
    let terminal_visible = editor.terminal_visible;
    
    // 计算主界面区域
    let main_area = if terminal_visible {
        let terminal_height = editor.terminal_height.min((area.height as u16) / 2);
        Rect::new(0, 0, area.width, area.height - terminal_height)
    } else {
        // 减去状态栏和命令行的高度（每个都占用1行）
        Rect::new(0, 0, area.width, area.height - 2)
    };
    
    // 绘制编辑器主窗口
    draw_editor(f, editor, main_area);
    
    // 绘制终端区域（如果可见）
    if terminal_visible {
        let terminal_height = editor.terminal_height.min((area.height as u16) / 2);
        let terminal_area = Rect::new(0, area.height - terminal_height, area.width, terminal_height);
        draw_terminal(f, editor, terminal_area);
    }
    
    // 绘制状态栏和命令行，确保它们紧跟在主区域之后
    let status_y = if terminal_visible {
        main_area.height
    } else {
        area.height - 2
    };
    
    let cmd_y = if terminal_visible {
        main_area.height + 1
    } else {
        area.height - 1
    };
    
    // 绘制状态栏
    draw_status_bar(f, editor, Rect::new(0, status_y, area.width, 1));
    
    // 绘制命令行
    draw_command_line(f, editor, Rect::new(0, cmd_y, area.width, 1));
}

/// 绘制编辑器
fn draw_editor(f: &mut Frame, editor: &Editor, area: Rect) {
    // 获取当前Tab
    let tab = match editor.tab_manager.current_tab() {
        Ok(tab) => tab,
        Err(_) => return, // 无法获取Tab，直接返回
    };
    
    // 创建窗口布局
    let windows = tab.get_windows();
    let active_win_id = tab.active_window_id();
    
    if windows.is_empty() {
        // 没有窗口，显示欢迎信息
        draw_welcome_screen(f, editor, area);
        return;
    }
    
    // 创建布局区域
    let layout = tab.get_layout();
    let ratatui_areas: Vec<Rect> = windows.iter().enumerate().map(|(idx, _)| {
        // 将ratatui的Rect转换为editor的Rect
        let editor_rect = convert_ratatui_to_editor_rect(area);
        let window_rect = layout.calculate_area(editor_rect, idx, windows.len());
        // 再将结果转回ratatui的Rect
        let window_area = convert_editor_to_ratatui_rect(window_rect);
        window_area
    }).collect();
    
    // 遍历所有窗口，绘制每个窗口
    for (i, window) in windows.iter().enumerate() {
        let win_area = ratatui_areas[i];
        let is_active = Some(window.id()) == active_win_id;
        
        // 获取缓冲区
        let buffer_id = window.buffer_id();
        if buffer_id >= editor.buffers.len() {
            continue; // 无效的缓冲区ID
        }
        
        let buffer = &editor.buffers[buffer_id];
        
        // 绘制窗口内容
        draw_window(f, editor, window, buffer, win_area, is_active);
    }
}

/// 绘制单个窗口
fn draw_window(
    f: &mut Frame, 
    editor: &Editor, 
    window: &crate::editor::Window, 
    buffer: &crate::buffer::Buffer, 
    area: Rect, 
    is_active: bool
) {
    // 创建窗口边框
    let title = if buffer.file_path.is_some() {
        let path = buffer.file_path.as_ref().unwrap();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if buffer.modified {
            format!("{} [+]", filename)
        } else {
            filename.to_string()
        }
    } else {
        if buffer.modified {
            "[未命名] [+]".to_string()
        } else {
            "[未命名]".to_string()
        }
    };
    
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Gray)
    };
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);
    
    // 可视区域
    let inner_area = block.inner(area);
    f.render_widget(block, area);
    
    // 计算可见行范围
    let line_offset = window.scroll_offset().0;
    let visible_height = inner_area.height as usize;
    let lines = buffer.text.lines().collect::<Vec<_>>();
    
    let visible_lines = lines.iter()
        .skip(line_offset)
        .take(visible_height)
        .collect::<Vec<_>>();
    
    // 构建文本展示
    let mut text_spans = Vec::with_capacity(visible_height);
    
    // 获取语法高亮
    let highlights = buffer.get_highlights();
    
    // 行号显示宽度，最小为4，确保有足够的空间显示更大的行号
    let line_number_width = if editor.config.show_line_numbers {
        (buffer.text.len_lines().to_string().len() + 1).max(4)
    } else {
        0
    };
    
    for (i, line) in visible_lines.iter().enumerate() {
        let line_idx = line_offset + i;
        let mut line_text = line.to_string(); // 将RopeSlice转换为String
        
        // 如果开启了行号显示，在每行前添加行号
        if editor.config.show_line_numbers {
            // 行号从1开始计数，右对齐显示
            let line_number = format!("{:>width$} ", line_idx + 1, width = line_number_width - 1);
            line_text = format!("{}{}", line_number, line_text);
        }
        
        let line_highlights = get_highlight_spans_for_line(buffer, line_idx, highlights);
        
        // 需要调整高亮的起始位置，考虑行号占用的空间
        let adjusted_highlights = if editor.config.show_line_numbers {
            line_highlights.iter().map(|span| {
                let mut new_span = span.clone();
                new_span.start_col += line_number_width;
                new_span.end_col += line_number_width;
                new_span
            }).collect()
        } else {
            line_highlights
        };
        
        // 将高亮转换为样式
        let styled_line = render_line_with_highlights(&line_text, &adjusted_highlights);
        text_spans.push(Line::from(styled_line));
    }
    
    // 渲染文本内容
    let paragraph = Paragraph::new(text_spans)
        .scroll((0, 0));
    
    f.render_widget(paragraph, inner_area);
    
    // 如果是活动窗口，绘制光标
    if is_active {
        // 计算光标位置
        let cursor_y = editor.cursor_line.saturating_sub(line_offset);
        let cursor_x = editor.cursor_col;
        
        // 确保行号在有效范围内
        if editor.cursor_line < buffer.text.len_lines() {
            // 确保列号在有效范围内
            let line_len = buffer.get_line(editor.cursor_line).map(|l| l.len()).unwrap_or(0);
            
            // 确保光标在有效位置
            if cursor_y < visible_height {
                // 确保光标位置正确考虑行号宽度
                let adjusted_cursor_x = if editor.config.show_line_numbers {
                    line_number_width + cursor_x.min(line_len)
                } else {
                    cursor_x.min(line_len)
                };
                
                // 设置实际的光标位置
                f.set_cursor_position((
                    inner_area.x + adjusted_cursor_x as u16,
                    inner_area.y + cursor_y as u16
                ));
            }
        }
    }
}

/// 获取带高亮的行
fn get_highlight_spans_for_line(_buffer: &crate::buffer::Buffer, line: usize, highlights: Option<&Vec<HighlightSpan>>) -> Vec<HighlightSpan> {
    // 从高亮列表中过滤出当前行的高亮
    if let Some(all_highlights) = highlights {
        all_highlights.iter()
            .filter(|span| span.start_line <= line && span.end_line >= line)
            .cloned()
            .collect()
    } else {
        Vec::new()
    }
}

/// 渲染一行带有高亮的文本
fn render_line_with_highlights<'a>(line_text: &String, line_highlights: &Vec<HighlightSpan>) -> Vec<Span<'a>> {
    if line_highlights.is_empty() {
        // 没有高亮，直接返回原始文本
        return vec![Span::raw(line_text.clone())];
    }
    
    let mut spans = Vec::new();
    let mut start = 0;
    
    // 应用高亮
    for highlight in line_highlights {
        // 添加前面非高亮部分
        if highlight.start_col > start {
            let regular_text = &line_text[start..highlight.start_col];
            spans.push(Span::raw(regular_text.to_string()));
        }
        
        // 添加高亮部分
        if highlight.end_col > highlight.start_col {
            let highlighted_text = &line_text[highlight.start_col..highlight.end_col];
            let style = Style::default().fg(get_color_from_style(&highlight.style));
            spans.push(Span::styled(highlighted_text.to_string(), style));
        }
        
        start = highlight.end_col;
    }
    
    // 添加末尾非高亮部分
    if start < line_text.len() {
        let regular_text = &line_text[start..];
        spans.push(Span::raw(regular_text.to_string()));
    }
    
    spans
}

/// 绘制文件浏览器
fn draw_file_browser(
    f: &mut Frame,
    browser: &mut FileBrowser,
    area: Rect
) -> Result<()> {
    // 创建布局
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ].as_ref())
        .split(area);
    
    // 创建项目列表
    let mut items: Vec<ListItem> = Vec::new();
    
    for (i, item) in browser.entries.iter().enumerate() {
        let style = if i == browser.cursor {
            Style::default().fg(Color::Black).bg(Color::White)
        } else if item.is_dir {
            Style::default().fg(Color::Blue)
        } else {
            Style::default().fg(Color::Reset)
        };
        
        let icon = if item.is_dir { "📁 " } else { "📄 " };
        let name = format!("{}{}", icon, item.name);
        
        items.push(ListItem::new(Span::styled(name, style)));
    }
    
    let list = List::new(items)
        .block(Block::default()
            .title("文件浏览器")
            .borders(Borders::ALL))
        .highlight_style(Style::default()
            .bg(Color::White)
            .fg(Color::Black));
    
    let mut state = ListState::default();
    state.select(Some(browser.cursor));
    
    f.render_stateful_widget(list, chunks[0], &mut state);
    
    // 如果启用了预览，则显示预览内容
    if browser.preview_enabled {
        let selected = browser.selected();
        
        let preview_content = if let Some(item) = selected {
            if item.is_dir {
                "这是一个目录".to_string()
            } else {
                match fs::read_to_string(&item.path) {
                    Ok(content) => {
                        // 对于二进制文件，只显示前面的一部分
                        if content.chars().any(|c| c == '\0' || !c.is_ascii_graphic() && !c.is_ascii_whitespace()) {
                            "[二进制文件]".to_string()
                        } else {
                            content
                        }
                    },
                    Err(_) => "[无法读取文件内容]".to_string()
                }
            }
        } else {
            "未选择文件".to_string()
        };
        
        let preview = Paragraph::new(preview_content)
            .block(Block::default()
                .title("预览")
                .borders(Borders::ALL));
        
        f.render_widget(preview, chunks[1]);
    }
    
    // 显示额外信息
    if let Some(item) = browser.selected() {
        let mut info_items = vec![
            format!("文件名: {}", item.name),
            format!("类型: {}", if item.is_dir { "目录" } else { "文件" }),
            format!("大小: {} 字节", item.size),
        ];
        
        if let Some(modified) = item.modified {
            let duration = modified.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            let time = chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "未知时间".to_string());
            info_items.push(format!("修改时间: {}", time));
        }
        
        let info = Paragraph::new(Text::from(info_items.join("\n")))
            .block(Block::default()
                .title("文件信息")
                .borders(Borders::ALL));
        
        f.render_widget(info, chunks[1]);
    }
    
    Ok(())
}

/// 绘制欢迎屏幕
fn draw_welcome_screen(f: &mut Frame, editor: &Editor, area: Rect) {
    // 绘制欢迎屏幕
    let block = Block::default()
        .title("欢迎使用 fkvim")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    
    let inner_area = block.inner(area);
    f.render_widget(block, area);
    
    // 计算欢迎文本
    let logo = vec![
        "   ______  ___                 _         ",
        "   |  ___|/ | \\\\               (_)        ",
        "   | |_  |/   \\\\_|  __   _ _ __ _ _ __ ___",
        "   |  _| |  /\\\\ | |/ /  | | '__| | '_ ` _ \\\\",
        "   | |   | |  || |V /  | | |  | | | | | | |",
        "   \\\\_|   |_|  |_|\\\\_/   |_|_|  |_|_| |_| |_|",
        "",
        "          轻量，快速，可扩展的编辑器",
        "",
        "     按 :e <文件名> 打开文件",
        "     按 :q 退出",
    ];
    
    let _paragraphs: Vec<Paragraph> = Vec::new();
    for (i, line) in logo.iter().enumerate() {
        let paragraph = Paragraph::new(line.to_string())
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center);
        
        // 计算每行位置
        let y = inner_area.y + (inner_area.height - logo.len() as u16) / 2 + i as u16;
        let line_area = Rect::new(inner_area.x, y, inner_area.width, 1);
        
        f.render_widget(paragraph, line_area);
    }
    
    // 显示插件信息
    let plugin_text = format!(
        "已加载 {} 插件",
        editor.plugin_manager.plugin_count()
    );
    
    let plugin_para = Paragraph::new(plugin_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    
    let plugin_area = Rect::new(
        inner_area.x,
        inner_area.y + inner_area.height - 3,
        inner_area.width,
        1
    );
    
    f.render_widget(plugin_para, plugin_area);
}

/// 绘制状态栏
fn draw_status_bar(
    f: &mut Frame,
    editor: &Editor,
    area: Rect
) {
    // 创建状态栏段落
    let items = render_status_bar(editor);
    
    let status_bar = Paragraph::new(Line::from(items))
        .block(Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Blue)))
        .style(Style::default().bg(Color::Black));
    
    f.render_widget(status_bar, area);
}

/// 绘制命令行
fn draw_command_line(
    f: &mut Frame,
    editor: &Editor,
    area: Rect
) {
    let items = render_command_line(editor);
    
    let command_line = Paragraph::new(Line::from(items))
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().bg(Color::Black));
    
    // 设置光标位置
    if editor.command_line.mode == CommandLineMode::Command {
        // 光标位置是前缀 ":" 之后，加上当前内容的长度
        let cursor_offset = 1 + editor.command_line.cursor_pos;
        f.set_cursor_position((area.x + cursor_offset as u16, area.y));
    }
    
    f.render_widget(command_line, area);
}

/// 绘制搜索信息区
fn draw_search_info(
    f: &mut Frame,
    editor: &Editor,
    area: Rect
) {
    let items = render_search_info(editor);
    
    let search_info = Paragraph::new(Line::from(items))
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().bg(Color::Black));
    
    f.render_widget(search_info, area);
}

/// 绘制终端区域
fn draw_terminal(f: &mut Frame, editor: &Editor, area: Rect) {
    // 终端标题
    let title = if let Some(tab_name) = editor.terminal.get_current_tab_name() {
        format!("终端 - {}", tab_name)
    } else {
        "终端".to_string()
    };
    
    // 创建终端区域
    let terminal_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));
    
    let inner_area = terminal_block.inner(area);
    f.render_widget(terminal_block, area);
    
    // 获取终端内容
    let terminal_lines = editor.terminal.get_visible_lines(inner_area.height as usize);
    
    // 创建文本内容
    let mut spans_vec = Vec::new();
    for line in terminal_lines {
        spans_vec.push(Line::from(line));
    }
    
    // 渲染终端内容
    let terminal_content = Paragraph::new(spans_vec)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false });
    
    f.render_widget(terminal_content, inner_area);
    
    // 如果终端模式激活，设置光标
    if editor.mode == EditorMode::Terminal {
        if let Some(active_session) = editor.terminal.get_active_session() {
            let (cursor_x, cursor_y) = active_session.get_cursor_position();
            if cursor_y < inner_area.height as usize {
                f.set_cursor_position((
                    inner_area.x + cursor_x as u16,
                    inner_area.y + cursor_y as u16
                ));
            }
        }
    }
}

/// 获取当前行的语法高亮信息
fn get_highlight_spans(buffer: &Buffer, line: usize) -> Vec<HighlightSpan> {
    if let Some(highlights) = &buffer.syntax_highlights {
        // 仅返回与当前行相关的高亮
        highlights.iter()
            .filter(|span| span.start_line <= line && span.end_line >= line)
            .cloned()
            .collect()
    } else {
        Vec::new()
    }
}

/// 渲染语法高亮
fn render_syntax_highlight(line: &str, line_idx: usize, line_highlights: &[HighlightSpan], get_highlight_style: impl Fn(&HighlightStyle) -> Style) -> Vec<(usize, usize, Style)> {
    let mut styled_spans = Vec::new();
    
    // 处理每个高亮区域
    for span in line_highlights {
        // 只处理当前行内的高亮部分
        if span.start_line == line_idx && span.end_line == line_idx {
            // 完全在当前行内的高亮
            styled_spans.push((
                span.start_col,
                span.end_col,
                get_highlight_style(&span.style),
            ));
        } else if span.start_line == line_idx {
            // 跨行高亮的起始部分
            styled_spans.push((
                span.start_col,
                line.len(),
                get_highlight_style(&span.style),
            ));
        } else if span.end_line == line_idx {
            // 跨行高亮的结束部分
            styled_spans.push((
                0,
                span.end_col,
                get_highlight_style(&span.style),
            ));
        } else if span.start_line < line_idx && span.end_line > line_idx {
            // 跨多行高亮的中间部分
            styled_spans.push((
                0,
                line.len(),
                get_highlight_style(&span.style),
            ));
        }
    }
    
    // 按照起始位置排序
    styled_spans.sort_by_key(|(start, _, _)| *start);
    
    styled_spans
}

/// 将ratatui的Rect转换为编辑器的Rect
fn convert_ratatui_to_editor_rect(rect: ratatui::layout::Rect) -> crate::editor::window::Rect {
    crate::editor::window::Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// 将编辑器的Rect转换为ratatui的Rect
fn convert_editor_to_ratatui_rect(rect: crate::editor::window::Rect) -> ratatui::layout::Rect {
    ratatui::layout::Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

/// 生成状态栏内容
fn render_status_bar(editor: &Editor) -> Vec<Span> {
    // 获取当前缓冲区
    let buffer = match editor.current_buffer() {
        Ok(buf) => buf,
        Err(_) => {
            // 无效的缓冲区，返回空状态栏
            return vec![
                Span::styled("无可用缓冲区", Style::default().fg(Color::Red))
            ];
        }
    };

    // 左侧：文件名、修改状态
    let file_name = buffer
        .file_path
        .as_ref()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .unwrap_or_else(|| "[未命名]".to_string());
    
    let modified = if buffer.modified { "[+]" } else { "" };
    
    // 右侧：行号、列号、模式
    let position = format!("{}:{}", editor.cursor_line + 1, editor.cursor_col + 1);
    let mode = format!("{:?}", editor.mode);

    // 组合所有组件
    vec![
        // 左侧信息
        Span::styled(format!(" {} {}", file_name, modified), 
            Style::default().fg(Color::White)),
        
        // 中间填充
        Span::raw(" ".repeat(
            editor.screen_width.saturating_sub(
                file_name.len() + modified.len() + position.len() + mode.len() + 4
            )
        )),
        
        // 右侧信息
        Span::styled(format!("{} | {} ", position, mode),
            Style::default().fg(Color::White))
    ]
}

/// 生成命令行内容
fn render_command_line(editor: &Editor) -> Vec<Span> {
    match editor.command_line.mode {
        CommandLineMode::Normal => {
            // 普通模式下不显示命令行，如果有状态消息则显示
            if let Some(status_msg) = &editor.status_message {
                let style = match status_msg.msg_type {
                    StatusMessageType::Info => Style::default().fg(Color::White),
                    StatusMessageType::Warning => Style::default().fg(Color::Yellow),
                    StatusMessageType::Error => Style::default().fg(Color::Red),
                    StatusMessageType::Success => Style::default().fg(Color::Green),
                };
                
                vec![
                    Span::styled(format!(" {}", status_msg.content), style)
                ]
            } else {
                // 无状态消息时显示空命令行或模式信息
                let mode_str = match editor.mode {
                    EditorMode::Normal => "普通",
                    EditorMode::Insert => "插入",
                    EditorMode::Visual => "可视",
                    EditorMode::Command => "命令",
                    EditorMode::Replace => "替换",
                    EditorMode::Terminal => "终端",
                };
                vec![Span::styled(format!(" {} 模式", mode_str), Style::default().fg(Color::White))]
            }
        },
        CommandLineMode::Command => {
            // 命令模式显示当前输入的命令
            vec![
                Span::styled(":", Style::default().fg(Color::White)),
                Span::styled(&editor.command_line.content, Style::default().fg(Color::White))
            ]
        },
        CommandLineMode::Search => {
            // 搜索模式显示搜索内容
            vec![
                Span::styled("/", Style::default().fg(Color::White)),
                Span::styled(&editor.command_line.content, Style::default().fg(Color::White))
            ]
        },
        CommandLineMode::ReplaceConfirm => {
            // 替换确认模式
            vec![
                Span::styled("替换此处? (y/n/a/q)", Style::default().fg(Color::Yellow))
            ]
        }
    }
}

/// 生成搜索信息内容
fn render_search_info(editor: &Editor) -> Vec<Span> {
    if let Ok(buffer) = editor.current_buffer() {
        // 显示搜索信息（如果有）
        if let Some(search_results) = &buffer.search_results {
            if !search_results.is_empty() {
                let current_idx = buffer.current_search_idx.min(search_results.len() - 1);
                return vec![
                    Span::styled(
                        format!(" 找到 {} 个匹配 ({}/{})",
                            search_results.len(),
                            current_idx + 1,
                            search_results.len()
                        ),
                        Style::default().fg(Color::Green)
                    )
                ];
            }
        }
    }
    
    // 无搜索结果时返回空内容
    vec![Span::raw("")]
}

/// 根据高亮样式获取颜色
fn get_color_from_style(style: &HighlightStyle) -> Color {
    match style {
        HighlightStyle::Keyword => Color::Yellow,
        HighlightStyle::Identifier => Color::White,
        HighlightStyle::String => Color::Green,
        HighlightStyle::Comment => Color::DarkGray,
        HighlightStyle::Number => Color::Cyan,
        HighlightStyle::Function => Color::LightBlue,
        HighlightStyle::FunctionCall => Color::Blue,
        HighlightStyle::Type => Color::Magenta,
        HighlightStyle::Preprocessor => Color::Red,
        HighlightStyle::Operator => Color::White,
        HighlightStyle::Variable => Color::White,
        HighlightStyle::Constant => Color::LightCyan,
        HighlightStyle::Property => Color::LightMagenta,
        HighlightStyle::Field => Color::LightYellow,
        HighlightStyle::Method => Color::LightBlue,
        HighlightStyle::MethodCall => Color::Blue,
        HighlightStyle::Parameter => Color::White,
        _ => Color::Reset,
    }
}

fn render_filenames_panel(f: &mut Frame, rect: Rect, editor: &Editor) {
    // 创建文件列表
    let items: Vec<ListItem> = editor.buffers.iter().enumerate()
        .map(|(idx, buffer)| {
            let mut filename = buffer.file_path.as_ref()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("[Untitled]")
                .to_string();
            
            // 如果有未保存的更改，添加标记
            if buffer.modified {
                filename = format!("{} [+]", filename);
            }
            
            // 当前活动缓冲区高亮显示
            let style = if idx == editor.current_buffer {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            
            ListItem::new(Line::from(vec![Span::styled(filename, style)]))
        })
        .collect();
    
    // 创建文件列表组件
    let list = List::new(items)
        .block(Block::default()
            .title("文件")
            .borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    // 渲染列表
    f.render_widget(list, rect);
}