use crate::error::{Result, FKVimError};

/// 窗口ID类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub usize);

/// 标签ID类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub usize);

/// 分割方向
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Split {
    /// 水平分割
    Horizontal,
    /// 垂直分割
    Vertical,
}

/// 标签页管理器
pub struct TabManager {
    /// 标签页列表
    pub tabs: Vec<Tab>,
    /// 当前活动标签页索引
    pub current_tab: usize,
    /// 下一个窗口ID
    next_window_id: usize,
}

impl TabManager {
    /// 创建新的标签页管理器
    pub fn new() -> Self {
        let mut manager = Self {
            tabs: Vec::new(),
            current_tab: 0,
            next_window_id: 0,
        };
        
        // 创建第一个标签页
        manager.new_tab("Tab 1".to_string()).ok();
        
        manager
    }
    
    /// 创建新的标签页
    pub fn new_tab(&mut self, name: String) -> Result<usize> {
        // 创建新的标签页
        let tab = Tab::new(name, self.next_window_id());
        self.tabs.push(tab);
        let index = self.tabs.len() - 1;
        self.current_tab = index;
        Ok(index)
    }
    
    /// 切换到下一个标签页
    pub fn next_tab(&mut self) -> Result<()> {
        if !self.tabs.is_empty() {
            self.current_tab = (self.current_tab + 1) % self.tabs.len();
        }
        Ok(())
    }
    
    /// 切换到上一个标签页
    pub fn prev_tab(&mut self) -> Result<()> {
        if !self.tabs.is_empty() {
            self.current_tab = if self.current_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.current_tab - 1
            };
        }
        Ok(())
    }
    
    /// 关闭当前标签页
    pub fn close_current_tab(&mut self) -> Result<()> {
        if self.tabs.len() <= 1 {
            return Err(FKVimError::EditorError("不能关闭最后一个标签页".to_string()));
        }
        
        self.tabs.remove(self.current_tab);
        if self.current_tab >= self.tabs.len() {
            self.current_tab = self.tabs.len() - 1;
        }
        
        Ok(())
    }
    
    /// 获取当前标签页
    pub fn current_tab(&self) -> Result<&Tab> {
        self.tabs.get(self.current_tab)
            .ok_or_else(|| FKVimError::EditorError("无效的标签页索引".to_string()))
    }
    
    /// 获取当前标签页的可变引用
    pub fn current_tab_mut(&mut self) -> Result<&mut Tab> {
        self.tabs.get_mut(self.current_tab)
            .ok_or_else(|| FKVimError::EditorError("无效的标签页索引".to_string()))
    }
    
    /// 生成下一个窗口ID
    fn next_window_id(&mut self) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        id
    }

    /// 检查是否没有标签页
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
    
    /// 获取所有标签页ID
    pub fn get_tab_ids(&self) -> Vec<TabId> {
        (0..self.tabs.len()).map(|i| TabId(i)).collect()
    }
    
    /// 获取指定标签页(可变)
    pub fn get_tab_mut(&mut self, tab_id: TabId) -> Result<&mut Tab> {
        self.tabs.get_mut(tab_id.0)
            .ok_or_else(|| FKVimError::EditorError(format!("标签页 {:?} 不存在", tab_id)))
    }
    
    /// 获取指定标签页(只读)
    pub fn get_tab(&self, tab_id: TabId) -> Result<&Tab> {
        self.tabs.get(tab_id.0)
            .ok_or_else(|| FKVimError::EditorError(format!("标签页 {:?} 不存在", tab_id)))
    }
}

/// 标签页结构
pub struct Tab {
    /// 标签页名称
    pub name: String,
    /// 窗口列表
    pub windows: Vec<Window>,
    /// 活动窗口索引
    pub active_window: usize,
    /// 窗口布局
    pub layout: Layout,
}

impl Tab {
    /// 创建新的标签页
    pub fn new(name: String, window_id: WindowId) -> Self {
        // 创建初始窗口
        let window = Window::new(window_id, 0);
        
        Self {
            name,
            windows: vec![window],
            active_window: 0,
            layout: Layout::Single,
        }
    }
    
