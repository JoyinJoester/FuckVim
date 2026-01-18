// filepath: /home/joyin/桌面/fkvim/src/command/help.rs
use std::collections::HashMap;

/// 命令帮助信息
#[derive(Debug, Clone)]
pub struct CommandHelp {
    /// 命令名称
    pub name: String,
    
    /// 命令描述
    pub description: String,
    
    /// 命令用法
    pub usage: String,
    
    /// 示例
    pub examples: Vec<String>,
    
    /// 分类
    pub category: String,
}

/// 帮助分类
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HelpCategory {
    /// 编辑器基础
    Basics,
    
    /// 文件操作
    Files,
    
    /// 编辑操作
    Editing,
    
    /// 搜索和替换
    Search,
    
    /// 窗口管理
    Windows,
    
    /// 标签页管理
    Tabs,
    
    /// 终端
    Terminal,
    
    /// Lua脚本
    Lua,
    
    /// 插件
    Plugin,
    
    /// 其他
    Misc,
}

impl HelpCategory {
    /// 获取分类名称
    pub fn name(&self) -> &str {
        match self {
            HelpCategory::Basics => "basics",
            HelpCategory::Files => "files",
            HelpCategory::Editing => "editing",
            HelpCategory::Search => "search",
            HelpCategory::Windows => "windows",
            HelpCategory::Tabs => "tabs",
            HelpCategory::Terminal => "terminal",
            HelpCategory::Lua => "lua",
            HelpCategory::Plugin => "plugin",
            HelpCategory::Misc => "misc",
        }
    }
    
    /// 获取分类描述
    pub fn description(&self) -> &str {
        match self {
            HelpCategory::Basics => "编辑器基础命令",
            HelpCategory::Files => "文件操作相关命令",
            HelpCategory::Editing => "编辑操作相关命令",
            HelpCategory::Search => "搜索和替换相关命令",
            HelpCategory::Windows => "窗口管理相关命令",
            HelpCategory::Tabs => "标签页管理相关命令",
            HelpCategory::Terminal => "终端相关命令",
            HelpCategory::Lua => "Lua脚本相关命令",
            HelpCategory::Plugin => "插件相关命令",
            HelpCategory::Misc => "其他命令",
        }
    }
}

/// 帮助系统
pub struct HelpSystem {
    /// 命令帮助信息
    commands: HashMap<String, CommandHelp>,
    
    /// 分类命令列表
    categories: HashMap<HelpCategory, Vec<String>>,
}

impl HelpSystem {
    /// 创建帮助系统
    pub fn new() -> Self {
        let mut help_system = Self {
            commands: HashMap::new(),
            categories: HashMap::new(),
        };
        
        help_system.init_commands();
        help_system
    }
    
    /// 初始化命令帮助信息
    fn init_commands(&mut self) {
        // 基础命令
        self.add_command(CommandHelp {
            name: "q".to_string(),
            description: "退出编辑器".to_string(),
            usage: ":q[uit]".to_string(),
            examples: vec![":q".to_string(), ":quit".to_string()],
            category: "basics".to_string(),
        });
        
        self.add_command(CommandHelp {
            name: "w".to_string(),
            description: "保存当前文件".to_string(),
            usage: ":w[rite] [file]".to_string(),
            examples: vec![":w".to_string(), ":write".to_string(), ":w newfile.txt".to_string()],
            category: "files".to_string(),
        });
        
        self.add_command(CommandHelp {
            name: "wq".to_string(),
            description: "保存并退出".to_string(),
            usage: ":wq [file]".to_string(),
            examples: vec![":wq".to_string(), ":wq newfile.txt".to_string()],
            category: "basics".to_string(),
        });
        
        // 文件操作命令
        self.add_command(CommandHelp {
            name: "e".to_string(),
            description: "编辑文件".to_string(),
            usage: ":e[dit] <file>".to_string(),
            examples: vec![":e file.txt".to_string(), ":edit src/main.rs".to_string()],
            category: "files".to_string(),
        });
        
        // 添加更多命令帮助信息
        // 搜索和替换
        self.add_command(CommandHelp {
            name: "find".to_string(),
            description: "在当前文件中搜索".to_string(),
            usage: ":find <pattern>".to_string(),
            examples: vec![":find hello".to_string()],
            category: "search".to_string(),
        });
        
        // 窗口管理
        self.add_command(CommandHelp {
            name: "split".to_string(),
            description: "水平分割窗口".to_string(),
            usage: ":sp[lit] [file]".to_string(),
            examples: vec![":split".to_string(), ":sp file.txt".to_string()],
            category: "windows".to_string(),
        });
        
        self.add_command(CommandHelp {
            name: "vsplit".to_string(),
            description: "垂直分割窗口".to_string(),
            usage: ":vs[plit] [file]".to_string(),
            examples: vec![":vsplit".to_string(), ":vs file.txt".to_string()],
            category: "windows".to_string(),
        });
        
        // 终端命令
        self.add_command(CommandHelp {
            name: "terminal".to_string(),
            description: "打开或关闭终端".to_string(),
            usage: ":term[inal]".to_string(),
            examples: vec![":terminal".to_string(), ":term".to_string()],
            category: "terminal".to_string(),
        });
    }
    
