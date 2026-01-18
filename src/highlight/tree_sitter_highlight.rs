use std::sync::{Arc, Once, Mutex};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use lazy_static::lazy_static;
use tree_sitter::{Parser, Language};
use crate::error::{Result};
use crate::highlight::{HighlightSpan, HighlightStyle, SyntaxHighlighter};
use log::debug;

// 缓存过期时间：5分钟（增加了缓存持续时间）
const CACHE_EXPIRY_TIME: Duration = Duration::from_secs(300);
// 最大缓存大小：200项（增加了缓存大小）
const MAX_CACHE_SIZE: usize = 200;
// 解析器池最大大小
const MAX_PARSER_POOL_SIZE: usize = 5;
// 解析最大时间（毫秒）
const MAX_PARSING_TIME_MS: u64 = 200;
// 失败记录的过期时间：5分钟
const FAILURE_EXPIRY_TIME: Duration = Duration::from_secs(300);
// 最大重试次数
const MAX_RETRY_COUNT: u32 = 3;
// 增量解析最大差异比例
const MAX_INCREMENTAL_DIFF_RATIO: f32 = 0.3;

/// 解析器池 - 管理一组可重用的解析器
#[derive(Clone)]
struct ParserPool {
    parsers: Arc<Mutex<VecDeque<Parser>>>,
    initialized: Arc<Once>,
}

impl ParserPool {
    fn new() -> Self {
        Self {
            parsers: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_PARSER_POOL_SIZE))),
            initialized: Arc::new(Once::new()),
        }
    }
    
    /// 初始化解析器池
    fn initialize(&self) {
        let parsers = self.parsers.clone();
        self.initialized.call_once(move || {
            let mut pool = parsers.lock().unwrap();
            for _ in 0..MAX_PARSER_POOL_SIZE {
                let mut parser = Parser::new();
                // 默认设置大一些的缓冲区，避免频繁重新分配
                parser.set_included_ranges(&[]);
                pool.push_back(parser);
            }
        });
    }
    
    /// 获取一个解析器
    fn get_parser(&self) -> Option<Parser> {
        self.initialize();
        self.parsers.lock().unwrap().pop_front()
    }
    
    /// 返回一个解析器到池中
    fn return_parser(&self, mut parser: Parser) {
        // 重置解析器状态
        parser.reset();
        parser.set_included_ranges(&[]);
        
        let mut pool = self.parsers.lock().unwrap();
        // 如果池已满，丢弃该解析器
        if pool.len() < MAX_PARSER_POOL_SIZE {
            pool.push_back(parser);
        }
    }
}

lazy_static! {
    // 高亮结果缓存: 哈希值 -> (高亮结果, 时间戳, 访问次数, 内容长度)
    static ref HIGHLIGHT_CACHE: Mutex<HashMap<u64, (Vec<HighlightSpan>, Instant, u32, usize)>> = Mutex::new(HashMap::new());
    // 最近使用的缓存键顺序
    static ref HIGHLIGHT_CACHE_LRU: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::with_capacity(MAX_CACHE_SIZE));
    
    // 解析树缓存: 哈希值 -> (解析树, 时间戳, 访问次数, 内容长度, 上次内容哈希)
    static ref PARSE_TREE_CACHE: Mutex<HashMap<u64, (Arc<tree_sitter::Tree>, Instant, u32, usize, u64)>> = Mutex::new(HashMap::new());
    // 最近使用的解析树缓存键顺序
    static ref PARSE_TREE_CACHE_LRU: Mutex<VecDeque<u64>> = Mutex::new(VecDeque::with_capacity(MAX_CACHE_SIZE));
    
    // 已加载的语言缓存
    static ref LANGUAGES: Mutex<HashMap<String, Language>> = Mutex::new(HashMap::new());
    static ref TREE_SITTER_LANGUAGES: Mutex<HashMap<String, Language>> = Mutex::new(HashMap::new());
    static ref TREE_SITTER_QUERIES: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref PARSER_POOL: ParserPool = ParserPool::new();
    // 失败记录：记录哪些语言/文件组合解析失败过 (哈希值 -> (时间戳, 失败次数, 上次使用的回退策略))
    static ref FAILURE_RECORDS: Mutex<HashMap<u64, (Instant, u32, Option<String>)>> = Mutex::new(HashMap::new());
    // 缓存统计信息
    static ref CACHE_STATS: Mutex<CacheStats> = Mutex::new(CacheStats::new());
    // 全局策略成功率统计
    static ref STRATEGY_SUCCESS_RATES: Mutex<HashMap<String, StrategySuccessRate>> = Mutex::new(HashMap::new());
}