    /// 获取当前窗口
    pub fn active_window(&self) -> Result<&Window> {
        self.windows.get(self.active_window)
            .ok_or_else(|| FKVimError::EditorError("无效的窗口索引".to_string()))
    }
    
    /// 获取当前窗口的可变引用
    pub fn active_window_mut(&mut self) -> Result<&mut Window> {
        self.windows.get_mut(self.active_window)
            .ok_or_else(|| FKVimError::EditorError("无效的窗口索引".to_string()))
    }
    
    /// 获取活动窗口ID
    pub fn active_window_id(&self) -> Option<WindowId> {
        self.windows.get(self.active_window).map(|w| w.id)
    }
    
    /// 设置活动窗口
    pub fn set_active_window(&mut self, window_id: WindowId) -> Result<()> {
        if self.windows.iter().any(|w| w.id == window_id) {
            self.active_window = self.windows.iter().position(|w| w.id == window_id).unwrap();
            Ok(())
        } else {
            Err(FKVimError::EditorError(format!("窗口不存在：{:?}", window_id)))
        }
    }
    
    /// 水平分割当前窗口
    pub fn split_horizontal(&mut self, buffer_idx: usize) -> Result<WindowId> {
        if self.windows.is_empty() {
            return Err(FKVimError::EditorError("没有窗口可分割".to_string()));
        }
        
        // 获取当前窗口
        let current_win_idx = self.active_window;
        let current_win = &self.windows[current_win_idx];
        
        // 生成新窗口ID
        let new_win_id = WindowId(current_win.id.0 + 1); // 简化生成，实际应从TabManager获取
        
        // 创建新窗口
        let new_win = Window::new(new_win_id, buffer_idx);
        
        // 添加到窗口列表
        self.windows.push(new_win);
        
        // 更新布局
        self.layout = match self.windows.len() {
            2 => Layout::Horizontal,
            3 => Layout::HorizontalTriple,
            4 => Layout::Grid,
            _ => Layout::Complex,
        };
        
        // 切换到新窗口
        self.active_window = self.windows.len() - 1;
        
        Ok(new_win_id)
    }
    
    /// 垂直分割当前窗口
    pub fn split_vertical(&mut self, buffer_idx: usize) -> Result<WindowId> {
        if self.windows.is_empty() {
            return Err(FKVimError::EditorError("没有窗口可分割".to_string()));
        }
        
        // 获取当前窗口
        let current_win_idx = self.active_window;
        let current_win = &self.windows[current_win_idx];
        
        // 生成新窗口ID
        let new_win_id = WindowId(current_win.id.0 + 1); // 简化生成，实际应从TabManager获取
        
        // 创建新窗口
        let new_win = Window::new(new_win_id, buffer_idx);
        
        // 添加到窗口列表
        self.windows.push(new_win);
        
        // 更新布局
        self.layout = match self.windows.len() {
            2 => Layout::Vertical,
            3 => Layout::VerticalTriple,
            4 => Layout::Grid,
            _ => Layout::Complex,
        };
        
        // 切换到新窗口
        self.active_window = self.windows.len() - 1;
        
        Ok(new_win_id)
    }
    
    /// 关闭当前窗口
    pub fn close_active_window(&mut self) -> Result<()> {
        if self.windows.len() <= 1 {
            return Err(FKVimError::EditorError("不能关闭最后一个窗口".to_string()));
        }
        
        // 删除当前窗口
        self.windows.remove(self.active_window);
        
        // 更新活动窗口索引
        if self.active_window >= self.windows.len() {
            self.active_window = self.windows.len() - 1;
        }
        
        // 更新布局
        self.layout = match self.windows.len() {
            1 => Layout::Single,
            2 => Layout::Horizontal, // 默认回退到水平布局
            3 => Layout::HorizontalTriple, // 默认回退到三分布局
            4 => Layout::Grid,
            _ => Layout::Complex,
        };
        
        Ok(())
    }
    
