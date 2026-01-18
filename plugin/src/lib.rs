//! FuckVim WASM Plugin
//!
//! 这个插件模拟 "AI处理" 逻辑。
//! 它接收编辑器缓冲区内容，查找以 ";;" 开头的行，
//! 并将 ";;" 后面的文本转换为大写（模拟AI生成）。

use extism_pdk::*;

/// 命令前缀 - 以此开头的行会被"AI"处理
const COMMAND_PREFIX: &str = ";;";

/// process_command - 主入口函数
///
/// # 为什么用 #[plugin_fn]？
/// 这个宏将函数导出为WASM可调用的外部函数。
/// Go Host通过Extism SDK调用这个函数名来执行逻辑。
///
/// # 参数
/// - input: 整个编辑器缓冲区的文本（多行，用换行符分隔）
///
/// # 返回
/// - 处理后的缓冲区文本（";;" 后的内容已被转换为大写）
#[plugin_fn]
pub fn process_command(input: String) -> FnResult<String> {
    // 按行处理缓冲区
    let processed_lines: Vec<String> = input
        .lines()
        .map(|line| process_single_line(line))
        .collect();

    // 用换行符重新连接所有行
    Ok(processed_lines.join("\n"))
}

/// 处理单行文本
///
/// 如果行以 ";;" 开头，将后面的内容转换为大写。
/// 这是一个简化的 "AI模拟" - 在真实场景中，这里可以调用LLM API。
fn process_single_line(line: &str) -> String {
    if let Some(content) = line.strip_prefix(COMMAND_PREFIX) {
        // 保留前缀，将内容转为大写
        // 例如: ";;hello world" -> ";;HELLO WORLD"
        format!("{}{}", COMMAND_PREFIX, content.to_uppercase())
    } else {
        // 非命令行保持原样
        line.to_string()
    }
}

/// predict_code - 预测下一段代码 (Ghost Text)
///
/// 这是一个模拟 AI 补全的函数。
/// 输入: 当前正在编辑的行
/// 输出: 建议的补全文本 (Ghost Text)
#[plugin_fn]
pub fn predict_code(input: String) -> FnResult<String> {
    // 简单的关键词匹配模拟
    let prediction = if input.trim_end().ends_with("func") {
        " main() {"
    } else if input.trim_end().ends_with("if") {
        " err != nil {"
    } else if input.trim_end().ends_with("return") {
        " nil"
    } else {
        ""
    };

    Ok(prediction.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_processing() {
        assert_eq!(process_single_line(";;hello"), ";;HELLO");
        assert_eq!(process_single_line(";;Hello World"), ";;HELLO WORLD");
        assert_eq!(process_single_line("normal line"), "normal line");
        assert_eq!(process_single_line(""), "");
    }
}