/// 缓存状态记录
struct CacheStats {
    hits: u64,
    misses: u64,
    evictions: u64,
    failures: u64,
    fallbacks: u64,
    last_report: Instant,
}

impl CacheStats {
    fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            failures: 0,
            fallbacks: 0,
            last_report: Instant::now(),
        }
    }
    
    fn record_hit(&mut self) {
        self.hits += 1;
        self.maybe_report();
    }
    
    fn record_miss(&mut self) {
        self.misses += 1;
        self.maybe_report();
    }
    
    fn record_eviction(&mut self) {
        self.evictions += 1;
        self.maybe_report();
    }
    
    fn record_failure(&mut self) {
        self.failures += 1;
        self.maybe_report();
    }
    
    fn record_fallback(&mut self) {
        self.fallbacks += 1;
        self.maybe_report();
    }
    
    fn maybe_report(&self) {
        let now = Instant::now();
        // 每十分钟或每10000次操作报告一次缓存状态
        if now.duration_since(self.last_report) > Duration::from_secs(600) || 
           (self.hits + self.misses + self.evictions) % 10000 == 0 {
            self.report();
        }
    }
    
    fn report(&self) {
        let total = self.hits + self.misses;
        let hit_rate = if total > 0 { self.hits as f32 / total as f32 * 100.0 } else { 0.0 };
        debug!("Cache stats: 命中率: {:.2}% ({} 命中, {} 未命中), {} 淘汰, {} 失败, {} 回退", 
            hit_rate, self.hits, self.misses, self.evictions, self.failures, self.fallbacks);
    }
}

/// 增强型自适应回退策略链 - 可以智能切换多个策略
#[derive(Clone)]
struct AdaptiveFallbackChain {
    // 保存语言到回退策略的映射
    language_strategies: HashMap<String, Vec<String>>,
    // 保存当前已使用的策略
    used_strategies: Vec<String>,
    // 最大重试次数
    max_retries: usize,
}

impl AdaptiveFallbackChain {
    fn new() -> Self {
        Self {
            language_strategies: HashMap::new(),
            used_strategies: Vec::new(),
            max_retries: 3,
        }
    }
    
    /// 获取下一个可用的回退策略
    fn next_strategy(&mut self, language: &str, content_length: usize) -> Option<FallbackStrategy> {
        // 如果已经用尽了所有重试次数，返回None
        if self.used_strategies.len() >= self.max_retries {
            return None;
        }
        
        // 从成功率记录中获取每个策略的评分
        let mut strategy_scores = HashMap::new();
        {
            let success_rates = STRATEGY_SUCCESS_RATES.lock().unwrap();
            for strategy in FallbackStrategy::all_strategies() {
                let name = strategy.name();
                if let Some(rate) = success_rates.get(name) {
                    strategy_scores.insert(name.to_string(), rate.success_rate());
                } else {
                    // 默认评分为0.5
                    strategy_scores.insert(name.to_string(), 0.5f32);
                }
            }
        }
        
        // 针对特定语言调整评分
        self.adjust_scores_for_language(&mut strategy_scores, language, content_length);
        
        // 从评分中移除已使用的策略
        for used in &self.used_strategies {
            strategy_scores.remove(used);
        }
        
        // 如果没有可用策略，返回None
        if strategy_scores.is_empty() {
            return None;
        }
        
        // 基于评分选择最佳策略
        let best_strategy = strategy_scores.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| "simple_keyword".to_string());
        
        // 记录已使用的策略
        self.used_strategies.push(best_strategy.clone());
        
