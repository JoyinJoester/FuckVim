use tui::style::{Color, Style, Modifier};
use serde::{Deserialize, Serialize};

/// 编辑器主题定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// 主题名称
    pub name: String,
    /// 是否是深色主题
    pub is_dark: bool,
    /// 背景色
    pub background: Color,
    /// 前景色
    pub foreground: Color,
    /// 光标颜色
    pub cursor: Color,
    /// 行号颜色
    pub line_number: Color,
    /// 当前行背景色
    pub current_line: Color,
    /// 状态栏背景色
    pub status_background: Color,
    /// 状态栏前景色
    pub status_foreground: Color,
    /// 不同模式的状态栏颜色
    pub mode_colors: ModeColors,
    /// 语法高亮颜色
    pub syntax: SyntaxColors,
}

/// 不同模式的颜色
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeColors {
    pub normal: Color,
    pub insert: Color,
    pub visual: Color,
    pub command: Color,
    pub replace: Color,
}

/// 语法高亮颜色
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxColors {
    pub keyword: Color,
    pub identifier: Color,
    pub string: Color,
    pub comment: Color,
    pub number: Color,
    pub function: Color,
    pub type_name: Color,
    pub preprocessor: Color,
    pub operator: Color,
    pub variable: Color,
    pub constant: Color,
    pub text: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // 默认深色主题
        Self {
            name: "默认深色".to_string(),
            is_dark: true,
            background: Color::Black,
            foreground: Color::White,
            cursor: Color::LightYellow,
            line_number: Color::DarkGray,
            current_line: Color::DarkGray,
            status_background: Color::Blue,
            status_foreground: Color::White,
            mode_colors: ModeColors {
                normal: Color::Green,
                insert: Color::Blue,
                visual: Color::Yellow,
                command: Color::Magenta,
                replace: Color::Red,
            },
            syntax: SyntaxColors {
                keyword: Color::Magenta,
                identifier: Color::White,
                string: Color::Green,
                comment: Color::Gray,
                number: Color::Yellow,
                function: Color::Blue,
                type_name: Color::Cyan,
                preprocessor: Color::Red,
                operator: Color::White,
                variable: Color::White,
                constant: Color::Yellow,
                text: Color::White,
                error: Color::Red,
            },
        }
    }
}

/// 预定义主题
impl Theme {
    /// 获取一个浅色主题
    pub fn light() -> Self {
        Self {
            name: "默认浅色".to_string(),
            is_dark: false,
            background: Color::White,
            foreground: Color::Black,
            cursor: Color::DarkBlue,
            line_number: Color::Gray,
            current_line: Color::LightGray,
            status_background: Color::Blue,
            status_foreground: Color::White,
            mode_colors: ModeColors {
                normal: Color::Green,
                insert: Color::Blue,
                visual: Color::Yellow,
                command: Color::Magenta,
                replace: Color::Red,
            },
            syntax: SyntaxColors {
                keyword: Color::Magenta,
                identifier: Color::Black,
                string: Color::DarkGreen,
                comment: Color::DarkGray,
                number: Color::DarkYellow,
                function: Color::DarkBlue,
                type_name: Color::DarkCyan,
                preprocessor: Color::DarkRed,
                operator: Color::Black,
                variable: Color::Black,
                constant: Color::DarkYellow,
                text: Color::Black,
                error: Color::Red,
            },
        }
    }

    /// 获取 Dracula 主题
    pub fn dracula() -> Self {
        Self {
            name: "Dracula".to_string(),
            is_dark: true,
            background: Color::Rgb(40, 42, 54),
            foreground: Color::Rgb(248, 248, 242),
            cursor: Color::Rgb(248, 248, 242),
            line_number: Color::Rgb(98, 114, 164),
            current_line: Color::Rgb(68, 71, 90),
            status_background: Color::Rgb(68, 71, 90),
            status_foreground: Color::Rgb(248, 248, 242),
            mode_colors: ModeColors {
                normal: Color::Rgb(80, 250, 123),
                insert: Color::Rgb(139, 233, 253),
                visual: Color::Rgb(255, 184, 108),
                command: Color::Rgb(189, 147, 249),
                replace: Color::Rgb(255, 85, 85),
            },
            syntax: SyntaxColors {
                keyword: Color::Rgb(255, 121, 198),
                identifier: Color::Rgb(248, 248, 242),
                string: Color::Rgb(241, 250, 140),
                comment: Color::Rgb(98, 114, 164),
                number: Color::Rgb(189, 147, 249),
                function: Color::Rgb(80, 250, 123),
                type_name: Color::Rgb(139, 233, 253),
                preprocessor: Color::Rgb(255, 85, 85),
                operator: Color::Rgb(248, 248, 242),
                variable: Color::Rgb(248, 248, 242),
                constant: Color::Rgb(189, 147, 249),
                text: Color::Rgb(248, 248, 242),
                error: Color::Rgb(255, 85, 85),
            },
        }
    }