    /// 获取所有窗口
    pub fn get_windows(&self) -> &[Window] {
        &self.windows
    }
    
    /// 获取窗口布局
    pub fn get_layout(&self) -> &Layout {
        &self.layout
    }
    
    /// 切换到下一个窗口
    pub fn next_window(&mut self) -> Result<()> {
        if !self.windows.is_empty() {
            self.active_window = (self.active_window + 1) % self.windows.len();
        }
        Ok(())
    }
    
    /// 切换到上一个窗口
    pub fn prev_window(&mut self) -> Result<()> {
        if !self.windows.is_empty() {
            self.active_window = if self.active_window == 0 {
                self.windows.len() - 1
            } else {
                self.active_window - 1
            };
        }
        Ok(())
    }
    
    /// 焦点移到左侧窗口
    pub fn focus_left_window(&mut self) -> Result<()> {
        // 简化实现，仅切换到上一个窗口
        self.prev_window()
    }
    
    /// 焦点移到右侧窗口
    pub fn focus_right_window(&mut self) -> Result<()> {
        // 简化实现，仅切换到下一个窗口
        self.next_window()
    }
    
    /// 焦点移到上方窗口
    pub fn focus_up_window(&mut self) -> Result<()> {
        // 简化实现，仅切换到上一个窗口
        self.prev_window()
    }
    
    /// 焦点移到下方窗口
    pub fn focus_down_window(&mut self) -> Result<()> {
        // 简化实现，仅切换到下一个窗口
        self.next_window()
    }

    /// 获取窗口通过ID
    pub fn get_window_mut(&mut self, window_id: WindowId) -> Option<&mut Window> {
        for window in &mut self.windows {
            if window.id == window_id {
                return Some(window);
            }
        }
        None
    }
    
    /// 获取窗口(只读)
    pub fn get_window(&self, window_id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|window| window.id == window_id)
    }
    
    /// 添加一个新窗口
    pub fn add_window(&mut self, window: Window) -> WindowId {
        let window_id = window.id;
        self.windows.push(window);
        window_id
    }
    
    /// 分割窗口
    pub fn split(&mut self, source_id: WindowId, new_id: WindowId, direction: Split) -> Result<()> {
        // 确保两个窗口都存在
        if self.get_window(source_id).is_none() {
            return Err(FKVimError::EditorError(format!("源窗口 {:?} 不存在", source_id)));
        }
        
        if self.get_window(new_id).is_none() {
            return Err(FKVimError::EditorError(format!("新窗口 {:?} 不存在", new_id)));
        }
        
        // 根据分割方向更新布局
        match direction {
            Split::Horizontal => {
                self.layout = if self.windows.len() <= 2 {
                    Layout::Horizontal
                } else {
                    Layout::Complex
                };
            },
            Split::Vertical => {
                self.layout = if self.windows.len() <= 2 {
                    Layout::Vertical
                } else {
                    Layout::Complex
                };
            }
        }
        
        Ok(())
    }
    
    /// 删除窗口
    pub fn remove_window(&mut self, window_id: WindowId) -> Result<()> {
        let idx = self.windows.iter().position(|w| w.id == window_id)
            .ok_or_else(|| FKVimError::EditorError(format!("窗口 {:?} 不存在", window_id)))?;
        
        self.windows.remove(idx);
        
        if self.active_window >= self.windows.len() && !self.windows.is_empty() {
            self.active_window = self.windows.len() - 1;
        }
        
        Ok(())
    }
    
    /// 焦点移动到左侧窗口
    pub fn focus_left(&mut self) -> Result<()> {
        // 简化实现：只是移动到上一个窗口
        self.prev_window()
    }
    
    /// 焦点移动到右侧窗口
    pub fn focus_right(&mut self) -> Result<()> {
        // 简化实现：只是移动到下一个窗口
        self.next_window()
    }
    
    /// 焦点移动到上方窗口
    pub fn focus_up(&mut self) -> Result<()> {
        // 简化实现：只是移动到上一个窗口
        self.prev_window()
    }
    
    /// 焦点移动到下方窗口
    pub fn focus_down(&mut self) -> Result<()> {
        // 简化实现：只是移动到下一个窗口
        self.next_window()
    }
    
    /// 获取所有窗口ID
    pub fn get_window_ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|w| w.id).collect()
    }

    /// 设置标签页标题
    pub fn set_title(&mut self, title: String) {
        self.name = title;
    }
}