        // 转换策略名为枚举值
        match best_strategy.as_str() {
            "simple_keyword" => Some(FallbackStrategy::SimpleKeyword),
            "regex_based" => Some(FallbackStrategy::RegexBased),
            "fragment_parsing" => Some(FallbackStrategy::FragmentParsing),
            "heuristic_highlight" => Some(FallbackStrategy::HeuristicHighlight),
            "mimic_most_similar" => Some(FallbackStrategy::MimicMostSimilar),
            "partial_highlight" => Some(FallbackStrategy::PartialHighlight),
            "content_adaptive" => Some(FallbackStrategy::ContentAdaptive),
            "language_specific" => Some(FallbackStrategy::LanguageSpecific),
            _ => Some(FallbackStrategy::SimpleKeyword), // 默认
        }
    }
    
    /// 基于文件内容特征分析选择最适合的策略
    fn analyze_content_for_strategy(&self, content: &str, language: &str) -> Option<String> {
        // 内容长度和特征分析
        let content_length = content.len();
        let line_count = content.matches('\n').count() + 1;
        let avg_line_length = if line_count > 0 { content_length / line_count } else { 0 };
        
        // 检测代码复杂度特征
        let has_complex_structures = content.contains('{') && content.contains('}');
        let has_functions = content.contains("fn ") || content.contains("function ") || 
                           content.contains("def ") || content.contains("void ");
        let has_classes = content.contains("class ") || content.contains("struct ") || 
                         content.contains("trait ") || content.contains("interface ");
        
        // 基于特征选择最合适的策略
        if content_length < 1000 {
            // 小文件，简单策略就足够
            return Some("simple_keyword".to_string());
        } else if has_complex_structures && (has_functions || has_classes) {
            // 复杂代码结构，使用片段解析
            return Some("fragment_parsing".to_string());
        } else if avg_line_length > 100 {
            // 长行代码，可能是压缩过的或生成的代码，使用正则
            return Some("regex_based".to_string());
        } else if language == "markdown" || language == "text" {
            // 对于文本类文件，使用启发式高亮
            return Some("heuristic_highlight".to_string());
        }
        
        // 增强语言特征分析
        if content.len() > 5000 && (language == "rust" || language == "cpp" || language == "c") {
            // 对于大型代码文件，使用部分高亮策略提高性能
            return Some("partial_highlight".to_string());
        } else if language == "python" && content.contains("def __init__") {
            // Python类通常有特定结构，使用专门的策略
            return Some("python_class_highlight".to_string());
        } else if language == "javascript" || language == "typescript" {
            if content.contains("import ") && content.contains("from ") {
                return Some("modern_js_highlight".to_string());
            }
        }
        
        // 没有明显特征，返回None让系统使用默认策略选择逻辑
        None
    }
    
    /// 根据语言和内容长度调整策略评分
    fn adjust_scores_for_language(&self, scores: &mut HashMap<String, f32>, language: &str, content_length: usize) {
        // 根据语言调整
        match language {
            "rust" | "python" | "javascript" | "typescript" => {
                // 这些语言关键字识别效果好，提高SimpleKeyword评分
                if let Some(score) = scores.get_mut("simple_keyword") {
                    *score *= 1.2;
                }
            },
            "html" | "xml" | "css" => {
                // 这些语言适合正则表达式处理
                if let Some(score) = scores.get_mut("regex_based") {
                    *score *= 1.3;
                }
            },
            "markdown" | "json" | "yaml" | "toml" => {
                // 这些结构化文本适合片段解析
                if let Some(score) = scores.get_mut("fragment_parsing") {
                    *score *= 1.25;
                }
            },
            "c" | "cpp" | "go" | "java" => {
                // 这些复杂语言适合启发式高亮
                if let Some(score) = scores.get_mut("heuristic_highlight") {
                    *score *= 1.2;
                }
                // 同时它们也可以借鉴相似语言的高亮
                if let Some(score) = scores.get_mut("mimic_most_similar") {
                    *score *= 1.15;
                }
            },
            // 对于未知语言，试图模仿最相似的语言
            _ => {
                if let Some(score) = scores.get_mut("mimic_most_similar") {
                    *score *= 1.3;
                }
            }
        }
        
        // 根据内容长度调整
        if content_length > 10000 {
            // 对于大文件，降低计算密集型策略的评分
            if let Some(score) = scores.get_mut("fragment_parsing") {
                *score *= 0.8;
            }
            if let Some(score) = scores.get_mut("heuristic_highlight") {
                *score *= 0.85;
            }
        } else if content_length < 1000 {
            // 对于小文件，可以使用更复杂的策略
            if let Some(score) = scores.get_mut("fragment_parsing") {
                *score *= 1.2;
            }
            if let Some(score) = scores.get_mut("heuristic_highlight") {
                *score *= 1.1;
            }
        }
        
        // 检查特定语言的自定义策略
        if let Some(strategies) = self.language_strategies.get(language) {
            for strategy in strategies {
                if let Some(score) = scores.get_mut(strategy) {
                    // 提高预配置策略的评分
                    *score *= 1.5;
                }
            }
        }
    }
    
    /// 记录策略成功
    fn record_success(&self, language: &str, strategy: &str) {
        let mut success_rates = STRATEGY_SUCCESS_RATES.lock().unwrap();
        let rate = success_rates.entry(strategy.to_string())
            .or_insert_with(|| StrategySuccessRate::new());
        rate.record_success();
        
        // 更新语言特定策略记录
        let mut chain = self.clone();
        chain.language_strategies.entry(language.to_string())
            .or_insert_with(Vec::new)
            .push(strategy.to_string());
        
        // 确保每种语言最多记录5个最成功的策略
        if let Some(strategies) = chain.language_strategies.get_mut(language) {
            if strategies.len() > 5 {
                // 移除最不成功的策略
                strategies.sort_by(|a, b| {
                    let a_rate = success_rates.get(a).map(|r| r.success_rate()).unwrap_or(0.0);
                    let b_rate = success_rates.get(b).map(|r| r.success_rate()).unwrap_or(0.0);
                    b_rate.partial_cmp(&a_rate).unwrap_or(std::cmp::Ordering::Equal)
                });
                strategies.truncate(5);
            }
        }
    }
    
    /// 记录策略失败
    fn record_failure(&self, strategy: &str) {
        let mut success_rates = STRATEGY_SUCCESS_RATES.lock().unwrap();
        let rate = success_rates.entry(strategy.to_string())
            .or_insert_with(|| StrategySuccessRate::new());
        rate.record_failure();
    }
    
    /// 重置已使用的策略列表
    fn reset(&mut self) {
        self.used_strategies.clear();
    }
}