    /// 添加命令帮助信息
    fn add_command(&mut self, help: CommandHelp) {
        // 添加到命令帮助信息
        self.commands.insert(help.name.clone(), help.clone());
        
        // 添加到分类
        let category = self.parse_category(&help.category).unwrap_or(HelpCategory::Misc);
        let commands = self.categories.entry(category).or_insert_with(Vec::new);
        commands.push(help.name);
    }
    
    /// 获取命令帮助信息
    pub fn get_command(&self, name: &str) -> Option<&CommandHelp> {
        self.commands.get(name)
    }
    
    /// 模糊匹配命令
    pub fn fuzzy_match(&self, partial_name: &str) -> Vec<&CommandHelp> {
        let partial_lower = partial_name.to_lowercase();
        self.commands
            .values()
            .filter(|cmd| cmd.name.to_lowercase().contains(&partial_lower) || 
                         cmd.description.to_lowercase().contains(&partial_lower))
            .collect()
    }
    
    /// 解析分类
    pub fn parse_category(&self, category_name: &str) -> Option<HelpCategory> {
        match category_name.to_lowercase().as_str() {
            "basics" => Some(HelpCategory::Basics),
            "files" => Some(HelpCategory::Files),
            "editing" => Some(HelpCategory::Editing),
            "search" => Some(HelpCategory::Search),
            "windows" => Some(HelpCategory::Windows),
            "tabs" => Some(HelpCategory::Tabs),
            "terminal" => Some(HelpCategory::Terminal),
            "lua" => Some(HelpCategory::Lua),
            "plugin" => Some(HelpCategory::Plugin),
            "misc" => Some(HelpCategory::Misc),
            _ => None,
        }
    }
    
    /// 获取分类下的命令列表
    pub fn get_category_commands(&self, category: &HelpCategory) -> Vec<&CommandHelp> {
        if let Some(command_names) = self.categories.get(category) {
            command_names
                .iter()
                .filter_map(|name| self.commands.get(name))
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// 格式化命令帮助信息
    pub fn format_command_help(&self, help: &CommandHelp) -> String {
        let mut text = String::new();
        
        text.push_str(&format!("命令: {}\n", help.name));
        text.push_str(&format!("描述: {}\n", help.description));
        text.push_str(&format!("用法: {}\n", help.usage));
        
        if !help.examples.is_empty() {
            text.push_str("\n示例:\n");
            for example in &help.examples {
                text.push_str(&format!("  {}\n", example));
            }
        }
        
        text
    }
    
    /// 格式化分类帮助信息
    pub fn format_category_help(&self, category: &HelpCategory) -> String {
        let mut text = String::new();
        
        text.push_str(&format!("分类: {}\n", category.name()));
        text.push_str(&format!("描述: {}\n\n", category.description()));
        
        let commands = self.get_category_commands(category);
        if !commands.is_empty() {
            text.push_str("命令列表:\n");
            for cmd in commands {
                text.push_str(&format!("  {:10} - {}\n", cmd.name, cmd.description));
            }
        } else {
            text.push_str("该分类下暂无命令\n");
        }
        
        text
    }
    
    /// 格式化帮助总览
    pub fn format_help_overview(&self) -> String {
        let mut text = String::new();
        
        text.push_str("FKVim 帮助系统\n\n");
        text.push_str("分类列表:\n");
        
        let categories = [
            HelpCategory::Basics,
            HelpCategory::Files,
            HelpCategory::Editing,
            HelpCategory::Search,
            HelpCategory::Windows,
            HelpCategory::Tabs,
            HelpCategory::Terminal,
            HelpCategory::Lua,
            HelpCategory::Plugin,
            HelpCategory::Misc,
        ];
        
        for category in &categories {
            let cmd_count = self.categories.get(category).map_or(0, |cmds| cmds.len());
            text.push_str(&format!("  {:10} - {} ({} 个命令)\n", 
                                  category.name(), category.description(), cmd_count));
        }
        
        text.push_str("\n使用 :help <命令名> 查看特定命令的帮助\n");
        text.push_str("使用 :help <分类名> 查看特定分类的命令列表\n");
        
        text
    }

    /// 获取通用帮助信息
    pub fn get_general_help(&self) -> String {
        self.format_help_overview()
    }
    
    /// 获取特定主题的帮助信息
    pub fn get_topic_help(&self, topic: &str) -> String {
        // 尝试查找对应的命令
        if let Some(command) = self.get_command(topic) {
            return self.format_command_help(command);
        }
        
        // 尝试查找对应的分类
        if let Some(category) = self.parse_category(topic) {
            return self.format_category_help(&category);
        }
        
        // 尝试模糊匹配命令
        let matches = self.fuzzy_match(topic);
        if !matches.is_empty() {
            let mut result = format!("找到与 \"{}\" 相关的命令:\n\n", topic);
            for cmd in matches {
                result.push_str(&format!("{:10} - {}\n", cmd.name, cmd.description));
            }
            return result;
        }
        
        // 没有找到相关信息，返回默认帮助
        format!("没有找到关于 \"{}\" 的帮助信息。\n\n以下是可用的命令分类：\n{}", 
                topic, self.format_help_overview())
    }
}