/// 窗口结构
pub struct Window {
    /// 窗口ID
    pub id: WindowId,
    /// 缓冲区索引
    pub buffer_idx: usize,
    /// 窗口标题
    pub title: Option<String>,
    /// 视口滚动位置（行偏移，列偏移）
    pub scroll: (usize, usize),
    /// 光标行位置
    pub cursor_line: usize,
    /// 光标列位置
    pub cursor_col: usize,
    /// 窗口高度
    pub height: usize,
    /// 窗口宽度
    pub width: usize,
}

impl Window {
    /// 创建新的窗口
    pub fn new(id: WindowId, buffer_idx: usize) -> Self {
        Self {
            id,
            buffer_idx,
            title: None,
            scroll: (0, 0),
            cursor_line: 0,
            cursor_col: 0,
            height: 10, // 默认高度
            width: 80,  // 默认宽度
        }
    }
    
    /// 获取窗口ID
    pub fn id(&self) -> WindowId {
        self.id
    }
    
    /// 获取缓冲区索引
    pub fn buffer_id(&self) -> usize {
        self.buffer_idx
    }
    
    /// 设置缓冲区ID
    pub fn set_buffer(&mut self, buffer_idx: usize) {
        self.buffer_idx = buffer_idx;
    }
    
    /// 获取滚动位置
    pub fn scroll_offset(&self) -> (usize, usize) {
        self.scroll
    }
    
    /// 设置滚动位置
    pub fn set_scroll(&mut self, line: usize, col: usize) {
        self.scroll = (line, col);
    }
    
    /// 设置窗口尺寸
    pub fn set_size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }
    
    /// 确保光标可见
    pub fn ensure_cursor_visible(&mut self) {
        // 使用实际窗口高度和宽度，而不是硬编码的值
        let height = self.height;
        let width = self.width;
        
        // 调整垂直滚动
        if self.cursor_line < self.scroll.0 {
            // 光标在可见区域上方，向上滚动
            self.scroll.0 = self.cursor_line;
        } else if self.cursor_line >= self.scroll.0 + height {
            // 光标在可见区域下方，向下滚动
            self.scroll.0 = self.cursor_line - height + 1;
        }
        
        // 特殊处理：当光标在最后一行且位于行首时，确保有足够的上下文
        // 这有助于在按Enter键创建新行时提供更好的视觉体验
        if self.cursor_col == 0 && self.cursor_line > 0 {
            // 确保前一行也可见，提供上下文
            if self.cursor_line == self.scroll.0 {
                // 如果光标行正好是第一个可见行，向上滚动一行
                if self.scroll.0 > 0 {
                    self.scroll.0 -= 1;
                }
            }
        }
        
        // 调整水平滚动
        if self.cursor_col < self.scroll.1 {
            // 光标在可见区域左侧，向左滚动
            self.scroll.1 = self.cursor_col;
        } else if self.cursor_col >= self.scroll.1 + width {
            // 光标在可见区域右侧，向右滚动
            self.scroll.1 = self.cursor_col - width + 1;
        }
    }
    
    /// 更新光标位置并确保可见
    pub fn update_cursor(&mut self, line: usize, col: usize) {
        self.cursor_line = line;
        self.cursor_col = col;
        self.ensure_cursor_visible();
    }
}

/// 窗口布局类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layout {
    /// 单窗口
    Single,
    /// 水平分割（上下两个窗口）
    Horizontal,
    /// 垂直分割（左右两个窗口）
    Vertical,
    /// 水平三分（上中下三个窗口）
    HorizontalTriple,
    /// 垂直三分（左中右三个窗口）
    VerticalTriple,
    /// 四分格（2x2网格）
    Grid,
    /// 复杂布局（更多窗口）
    Complex,
}