/// 回退策略成功率记录
struct StrategySuccessRate {
    successes: usize,
    failures: usize,
    last_updated: Instant,
}

impl StrategySuccessRate {
    fn new() -> Self {
        Self {
            successes: 0,
            failures: 0,
            last_updated: Instant::now(),
        }
    }
    
    fn record_success(&mut self) {
        self.successes += 1;
        self.last_updated = Instant::now();
    }
    
    fn record_failure(&mut self) {
        self.failures += 1;
        self.last_updated = Instant::now();
    }
    
    fn success_rate(&self) -> f32 {
        let total = self.successes + self.failures;
        if total == 0 {
            return 0.5; // 默认评分
        }
        
        // 计算成功率
        let base_rate = self.successes as f32 / total as f32;
        
        // 加入时间衰减因子，使较旧的记录影响减小
        let age_seconds = Instant::now().duration_since(self.last_updated).as_secs() as f32;
        let decay_factor = (-0.1 * age_seconds / 86400.0).exp(); // 每天衰减约10%
        
        // 结合基础成功率和时间因素
        0.5 + (base_rate - 0.5) * decay_factor
    }
}

/// 回退策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackStrategy {
    /// 简单关键字高亮
    SimpleKeyword,
    /// 基于正则表达式的高亮
    RegexBased,
    /// 部分解析高亮
    FragmentParsing,
    /// 启发式规则高亮
    HeuristicHighlight,
    /// 模仿最相似语言的高亮
    MimicMostSimilar,
    /// 部分区域高亮（只高亮可见区域或部分内容）
    PartialHighlight,
    /// 内容特征自适应高亮
    ContentAdaptive,
    /// 特定语言优化高亮
    LanguageSpecific,
}

