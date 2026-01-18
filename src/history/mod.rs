/// 可撤销的编辑操作接口
pub trait ReversibleEdit: std::fmt::Debug {
    /// 撤销操作
    fn undo(&self) -> Operation;
    
    /// 重做操作
    fn redo(&self) -> Operation;
}

/// 编辑操作
#[derive(Debug, Clone)]
pub enum Operation {
    /// 插入操作: (行, 列, 插入的文本)
    Insert(usize, usize, String),
    
    /// 删除操作: (行, 列, 删除的文本)
    Delete(usize, usize, String),
    
    /// 替换操作: (行, 列, 原文本, 新文本)
    Replace(usize, usize, String, String),
}

/// 创建插入操作
pub fn create_insert_operation(line: usize, col: usize, text: &str) -> Box<dyn ReversibleEdit> {
    Box::new(EditOperation {
        undo_op: Operation::Delete(line, col, text.to_string()),
        redo_op: Operation::Insert(line, col, text.to_string()),
    })
}

/// 创建删除操作
pub fn create_delete_operation(start_line: usize, start_col: usize, _end_line: usize, _end_col: usize, text: &str) -> Box<dyn ReversibleEdit> {
    Box::new(EditOperation {
        undo_op: Operation::Insert(start_line, start_col, text.to_string()),
        redo_op: Operation::Delete(start_line, start_col, text.to_string()),
    })
}

/// 编辑操作
#[derive(Debug, Clone)]
pub struct EditOperation {
    /// 撤销操作
    pub undo_op: Operation,
    
    /// 重做操作
    pub redo_op: Operation,
}

impl ReversibleEdit for EditOperation {
    fn undo(&self) -> Operation {
        self.undo_op.clone()
    }
    
    fn redo(&self) -> Operation {
        self.redo_op.clone()
    }
}

/// 编辑历史
#[derive(Debug)]
pub struct History {
    /// 撤销栈
    undo_stack: Vec<Box<dyn ReversibleEdit>>,
    
    /// 重做栈
    redo_stack: Vec<Box<dyn ReversibleEdit>>,
    
    /// 最大历史记录数
    max_history: usize,
    
    /// 是否在撤销/重做模式
    in_undo_redo: bool,
    
    /// 复合操作栈
    compound_operations: Vec<Box<dyn ReversibleEdit>>,
    
    /// 是否在复合操作模式
    in_compound: bool,
}

impl History {
    /// 创建新的历史记录
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history,
            in_undo_redo: false,
            compound_operations: Vec::new(),
            in_compound: false,
        }
    }
    
    /// 添加编辑操作
    pub fn push<E: ReversibleEdit + 'static>(&mut self, edit: E) {
        if self.in_undo_redo {
            return;
        }
        
        // 如果在复合操作模式，添加到复合操作栈
        if self.in_compound {
            self.compound_operations.push(Box::new(edit));
            return;
        }
        
        // 清空重做栈
        self.redo_stack.clear();
        
        // 添加到撤销栈
        self.undo_stack.push(Box::new(edit));
        
        // 如果超过最大历史记录数，移除最老的记录
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
    }
    
    /// 是否可以撤销
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    
    /// 是否可以重做
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
    
    /// 撤销操作
    pub fn undo(&mut self) -> Option<Operation> {
        if self.in_undo_redo {
            return None;
        }
        
        self.in_undo_redo = true;
        
        if let Some(edit) = self.undo_stack.pop() {
            let op = edit.undo();
            self.redo_stack.push(edit);
            Some(op)
        } else {
            None
        }
    }
    
    /// 重做操作
    pub fn redo(&mut self) -> Option<Operation> {
        if self.in_undo_redo {
            return None;
        }
        
        self.in_undo_redo = true;
        
        if let Some(edit) = self.redo_stack.pop() {
            let op = edit.redo();
            self.undo_stack.push(edit);
            Some(op)
        } else {
            None
        }
    }
    
    /// 完成撤销/重做操作
    pub fn finish_undo_redo(&mut self) {
        self.in_undo_redo = false;
    }
    
    /// 开始复合操作
    pub fn start_compound_operation(&mut self) {
        if !self.in_compound {
            self.in_compound = true;
            self.compound_operations.clear();
        }
    }
    
    /// 结束复合操作
    pub fn end_compound_operation(&mut self) {
        if self.in_compound {
            self.in_compound = false;
            
            // 如果复合操作栈不为空，将其合并为一个单一的编辑操作
            if !self.compound_operations.is_empty() {
                let compound_edit = CompoundEdit {
                    edits: std::mem::take(&mut self.compound_operations),
                };
                
                self.push(compound_edit);
            }
        }
    }
    
    /// 清空历史记录
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.compound_operations.clear();
        self.in_undo_redo = false;
        self.in_compound = false;
    }
}

/// 复合编辑操作
#[derive(Debug)]
pub struct CompoundEdit {
    /// 包含的编辑操作列表
    edits: Vec<Box<dyn ReversibleEdit>>,
}

impl ReversibleEdit for CompoundEdit {
    fn undo(&self) -> Operation {
        // 复合操作的撤销应该是按照相反的顺序执行每个操作的撤销
        // 但这里简化返回最后一个操作的撤销结果
        if let Some(last_edit) = self.edits.last() {
            last_edit.undo()
        } else {
            // 返回一个空的删除操作作为默认值
            Operation::Delete(0, 0, String::new())
        }
    }
    
    fn redo(&self) -> Operation {
        // 复合操作的重做应该是按照原始顺序执行每个操作的重做
        // 但这里简化返回第一个操作的重做结果
        if let Some(first_edit) = self.edits.first() {
            first_edit.redo()
        } else {
            // 返回一个空的插入操作作为默认值
            Operation::Insert(0, 0, String::new())
        }
    }
}

// 为 Box<dyn ReversibleEdit> 实现 ReversibleEdit trait
impl ReversibleEdit for Box<dyn ReversibleEdit> {
    fn undo(&self) -> Operation {
        (**self).undo()
    }
    
    fn redo(&self) -> Operation {
        (**self).redo()
    }
}