    /// 获取 Nord 主题
    pub fn nord() -> Self {
        Self {
            name: "Nord".to_string(),
            is_dark: true,
            background: Color::Rgb(46, 52, 64),
            foreground: Color::Rgb(216, 222, 233),
            cursor: Color::Rgb(216, 222, 233),
            line_number: Color::Rgb(76, 86, 106),
            current_line: Color::Rgb(59, 66, 82),
            status_background: Color::Rgb(59, 66, 82),
            status_foreground: Color::Rgb(236, 239, 244),
            mode_colors: ModeColors {
                normal: Color::Rgb(163, 190, 140),
                insert: Color::Rgb(129, 161, 193),
                visual: Color::Rgb(208, 135, 112),
                command: Color::Rgb(180, 142, 173),
                replace: Color::Rgb(191, 97, 106),
            },
            syntax: SyntaxColors {
                keyword: Color::Rgb(180, 142, 173),
                identifier: Color::Rgb(216, 222, 233),
                string: Color::Rgb(163, 190, 140),
                comment: Color::Rgb(97, 110, 136),
                number: Color::Rgb(180, 142, 173),
                function: Color::Rgb(129, 161, 193),
                type_name: Color::Rgb(143, 188, 187),
                preprocessor: Color::Rgb(191, 97, 106),
                operator: Color::Rgb(216, 222, 233),
                variable: Color::Rgb(216, 222, 233),
                constant: Color::Rgb(180, 142, 173),
                text: Color::Rgb(216, 222, 233),
                error: Color::Rgb(191, 97, 106),
            },
        }
    }
    
    /// 获取特定语法元素的样式
    pub fn get_syntax_style(&self, element: &crate::highlight::HighlightStyle) -> Style {
        use crate::highlight::HighlightStyle;
        
        let color = match element {
            HighlightStyle::Keyword => self.syntax.keyword,
            HighlightStyle::Identifier => self.syntax.identifier,
            HighlightStyle::String => self.syntax.string,
            HighlightStyle::Comment => self.syntax.comment,
            HighlightStyle::Number => self.syntax.number,
            HighlightStyle::Function => self.syntax.function,
            HighlightStyle::Type => self.syntax.type_name,
            HighlightStyle::Preprocessor => self.syntax.preprocessor,
            HighlightStyle::Operator => self.syntax.operator,
            HighlightStyle::Variable => self.syntax.variable,
            HighlightStyle::Constant => self.syntax.constant,
            HighlightStyle::Text => self.syntax.text,
            HighlightStyle::Error => self.syntax.error,
        };
        
        let mut style = Style::default().fg(color);
        
        // 为某些元素添加修饰符
        match element {
            HighlightStyle::Keyword => style = style.add_modifier(Modifier::BOLD),
            HighlightStyle::Function => style = style.add_modifier(Modifier::BOLD),
            HighlightStyle::Comment => style = style.add_modifier(Modifier::ITALIC),
            HighlightStyle::Constant => style = style.add_modifier(Modifier::BOLD),
            HighlightStyle::Error => style = style.add_modifier(Modifier::BOLD),
            _ => {}
        }
        
        style
    }
    
    /// 获取状态栏模式颜色
    pub fn get_mode_color(&self, mode: &crate::editor::EditorMode) -> Color {
        use crate::editor::EditorMode;
        
        match mode {
            EditorMode::Normal => self.mode_colors.normal,
            EditorMode::Insert => self.mode_colors.insert,
            EditorMode::Visual => self.mode_colors.visual,
            EditorMode::Command => self.mode_colors.command,
            EditorMode::Replace => self.mode_colors.replace,
        }
    }
}

/// 所有可用主题的集合
#[derive(Debug, Clone)]
pub struct ThemeManager {
    pub themes: Vec<Theme>,
    pub current_theme_index: usize,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            themes: vec![
                Theme::default(),
                Theme::light(),
                Theme::dracula(),
                Theme::nord(),
            ],
            current_theme_index: 0,
        }
    }
}

impl ThemeManager {
    /// 获取当前主题
    pub fn current_theme(&self) -> &Theme {
        &self.themes[self.current_theme_index]
    }
    
    /// 切换到下一个主题
    pub fn next_theme(&mut self) {
        self.current_theme_index = (self.current_theme_index + 1) % self.themes.len();
    }
    
    /// 切换到上一个主题
    pub fn prev_theme(&mut self) {
        if self.current_theme_index == 0 {
            self.current_theme_index = self.themes.len() - 1;
        } else {
            self.current_theme_index -= 1;
        }
    }
    
    /// 按名称切换主题
    pub fn set_theme_by_name(&mut self, name: &str) -> bool {
        if let Some(index) = self.themes.iter().position(|t| t.name == name) {
            self.current_theme_index = index;
            true
        } else {
            false
        }
    }
    
    /// 添加自定义主题
    pub fn add_theme(&mut self, theme: Theme) {
        self.themes.push(theme);
    }
}