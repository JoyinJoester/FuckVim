mod buffer;
mod command;
mod config;
mod editor;
mod input;
mod plugin;
mod ui;
mod error;
mod terminal;
mod highlight;
mod history;
mod file_browser;

use std::path::Path;
use std::env;
use error::{Result, FKVimError};

fn run() -> Result<()> {
    // 初始化日志
    env_logger::init();
    
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();
    
    // 加载配置
    let config = config::load_config()?;
    
    // 初始化 Lua 环境
    let lua_env = plugin::lua::LuaEnv::new(&config)?;
    
    // 初始化编辑器状态
    let mut editor = editor::Editor::new(config, lua_env)?;
    
    // 如果有文件参数，尝试打开文件
    if args.len() > 1 {
        // 打开第一个文件
        let first_file_path = Path::new(&args[1]);
        editor.open_file(first_file_path)?;
        
        // 如果有多个文件，以水平分割窗口的方式打开其余文件
        for arg in args.iter().skip(2) {
            let file_path = Path::new(arg);
            // 打开文件并获取缓冲区索引
            let buffer_idx = editor.open_file_to_buffer(file_path)?;
            // 水平分割窗口并在新窗口中显示该文件
            editor.split_window_horizontal_with_buffer(buffer_idx)?;
        }
    }
    
    // 启动 UI
    ui::start(&mut editor)?;
    
    Ok(())
}

fn main() {
    // 获取可执行文件名，用于日志文件名称
    let exec_name = env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_else(|| String::from("fkvim"));
    
    // 确定日志文件名
    let log_filename = format!("{}_error.log", exec_name);
    
    if let Err(e) = run() {
        // 同时将错误写入文件
        let error_msg = format!("错误: {:?}", e);
        eprintln!("{}", error_msg);
        
        // 将错误写入日志文件
        std::fs::write(&log_filename, error_msg)
            .unwrap_or_else(|_| eprintln!("无法将错误写入日志文件"));
            
        std::process::exit(1);
    }
}
