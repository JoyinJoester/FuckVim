/// 编辑器状态消息类型
#[derive(Debug, Clone, PartialEq)]
pub enum StatusMessageType {
    /// 信息消息
    Info,
    /// 警告消息
    Warning,
    /// 错误消息
    Error,
    /// 成功消息
    Success,
}

/// 编辑器状态消息
#[derive(Debug, Clone)]
pub struct Status {
    /// 消息内容
    pub message: String,
    /// 消息类型
    pub message_type: StatusMessageType,
    /// 消息时间戳
    pub timestamp: std::time::Instant,
}

impl Status {
    /// 创建一个新的普通信息消息
    pub fn info<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
            message_type: StatusMessageType::Info,
            timestamp: std::time::Instant::now(),
        }
    }

    /// 创建一个新的警告消息
    pub fn warning<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
            message_type: StatusMessageType::Warning,
            timestamp: std::time::Instant::now(),
        }
    }

    /// 创建一个新的错误消息
    pub fn error<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
            message_type: StatusMessageType::Error,
            timestamp: std::time::Instant::now(),
        }
    }

    /// 创建一个新的成功消息
    pub fn success<T: Into<String>>(message: T) -> Self {
        Self {
            message: message.into(),
            message_type: StatusMessageType::Success,
            timestamp: std::time::Instant::now(),
        }
    }
} 