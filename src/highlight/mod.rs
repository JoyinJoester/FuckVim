mod tree_sitter_highlight;

use std::collections::HashMap;
use std::path::Path;
use crate::error::{Result, FKVimError};
use crossterm::style::{Color, Attribute};

/// 语法高亮风格
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightStyle {
    /// 普通文本
    Normal,
    
    /// 关键字
    Keyword,
    
    /// 字符串
    String,
    
    /// 数字
    Number,
    
    /// 注释
    Comment,
    
    /// 函数名
    Function,
    
    /// 类型
    Type,
    
    /// 运算符
    Operator,
    
    /// 预处理指令
    Preprocessor,
    
    /// 特殊字符
    Special,
    
    /// 错误
    Error,
    
    /// 搜索匹配
    Search,
    
    /// 当前行
    CurrentLine,
    
    /// 标识符
    Identifier,
    
    /// 函数调用
    FunctionCall,
    
    /// 变量
    Variable,
    
    /// 常量
    Constant,
    
    /// 属性
    Property,
    
    /// 字段
    Field,
    
    /// 方法
    Method,
    
    /// 方法调用
    MethodCall,
    
    /// 参数
    Parameter,
    
    /// 文本
    Text,
    
    /// 行号
    LineNumber,
    
    /// 活动行的行号
    LineNumberActive,
}

/// 高亮区域
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    /// 起始行
    pub start_line: usize,
    
    /// 起始列
    pub start_col: usize,
    
    /// 结束行
    pub end_line: usize,
    
    /// 结束列
    pub end_col: usize,
    
    /// 高亮风格
    pub style: HighlightStyle,
}

/// 语法高亮主题
#[derive(Debug, Clone)]
pub struct Theme {
    /// 主题名称
    name: String,
    /// 样式映射
    styles: HashMap<HighlightStyle, StyleAttributes>,
    /// 是否是深色主题
    is_dark: bool,
}

/// 样式属性
#[derive(Debug, Clone)]
pub struct StyleAttributes {
    /// 前景色
    foreground: Option<Color>,
    /// 背景色
    background: Option<Color>,
    /// 文本属性 (粗体、斜体等)
    attributes: Vec<Attribute>,
}

impl StyleAttributes {
    /// 创建新的样式属性
    pub fn new(foreground: Option<Color>, background: Option<Color>, attributes: Vec<Attribute>) -> Self {
        Self {
            foreground,
            background,
            attributes,
        }
    }
    
    /// 获取前景色
    pub fn foreground(&self) -> Option<Color> {
        self.foreground
    }
    
    /// 获取背景色
    pub fn background(&self) -> Option<Color> {
        self.background
    }
    
