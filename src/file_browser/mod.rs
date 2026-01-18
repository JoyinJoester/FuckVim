use std::path::{Path, PathBuf};
use std::fs;
use std::collections::{HashSet};
use std::time::SystemTime;
use crate::error::{Result, FKVimError};

/// 文件排序方式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode {
    /// 按名称排序
    Name,
    /// 按修改时间排序
    Time,
    /// 按文件大小排序
    Size,
    /// 按文件类型排序
    Type,
}

/// 文件浏览器视图模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    /// 简单视图，仅显示文件名
    Simple,
    /// 详细视图，显示文件详情
    Detail,
}

/// 文件浏览器过滤器
#[derive(Debug, Clone)]
pub struct FileFilter {
    /// 隐藏或显示隐藏文件
    pub show_hidden: bool,
    /// 文件通配符
    pub pattern: Option<String>,
}

/// 文件项详细信息
#[derive(Debug, Clone)]
pub struct FileItem {
    /// 文件路径
    pub path: PathBuf,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件名
    pub name: String,
    /// 文件大小 (字节)
    pub size: u64,
    /// 最后修改时间
    pub modified: Option<SystemTime>,
    /// 文件类型/扩展名
    pub file_type: String,
}

/// 文件项类型
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// 文件路径
    pub path: PathBuf,
    /// 是否为目录
    pub is_dir: bool,
    /// 文件名
    pub name: String,
    /// 文件大小
    pub size: u64,
    /// 是否被选中
    pub selected: bool,
}

/// 文件浏览器
pub struct FileBrowser {
    /// 当前目录
    pub current_dir: PathBuf,
    
    /// 当前目录中的项目（文件和目录）
    pub items: Vec<PathBuf>,
    
    /// 处理后的文件项列表
    pub file_items: Vec<FileItem>,
    
    /// 筛选后的文件项索引
    pub filtered_indices: Vec<usize>,
    
    /// 当前选择的项目索引
    pub selected_idx: usize,
    
    /// 滚动位置
    pub scroll: usize,
    
    /// 排序模式
    pub sort_mode: SortMode,
    
    /// 是否倒序
    pub sort_reverse: bool,
    
    /// 搜索/过滤关键词
    pub filter_text: String,
    
    /// 是否预览文件
    pub preview_enabled: bool,
    
    /// 预览内容
    pub preview_content: String,
    
    /// 收藏夹目录列表
    pub bookmarks: HashSet<PathBuf>,
    
    /// 是否显示隐藏文件
    pub show_hidden: bool,
    
    /// 目录内容
    pub entries: Vec<FileEntry>,
    
    /// 光标位置
    pub cursor: usize,
    
    /// 视图模式
    pub view_mode: ViewMode,
    
    /// 文件过滤器
    pub filter: FileFilter,
    
    /// 搜索结果
    pub search_results: Option<Vec<usize>>,
    
    /// 当前搜索结果索引
    pub search_idx: usize,
}

// 为FileBrowser实现Clone特性
impl Clone for FileBrowser {
    fn clone(&self) -> Self {
        Self {
            current_dir: self.current_dir.clone(),
            items: self.items.clone(),
            file_items: self.file_items.clone(),
            filtered_indices: self.filtered_indices.clone(),
            selected_idx: self.selected_idx,
            scroll: self.scroll,
            sort_mode: self.sort_mode,
            sort_reverse: self.sort_reverse,
            filter_text: self.filter_text.clone(),
            preview_enabled: self.preview_enabled,
            preview_content: self.preview_content.clone(),
            bookmarks: self.bookmarks.clone(),
            show_hidden: self.show_hidden,
            entries: self.entries.clone(),
            cursor: self.cursor,
            view_mode: self.view_mode,
            filter: self.filter.clone(),
            search_results: self.search_results.clone(),
            search_idx: self.search_idx,
        }
    }
}