impl FallbackStrategy {
    /// 获取所有策略
    pub fn all_strategies() -> Vec<FallbackStrategy> {
        vec![
            FallbackStrategy::SimpleKeyword,
            FallbackStrategy::RegexBased,
            FallbackStrategy::FragmentParsing,
            FallbackStrategy::HeuristicHighlight,
            FallbackStrategy::MimicMostSimilar,
            FallbackStrategy::PartialHighlight,
            FallbackStrategy::ContentAdaptive,
            FallbackStrategy::LanguageSpecific,
        ]
    }
    
    /// 获取策略名称
    pub fn name(&self) -> &'static str {
        match self {
            FallbackStrategy::SimpleKeyword => "simple_keyword",
            FallbackStrategy::RegexBased => "regex_based",
            FallbackStrategy::FragmentParsing => "fragment_parsing",
            FallbackStrategy::HeuristicHighlight => "heuristic_highlight",
            FallbackStrategy::MimicMostSimilar => "mimic_most_similar",
            FallbackStrategy::PartialHighlight => "partial_highlight",
            FallbackStrategy::ContentAdaptive => "content_adaptive",
            FallbackStrategy::LanguageSpecific => "language_specific",
        }
    }
    
    /// 应用回退策略
    fn apply(&self, content: &str, language: &str) -> Result<Vec<HighlightSpan>> {
        match self {
            FallbackStrategy::SimpleKeyword => Self::apply_simple_keyword(content, language),
            FallbackStrategy::RegexBased => Self::apply_regex_based(content, language),
            FallbackStrategy::FragmentParsing => Self::apply_fragment_parsing(content, language),
            FallbackStrategy::HeuristicHighlight => Self::apply_heuristic(content, language),
            FallbackStrategy::MimicMostSimilar => Self::apply_mimic_similar(content, language),
            FallbackStrategy::PartialHighlight => Self::apply_simple_keyword(content, language),
            FallbackStrategy::ContentAdaptive => Self::apply_simple_keyword(content, language),
            FallbackStrategy::LanguageSpecific => Self::apply_simple_keyword(content, language),
        }
    }
    
    /// 实现简单关键字高亮
    fn apply_simple_keyword(content: &str, language: &str) -> Result<Vec<HighlightSpan>> {
        let keywords = match language {
            "rust" => vec!["fn", "let", "mut", "if", "else", "match", "for", "while", "struct", "enum", "trait", "impl", "pub", "use", "mod", "crate", "self", "super", "return"],
            "python" => vec!["def", "class", "if", "else", "elif", "for", "while", "try", "except", "finally", "import", "from", "as", "with", "return", "yield", "lambda", "self"],
            "javascript" | "typescript" => vec!["function", "let", "const", "var", "if", "else", "for", "while", "try", "catch", "finally", "return", "class", "export", "import", "from", "of", "in"],
            // 为更多语言添加关键字...
            _ => vec![], // 未知语言返回空列表
        };
        
        let mut spans = Vec::new();
        
        // 简单的字符串匹配查找关键字
        for keyword in keywords {
            let mut pos = 0;
            while let Some(idx) = content[pos..].find(keyword) {
                let abs_pos = pos + idx;
                
                // 确保找到的是完整的单词，而不是其中一部分
                let is_word_start = abs_pos == 0 || !content.chars().nth(abs_pos - 1).unwrap_or(' ').is_alphanumeric();
                let word_end = abs_pos + keyword.len();
                let is_word_end = word_end >= content.len() || !content.chars().nth(word_end).unwrap_or(' ').is_alphanumeric();
                
                if is_word_start && is_word_end {
                    // 找到一个关键字，创建高亮
                    let line_starts = get_line_starts(content);
                    if let Some(highlight) = create_highlight(HighlightStyle::Keyword, abs_pos, keyword, &line_starts) {
                        spans.push(highlight);
                    }
                }
                
                pos = abs_pos + 1;
            }
        }
        
        Ok(spans)
    }
    
    /// 实现基于正则表达式的高亮
    fn apply_regex_based(_content: &str, _language: &str) -> Result<Vec<HighlightSpan>> {
        // 基于正则的高亮实现...
        // 由于正则表达式依赖，这里简化处理，返回空列表
        Ok(Vec::new())
    }
    
    /// 实现基于片段解析的高亮
    fn apply_fragment_parsing(_content: &str, _language: &str) -> Result<Vec<HighlightSpan>> {
        // 基于片段解析的高亮实现...
        // 可以尝试对内容的小片段应用parser，避免整个文件解析失败
        Ok(Vec::new())
    }
    
    /// 实现启发式高亮
    fn apply_heuristic(_content: &str, _language: &str) -> Result<Vec<HighlightSpan>> {
        // 启发式高亮实现...
        // 使用一些常见的代码模式识别
        Ok(Vec::new())
    }
    
    /// 实现模仿最相似语言的高亮
    fn apply_mimic_similar(_content: &str, language: &str) -> Result<Vec<HighlightSpan>> {
        // 选择一个类似的语言进行高亮
        let _similar_language = match language {
            "rust" => "c",
            "typescript" => "javascript",
            "jsx" => "javascript",
            "tsx" => "typescript",
            "vue" => "html",
            "scss" | "less" => "css",
            // 添加更多语言映射...
            _ => return Ok(Vec::new()), // 没有找到相似语言
        };
        
        // 使用相似语言的高亮规则
        // 这里实际应用可能需要调用树状解析器或其他高亮方法
        Ok(Vec::new())
    }
}