/// 矩形区域
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Layout {
    /// 计算窗口区域
    pub fn calculate_areas(&self, total_area: Rect, window_count: usize) -> Vec<Rect> {
        match (self, window_count) {
            (Layout::Single, _) | (_, 0) | (_, 1) => {
                // 单窗口或只有一个窗口
                vec![total_area]
            },
            (Layout::Horizontal, 2) => {
                // 水平分割两个窗口
                let half_height = total_area.height / 2;
                vec![
                    Rect { x: total_area.x, y: total_area.y, width: total_area.width, height: half_height },
                    Rect { x: total_area.x, y: total_area.y + half_height, width: total_area.width, height: total_area.height - half_height },
                ]
            },
            (Layout::Vertical, 2) => {
                // 垂直分割两个窗口
                let half_width = total_area.width / 2;
                vec![
                    Rect { x: total_area.x, y: total_area.y, width: half_width, height: total_area.height },
                    Rect { x: total_area.x + half_width, y: total_area.y, width: total_area.width - half_width, height: total_area.height },
                ]
            },
            (Layout::HorizontalTriple, 3) => {
                // 水平三分
                let height_per_window = total_area.height / 3;
                vec![
                    Rect { x: total_area.x, y: total_area.y, width: total_area.width, height: height_per_window },
                    Rect { x: total_area.x, y: total_area.y + height_per_window, width: total_area.width, height: height_per_window },
                    Rect { x: total_area.x, y: total_area.y + 2 * height_per_window, width: total_area.width, height: total_area.height - 2 * height_per_window },
                ]
            },
            (Layout::VerticalTriple, 3) => {
                // 垂直三分
                let width_per_window = total_area.width / 3;
                vec![
                    Rect { x: total_area.x, y: total_area.y, width: width_per_window, height: total_area.height },
                    Rect { x: total_area.x + width_per_window, y: total_area.y, width: width_per_window, height: total_area.height },
                    Rect { x: total_area.x + 2 * width_per_window, y: total_area.y, width: total_area.width - 2 * width_per_window, height: total_area.height },
                ]
            },
            (Layout::Grid, 4) => {
                // 2x2网格
                let half_width = total_area.width / 2;
                let half_height = total_area.height / 2;
                vec![
                    Rect { x: total_area.x, y: total_area.y, width: half_width, height: half_height },
                    Rect { x: total_area.x + half_width, y: total_area.y, width: total_area.width - half_width, height: half_height },
                    Rect { x: total_area.x, y: total_area.y + half_height, width: half_width, height: total_area.height - half_height },
                    Rect { x: total_area.x + half_width, y: total_area.y + half_height, width: total_area.width - half_width, height: total_area.height - half_height },
                ]
            },
            _ => {
                // 复杂布局或窗口数与布局不匹配
                // 默认使用水平等分
                let mut areas = Vec::with_capacity(window_count);
                if window_count > 0 {
                    let height_per_window = total_area.height / window_count as u16;
                    for i in 0..window_count {
                        let is_last = i == window_count - 1;
                        let area_height = if is_last {
                            total_area.height - (height_per_window * (window_count - 1) as u16)
                        } else {
                            height_per_window
                        };
                        
                        areas.push(Rect {
                            x: total_area.x,
                            y: total_area.y + (height_per_window * i as u16),
                            width: total_area.width,
                            height: area_height,
                        });
                    }
                }
                areas
            }
        }
    }
    
    /// 计算单个窗口区域
    pub fn calculate_area(&self, total_area: Rect, window_idx: usize, window_count: usize) -> Rect {
        let areas = self.calculate_areas(total_area, window_count);
        if window_idx < areas.len() {
            areas[window_idx]
        } else {
            // 如果索引超出范围，返回整个区域
            total_area
        }
    }
}