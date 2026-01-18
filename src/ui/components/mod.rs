pub mod minimap;
pub mod tabs;
pub mod status_bar;
pub mod code_folding;
pub mod theme;  // 新增主题模块
pub mod terminal;  // 导出终端组件

// 重新导出组件
pub use minimap::draw_minimap;
pub use terminal::TerminalComponent;