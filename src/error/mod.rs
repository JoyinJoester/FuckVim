use std::io;
use std::fmt;
use std::error::Error;
use mlua::Error as LuaError;

/// 编辑器错误类型
#[derive(Debug)]
pub enum FKVimError {
    /// IO 错误
    IoError(io::Error),
    
    /// 缓冲区错误
    BufferError(String),
    
    /// 编辑器错误
    EditorError(String),
    
    /// 命令错误
    CommandError(String),
    
    /// 配置错误
    ConfigError(String),
    
    /// Lua 脚本错误
    LuaError(LuaError),
    
    /// 插件错误
    PluginError(String),
    
    /// 文件浏览器错误
    FileBrowserError(String),
    
    /// 正则表达式错误
    RegexError(String),
    
    /// 终端错误
    TerminalError(String),
    
    /// 通用错误
    Generic(String),
}

impl fmt::Display for FKVimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FKVimError::IoError(err) => write!(f, "IO错误: {}", err),
            FKVimError::BufferError(msg) => write!(f, "缓冲区错误: {}", msg),
            FKVimError::EditorError(msg) => write!(f, "编辑器错误: {}", msg),
            FKVimError::CommandError(msg) => write!(f, "命令错误: {}", msg),
            FKVimError::ConfigError(msg) => write!(f, "配置错误: {}", msg),
            FKVimError::LuaError(err) => write!(f, "Lua脚本错误: {}", err),
            FKVimError::PluginError(msg) => write!(f, "插件错误: {}", msg),
            FKVimError::FileBrowserError(msg) => write!(f, "文件浏览器错误: {}", msg),
            FKVimError::RegexError(msg) => write!(f, "正则表达式错误: {}", msg),
            FKVimError::TerminalError(msg) => write!(f, "终端错误: {}", msg),
            FKVimError::Generic(msg) => write!(f, "通用错误: {}", msg),
        }
    }
}

impl Error for FKVimError {}

impl From<io::Error> for FKVimError {
    fn from(err: io::Error) -> Self {
        FKVimError::IoError(err)
    }
}

impl From<LuaError> for FKVimError {
    fn from(err: LuaError) -> Self {
        FKVimError::LuaError(err)
    }
}

pub type Result<T> = std::result::Result<T, FKVimError>;