fn create_highlight(style: HighlightStyle, abs_pos: usize, text: &str, line_starts: &[usize]) -> Option<HighlightSpan> {
    // 计算行号和列号
    let mut start_line = 0;
    let mut end_line = 0;
    let mut start_col = 0;
    let mut end_col = 0;
    
    for (line_idx, &line_start) in line_starts.iter().enumerate() {
        if line_start <= abs_pos {
            start_line = line_idx;
            start_col = abs_pos - line_start;
        }
        
        let end_pos = abs_pos + text.len();
        if line_start <= end_pos {
            end_line = line_idx;
            end_col = end_pos - line_start;
        } else {
            break;
        }
    }
    
    Some(HighlightSpan {
        start_line,
        start_col,
        end_line,
        end_col,
        style,
    })
}

/// 获取文本中每行的起始位置
fn get_line_starts(content: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    let mut pos = 0;
    
    for c in content.chars() {
        pos += 1;
        if c == '\n' {
            line_starts.push(pos);
        }
    }
    
    line_starts
}

// 实现RustHighlighter
pub struct RustHighlighter {
    // 这里应该包含实际的Rust语法高亮实现
}

impl RustHighlighter {
    pub fn new() -> Self {
        Self {}
    }
}

impl SyntaxHighlighter for RustHighlighter {
    fn highlight(&self, text: &str) -> Result<Vec<HighlightSpan>> {
        // 简单实现，实际应该使用tree-sitter解析Rust代码
        let mut highlights = Vec::new();
        
        // 模拟一些基本的Rust关键字高亮
        for (i, line) in text.lines().enumerate() {
            // 高亮关键字
            for keyword in &["fn", "let", "mut", "pub", "struct", "enum", "impl", "trait", "use", "mod", "match", "if", "else", "for", "while", "return", "self", "Self"] {
                let mut start = 0;
                while let Some(pos) = line[start..].find(keyword) {
                    let actual_start = start + pos;
                    // 确保是独立的关键字，而不是更大词的一部分
                    let is_word_boundary_before = actual_start == 0 || !line.chars().nth(actual_start - 1).unwrap_or(' ').is_alphanumeric();
                    let is_word_boundary_after = actual_start + keyword.len() >= line.len() || 
                                          !line.chars().nth(actual_start + keyword.len()).unwrap_or(' ').is_alphanumeric();
                    
                    if is_word_boundary_before && is_word_boundary_after {
                        highlights.push(HighlightSpan {
                            start_line: i,
                            start_col: actual_start,
                            end_line: i,
                            end_col: actual_start + keyword.len(),
                            style: HighlightStyle::Keyword,
                        });
                    }
                    start = actual_start + keyword.len();
                    if start >= line.len() {
                        break;
                    }
                }
            }
            
            // 高亮字符串
            let mut in_string = false;
            let mut string_start = 0;
            for (j, c) in line.char_indices() {
                if c == '"' && (j == 0 || &line[j-1..j] != "\\") {
                    if !in_string {
                        in_string = true;
                        string_start = j;
                    } else {
                        in_string = false;
                        highlights.push(HighlightSpan {
                            start_line: i,
                            start_col: string_start,
                            end_line: i,
                            end_col: j + 1,
                            style: HighlightStyle::String,
                        });
                    }
                }
            }
            
            // 高亮注释
            if let Some(comment_start) = line.find("//") {
                highlights.push(HighlightSpan {
                    start_line: i,
                    start_col: comment_start,
                    end_line: i,
                    end_col: line.len(),
                    style: HighlightStyle::Comment,
                });
            }
        }
        
        Ok(highlights)
    }
    
