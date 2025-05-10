use ropey::Rope;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use regex::Regex;
use crate::error::{Result, FKVimError};
use crate::highlight::{HighlightSpan, Highlighter};
use crate::history::{History, create_insert_operation, create_delete_operation, Operation};

/// 表示编辑器中的一个缓冲区
pub struct Buffer {
    /// 缓冲区内容
    pub text: Rope,
    
    /// 文件路径
    pub file_path: Option<PathBuf>,
    
    /// 缓冲区是否已修改
    pub modified: bool,
    
    /// 最后修改时间
    pub last_modified: u64,
    
    /// 文件类型
    pub file_type: Option<String>,
    
    /// 语法高亮缓存
    pub syntax_highlights: Option<Vec<HighlightSpan>>,
    
    /// 高亮是否需要更新
    pub highlight_dirty: bool,
    
    /// 编辑历史
    pub history: History,
    
    /// 是否正在执行撤销/重做操作
    is_undoing: bool,

    /// 查找结果
    pub search_results: Option<Vec<SearchResult>>,
    
    /// 当前选中的查找结果索引
    pub current_search_idx: usize,
    
    /// 上次搜索的查询内容
    pub last_search_query: Option<SearchQuery>,
    
    /// 上次替换的内容
    pub last_replace_text: Option<String>,
    
    /// 是否显示搜索高亮
    pub show_search_highlight: bool,
}

/// 查找结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// 起始行
    pub start_line: usize,
    
    /// 起始列
    pub start_col: usize,
    
    /// 结束行
    pub end_line: usize,
    
    /// 结束列
    pub end_col: usize,
}

/// 搜索查询参数
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// 搜索文本或正则表达式
    pub pattern: String,
    
    /// 是否区分大小写
    pub case_sensitive: bool,
    
    /// 是否使用正则表达式
    pub use_regex: bool,
    
    /// 是否全词匹配
    pub whole_word: bool,
    
    /// 是否在选择范围内搜索 (如果有选择)
    pub in_selection: bool,
}

impl SearchQuery {
    /// 创建新的搜索查询
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            in_selection: false,
        }
    }
}

