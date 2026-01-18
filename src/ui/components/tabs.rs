use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Tabs, Widget},
    Frame,
};
use crate::buffer::Buffer;
use crate::editor::Window;
use std::io;

/// 标签页组件，用于显示和管理多个打开的文件
pub struct TabsComponent {
    /// 打开的文件名列表
    pub titles: Vec<String>,
    /// 当前选中的标签索引
    pub selected: usize,
}

impl TabsComponent {
    /// 创建新的标签页组件
    pub fn new() -> Self {
        Self {
            titles: Vec::new(),
            selected: 0,
        }
    }

    /// 添加新标签
    pub fn add_tab(&mut self, title: String) {
        self.titles.push(title);
    }

    /// 删除标签
    pub fn remove_tab(&mut self, index: usize) {
        if index < self.titles.len() {
            self.titles.remove(index);
            // 确保选中的标签仍在有效范围内
            if self.selected >= self.titles.len() && !self.titles.is_empty() {
                self.selected = self.titles.len() - 1;
            }
        }
    }

    /// 选择指定标签
    pub fn select(&mut self, index: usize) {
        if index < self.titles.len() {
            self.selected = index;
        }
    }

    /// 选择下一个标签
    pub fn next(&mut self) {
        if !self.titles.is_empty() {
            self.selected = (self.selected + 1) % self.titles.len();
        }
    }

    /// 选择上一个标签
    pub fn previous(&mut self) {
        if !self.titles.is_empty() {
            self.selected = if self.selected > 0 {
                self.selected - 1
            } else {
                self.titles.len() - 1
            };
        }
    }

    /// 渲染标签页组件
    pub fn render<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        if self.titles.is_empty() {
            return;
        }

        // 创建标签标题
        let titles: Vec<Spans> = self.titles
            .iter()
            .map(|t| {
                // 截断长文件名，只显示最后的部分
                let title = if t.len() > 20 {
                    let shortened = &t[t.len().saturating_sub(19)..];
                    format!("...{}", shortened)
                } else {
                    t.clone()
                };

                Spans::from(vec![Span::styled(
                    format!(" {} ", title),
                    Style::default().fg(Color::White),
                )])
            })
            .collect();

        // 创建标签组件
        let tabs = Tabs::new(titles)
            .select(self.selected)
            .block(Block::default().borders(Borders::BOTTOM))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(tabs, area);
    }
}