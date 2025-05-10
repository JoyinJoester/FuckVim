use tui::{
    backend::Backend,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::buffer::Buffer;
use crate::editor::Window;

/// 绘制迷你地图
pub fn draw_minimap<B: Backend>(
    f: &mut Frame<B>,
    buffer: &Buffer,
    window: &Window,
    area: Rect,
) {
    // 计算缩放比例
    let buffer_line_count = buffer.get_lines().len();
    let visible_lines = area.height as usize - 2; // 减去边框高度
    let scale_factor = if buffer_line_count > visible_lines {
        buffer_line_count as f32 / visible_lines as f32
    } else {
        1.0
    };

    // 构建迷你地图内容
    let mut content = String::new();
    for i in 0..visible_lines {
        let line_idx = (i as f32 * scale_factor) as usize;
        if line_idx < buffer_line_count {
            let line = &buffer.get_lines()[line_idx];
            
            // 对行内容进行简化，使用特殊字符表示
            let simplified_line = line
                .chars()
                .map(|c| {
                    if c.is_whitespace() { ' ' }
                    else if c.is_alphabetic() { '█' }
                    else if c.is_numeric() { '▓' }
                    else { '▒' }
                })
                .collect::<String>();
            
            // 截断行，以适应迷你地图宽度
            let max_width = area.width as usize - 2; // 减去边框宽度
            let display_line = if simplified_line.len() > max_width {
                &simplified_line[0..max_width]
            } else {
                &simplified_line
            };
            
            content.push_str(display_line);
        }
        content.push('\n');
    }

    // 计算当前视图在迷你地图中的位置
    let current_position = (window.scroll_y as f32 / scale_factor) as usize;
    let viewport_height = (window.rect.height as f32 / scale_factor) as usize;

    // 创建段落组件
    let minimap = Paragraph::new(content)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("迷你地图"))
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: false });

    // 渲染迷你地图
    f.render_widget(minimap, area);

    // 如果想在迷你地图上突出显示当前视图位置，可以添加下面的代码
    // (需要使用 Layer 或类似的叠加渲染机制，这里简化处理)
}