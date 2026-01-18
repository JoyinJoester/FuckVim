use tui::{
    backend::Backend,
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::Paragraph,
    Frame,
};
use crate::buffer::Buffer;
use crate::editor::Window;
use std::collections::HashMap;

/// 代码折叠信息
#[derive(Default)]
pub struct CodeFolding {
    /// 已折叠的行区间: (起始行, 结束行)
    pub folded_regions: HashMap<usize, usize>,
}

impl CodeFolding {
    /// 创建新的代码折叠实例
    pub fn new() -> Self {
        Self {
            folded_regions: HashMap::new(),
        }
    }

    /// 折叠指定区域
    pub fn fold_region(&mut self, start_line: usize, end_line: usize) {
        if start_line < end_line {
            self.folded_regions.insert(start_line, end_line);
        }
    }

    /// 展开指定行的折叠区域
    pub fn unfold_region(&mut self, line: usize) {
        self.folded_regions.remove(&line);
    }

    /// 检查行是否位于折叠区域内部
    pub fn is_line_folded(&self, line: usize) -> bool {
        for (&start, &end) in &self.folded_regions {
            if line > start && line <= end {
                return true;
            }
        }
        false
    }

    /// 检查行是否为折叠区域起始
    pub fn is_fold_start(&self, line: usize) -> bool {
        self.folded_regions.contains_key(&line)
    }

    /// 获取折叠区域末尾行
    pub fn get_fold_end(&self, line: usize) -> Option<usize> {
        self.folded_regions.get(&line).copied()
    }

    /// 切换指定行的折叠状态
    pub fn toggle_fold(&mut self, line: usize, buffer: &Buffer) {
        if self.is_fold_start(line) {
            self.unfold_region(line);
        } else {
            // 查找可折叠区域
            if let Some(end_line) = find_foldable_region(buffer, line) {
                self.fold_region(line, end_line);
            }
        }
    }
}

/// 寻找可折叠的区域
/// 使用一个简单的启发式方法：寻找下一个与当前行缩进相同或更小的行
fn find_foldable_region(buffer: &Buffer, start_line: usize) -> Option<usize> {
    let lines = buffer.get_lines();
    if start_line >= lines.len() {
        return None;
    }

    // 计算当前行的缩进级别
    let current_line = &lines[start_line];
    let current_indent = count_leading_spaces(current_line);

    // 寻找结束行
    for i in (start_line + 1)..lines.len() {
        let line = &lines[i];
        let indent = count_leading_spaces(line);
        
        // 找到了一个缩进更小或相等的非空行
        if indent <= current_indent && !line.trim().is_empty() {
            if i > start_line + 1 {
                return Some(i - 1);
            }
            break;
        }
    }

    // 如果没有找到合适的结束行，则使用文件末尾
    if start_line + 1 < lines.len() {
        Some(lines.len() - 1)
    } else {
        None
    }
}

/// 计算行首的空格数量
fn count_leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

/// 为文本显示提供折叠行指示
pub fn draw_code_folding<B: Backend>(
    f: &mut Frame<B>,
    buffer: &Buffer,
    window: &Window,
    code_folding: &CodeFolding,
    area: Rect,
) {
    // 在编辑器中绘制折叠指示符
    for (&start_line, &end_line) in &code_folding.folded_regions {
        if start_line >= window.scroll_y && start_line < window.scroll_y + area.height as usize {
            let y = start_line - window.scroll_y;
            let foldable_lines = end_line - start_line;
            
            let fold_indicator = Span::styled(
                format!(" [折叠: {}行] ", foldable_lines),
                Style::default()
                    .fg(Color::Yellow)
            );
            
            // 本函数仅返回要绘制的组件，实际绘制需要在UI主循环中执行
            // 这里我们假设UI主循环会读取这些信息并进行绘制
            
            // 注意：这里的实现需要集成到您的主绘制循环中
            // 简单起见，这个函数作为一个示例，展示如何为每个折叠区域创建指示符
        }
    }
}