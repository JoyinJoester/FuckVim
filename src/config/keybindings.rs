use std::collections::HashMap;

/// 键绑定配置
#[derive(Clone, Debug)]
pub struct KeyBindings {
    /// 各模式下的键绑定映射
    pub mappings: HashMap<String, HashMap<String, String>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut mappings = HashMap::new();
        
        // 普通模式下的默认键映射
        let mut normal_mappings = HashMap::new();
        normal_mappings.insert("<C-s>".to_string(), "write".to_string());
        normal_mappings.insert("<C-q>".to_string(), "quit".to_string());
        normal_mappings.insert("<C-f>".to_string(), "find".to_string());
        normal_mappings.insert("<C-n>".to_string(), "new".to_string());
        normal_mappings.insert("<C-o>".to_string(), "open".to_string());
        normal_mappings.insert("<Tab>".to_string(), "bnext".to_string());
        normal_mappings.insert("<S-Tab>".to_string(), "bprevious".to_string());
        
        // 插入模式下的默认键映射
        let mut insert_mappings = HashMap::new();
        insert_mappings.insert("<C-s>".to_string(), "write".to_string());
        insert_mappings.insert("<C-q>".to_string(), "quit".to_string());
        
        // 可视模式下的默认键映射
        let mut visual_mappings = HashMap::new();
        visual_mappings.insert("<C-s>".to_string(), "write".to_string());
        visual_mappings.insert("<C-q>".to_string(), "quit".to_string());
        
        // 命令模式下的默认键映射
        let mut command_mappings = HashMap::new();
        command_mappings.insert("<C-s>".to_string(), "write".to_string());
        command_mappings.insert("<C-q>".to_string(), "quit".to_string());
        
        // 添加到映射表
        mappings.insert("normal".to_string(), normal_mappings);
        mappings.insert("insert".to_string(), insert_mappings);
        mappings.insert("visual".to_string(), visual_mappings);
        mappings.insert("command".to_string(), command_mappings);
        
        Self { mappings }
    }
}

impl KeyBindings {
    /// 创建新的键绑定配置
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 添加键映射
    pub fn add_mapping(&mut self, mode: &str, key: &str, command: &str) {
        if let Some(mode_map) = self.mappings.get_mut(mode) {
            mode_map.insert(key.to_string(), command.to_string());
        } else {
            let mut mode_map = HashMap::new();
            mode_map.insert(key.to_string(), command.to_string());
            self.mappings.insert(mode.to_string(), mode_map);
        }
    }
    
    /// 获取指定模式下的键映射
    pub fn get_mapping(&self, mode: &str, key: &str) -> Option<String> {
        if let Some(mode_map) = self.mappings.get(mode) {
            mode_map.get(key).cloned()
        } else {
            None
        }
    }
    
    /// 删除键映射
    pub fn remove_mapping(&mut self, mode: &str, key: &str) -> bool {
        if let Some(mode_map) = self.mappings.get_mut(mode) {
            mode_map.remove(key).is_some()
        } else {
            false
        }
    }
    
    /// 获取指定模式下的所有键映射
    pub fn get_mode_mappings(&self, mode: &str) -> Option<&HashMap<String, String>> {
        self.mappings.get(mode)
    }
    
    /// 清空指定模式下的所有键映射
    pub fn clear_mode_mappings(&mut self, mode: &str) {
        if let Some(mode_map) = self.mappings.get_mut(mode) {
            mode_map.clear();
        }
    }
    
    /// 合并另一个键绑定配置
    pub fn merge(&mut self, other: &KeyBindings) {
        for (mode, mappings) in &other.mappings {
            for (key, command) in mappings {
                self.add_mapping(mode, key, command);
            }
        }
    }
    
    /// 从配置文件加载键绑定
    pub fn from_config(config: &crate::config::Config) -> Self {
        let mut bindings = Self::default();
        
        // 从配置对象中合并键绑定
        bindings.mappings = config.keymaps.clone();
        
        bindings
    }
} 