    /// 获取文本属性
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl Theme {
    /// 创建新的主题
    pub fn new(name: &str, is_dark: bool) -> Self {
        Self {
            name: name.to_string(),
            styles: HashMap::new(),
            is_dark,
        }
    }
    
    /// 设置样式
    pub fn set_style(&mut self, style: HighlightStyle, attributes: StyleAttributes) {
        self.styles.insert(style, attributes);
    }
    
    /// 获取样式
    pub fn get_style(&self, style: &HighlightStyle) -> Option<&StyleAttributes> {
        self.styles.get(style)
    }
    
    /// 是否是深色主题
    pub fn is_dark(&self) -> bool {
        self.is_dark
    }
    
    /// 获取主题名称
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// 创建默认明亮主题
    pub fn default_light() -> Self {
        let mut theme = Self::new("Default Light", false);
        
        // 设置默认样式
        theme.set_style(
            HighlightStyle::Keyword, 
            StyleAttributes::new(Some(Color::Blue), None, vec![Attribute::Bold])
        );
        theme.set_style(
            HighlightStyle::String, 
            StyleAttributes::new(Some(Color::Green), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Comment, 
            StyleAttributes::new(Some(Color::Grey), None, vec![Attribute::Italic])
        );
        theme.set_style(
            HighlightStyle::Number, 
            StyleAttributes::new(Some(Color::Magenta), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Function, 
            StyleAttributes::new(Some(Color::Yellow), None, vec![Attribute::Bold])
        );
        theme.set_style(
            HighlightStyle::FunctionCall, 
            StyleAttributes::new(Some(Color::Yellow), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Type, 
            StyleAttributes::new(Some(Color::Cyan), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Variable, 
            StyleAttributes::new(Some(Color::White), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Constant, 
            StyleAttributes::new(Some(Color::Magenta), None, vec![Attribute::Bold])
        );
        theme.set_style(
            HighlightStyle::Operator, 
            StyleAttributes::new(Some(Color::Red), None, vec![])
        );
        
        // 为其他高亮样式设置默认值
        // 这里只列出了几个示例，实际应用中需要为所有样式设置合适的值
        
        theme
    }
    
    /// 创建默认深色主题
    pub fn default_dark() -> Self {
        let mut theme = Self::new("Default Dark", true);
        
        // 设置默认样式
        theme.set_style(
            HighlightStyle::Keyword, 
            StyleAttributes::new(Some(Color::Blue), None, vec![Attribute::Bold])
        );
        theme.set_style(
            HighlightStyle::String, 
            StyleAttributes::new(Some(Color::Green), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Comment, 
            StyleAttributes::new(Some(Color::DarkGrey), None, vec![Attribute::Italic])
        );
        theme.set_style(
            HighlightStyle::Number, 
            StyleAttributes::new(Some(Color::Magenta), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Function, 
            StyleAttributes::new(Some(Color::Yellow), None, vec![Attribute::Bold])
        );
        theme.set_style(
            HighlightStyle::FunctionCall, 
            StyleAttributes::new(Some(Color::Yellow), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Type, 
            StyleAttributes::new(Some(Color::Cyan), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Variable, 
            StyleAttributes::new(Some(Color::White), None, vec![])
        );
        theme.set_style(
            HighlightStyle::Constant, 
            StyleAttributes::new(Some(Color::Magenta), None, vec![Attribute::Bold])
        );
        theme.set_style(
            HighlightStyle::Operator, 
            StyleAttributes::new(Some(Color::Red), None, vec![])
        );
        
        // 为其他高亮样式设置默认值
        // 这里只列出了几个示例，实际应用中需要为所有样式设置合适的值
        
        theme
    }
    
    /// 从配置加载主题
    pub fn from_config(config: &HashMap<String, String>, name: &str, is_dark: bool) -> Result<Self> {
        let mut theme = Self::new(name, is_dark);
        
        // 这里是一个示例，实际实现中会解析配置文件中的主题设置
        // 例如，config 可能包含 "keyword.foreground=blue" 等键值对
        
        for (key, value) in config {
            if let Some((style_name, attr_name)) = key.split_once('.') {
                let style = parse_style_name(style_name)?;
                let current_attrs = theme.styles.entry(style).or_insert_with(|| 
                    StyleAttributes::new(None, None, vec![]));
                
                match attr_name {
                    "foreground" => {
                        current_attrs.foreground = parse_color(value)?;
                    },
                    "background" => {
                        current_attrs.background = parse_color(value)?;
                    },
                    "bold" => {
                        if value.to_lowercase() == "true" {
                            current_attrs.attributes.push(Attribute::Bold);
                        }
                    },
                    "italic" => {
                        if value.to_lowercase() == "true" {
                            current_attrs.attributes.push(Attribute::Italic);
                        }
                    },
                    // 其他属性...
                    _ => {}
                }
            }
        }
        
        Ok(theme)
    }
}

/// 解析样式名称
fn parse_style_name(name: &str) -> Result<HighlightStyle> {
    match name.to_lowercase().as_str() {
        "keyword" => Ok(HighlightStyle::Keyword),
        "identifier" => Ok(HighlightStyle::Identifier),
        "string" => Ok(HighlightStyle::String),
        "comment" => Ok(HighlightStyle::Comment),
        "number" => Ok(HighlightStyle::Number),
        "function" => Ok(HighlightStyle::Function),
        "functioncall" => Ok(HighlightStyle::FunctionCall),
        "type" => Ok(HighlightStyle::Type),
        "preprocessor" => Ok(HighlightStyle::Preprocessor),
        "operator" => Ok(HighlightStyle::Operator),
        "variable" => Ok(HighlightStyle::Variable),
        "constant" => Ok(HighlightStyle::Constant),
        "property" => Ok(HighlightStyle::Property),
        "field" => Ok(HighlightStyle::Field),
        "method" => Ok(HighlightStyle::Method),
        "methodcall" => Ok(HighlightStyle::MethodCall),
        "parameter" => Ok(HighlightStyle::Parameter),
        // 其他样式类型...
        _ => Err(FKVimError::ConfigError(format!("未知的样式名称: {}", name)))
    }
}

/// 解析颜色
fn parse_color(color_name: &str) -> Result<Option<Color>> {
    match color_name.to_lowercase().as_str() {
        "black" => Ok(Some(Color::Black)),
        "darkgrey" | "darkgray" => Ok(Some(Color::DarkGrey)),
        "red" => Ok(Some(Color::Red)),
        "darkred" => Ok(Some(Color::DarkRed)),
        "green" => Ok(Some(Color::Green)),
        "darkgreen" => Ok(Some(Color::DarkGreen)),
        "yellow" => Ok(Some(Color::Yellow)),
        "darkyellow" => Ok(Some(Color::DarkYellow)),
        "blue" => Ok(Some(Color::Blue)),
        "darkblue" => Ok(Some(Color::DarkBlue)),
        "magenta" => Ok(Some(Color::Magenta)),
        "darkmagenta" => Ok(Some(Color::DarkMagenta)),
        "cyan" => Ok(Some(Color::Cyan)),
        "darkcyan" => Ok(Some(Color::DarkCyan)),
        "white" => Ok(Some(Color::White)),
        "grey" | "gray" => Ok(Some(Color::Grey)),
        "none" | "" => Ok(None),
        _ => {
            // 尝试解析RGB颜色，格式为 #RRGGBB
            if color_name.starts_with('#') && color_name.len() == 7 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&color_name[1..3], 16),
                    u8::from_str_radix(&color_name[3..5], 16),
                    u8::from_str_radix(&color_name[5..7], 16)
                ) {
                    return Ok(Some(Color::Rgb { r, g, b }));
                }
            }
            
            Err(FKVimError::ConfigError(format!("无效的颜色名称: {}", color_name)))
        }
    }
}

/// 语法高亮处理器特性
pub trait SyntaxHighlighter: Send + Sync {
    /// 高亮文本
    fn highlight(&self, text: &str) -> Result<Vec<HighlightSpan>>;
    
    /// 获取语法高亮处理器的名称
    fn name(&self) -> &str;
}

/// 语法高亮处理器
pub struct Highlighter {
    /// 语言定义
    language_map: HashMap<String, Box<dyn SyntaxHighlighter>>,
    /// 当前主题
    current_theme: Theme,
}

impl Highlighter {
    /// 创建新的语法高亮处理器
    pub fn new() -> Self {
        let mut highlighter = Self {
            language_map: HashMap::new(),
            current_theme: Theme::default_dark(), // 默认使用深色主题
        };
        
        // 初始化默认的语法高亮处理器
        highlighter.register_default_highlighters();
        
        highlighter
    }
    
    /// 注册默认的语法高亮处理器
    fn register_default_highlighters(&mut self) {
        // 在实际实现中，这里会加载各种语言的语法高亮
        // 例如，为 Rust、C/C++、Python 等添加高亮支持
        self.register_highlighter("rs", Box::new(tree_sitter_highlight::RustHighlighter::new()));
        self.register_highlighter("lua", Box::new(tree_sitter_highlight::LuaHighlighter::new()));
    }
    
    /// 注册语法高亮处理器
    pub fn register_highlighter(&mut self, extension: &str, highlighter: Box<dyn SyntaxHighlighter>) {
        self.language_map.insert(extension.to_string(), highlighter);
    }
    
    /// 根据文件扩展名获取适当的语法高亮处理器
    pub fn get_highlighter_for_file(&self, file_path: &Path) -> Option<&dyn SyntaxHighlighter> {
        file_path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.language_map.get(ext))
            .map(|h| h.as_ref())
    }
    
    /// 根据文件类型获取适当的语法高亮处理器
    pub fn get_highlighter_for_filetype(&self, file_type: &str) -> Option<&dyn SyntaxHighlighter> {
        self.language_map.get(file_type).map(|h| h.as_ref())
    }
    
    /// 高亮文本
    pub fn highlight(&self, text: &str, file_type: Option<&str>, file_path: Option<&Path>) -> Result<Vec<HighlightSpan>> {
        // 首先尝试通过文件类型获取高亮处理器
        if let Some(file_type) = file_type {
            if let Some(highlighter) = self.get_highlighter_for_filetype(file_type) {
                return highlighter.highlight(text);
            }
        }
        
        // 如果没有文件类型，尝试通过文件路径获取高亮处理器
        if let Some(file_path) = file_path {
            if let Some(highlighter) = self.get_highlighter_for_file(file_path) {
                return highlighter.highlight(text);
            }
        }
        
        // 如果找不到合适的高亮处理器，返回空的高亮结果
        Ok(Vec::new())
    }
    
    /// 设置当前主题
    pub fn set_theme(&mut self, theme: Theme) {
        self.current_theme = theme;
    }
    
    /// 获取当前主题
    pub fn current_theme(&self) -> &Theme {
        &self.current_theme
    }
    
    /// 获取高亮样式的渲染属性
    pub fn get_style_attributes(&self, style: &HighlightStyle) -> Option<&StyleAttributes> {
        self.current_theme.get_style(style)
    }
}