    fn name(&self) -> &str {
        "rust"
    }
}

// 实现LuaHighlighter
pub struct LuaHighlighter {
    // 这里应该包含实际的Lua语法高亮实现
}

impl LuaHighlighter {
    pub fn new() -> Self {
        Self {}
    }
}

impl SyntaxHighlighter for LuaHighlighter {
    fn highlight(&self, text: &str) -> Result<Vec<HighlightSpan>> {
        // 简单实现，实际应该使用tree-sitter解析Lua代码
        let mut highlights = Vec::new();
        
        // 模拟一些基本的Lua关键字高亮
        for (i, line) in text.lines().enumerate() {
            // 高亮关键字
            for keyword in &["function", "local", "end", "if", "then", "else", "elseif", "for", "do", "while", "repeat", "until", "break", "return", "nil", "true", "false"] {
                let mut start = 0;
                while let Some(pos) = line[start..].find(keyword) {
                    let actual_start = start + pos;
                    // 确保是独立的关键字，而不是更大词的一部分
                    let is_word_boundary_before = actual_start == 0 || !line.chars().nth(actual_start - 1).unwrap_or(' ').is_alphanumeric();
                    let is_word_boundary_after = actual_start + keyword.len() >= line.len() || 
                                          !line.chars().nth(actual_start + keyword.len()).unwrap_or(' ').is_alphanumeric();
                    
                    if is_word_boundary_before && is_word_boundary_after {
                        highlights.push(HighlightSpan {
                            start_line: i,
                            start_col: actual_start,
                            end_line: i,
                            end_col: actual_start + keyword.len(),
                            style: HighlightStyle::Keyword,
                        });
                    }
                    start = actual_start + keyword.len();
                    if start >= line.len() {
                        break;
                    }
                }
            }
            
            // 高亮字符串
            let mut in_string = false;
            let mut string_start = 0;
            for (j, c) in line.char_indices() {
                if (c == '"' || c == '\'') && (j == 0 || &line[j-1..j] != "\\") {
                    if !in_string {
                        in_string = true;
                        string_start = j;
                    } else {
                        in_string = false;
                        highlights.push(HighlightSpan {
                            start_line: i,
                            start_col: string_start,
                            end_line: i,
                            end_col: j + 1,
                            style: HighlightStyle::String,
                        });
                    }
                }
            }
            
            // 高亮注释
            if let Some(comment_start) = line.find("--") {
                highlights.push(HighlightSpan {
                    start_line: i,
                    start_col: comment_start,
                    end_line: i,
                    end_col: line.len(),
                    style: HighlightStyle::Comment,
                });
            }
        }
        
        Ok(highlights)
    }
    
    fn name(&self) -> &str {
        "lua"
    }
}