impl Buffer {
    /// 创建一个新的空缓冲区
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            file_path: None,
            modified: false,
            last_modified: current_time_secs(),
            file_type: None,
            syntax_highlights: None,
            highlight_dirty: true,
            history: History::new(1000), // 最多保存1000条历史记录
            is_undoing: false,
            search_results: None,
            current_search_idx: 0,
            last_search_query: None,
            last_replace_text: None,
            show_search_highlight: false,
        }
    }
    
    /// 从文件加载缓冲区
    pub fn from_file(path: &Path) -> Result<Self> {
        // 尝试加载文件内容，如果文件不存在则创建空缓冲区
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    // 文件不存在，创建一个空缓冲区并设置文件路径
                    log::debug!("文件不存在，创建空缓冲区: {}", path.display());
                    String::new()
                } else {
                    // 其他IO错误
                    return Err(FKVimError::IoError(e));
                }
            }
        };
        
        let file_type = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_string());
        
        Ok(Self {
            text: Rope::from_str(&content),
            file_path: Some(path.to_path_buf()),
            modified: false,
            last_modified: current_time_secs(),
            file_type,
            syntax_highlights: None,
            highlight_dirty: true,
            history: History::new(1000),
            is_undoing: false,
            search_results: None,
            current_search_idx: 0,
            last_search_query: None,
            last_replace_text: None,
            show_search_highlight: false,
        })
    }
    
    /// 保存缓冲区到文件
    pub fn save(&mut self) -> Result<()> {
        if let Some(path) = &self.file_path {
            fs::write(path, self.text.to_string())
                .map_err(|e| FKVimError::IoError(e))?;
            
            self.modified = false;
            self.last_modified = current_time_secs();
            Ok(())
        } else {
            Err(FKVimError::BufferError("缓冲区没有关联的文件路径".to_string()))
        }
    }
    
    /// 保存缓冲区到指定文件
    pub fn save_as(&mut self, path: &Path) -> Result<()> {
        fs::write(path, self.text.to_string())
            .map_err(|e| FKVimError::IoError(e))?;
        
        self.file_path = Some(path.to_path_buf());
        self.modified = false;
        self.last_modified = current_time_secs();
        
        // 更新文件类型
        self.file_type = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_string());
        
        Ok(())
    }
    
    /// 插入文本
    pub fn insert(&mut self, line: usize, col: usize, text: &str) -> Result<()> {
        // 检查行列是否有效
        if line >= self.text.len_lines() {
            return Err(FKVimError::BufferError(format!("行号超出范围: {}", line)));
        }
        
        let line_start = self.text.line_to_char(line);
        let line_len = if let Some(line_text) = self.text.get_line(line) {
            line_text.len_chars()
        } else {
            0
        };
        
        // 确保列索引不超过行的长度
        let col = col.min(line_len);
        
        // 计算插入位置的字符索引
        let char_idx = line_start + col;
        
        // 如果不是在撤销操作中，记录此操作
        if !self.is_undoing {
            let edit = create_insert_operation(line, col, text);
            self.history.push(edit);
        }
        
        // 执行插入操作
        self.text.insert(char_idx, text);
        
        // 标记为已修改
        self.modified = true;
        self.last_modified = current_time_secs();
        self.highlight_dirty = true;  // 标记高亮需要更新
        
        Ok(())
    }
    
    /// 删除文本
    pub fn delete(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Result<()> {
        let start_idx = self.line_col_to_char_idx(start_line, start_col)?;
        let end_idx = self.line_col_to_char_idx(end_line, end_col)?;
        
        if start_idx > end_idx {
            return Err(FKVimError::BufferError("无效的删除范围".to_string()));
        }
        
        // 获取将被删除的文本
        let text_to_delete = self.text.slice(start_idx..end_idx).to_string();
        
        // 如果不是在撤销操作中，记录此操作
        if !self.is_undoing {
            let edit = create_delete_operation(start_line, start_col, end_line, end_col, &text_to_delete);
            self.history.push(edit);
        }
        
        self.text.remove(start_idx..end_idx);
        self.modified = true;
        self.last_modified = current_time_secs();
        self.highlight_dirty = true;  // 标记高亮需要更新
        Ok(())
    }
    
    /// 获取指定行的文本
    pub fn get_line(&self, line: usize) -> Option<String> {
        if line >= self.text.len_lines() {
            return None;
        }
        
        let line_start = self.text.line_to_char(line);
        let line_end = if line + 1 < self.text.len_lines() {
            self.text.line_to_char(line + 1) - 1 // 不包括换行符
        } else {
            self.text.len_chars()
        };
        
        Some(self.text.slice(line_start..line_end).to_string())
    }
    
    /// 获取所有行
    pub fn get_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for i in 0..self.text.len_lines() {
            if let Some(line) = self.get_line(i) {
                lines.push(line);
            }
        }
        lines
    }
    
    /// 将行列转换为字符索引
    pub fn line_col_to_char_idx(&self, line: usize, col: usize) -> Result<usize> {
        if line >= self.text.len_lines() {
            return Err(FKVimError::BufferError(format!("行号超出范围: {}", line)));
        }
        
        let line_start = self.text.line_to_char(line);
        let line_len = if line + 1 < self.text.len_lines() {
            self.text.line_to_char(line + 1) - line_start - 1 // 不包括换行符
        } else {
            self.text.len_chars() - line_start
        };
        
        if col > line_len {
            return Err(FKVimError::BufferError(format!("列号超出范围: {}", col)));
        }
        
        Ok(line_start + col)
    }
    
    /// 应用语法高亮
    pub fn apply_syntax_highlight(&mut self, highlighter: &Highlighter) -> Result<()> {
        if !self.highlight_dirty && self.syntax_highlights.is_some() {
            return Ok(());
        }
        
        let text = self.text.to_string();
        let highlights = highlighter.highlight(
            &text,
            self.file_type.as_deref(),
            self.file_path.as_deref()
        )?;
        
        self.syntax_highlights = Some(highlights);
        self.highlight_dirty = false;
        
        Ok(())
    }
    
    /// 撤销上一步操作
    pub fn undo(&mut self, cursor_line: &mut usize, cursor_col: &mut usize) -> Result<bool> {
        if !self.history.can_undo() {
            return Ok(false);
        }
        
        self.is_undoing = true;
        
        if let Some(op) = self.history.undo() {
            match op {
                Operation::Insert(line, col, text) => {
                    // 撤销插入操作就是删除插入的文本
                    self.delete_text(line, col, line, col + text.len())?;
                    *cursor_line = line;
                    *cursor_col = col;
                },
                Operation::Delete(line, col, text) => {
                    // 撤销删除操作就是重新插入删除的文本
                    self.insert_text(line, col, &text)?;
                    *cursor_line = line;
                    *cursor_col = col + text.len();
                },
                Operation::Replace(line, col, old_text, _) => {
                    // 撤销替换操作就是恢复旧文本
                    let end_col = col + old_text.len();
                    self.delete_text(line, col, line, end_col)?;
                    self.insert_text(line, col, &old_text)?;
                    *cursor_line = line;
                    *cursor_col = col + old_text.len();
                }
            }
        }
        
        self.history.finish_undo_redo();
        self.is_undoing = false;
        self.modified = true;
        self.highlight_dirty = true;
        
        Ok(true)
    }
    
    /// 重做操作
    pub fn redo(&mut self, cursor_line: &mut usize, cursor_col: &mut usize) -> Result<bool> {
        if !self.history.can_redo() {
            return Ok(false);
        }
        
        self.is_undoing = true;
        
        if let Some(op) = self.history.redo() {
            match op {
                Operation::Insert(line, col, text) => {
                    // 重做插入操作
                    self.insert_text(line, col, &text)?;
                    *cursor_line = line;
                    *cursor_col = col + text.len();
                },
                Operation::Delete(line, col, text) => {
                    // 重做删除操作
                    let end_col = col + text.len();
                    self.delete_text(line, col, line, end_col)?;
                    *cursor_line = line;
                    *cursor_col = col;
                },
                Operation::Replace(line, col, _, new_text) => {
                    // 重做替换操作
                    let end_col = col + new_text.len();
                    self.delete_text(line, col, line, end_col)?;
                    self.insert_text(line, col, &new_text)?;
                    *cursor_line = line;
                    *cursor_col = col + new_text.len();
                }
            }
        }
        
        self.history.finish_undo_redo();
        self.is_undoing = false;
        self.modified = true;
        self.highlight_dirty = true;
        
        Ok(true)
    }
    
    /// 查找文本
    pub fn search(&mut self, query: &str, case_sensitive: bool) -> Result<usize> {
        if query.is_empty() {
            self.search_results = None;
            return Ok(0);
        }
        
        let _text = self.text.to_string();
        let mut results = Vec::new();
        
        // 转换查询字符串为小写 (如果不区分大小写)
        let search_query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };
        
        // 按行搜索
        for line_idx in 0..self.text.len_lines() {
            let line_start = self.text.line_to_char(line_idx);
            let line_end = if line_idx + 1 < self.text.len_lines() {
                self.text.line_to_char(line_idx + 1) - 1 // 不包括换行符
            } else {
                self.text.len_chars()
            };
            
            let line = self.text.slice(line_start..line_end).to_string();
            
            // 对每行进行搜索 (简单实现，可以改进为更高效的算法)
            let comparison_line = if case_sensitive { line.clone() } else { line.to_lowercase() };
            
            let mut col_idx = 0;
            while col_idx + search_query.len() <= comparison_line.len() {
                let candidate = &comparison_line[col_idx..col_idx + search_query.len()];
                if candidate == search_query {
                    results.push(SearchResult {
                        start_line: line_idx,
                        start_col: col_idx,
                        end_line: line_idx,
                        end_col: col_idx + search_query.len(),
                    });
                    
                    // 跳过当前匹配，继续搜索
                    col_idx += search_query.len();
                } else {
                    col_idx += 1;
                }
            }
        }
        
        let count = results.len();
        if count > 0 {
            self.search_results = Some(results);
            self.current_search_idx = 0;
        } else {
            self.search_results = None;
        }
        
        Ok(count)
    }
    
    /// 获取当前查找结果
    pub fn current_search_result(&self) -> Option<&SearchResult> {
        if let Some(results) = &self.search_results {
            if !results.is_empty() {
                return Some(&results[self.current_search_idx]);
            }
        }
        None
    }
    
    /// 获取下一个搜索结果
    pub fn next_search_result(&mut self) -> Option<&SearchResult> {
        if let Some(results) = &self.search_results {
            if results.is_empty() {
                return None;
            }
            
            if self.current_search_idx >= results.len() - 1 {
                self.current_search_idx = 0;
            } else {
                self.current_search_idx += 1;
            }
            
            return Some(&results[self.current_search_idx]);
        }
        
        None
    }
    
    /// 获取上一个搜索结果
    pub fn prev_search_result(&mut self) -> Option<&SearchResult> {
        if let Some(results) = &self.search_results {
            if results.is_empty() {
                return None;
            }
            
            if self.current_search_idx == 0 {
                self.current_search_idx = results.len() - 1;
            } else {
                self.current_search_idx -= 1;
            }
            
            return Some(&results[self.current_search_idx]);
        }
        
        None
    }
    
    /// 清除查找结果
    pub fn clear_search(&mut self) {
        self.search_results = None;
        self.current_search_idx = 0;
    }

    /// 使用高级搜索查询进行搜索
    pub fn advanced_search(&mut self, query: SearchQuery) -> Result<usize> {
        if query.pattern.is_empty() {
            self.search_results = None;
            self.last_search_query = None;
            return Ok(0);
        }
        
        let mut results = Vec::new();
        
        if query.use_regex {
            // 使用正则表达式搜索
            self.regex_search(&query, &mut results)?;
        } else {
            // 使用普通文本搜索
            self.text_search(&query, &mut results)?;
        }
        
        let count = results.len();
        if count > 0 {
            self.search_results = Some(results);
            self.current_search_idx = 0;
            self.last_search_query = Some(query);
        } else {
            self.search_results = None;
        }
        
        self.show_search_highlight = true;
        
        Ok(count)
    }
    
    /// 正则表达式搜索
    fn regex_search(&self, query: &SearchQuery, results: &mut Vec<SearchResult>) -> Result<()> {
        // 构建正则表达式
        let regex_str = if query.whole_word {
            format!(r"\b{}\b", &query.pattern)
        } else {
            query.pattern.clone()
        };
        
        let regex_options = if !query.case_sensitive {
            "(?i)"
        } else {
            ""
        };
        
        let regex_pattern = format!("{}{}", regex_options, regex_str);
        
        let regex = match Regex::new(&regex_pattern) {
            Ok(re) => re,
            Err(e) => return Err(FKVimError::RegexError(format!("正则表达式错误: {}", e))),
        };
        
        // 按行搜索
        for line_idx in 0..self.text.len_lines() {
            let line = self.get_line(line_idx).ok_or(FKVimError::BufferError(format!("无效的行号: {}", line_idx)))?;
            
            for capture in regex.find_iter(&line) {
                results.push(SearchResult {
                    start_line: line_idx,
                    start_col: capture.start(),
                    end_line: line_idx,
                    end_col: capture.end(),
                });
            }
        }
        
        Ok(())
    }
    
    /// 普通文本搜索
    fn text_search(&self, query: &SearchQuery, results: &mut Vec<SearchResult>) -> Result<()> {
        let search_pattern = if query.case_sensitive {
            query.pattern.clone()
        } else {
            query.pattern.to_lowercase()
        };
        
        for line_idx in 0..self.text.len_lines() {
            let line = self.get_line(line_idx).ok_or(FKVimError::BufferError(format!("无效的行号: {}", line_idx)))?;
            let comparison_line = if query.case_sensitive { line.clone() } else { line.to_lowercase() };
            
            let mut col_idx = 0;
            while col_idx + search_pattern.len() <= comparison_line.len() {
                let candidate = &comparison_line[col_idx..col_idx + search_pattern.len()];
                
                let is_match = if query.whole_word {
                    // 检查是否是完整单词
                    let is_word_boundary_before = col_idx == 0 || !comparison_line.chars().nth(col_idx - 1).unwrap_or(' ').is_alphanumeric();
                    let is_word_boundary_after = col_idx + search_pattern.len() >= comparison_line.len() || 
                                           !comparison_line.chars().nth(col_idx + search_pattern.len()).unwrap_or(' ').is_alphanumeric();
                    
                    candidate == search_pattern && is_word_boundary_before && is_word_boundary_after
                } else {
                    // 普通匹配
                    candidate == search_pattern
                };
                
                if is_match {
                    results.push(SearchResult {
                        start_line: line_idx,
                        start_col: col_idx,
                        end_line: line_idx,
                        end_col: col_idx + search_pattern.len(),
                    });
                    
                    // 跳过当前匹配，继续搜索
                    col_idx += search_pattern.len();
                } else {
                    col_idx += 1;
                }
            }
        }
        
        Ok(())
    }
    
    /// 替换当前匹配
    pub fn replace_current(&mut self, replacement: &str) -> Result<bool> {
        if let Some(result) = self.current_search_result() {
            let start_line = result.start_line;
            let start_col = result.start_col;
            let end_line = result.end_line;
            let end_col = result.end_col;
            
            // 删除当前匹配文本
            self.delete(start_line, start_col, end_line, end_col)?;
            
            // 插入替换文本
            self.insert(start_line, start_col, replacement)?;
            
            // 更新最后一次替换文本
            self.last_replace_text = Some(replacement.to_string());
            
            // 如果是最后一个结果，更新搜索结果（后面的匹配位置已经改变）
            if let Some(results) = &mut self.search_results {
                // 移除当前匹配
                results.remove(self.current_search_idx);
                
                if results.is_empty() {
                    self.search_results = None;
                    return Ok(true);
                }
                
                // 更新当前位置之后的匹配
                let replacement_len = replacement.len();
                let original_len = end_col - start_col;
                let offset = replacement_len as isize - original_len as isize;
                
                if offset != 0 {
                    for result in results.iter_mut().skip(self.current_search_idx) {
                        if result.start_line == start_line && result.start_col > start_col {
                            // 同一行，在替换后的位置
                            result.start_col = (result.start_col as isize + offset) as usize;
                            result.end_col = (result.end_col as isize + offset) as usize;
                        }
                    }
                }
                
                // 如果当前索引超出范围，调整它
                if self.current_search_idx >= results.len() {
                    self.current_search_idx = 0;
                }
            }
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// 替换所有匹配
    pub fn replace_all(&mut self, replacement: &str) -> Result<usize> {
        if self.search_results.is_none() {
            return Ok(0);
        }
        
        // 复制匹配结果，以防在替换过程中修改
        let results = self.search_results.clone().unwrap();
        let count = results.len();
        
        if count == 0 {
            return Ok(0);
        }
        
        // 从后往前替换，这样不会影响前面匹配的位置
        let mut replaced = 0;
        
        // 开始一个复合编辑操作
        self.history.start_compound_operation();
        
        for i in (0..count).rev() {
            let result = &results[i];
            let start_line = result.start_line;
            let start_col = result.start_col;
            let end_line = result.end_line;
            let end_col = result.end_col;
            
            // 删除匹配文本并插入替换文本
            self.delete(start_line, start_col, end_line, end_col)?;
            self.insert(start_line, start_col, replacement)?;
            
            replaced += 1;
        }
        
        // 结束复合编辑操作
        self.history.end_compound_operation();
        
        // 更新最后一次替换文本
        self.last_replace_text = Some(replacement.to_string());
        
        // 清除搜索结果，因为所有匹配都已替换
        self.search_results = None;
        
        Ok(replaced)
    }
    
    /// 使用正则表达式替换
    pub fn replace_regex(&mut self, replacement: &str) -> Result<usize> {
        if let Some(query) = &self.last_search_query {
            if !query.use_regex {
                return self.replace_all(replacement);
            }
            
            // 如果没有搜索结果，返回0
            if self.search_results.is_none() {
                return Ok(0);
            }
            
            let results = self.search_results.clone().unwrap();
            let count = results.len();
            
            if count == 0 {
                return Ok(0);
            }
            
            // 从正则表达式构建替换模式
            let regex_str = if query.whole_word {
                format!(r"\b{}\b", &query.pattern)
            } else {
                query.pattern.clone()
            };
            
            let regex_options = if !query.case_sensitive {
                "(?i)"
            } else {
                ""
            };
            
            let regex_pattern = format!("{}{}", regex_options, regex_str);
            
            let regex = match Regex::new(&regex_pattern) {
                Ok(re) => re,
                Err(e) => return Err(FKVimError::RegexError(format!("正则表达式错误: {}", e))),
            };
            
            // 获取文本内容
            let text = self.text.to_string();
            
            // 执行正则替换
            let new_text = regex.replace_all(&text, replacement);
            
            // 如果文本没有变化，无需更新
            if new_text == text {
                return Ok(0);
            }
            
            // 开始一个复合编辑操作
            self.history.start_compound_operation();
            
            // 清空当前文本
            let last_line = self.text.len_lines() - 1;
            let last_col = self.get_line(last_line).ok_or(FKVimError::BufferError(format!("无效的行号: {}", last_line)))?.len();
            self.delete(0, 0, last_line, last_col)?;
            
            // 插入新文本
            self.insert(0, 0, &new_text)?;
            
            // 结束复合编辑操作
            self.history.end_compound_operation();
            
            // 更新最后一次替换文本
            self.last_replace_text = Some(replacement.to_string());
            
            // 清除搜索结果
            self.search_results = None;
            
            Ok(count)
        } else {
            Ok(0)
        }
    }
    
    /// 插入文本（内部辅助方法）
    fn insert_text(&mut self, line: usize, col: usize, text: &str) -> Result<()> {
        self.insert(line, col, text)
    }
    
    /// 删除文本（内部辅助方法）
    fn delete_text(&mut self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Result<()> {
        self.delete(start_line, start_col, end_line, end_col)
    }

    /// 在缓冲区中查找指定的文本
    pub fn find(&mut self, query: &str, options: &crate::editor::SearchOptions) -> Result<usize> {
        // 创建查询
        let query = SearchQuery {
            pattern: query.to_string(),
            case_sensitive: options.case_sensitive,
            use_regex: options.use_regex,
            whole_word: options.whole_word,
            in_selection: options.in_selection,
        };
        
        // 执行高级搜索
        self.advanced_search(query)
    }

    /// 替换文本
    pub fn replace_term(&mut self, line: usize, col: usize, cursor_line: &mut usize, cursor_col: &mut usize, search_term: &str, new_text: String) -> Result<()> {
        // 查找当前行中的搜索词
        let line_text = match self.get_line(line) {
            Some(text) => text,
            None => return Err(FKVimError::BufferError(format!("行号超出范围: {}", line)))
        };
        
        // 在当前行查找搜索词
        if col < line_text.len() {
            if let Some(start_col) = line_text[col..].find(search_term) {
                let start_col = col + start_col;
                let end_col = start_col + search_term.len();
                
                // 删除原有文本并插入新文本
                self.delete_text(line, start_col, line, end_col)?;
                self.insert_text(line, start_col, &new_text)?;
                
                // 更新光标位置
                *cursor_line = line;
                *cursor_col = start_col + new_text.len();
                
                // 更新修改状态
                self.modified = true;
                
                return Ok(());
            }
        }
        
        Err(FKVimError::BufferError("未找到匹配的文本".to_string()))
    }

    /// 获取语法高亮
    pub fn get_highlights(&self) -> Option<&Vec<HighlightSpan>> {
        self.syntax_highlights.as_ref()
    }

    /// 从文件重新加载缓冲区内容
    pub fn load_from_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| FKVimError::IoError(e))?;
        
        // 清除当前内容
        let text_len = self.text.len_chars();
        if text_len > 0 {
            self.text.remove(0..text_len);
        }
        
        // 插入新内容
        self.text.insert(0, &content);
        
        // 更新文件路径
        self.file_path = Some(path.to_path_buf());
        
        // 尝试检测文件类型
        self.file_type = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase());
            
        // 重置修改状态
        self.modified = false;
        self.last_modified = current_time_secs();
        
        // 重置历史记录
        self.history = History::new(1000);
        
        // 标记高亮需要更新
        self.highlight_dirty = true;
        
        Ok(())
    }

    /// 在指定位置插入文本（简便方法）
    pub fn insert_at(&mut self, line: usize, col: usize, text: &str) -> bool {
        // 如果要插入的是空字符串，直接返回成功
        if text.is_empty() {
            return true;
        }
        
        // 确保行号不超出范围，如果超出就添加足够的空行
        while line >= self.text.len_lines() {
            self.text.insert(self.text.len_chars(), "\n");
        }
        
        // 获取行的长度，确保列号不超出范围
        let line_len = self.get_line(line).map(|s| s.len()).unwrap_or(0);
        let adjusted_col = col.min(line_len);
        
        if let Ok(_) = self.insert(line, adjusted_col, text) {
            return true;
        }
        false
    }
    
    /// 在指定位置删除指定数量的字符（简便方法）
    pub fn delete_at(&mut self, line: usize, col: usize, count: usize) -> bool {
        // 如果count为0，无需删除
        if count == 0 {
            return true;
        }
        
        // 确保行号在有效范围内
        if line >= self.text.len_lines() {
            return false;
        }
        
        // 获取当前行的长度
        let line_len = if let Some(line_text) = self.text.get_line(line) {
            line_text.len_chars()
        } else {
            return false;
        };
        
        // 如果要删除的位置超出行的长度，调整为行的实际长度
        let actual_col = col.min(line_len);
        
        // 计算结束列，不超过行的长度
        let end_col = (actual_col + count).min(line_len);
        
        // 如果起始位置和结束位置相同，无需删除
        if actual_col == end_col {
            return true;
        }
        
        // 执行删除操作
        if let Ok(_) = self.delete(line, actual_col, line, end_col) {
            return true;
        }
        false
    }
}

/// 获取当前时间（秒）
fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            file_path: self.file_path.clone(),
            modified: self.modified,
            last_modified: self.last_modified,
            file_type: self.file_type.clone(),
            syntax_highlights: self.syntax_highlights.clone(),
            highlight_dirty: self.highlight_dirty,
            // 对于 History 创建一个新的实例
            history: History::new(1000),
            is_undoing: self.is_undoing,
            search_results: self.search_results.clone(),
            current_search_idx: self.current_search_idx,
            last_search_query: self.last_search_query.clone(),
            last_replace_text: self.last_replace_text.clone(),
            show_search_highlight: self.show_search_highlight,
        }
    }
}