impl FileBrowser {
    /// 创建一个新的文件浏览器
    pub fn new(path: Option<&Path>) -> Result<Self> {
        let current_dir = if let Some(path) = path {
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    parent.to_path_buf()
                } else {
                    std::env::current_dir()?
                }
            } else {
                path.to_path_buf()
            }
        } else {
            std::env::current_dir()?
        };
        
        let mut file_browser = Self {
            current_dir,
            items: Vec::new(),
            file_items: Vec::new(),
            filtered_indices: Vec::new(),
            selected_idx: 0,
            scroll: 0,
            sort_mode: SortMode::Name,
            sort_reverse: false,
            filter_text: String::new(),
            preview_enabled: false,
            preview_content: String::new(),
            bookmarks: HashSet::new(),
            show_hidden: false,
            entries: Vec::new(),
            cursor: 0,
            view_mode: ViewMode::Detail,
            filter: FileFilter {
                show_hidden: false,
                pattern: None,
            },
            search_results: None,
            search_idx: 0,
        };
        
        file_browser.refresh()?;
        file_browser.update_file_items()?;
        
        Ok(file_browser)
    }
    
    /// 刷新当前目录内容
    pub fn refresh(&mut self) -> Result<()> {
        self.items.clear();
        self.entries.clear();
        
        // 添加 ".." 条目用于返回上一级目录
        let parent_dir = self.current_dir.join("..");
        self.items.push(parent_dir.clone());
        
        // 添加到entries
        self.entries.push(FileEntry {
            path: parent_dir,
            is_dir: true,
            name: "..".to_string(),
            size: 0,
            selected: false,
        });
        
        // 读取当前目录内容
        for entry in fs::read_dir(&self.current_dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_dir = path.is_dir();
            let name = path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "[未知]".to_string());
            
            let size = if is_dir {
                0 // 目录大小暂时不计算
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };
            
            self.items.push(path.clone());
            self.entries.push(FileEntry {
                path,
                is_dir,
                name,
                size,
                selected: false,
            });
        }
        
        // 对内容进行排序：目录在前，文件在后，每组内按名称排序
        self.entries.sort_by(|a, b| {
            if a.is_dir && !b.is_dir {
                std::cmp::Ordering::Less
            } else if !a.is_dir && b.is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });
        
        self.items.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            
            if a_is_dir && !b_is_dir {
                std::cmp::Ordering::Less
            } else if !a_is_dir && b_is_dir {
                std::cmp::Ordering::Greater
            } else {
                let a_name = a.file_name().unwrap_or_default();
                let b_name = b.file_name().unwrap_or_default();
                a_name.cmp(b_name)
            }
        });
        
        // 重置选择索引
        self.selected_idx = 0;
        self.cursor = 0;
        
        Ok(())
    }
    
    /// 移动选择
    pub fn move_selection(&mut self, offset: isize) {
        if self.items.is_empty() {
            return;
        }
        
        let new_idx = self.selected_idx as isize + offset;
        
        if new_idx < 0 {
            self.selected_idx = 0;
        } else if new_idx >= self.items.len() as isize {
            self.selected_idx = self.items.len() - 1;
        } else {
            self.selected_idx = new_idx as usize;
        }
    }
    
    /// 进入选中的目录或打开文件
    pub fn enter_selected(&mut self) -> Result<Option<PathBuf>> {
        if self.items.is_empty() {
            return Ok(None);
        }
        
        let selected_path = &self.items[self.selected_idx];
        
        if selected_path.is_dir() {
            // 进入目录
            self.current_dir = selected_path.canonicalize()?;
            self.refresh()?;
            Ok(None)
        } else {
            // 返回文件路径以便打开
            Ok(Some(selected_path.clone()))
        }
    }
    
    /// 返回上一级目录
    pub fn go_parent(&mut self) -> Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.refresh()?;
        } else {
            return Err(FKVimError::FileBrowserError("已经在根目录".to_string()));
        }
        
        Ok(())
    }
    
    /// 获取当前项目的显示名称
    pub fn get_display_name(&self, idx: usize) -> String {
        if idx >= self.items.len() {
            return String::new();
        }
        
        let path = &self.items[idx];
        let name = path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                if path.as_os_str() == ".." {
                    "..".to_string()
                } else {
                    path.to_string_lossy().to_string()
                }
            });
        
        if path.is_dir() {
            format!("{}/", name)
        } else {
            name
        }
    }

    /// 更新文件项列表，获取详细信息
    pub fn update_file_items(&mut self) -> Result<()> {
        self.file_items.clear();
        
        for path in &self.items {
            // 如果是隐藏文件且不显示隐藏文件，则跳过
            let name = path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            
            if !self.show_hidden && name.starts_with(".") && name != ".." {
                continue;
            }
            
            let is_dir = path.is_dir();
            
            // 获取文件大小
            let size = if is_dir {
                0 // 目录不计算大小
            } else if let Ok(metadata) = path.metadata() {
                metadata.len()
            } else {
                0
            };
            
            // 获取修改时间
            let modified = if let Ok(metadata) = path.metadata() {
                metadata.modified().ok()
            } else {
                None
            };
            
            // 获取文件类型/扩展名
            let file_type = if is_dir {
                "directory".to_string()
            } else {
                path.extension()
                    .map(|ext| ext.to_string_lossy().to_string())
                    .unwrap_or_default()
            };
            
            self.file_items.push(FileItem {
                path: path.clone(),
                is_dir,
                name,
                size,
                modified,
                file_type,
            });
        }
        
        // 根据当前排序模式对文件项进行排序
        self.apply_sort();
        
        // 应用过滤器
        self.apply_filter();
        
        Ok(())
    }
    
    /// 应用当前排序模式
    pub fn apply_sort(&mut self) {
        // 目录始终在前
        self.file_items.sort_by(|a, b| {
            if a.is_dir && !b.is_dir {
                std::cmp::Ordering::Less
            } else if !a.is_dir && b.is_dir {
                std::cmp::Ordering::Greater
            } else {
                // 根据不同的排序模式比较
                let cmp = match self.sort_mode {
                    SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortMode::Size => a.size.cmp(&b.size),
                    SortMode::Time => match (a.modified, b.modified) {
                        (Some(a_time), Some(b_time)) => a_time.cmp(&b_time),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    },
                    SortMode::Type => a.file_type.cmp(&b.file_type),
                };
                
                // 应用排序方向
                if self.sort_reverse {
                    cmp.reverse()
                } else {
                    cmp
                }
            }
        });
    }
    
    /// 切换排序模式
    pub fn toggle_sort_mode(&mut self, mode: SortMode) {
        if self.sort_mode == mode {
            // 如果已经是当前模式，切换排序方向
            self.sort_reverse = !self.sort_reverse;
        } else {
            // 否则切换到新模式，默认正序
            self.sort_mode = mode;
            self.sort_reverse = false;
        }
        
        self.apply_sort();
        self.apply_filter();
    }
    
    /// 应用过滤器
    pub fn apply_filter(&mut self) {
        self.filtered_indices.clear();
        
        if self.filter_text.is_empty() {
            // 如果没有过滤，包含所有项目
            self.filtered_indices = (0..self.file_items.len()).collect();
        } else {
            // 否则只包含匹配的项目
            let filter = self.filter_text.to_lowercase();
            for (idx, item) in self.file_items.iter().enumerate() {
                if item.name.to_lowercase().contains(&filter) {
                    self.filtered_indices.push(idx);
                }
            }
        }
        
        // 重置选择索引
        self.selected_idx = if self.filtered_indices.is_empty() { 0 } else { self.filtered_indices[0] };
    }
    
    /// 设置过滤文本
    pub fn set_filter(&mut self, filter: String) {
        self.filter_text = filter;
        self.apply_filter();
    }
    
    /// 添加当前目录到收藏夹
    pub fn add_to_bookmarks(&mut self) {
        self.bookmarks.insert(self.current_dir.clone());
    }
    
    /// 从收藏夹中移除当前目录
    pub fn remove_from_bookmarks(&mut self) {
        self.bookmarks.remove(&self.current_dir);
    }
    
    /// 切换显示隐藏文件
    pub fn toggle_hidden_files(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.update_file_items().unwrap_or_default();
    }
    
    /// 更新当前选择文件的预览
    pub fn update_preview(&mut self) -> Result<()> {
        if !self.preview_enabled {
            return Ok(());
        }
        
        self.preview_content = String::new();
        
        if self.filtered_indices.is_empty() || self.selected_idx >= self.filtered_indices.len() {
            return Ok(());
        }
        
        let file_idx = self.filtered_indices[self.selected_idx];
        if file_idx >= self.file_items.len() {
            return Ok(());
        }
        
        let selected_item = &self.file_items[file_idx];
        
        // 如果是目录，显示目录信息
        if selected_item.is_dir {
            self.preview_content = format!("目录: {}\n\n", selected_item.path.display());
            
            // 获取目录内容预览
            if let Ok(read_dir) = fs::read_dir(&selected_item.path) {
                for (idx, entry) in read_dir.take(10).enumerate() {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        let is_dir = if path.is_dir() { "/" } else { "" };
                        self.preview_content.push_str(&format!("{}.  {}{}\n", idx + 1, name, is_dir));
                    }
                }
            }
        } else {
            // 如果是文件，尝试读取前几行
            let path = &selected_item.path;
            
            // 文件信息
            if let Ok(metadata) = path.metadata() {
                let size = metadata.len();
                let modified = metadata.modified().ok()
                    .map(|time| format!("{:?}", time))
                    .unwrap_or_else(|| "未知".to_string());
                
                self.preview_content = format!(
                    "文件: {}\n大小: {} 字节\n修改时间: {}\n\n",
                    path.display(), size, modified
                );
            }
            
            // 对于文本文件，显示内容预览
            if let Ok(content) = fs::read_to_string(path) {
                // 只显示前20行或500个字符
                let preview: String = content.lines()
                    .take(20)
                    .collect::<Vec<_>>()
                    .join("\n");
                
                if preview.len() > 500 {
                    self.preview_content.push_str(&preview[..500]);
                    self.preview_content.push_str("\n... (文件过大，仅显示部分内容)");
                } else {
                    self.preview_content.push_str(&preview);
                }
            } else {
                self.preview_content.push_str("(二进制文件，无法预览)");
            }
        }
        
        Ok(())
    }
    
    /// 切换文件预览
    pub fn toggle_preview(&mut self) -> Result<()> {
        self.preview_enabled = !self.preview_enabled;
        if self.preview_enabled {
            self.update_preview()?;
        }
        Ok(())
    }
    
    /// 创建新文件
    pub fn create_file(&mut self, name: &str) -> Result<()> {
        let new_path = self.current_dir.join(name);
        fs::File::create(&new_path)?;
        self.refresh()?;
        self.update_file_items()?;
        
        // 选中新创建的文件
        if let Some(pos) = self.file_items.iter().position(|item| item.path == new_path) {
            self.selected_idx = pos;
        }
        
        Ok(())
    }
    
    /// 创建新目录
    pub fn create_directory(&mut self, name: &str) -> Result<()> {
        let new_dir = self.current_dir.join(name);
        fs::create_dir(&new_dir)?;
        self.refresh()?;
        self.update_file_items()?;
        
        // 选中新创建的目录
        if let Some(pos) = self.file_items.iter().position(|item| item.path == new_dir) {
            self.selected_idx = pos;
        }
        
        Ok(())
    }
    
    /// 删除当前选中的文件或目录
    pub fn delete_selected(&mut self) -> Result<()> {
        if self.filtered_indices.is_empty() || self.selected_idx >= self.filtered_indices.len() {
            return Ok(());
        }
        
        let file_idx = self.filtered_indices[self.selected_idx];
        if file_idx >= self.file_items.len() {
            return Ok(());
        }
        
        let selected_path = self.file_items[file_idx].path.clone();
        
        // 不能删除 ".." 目录
        if selected_path.file_name().unwrap_or_default() == ".." {
            return Err(FKVimError::FileBrowserError("不能删除上级目录引用".to_string()));
        }
        
        if selected_path.is_dir() {
            fs::remove_dir_all(selected_path)?;
        } else {
            fs::remove_file(selected_path)?;
        }
        
        self.refresh()?;
        self.update_file_items()?;
        
        Ok(())
    }
    
    /// 重命名选中的文件或目录
    pub fn rename_selected(&mut self, new_name: &str) -> Result<()> {
        if self.filtered_indices.is_empty() || self.selected_idx >= self.filtered_indices.len() {
            return Ok(());
        }
        
        let file_idx = self.filtered_indices[self.selected_idx];
        if file_idx >= self.file_items.len() {
            return Ok(());
        }
        
        let selected_path = self.file_items[file_idx].path.clone();
        
        // 不能重命名 ".." 目录
        if selected_path.file_name().unwrap_or_default() == ".." {
            return Err(FKVimError::FileBrowserError("不能重命名上级目录引用".to_string()));
        }
        
        let new_path = selected_path.parent()
            .ok_or_else(|| FKVimError::FileBrowserError("无法获取父目录".to_string()))?
            .join(new_name);
        
        fs::rename(selected_path, &new_path)?;
        
        self.refresh()?;
        self.update_file_items()?;
        
        // 选中重命名后的文件
        if let Some(pos) = self.file_items.iter().position(|item| item.path == new_path) {
            self.selected_idx = pos;
        }
        
        Ok(())
    }
    
    /// 复制选中的文件或目录到另一个位置
    pub fn copy_selected(&self) -> Result<PathBuf> {
        if self.filtered_indices.is_empty() || self.selected_idx >= self.filtered_indices.len() {
            return Err(FKVimError::FileBrowserError("没有选中项目".to_string()));
        }
        
        let file_idx = self.filtered_indices[self.selected_idx];
        if file_idx >= self.file_items.len() {
            return Err(FKVimError::FileBrowserError("选中项目无效".to_string()));
        }
        
        let selected_path = self.file_items[file_idx].path.clone();
        
        // 不能复制 ".." 目录
        if selected_path.file_name().unwrap_or_default() == ".." {
            return Err(FKVimError::FileBrowserError("不能复制上级目录引用".to_string()));
        }
        
        Ok(selected_path)
    }
    
    /// 粘贴之前复制的文件或目录到当前目录
    pub fn paste_file(&mut self, source_path: &Path) -> Result<()> {
        let file_name = source_path.file_name()
            .ok_or_else(|| FKVimError::FileBrowserError("无法获取文件名".to_string()))?;
        
        let dest_path = self.current_dir.join(file_name);
        
        // 如果目标路径已存在，添加数字后缀
        let mut final_dest_path = dest_path.clone();
        let mut counter = 1;
        
        while final_dest_path.exists() {
            let stem = dest_path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            
            let ext = dest_path.extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            
            final_dest_path = self.current_dir.join(format!("{} ({}){}", stem, counter, ext));
            counter += 1;
        }
        
        // 执行复制操作
        if source_path.is_dir() {
            // 复制目录需要递归实现
            self.copy_dir_recursively(source_path, &final_dest_path)?;
        } else {
            // 复制文件
            fs::copy(source_path, &final_dest_path)?;
        }
        
        self.refresh()?;
        self.update_file_items()?;
        
        // 选中粘贴后的文件
        if let Some(pos) = self.file_items.iter().position(|item| item.path == final_dest_path) {
            self.selected_idx = pos;
        }
        
        Ok(())
    }
    
    /// 递归复制目录
    fn copy_dir_recursively(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst)?;
        
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            
            if src_path.is_dir() {
                self.copy_dir_recursively(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        
        Ok(())
    }
    
    /// 获取可见项目用于UI显示
    pub fn visible_items(&self, height: usize) -> Vec<&FileItem> {
        let start = self.scroll.min(self.filtered_indices.len().saturating_sub(1));
        let end = (start + height).min(self.filtered_indices.len());
        
        self.filtered_indices[start..end]
            .iter()
            .map(|&idx| &self.file_items[idx])
            .collect()
    }
    
    /// 调整滚动位置确保当前选中项可见
    pub fn ensure_selection_visible(&mut self, height: usize) {
        if self.filtered_indices.is_empty() {
            self.scroll = 0;
            return;
        }
        
        // 查找当前选中项在过滤后列表中的位置
        let selected_pos = self.filtered_indices.iter()
            .position(|&idx| idx == self.selected_idx)
            .unwrap_or(0);
        
        // 如果选中项在可视区域上方，向上滚动
        if selected_pos < self.scroll {
            self.scroll = selected_pos;
        }
        // 如果选中项在可视区域下方，向下滚动
        else if selected_pos >= self.scroll + height {
            self.scroll = selected_pos - height + 1;
        }
    }
    
    /// 跳转到指定书签
    pub fn goto_bookmark(&mut self, bookmark: &Path) -> Result<()> {
        if bookmark.exists() && bookmark.is_dir() {
            self.current_dir = bookmark.to_path_buf();
            self.refresh()?;
            self.update_file_items()?;
            Ok(())
        } else {
            Err(FKVimError::FileBrowserError("无效的书签路径".to_string()))
        }
    }
    
    /// 处理按键事件
    pub fn handle_key(&mut self, key: &str) -> Result<bool> {
        match key {
            "j" | "<Down>" => {
                self.move_cursor_down();
                Ok(true)
            },
            "k" | "<Up>" => {
                self.move_cursor_up();
                Ok(true)
            },
            "<Enter>" => {
                let selected = self.get_selected_item();
                if let Some(selected) = selected {
                    if selected.is_dir {
                        self.enter_directory(&selected.path)?;
                        Ok(true)
                    } else {
                        // 文件浏览器无法自行打开文件，由调用者处理
                        Ok(true)
                    }
                } else {
                    // 没有选中项
                    Ok(false)
                }
            },
            "h" | "<Left>" | "<Backspace>" => {
                self.go_up_directory()?;
                Ok(true)
            },
            " " => { // 空格键切换选中状态
                self.toggle_selection()?;
                self.move_cursor_down(); // 选中后自动下移
                Ok(true)
            },
            "/" => {
                // 在文件浏览器中搜索
                Ok(true) // 实际实现会处理输入等
            },
            _ => Ok(false),
        }
    }
    
    /// 向下移动光标
    pub fn move_cursor_down(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        
        if self.cursor < self.filtered_indices.len() - 1 {
            self.cursor += 1;
        }
    }
    
    /// 向上移动光标
    pub fn move_cursor_up(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }
    
    /// 获取当前选中的文件项
    pub fn get_selected_item(&self) -> Option<FileItem> {
        if self.filtered_indices.is_empty() {
            // 返回None表示没有选中项
            return None;
        }
        
        if self.cursor >= self.filtered_indices.len() {
            return None;
        }
        
        let index = self.filtered_indices[self.cursor];
        if index < self.file_items.len() {
            Some(self.file_items[index].clone())
        } else {
            None
        }
    }
    
    /// 进入指定目录
    pub fn enter_directory(&mut self, path: &Path) -> Result<()> {
        if path.is_dir() {
            self.current_dir = path.to_path_buf();
            self.cursor = 0;
            self.scroll = 0;
            self.refresh()?;
            self.update_file_items()?;
            Ok(())
        } else {
            Err(FKVimError::FileBrowserError("不是有效的目录".to_string()))
        }
    }
    
    /// 返回上层目录
    pub fn go_up_directory(&mut self) -> Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.cursor = 0;
            self.scroll = 0;
            self.refresh()?;
            self.update_file_items()?;
            Ok(())
        } else {
            Err(FKVimError::FileBrowserError("已经在根目录".to_string()))
        }
    }
    
    /// 搜索文件
    pub fn search_files(&mut self, search_term: &str) -> Result<()> {
        self.filter_text = search_term.to_string();
        self.apply_filter();
        Ok(())
    }
    
    /// 获取被选中的项目
    pub fn selected(&self) -> Option<FileItem> {
        if self.filtered_indices.is_empty() || self.cursor >= self.filtered_indices.len() {
            None
        } else {
            let index = self.filtered_indices[self.cursor];
            Some(self.file_items[index].clone())
        }
    }
    
    /// 切换选中状态
    pub fn toggle_selection(&mut self) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        
        if self.cursor < self.entries.len() {
            self.entries[self.cursor].selected = !self.entries[self.cursor].selected;
        }
        
        Ok(())
    }
    
    /// 获取所有选中的项目
    pub fn get_selected_entries(&self) -> Vec<&FileEntry> {
        self.entries.iter().filter(|entry| entry.selected).collect()
    }
    
    /// 清除所有选中
    pub fn clear_selections(&mut self) {
        for entry in &mut self.entries {
            entry.selected = false;
        }
    }
}