// FuckVim - 意图优先的TUI编辑器 MVP
//
// 架构概述：
// - Go Host (Bubble Tea) 负责 UI 和 IO
// - Rust WASM Plugin 负责 "AI处理" 逻辑
// - 通过 Extism SDK 进行通信

package main

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"bufio"
	"io"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	extism "github.com/extism/go-sdk"
	"golang.org/x/term"
	"github.com/atotto/clipboard" // 系统剪贴板支持
	"github.com/charmbracelet/bubbles/textinput"

	// Chroma 语法高亮库
	"github.com/alecthomas/chroma/v2"
	"github.com/alecthomas/chroma/v2/formatters"
	"github.com/alecthomas/chroma/v2/lexers"
	"github.com/alecthomas/chroma/v2/styles"

	"github.com/charmbracelet/bubbles/viewport"
	"github.com/mattn/go-runewidth"
)

// =============================================================================
// 全局变量
// =============================================================================

// globalProgram 让 LSP 协程能发消息回 UI 主线程
var globalProgram *tea.Program

// =============================================================================
// 常量定义
// =============================================================================

const (
	// Layout Constants
	HeaderHeight    = 1  // Top Tab Grid
	StatusBarHeight = 1  // Bottom Status Bar
	WhichKeyHeight  = 15 // Bottom Menu (Fixed Height)

	// I18n Language Constants
	LangEN = "en"
	LangZH = "zh"

	// WASM插件路径 - 相对于执行目录
	PluginDir  = "./plugins"
	pluginPath = "plugin.wasm"

	// 预测去抖动时间 - 用户停止输入多久后触发AI预测
	predictionDebounce = 500 * time.Millisecond
)

// translations 多语言翻译字典
var translations = map[string]map[string]string{
	LangEN: {
		// Status Bar Modes
		"status.normal":  "NORMAL",
		"status.insert":  "INSERT",
		"status.command": "COMMAND",
		"status.visual":  "VISUAL",
		"status.tree":    "TREE",
		"status.finder":  "FINDER",
		"status.whichkey": "MENU",

		// WhichKey Menu Items
		"wk.find":      "Find Files",
		"wk.explorer":  "File Explorer",
		"wk.git":       "Git Dashboard",
		"wk.save":      "Save File",
		"wk.quit":      "Quit",
		"wk.split_v":   "Split Vertical",
		"wk.split_h":   "Split Horizontal",
		"wk.toggle_nu": "Toggle LineNum",
		"wk.paste":     "Paste",
		"wk.terminal":  "Terminal",
		"wk.lang":      "Switch Language",
		"wk.toggle_completion": "Toggle Completion",
		"wk.help":      "Help / Keys",

		// Git Dashboard
		"git.clean":     "✨ All Clean",
		"git.clean_sub": "Working tree clean.",
		"git.ahead":     "🚀 Ready to Push",
		"git.ahead_sub": "commits to push.",
		"git.behind":    "📥 Need to Pull",
		"git.push_hint": "[ Shift+P ] Push to origin",
		"git.pull_hint": "[ :pull ] Update local",
		"git.staging":   "⏳ Staging changes...",
		"git.pushing":   "⏳ Pushing...",
		"git.success":   "✅ Push Success!",
		"git.failed":    "❌ Push Failed",

		// Fuzzy Finder
		"find.title":       "🔍 Fuzzy Find Files",
		"find.placeholder": "Search files...",
		"find.scanning":    "Scanning files...",
		"find.found":       "Found %d files",

		// File Tree
		"tree.delete_confirm": "Delete %s? (y/n)",

		// General Messages
		"msg.saved":        "💾 Saved: %s",
		"msg.clipboard_empty": "ℹ Clipboard empty",
		"msg.pasted":       "📋 Pasted",
		"msg.lang_set":     "Language set to %s",
	},
	LangZH: {
		// Status Bar Modes
		"status.normal":  "普通",
		"status.insert":  "编辑",
		"status.command": "命令",
		"status.visual":  "可视",
		"status.tree":    "文件",
		"status.finder":  "搜索",
		"status.whichkey": "菜单",

		// WhichKey Menu Items
		"wk.find":      "查找文件",
		"wk.explorer":  "文件浏览",
		"wk.git":       "Git 面板",
		"wk.save":      "保存文件",
		"wk.quit":      "退出程序",
		"wk.split_v":   "左右分屏",
		"wk.split_h":   "上下分屏",
		"wk.toggle_nu": "切换行号",
		"wk.paste":     "粘贴",
		"wk.terminal":  "终端",
		"wk.lang":      "切换语言",
		"wk.toggle_completion": "开关补全",
		"wk.help":      "帮助 / 快捷键",

		// Git Dashboard
		"git.clean":     "✨ 代码库整洁",
		"git.clean_sub": "无需提交，工作区干净。",
		"git.ahead":     "🚀 准备推送",
		"git.ahead_sub": "个提交待上传。",
		"git.behind":    "📥 需要拉取",
		"git.push_hint": "[ Shift+P ] 推送到远程",
		"git.pull_hint": "[ :pull ] 拉取更新",
		"git.staging":   "⏳ 正在暂存...",
		"git.pushing":   "⏳ 正在推送...",
		"git.success":   "✅ 推送成功！",
		"git.failed":    "❌ 推送失败",

		// Fuzzy Finder
		"find.title":       "🔍 模糊搜索文件",
		"find.placeholder": "输入文件名搜索...",
		"find.scanning":    "正在扫描文件...",
		"find.found":       "找到 %d 个文件",

		// File Tree
		"tree.delete_confirm": "确认删除 %s 吗? (y/n)",

		// General Messages
		"msg.saved":        "💾 已保存: %s",
		"msg.clipboard_empty": "ℹ 剪贴板为空",
		"msg.pasted":       "📋 已粘贴",
		"msg.lang_set":     "语言已切换为 %s",
	},
}

// Mode 表示编辑器模式
type Mode int

const (
	NormalMode    Mode = iota // 普通模式 - 浏览和导航
	InsertMode                // 插入模式 - 输入文本
	CommandMode               // 命令模式 - 输入 Ex 命令 (:q, :w, etc.)
	FileTreeMode              // 文件树模式 - 浏览文件系统
	FuzzyFindMode             // 模糊搜索模式 - Telescope-style finder
	WhichKeyMode              // WhichKey 菜单模式 - 显示可用快捷键
	HelpMode                  // ? 帮助文档模式
	ModeGitCommit             // Git Commit Message Input
)

func (m Mode) String() string {
	switch m {

	case NormalMode:
		return "NORMAL"
	case InsertMode:
		return "INSERT"
	case CommandMode:
		return "COMMAND"
	case FileTreeMode:
		return "TREE"
	case FuzzyFindMode:
		return "FINDER"
	case WhichKeyMode:
		return "WHICH-KEY"
	default:
		return "UNKNOWN"
	}
}

// FileTreeModel 文件树组件
type FileTreeModel struct {
	rootPath   string
	cursor     int
	offset     int // 滚动偏移量
	Entries    []FileEntry
	IsLoading  bool // 是否正在加载
	
	// Yazi-style modal operations
	State     TreeState
	Action    TreeAction
	Input     textinput.Model
	Selected  string // File targeted for action
}

// TreeState 文件树状态
type TreeState int
const (
	TreeNormal TreeState = iota
	TreeInput             // Typing a filename
	TreeConfirmDelete     // Asking "Are you sure?"
)

// TreeAction 文件操作类型
type TreeAction int
const (
	ActionNone TreeAction = iota
	ActionCreate
	ActionRename
)

// FileEntry 文件条目
type FileEntry struct {
	name  string
	path  string
	isDir bool
}

// -----------------------------------------------------------------------------
// WhichKey Menu (LazyVim-style Leader Key Menu)
// -----------------------------------------------------------------------------

// KeyMenuItem represents a single item in the WhichKey menu
type KeyMenuItem struct {
	Key  string
	Desc string
}

// rootKeys defines the available shortcuts in WhichKey menu
var rootKeys = []KeyMenuItem{
	{Key: "f", Desc: "wk.find"},
	{Key: "e", Desc: "wk.explorer"},
	{Key: "g", Desc: "wk.git"},
	{Key: "w", Desc: "wk.save"},
	{Key: "q", Desc: "wk.quit"},
	{Key: "v", Desc: "wk.split_v"},
	{Key: "s", Desc: "wk.split_h"},
	{Key: "t", Desc: "wk.terminal"},
	{Key: "T", Desc: "wk.toggle_nu"},
	{Key: "p", Desc: "wk.paste"},
	{Key: "l", Desc: "wk.lang"},
	{Key: "c", Desc: "wk.toggle_completion"},
	{Key: "?", Desc: "wk.help"},
}

// Focus 表示当前焦点位置
type Focus int

const (
	FocusEditor   Focus = iota // 编辑器获得焦点
	FocusFileTree              // 文件树获得焦点
	FocusGit        // 焦点在 Git Dashboard
	FocusCommand    // 焦点在 Command Mode 
)

// GitStatus 表示文件状态
type GitStatus int

const (
	StatusUnmodified GitStatus = iota
	StatusModified
	StatusAdded
	StatusUntracked
	StatusDeleted
	StatusStaged
)

// GitFile Git 文件条目
type GitFile struct {
	Path   string
	Status GitStatus
	Staged bool // true if waiting to be committed
}

// GitModel Git 状态模型
type GitModel struct {
	Files    []GitFile
	Cursor   int
	RepoPath string
	IsRepo   bool // 是否是有效的 Git 仓库
	IsLoading bool // 是否正在加载
	Branch    string
	Ahead     int
	Behind    int
}

// =============================================================================
// 样式定义 (使用 Lipgloss)
// =============================================================================

var (
	// 状态栏样式 (Dark Grey)
	statusBarStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("234")).
			Foreground(lipgloss.Color("250")).
			Padding(0, 1)

	// 模式指示器样式
	normalModeStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("62")).
			Foreground(lipgloss.Color("230")).
			Bold(true).
			Padding(0, 1)

	insertModeStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("166")).
			Foreground(lipgloss.Color("230")).
			Bold(true).
			Padding(0, 1)

	commandModeStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("33")).
			Foreground(lipgloss.Color("230")).
			Bold(true).
			Padding(0, 1)

	// 消息区域样式
	messageStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("241")).
			Italic(true)

	// 光标行样式
	cursorLineStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("236"))

	// 行号样式
	lineNumberStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("241")).
			Width(4).
			Align(lipgloss.Right)

	// Ghost Text (AI 建议) 样式 - 灰色/暗淡
	suggestionStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("240")).
			Italic(true)

	// 文件树侧边栏样式
	sidebarStyle = lipgloss.NewStyle().
			Border(lipgloss.NormalBorder(), false, true, false, false).
			BorderForeground(lipgloss.Color("241")).
			Padding(0, 1).
			Width(25)

	// 文件树选中项样式
	treeSelectedStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("62")).
			Foreground(lipgloss.Color("230")).
			Bold(true)

	// 文件树普通项样式
	treeItemStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("252"))

	// 文件树目录样式
	treeDirStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("39")).
			Bold(true)

	// 文件树模式指示器
	treeModeStyle = lipgloss.NewStyle().
			Background(lipgloss.Color("28")).
			Foreground(lipgloss.Color("230")).
			Bold(true).
			Padding(0, 1)

	// Git 面板样式
	gitStyle = lipgloss.NewStyle().
			Border(lipgloss.NormalBorder(), false, true, false, false).
			BorderForeground(lipgloss.Color("241")).
			Padding(0, 1).
			Width(25)

	gitHeaderStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color("205")). // Pink for Git
			Bold(true)

	// Git 状态颜色
	gitStagedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("42")) // Green
	gitModifiedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("160")) // Red
	gitUntrackedStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("208")) // Orange
)

// =============================================================================
// 模型定义
// =============================================================================

// EditorPane 编辑器分屏窗格
type EditorPane struct {
	Viewport viewport.Model
	Lines    []string
	Filename string
	CursorX  int
	CursorY  int
	Width    int // Allocated outer width
	Height   int // Allocated outer height
	LSPVersion int // LSP 文档版本号（每次编辑递增）
}

// SplitType 分屏类型
type SplitType int

const (
	NoSplit SplitType = iota
	VerticalSplit
	HorizontalSplit
)

// Tab 代表一个工作区 (Workspace)
type Tab struct {
	Name       string        // Tab 显示名称 (通常是当前文件)
	Panes      []*EditorPane // 该 Tab 内的分屏列表
	ActivePane int           // 该 Tab 内的活动分屏索引
	SplitType  SplitType     // 该 Tab 的分屏布局类型
}

// Model 是 Bubble Tea 的核心状态结构
type terminalFinishedMsg struct{ err error }

type Model struct {
	// 多标签页系统 (Vim-style Tabs)
	tabs      []*Tab
	activeTab int

	// 编辑器模式
	mode Mode

	// 命令缓冲区 (用于 :command 模式)
	// 命令缓冲区 (Legacy, now using commandInput)
	commandBuffer string
	// Command Input Bar
	commandInput textinput.Model

	// 状态/消息显示
	statusMsg string

	// AI Ghost Text 建议
	suggestion       string // 当前显示的建议文本
	suggestionPending bool   // 是否正在等待预测（去抖动中）
	lastInputTime    time.Time // 最后一次输入的时间

	// I18n 语言设置
	language string

	// WASM 插件实例
	plugin *extism.Plugin

	// 插件加载错误（如果有）
	pluginError error

	// 终端尺寸
	width  int
	height int

	// 文件树侧边栏
	showSidebar bool
	fileTree    FileTreeModel

	// Git 面板
	// 注意：现在 sidebar 和 git 可以同时显示
	showGit bool
	git     GitModel
	selectingGitRoot bool // 是否正在选择 Git 初始化目录

	focus Focus // 当前焦点位置

	// 缓存的布局尺寸 (用于即时同步)
	cachedSidebarWidth  int
	cachedEditorWidth   int
	cachedContentHeight int

	// ----------------------------------------------------
	// 性能优化: 缓存 Chroma 高亮组件
	// 避免每行重新创建 Lexer/Style/Formatter
	// ----------------------------------------------------
	cachedLexer     chroma.Lexer
	cachedStyle     *chroma.Style
	cachedFormatter chroma.Formatter

	// ----------------------------------------------------
	// 异步任务通道
	// ----------------------------------------------------
	pushChan chan string // Git Push 实时输出通道

	// ----------------------------------------------------
	// Fuzzy Finder (Telescope-style) - Input + List Architecture
	// ----------------------------------------------------
	finderInput  textinput.Model // The typing area
	allFiles     []finderItem    // Cache of ALL files (to filter against)
	filteredFiles []finderItem   // Filtered results
	finderCursor int             // Cursor position in filtered list
	finderRoot   string          // Root directory for finder



	// ----------------------------------------------------
	// Help Viewport
	// ----------------------------------------------------
	helpViewport viewport.Model


	// ----------------------------------------------------
	// Editor Preferences
	// ----------------------------------------------------
	relativeLineNumbers bool // true = Hybrid Vim-style, false = Absolute standard

	lsp     *LSPClient
	lspInit bool // 是否已经初始化完成

	// 补全相关 (使用简单静态补全)
	completions       []CompletionItemSimple // 当前补全候选项
	showCompletion    bool                   // 是否显示补全菜单
	completionIdx     int                    // 当前选中的候选项索引
	completionPrefix  string                 // 触发补全时的前缀
	completionEnabled bool                   // 是否启用自动补全功能
}

// =============================================================================
// 初始化
// =============================================================================

// initialModel 创建初始模型状态
func initialModel() Model {
	cwd, _ := os.Getwd()

	// 初始窗格 (Pane 0)
	initialPane := &EditorPane{
		Viewport: viewport.New(0, 0),
		Lines:    []string{""},
		Filename: "", // 稍后由 loadFileMsg 更新，或者如果 args 有值
		CursorX:  0,
		CursorY:  0,
	}

	// 如果有命令行参数，尝试预设文件名 (实际加载在 Init() 中异步进行)
	if len(os.Args) > 1 {
		initialPane.Filename = os.Args[1]
	}

	// Initialize textinput for file tree
	ti := textinput.New()
	ti.Placeholder = "Name..."
	ti.CharLimit = 156
	ti.Width = 20

	// Initialize Command Input
	ci := textinput.New()
	ci.Prompt = ":"
	ci.Placeholder = ""
	ci.CharLimit = 200
	ci.Width = 50

	// Initialize Help Viewport
	vp := viewport.New(0, 0)
	vp.Style = lipgloss.NewStyle().Padding(0, 1)

	// Language Auto-Detection
	lang := LangEN
	envLang := os.Getenv("LANG")
	if strings.Contains(strings.ToLower(envLang), "zh") || strings.Contains(strings.ToLower(envLang), "cn") {
		lang = LangZH
	}

	// Initial Tab
	initialTab := &Tab{
		Name:       "[No Name]",
		Panes:      []*EditorPane{initialPane},
		ActivePane: 0,
		SplitType:  NoSplit,
	}
	if initialPane.Filename != "" {
		initialTab.Name = filepath.Base(initialPane.Filename)
	}

	m := Model{
		language:   lang,
		tabs:       []*Tab{initialTab},
		activeTab:  0,
		
		mode:      NormalMode,
		commandInput: ci,
		helpViewport: vp,

		statusMsg: "欢迎使用 FuckVim! 按 'i' 插入, :vsp 分屏, :q 退出",
		width:     80,
		height:    24,
		fileTree: FileTreeModel{
			rootPath:  cwd,
			IsLoading: true,
			State:     TreeNormal,
			Action:    ActionNone,
			Input:     ti,
		},
		git: GitModel{
			IsLoading: true,
		},
		// LSP 客户端
		lsp: NewLSPClient(),
		// 补全功能默认启用
		completionEnabled: true,
	}

	return m
}

// generateHelpContent Generates the multi-language help text
func (m Model) generateHelpContent() string {
	title := " 🔥 FUCKVIM CHEAT SHEET "
	if m.language == "zh" { title = " 🔥 FUCKVIM 快捷键大全 " }
	
	var s strings.Builder
	
	s.WriteString(lipgloss.NewStyle().Bold(true).Foreground(lipgloss.Color("205")).Render(title) + "\n\n")
	
	// Define sections
	sections := []struct{ TitleEN, TitleZH, ContentEN, ContentZH string }{
		{
			"Global / Navigation", "全局 / 导航",
			"  Space       : Open WhichKey Menu\n  Ctrl+p      : Fuzzy Find Files\n  Ctrl+t      : Open Terminal\n  Shift+h/l   : Switch Tabs\n  Ctrl+h/j/k/l: Move focus between Panes",
			"  Space       : 打开快捷键菜单\n  Ctrl+p      : 模糊搜索文件\n  Ctrl+t      : 打开终端\n  Shift+h/l   : 切换标签页\n  Ctrl+h/j/k/l: 在分屏间切换焦点",
		},
		{
			"Normal Mode", "普通模式",
			"  h/j/k/l     : Move Cursor\n  0 / $       : Line Start / End\n  i           : Insert Mode\n  :           : Command Mode\n  p           : Paste",
			"  h/j/k/l     : 移动光标\n  0 / $       : 行首 / 行尾\n  i           : 进入编辑模式\n  :           : 进入命令模式\n  p           : 粘贴",
		},
		{
			"Insert Mode", "插入模式",
			"  Esc         : Back to Normal\n  Enter       : New Line (Smart Indent)\n  Tab         : Accept Completion\n  Backspace   : Delete (Auto-Pairs)\n  Ctrl+v      : Paste",
			"  Esc         : 返回普通模式\n  Enter       : 换行 (智能缩进)\n  Tab         : 接受补全\n  Backspace   : 删除 (自动括号配对)\n  Ctrl+v      : 粘贴",
		},
		{
			"WhichKey Menu (Space)", "WhichKey 菜单 (空格)",
			"  f : Find Files    t : Terminal\n  e : File Tree     T : Line Numbers\n  g : Git Panel     p : Paste\n  w : Save          l : Language\n  q : Quit          c : Completion\n  v : VSplit        ? : Help\n  s : HSplit",
			"  f : 查找文件      t : 终端\n  e : 文件树        T : 行号模式\n  g : Git 面板      p : 粘贴\n  w : 保存          l : 切换语言\n  q : 退出          c : 开关补全\n  v : 垂直分屏      ? : 帮助\n  s : 水平分屏",
		},
		{
			"File Tree (Sidebar)", "文件树 (侧边栏)",
			"  j/k         : Navigate\n  Enter       : Open File / Toggle Dir\n  Backspace   : Go Up\n  a           : New File (add / for Dir)\n  d           : Delete\n  r           : Rename",
			"  j/k         : 上下移动\n  Enter       : 打开文件 / 折叠目录\n  Backspace   : 返回上级\n  a           : 新建文件 (加/创建目录)\n  d           : 删除\n  r           : 重命名",
		},
		{
			"Git Panel", "Git 面板",
			"  Space       : Stage / Unstage\n  c           : Commit (staged)\n  C           : Stage All + Commit\n  P           : Push\n  r           : Refresh\n  E           : Edit .git/config",
			"  Space       : 暂存 / 取消暂存\n  c           : 提交 (已暂存)\n  C           : 全部暂存 + 提交\n  P           : 推送\n  r           : 刷新状态\n  E           : 编辑 .git/config",
		},
		{
			"Commands", "常用命令",
			"  :w          : Save\n  :q          : Quit\n  :wq         : Save & Quit\n  :vsp [file] : Vertical Split\n  :sp [file]  : Horizontal Split\n  :tabnew     : New Tab\n  :tree       : Toggle File Tree\n  :git        : Toggle Git Panel\n  :lang [en/zh]: Switch Language",
			"  :w          : 保存\n  :q          : 退出\n  :wq         : 保存并退出\n  :vsp [文件] : 左右分屏\n  :sp [文件]  : 上下分屏\n  :tabnew     : 新标签页\n  :tree       : 开关文件树\n  :git        : 开关 Git 面板\n  :lang [en/zh]: 切换语言",
		},
	}

	styleHeader := lipgloss.NewStyle().Foreground(lipgloss.Color("33")).Bold(true) // Blue

	for _, sec := range sections {
		t := sec.TitleEN
		c := sec.ContentEN
		if m.language == "zh" {
			t = sec.TitleZH
			c = sec.ContentZH
		}
		s.WriteString(styleHeader.Render("# "+t) + "\n")
		s.WriteString(c + "\n\n")
	}
	
	return s.String()
}

func openTerminalCmd() tea.Cmd {
	shell := os.Getenv("SHELL")
	if shell == "" {
		shell = "bash"
		if runtime.GOOS == "windows" {
			shell = "powershell.exe"
		}
	}
	c := exec.Command(shell)
	return tea.ExecProcess(c, func(err error) tea.Msg {
		return terminalFinishedMsg{err}
	})
}



// createPaneFromFile 创建新窗格 (如果文件不存在则为空缓冲)
// sanitizeContent cleanses file content to prevent layout issues
// 1. Validates UTF-8
// 2. Expands Tabs to 4 Spaces (Critical for TUI layout)
func sanitizeContent(data []byte) string {
	// 1. Ensure Valid UTF-8
	if !utf8.Valid(data) {
		// Go handles invalid UTF-8 by inserting replacement chars when casting to string
		// So we just proceed. Explicit handling could go here.
	}
	content := string(data)

	// 2. GLOBAL TAB EXPANSION
	// Replace Tab with 4 spaces to prevent layout explosion
	content = strings.ReplaceAll(content, "\t", "    ")

	return content
}

func (m Model) createPaneFromFile(path string) (*EditorPane, error) {
	var content string
	var lines []string

	// 尝试读取文件
	bytes, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			// 新文件: 空内容，无错误
			content = ""
			lines = []string{""}
		} else {
			return nil, err
		}
	} else {
		// 2. Sanitize (The Shield)
		content = sanitizeContent(bytes)
		lines = strings.Split(content, "\n")
	}

	vp := viewport.New(0, 0)
	vp.SetContent(content)

	return &EditorPane{
		Viewport: vp,
		Lines:    lines,
		Filename: path,
		CursorX:  0,
		CursorY:  0,
	}, nil
}

// cloneActivePane 克隆当前活动窗格
func (m Model) cloneActivePane() *EditorPane {
	curTab := m.tabs[m.activeTab]
	curr := curTab.Panes[curTab.ActivePane]
	
	newVp := viewport.New(curr.Viewport.Width, curr.Viewport.Height)
	newVp.SetContent(curr.Viewport.View()) // Copy displayed content
	newVp.YOffset = curr.Viewport.YOffset

	newLines := make([]string, len(curr.Lines))
	copy(newLines, curr.Lines)

	return &EditorPane{
		Viewport: newVp,
		Lines:    newLines,
		Filename: curr.Filename,
		CursorX:  curr.CursorX,
		CursorY:  curr.CursorY,
	}
}

// createNewTab 创建新标签页并打开文件
func (m *Model) createNewTab(path string) {
	// Create Pane
	newPane, err := m.createPaneFromFile(path)
	if err != nil {
		newPane = m.createEmptyPane() // Fallback to empty
		m.statusMsg = fmt.Sprintf("Error opening file: %v", err)
	}

	// Create Tab
	name := filepath.Base(path)
	if path == "" { name = "[No Name]" }
	
	newTab := &Tab{
		Name:       name,
		Panes:      []*EditorPane{newPane},
		ActivePane: 0,
		SplitType:  NoSplit,
	}

	// Append and Focus
	m.tabs = append(m.tabs, newTab)
	m.activeTab = len(m.tabs) - 1
	m.syncSizes()
}

// closeActiveTab 关闭当前标签页
func (m *Model) closeActiveTab() {
	if len(m.tabs) <= 1 {
		// Only one tab left? Maybe quit? Or just empty it?
		// For now, let's keep one empty tab
		return 
	}
	
	// Remove current tab
	m.tabs = append(m.tabs[:m.activeTab], m.tabs[m.activeTab+1:]...)
	
	// Adjust index
	if m.activeTab >= len(m.tabs) {
		m.activeTab = len(m.tabs) - 1
	}
	if m.activeTab < 0 {
		m.activeTab = 0
	}
	m.syncSizes()
}

// tr (Translate) 辅助函数：获取当前语言的翻译
func (m Model) tr(key string) string {
	// 1. Try current language
	if dict, ok := translations[m.language]; ok {
		if val, ok := dict[key]; ok {
			return val
		}
	}
	
	// 2. Fallback to English
	if dict, ok := translations[LangEN]; ok {
		if val, ok := dict[key]; ok {
			return val
		}
	}
	
	// 3. Fallback to key itself
	return key
}

// createEmptyPane 创建一个空白窗格
func (m Model) createEmptyPane() *EditorPane {
	vp := viewport.New(0, 0)
	return &EditorPane{
		Viewport: vp,
		Lines:    []string{""},
		Filename: "[New]",
		CursorX:  0,
		CursorY:  0,
	}
}

// -----------------------------------------------------------------------------
// 异步加载命令 (Async Loader Commands)
// -----------------------------------------------------------------------------

// 消息定义
type fileLoadedMsg struct {
	filename string
	content  []string
	err      error
}



type directoryLoadedMsg struct {
	entries []FileEntry
	err     error
}

type gitStatusMsg struct {
	isRepo bool
	files  []GitFile
	err    error
	branch string
	ahead  int
	behind int
}

type pluginLoadedMsg struct {
	plugin *extism.Plugin
	err    error
}

// -----------------------------------------------------------------------------
// Fuzzy Finder Types and Commands
// -----------------------------------------------------------------------------

// findFilesMsg 模糊搜索文件结果
type findFilesMsg []finderItem

// finderItem 文件条目
type finderItem struct {
	path string
	desc string
}

func (i finderItem) Title() string       { return i.path }
func (i finderItem) Description() string { return i.desc }
func (i finderItem) FilterValue() string { return i.path }

// findFilesCmd 异步递归扫描目录
func findFilesCmd(root string) tea.Cmd {
	return func() tea.Msg {
		var items []finderItem
		filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return nil
			}
			// Skip hidden directories
			if info.IsDir() && strings.HasPrefix(info.Name(), ".") {
				return filepath.SkipDir
			}
			if !info.IsDir() {
				// Make path relative to root for cleaner display
				relPath, _ := filepath.Rel(root, path)
				items = append(items, finderItem{path: relPath, desc: "File"})
			}
			return nil
		})
		return findFilesMsg(items)
	}
}

// loadFileCmd 异步加载文件
func loadFileCmd(filename string) tea.Cmd {
	return func() tea.Msg {
		if filename == "" {
			return nil
		}
		content, err := os.ReadFile(filename)
		if err != nil {
			return fileLoadedMsg{err: err}
		}
		text := string(content)
		text = strings.ReplaceAll(text, "\r\n", "\n")
		text = strings.ReplaceAll(text, "\r", "\n")

		// Visual Sanitization: Expand tabs to 4 spaces for consistent rendering
		// This prevents layout explosion and provides consistent alignment
		text = strings.ReplaceAll(text, "\t", "    ")

		lines := strings.Split(text, "\n")
		if len(lines) == 0 {
			lines = []string{""}
		}
		return fileLoadedMsg{filename: filename, content: lines}
	}
}

// loadDirectoryCmd 异步加载目录
func loadDirectoryCmd(path string) tea.Cmd {
	return func() tea.Msg {
		entries, err := os.ReadDir(path)
		if err != nil {
			return directoryLoadedMsg{err: err}
		}

		var fileEntries []FileEntry
		for _, e := range entries {
			// 忽略隐藏文件 (.git, .vscode 等)
			if strings.HasPrefix(e.Name(), ".") {
				continue
			}

			info, err := e.Info()
			if err != nil {
				continue
			}

			entry := FileEntry{
				name:  e.Name(),
				path:  filepath.Join(path, e.Name()),
				isDir: e.IsDir(),
			}

			if !e.IsDir() {
				// 获取文件大小 (仅显示用，暂时不存)
				_ = info.Size()
			}
			fileEntries = append(fileEntries, entry)
		}

		// 排序: 文件夹在前，且按名称排序
		sort.Slice(fileEntries, func(i, j int) bool {
			if fileEntries[i].isDir != fileEntries[j].isDir {
				return fileEntries[i].isDir
			}
			return fileEntries[i].name < fileEntries[j].name
		})

		return directoryLoadedMsg{entries: fileEntries}
	}
}

// checkGitStatusCmd 异步检查 Git 状态
func checkGitStatusCmd() tea.Cmd {
	return func() tea.Msg {
		// 1. 检查是否是 Git 仓库
		checkCmd := exec.Command("git", "rev-parse", "--is-inside-work-tree")
		if err := checkCmd.Run(); err != nil {
			return gitStatusMsg{isRepo: false}
		}

		// 2. 获取状态
		cmd := exec.Command("git", "status", "--porcelain")
		output, err := cmd.CombinedOutput()
		if err != nil {
			return gitStatusMsg{err: err}
		}

		var gitFiles []GitFile
		lines := strings.Split(string(output), "\n")
		for _, line := range lines {
			if len(line) < 4 {
				continue
			}

			code := line[:2]
			path := strings.TrimSpace(line[3:])
			
			// 去除路径中的引号 (如果文件名包含空格)
			path = strings.Trim(path, "\"")

			var status GitStatus
			staged := false

			// 解析状态码 (X:Index, Y:WorkTree)
			x := code[0]
			y := code[1]

			if x != ' ' && x != '?' {
				staged = true
			}

			if x == '?' && y == '?' {
				status = StatusUntracked
			} else if x == 'A' || y == 'A' {
				status = StatusAdded
			} else if x == 'D' || y == 'D' {
				status = StatusDeleted
			} else if x == 'M' || y == 'M' {
				status = StatusModified
			} else {
				status = StatusUnmodified
			}

			gitFiles = append(gitFiles, GitFile{
				Path:   path,
				Status: status,
				Staged: staged,
			})
		}
		
		// 3. 获取分支信息
		branch := ""
		ahead := 0
		behind := 0
		
		branchCmd := exec.Command("git", "symbolic-ref", "--short", "HEAD")
		if out, err := branchCmd.Output(); err == nil {
			branch = strings.TrimSpace(string(out))
		} else {
			// Detached HEAD or error
			branch = "HEAD"
		}
		
		// 4. 获取 Ahead/Behind (如果有关联上游)
		countCmd := exec.Command("git", "rev-list", "--left-right", "--count", "HEAD...@{u}")
		if out, err := countCmd.Output(); err == nil {
			fields := strings.Fields(string(out))
			if len(fields) >= 2 {
				fmt.Sscanf(fields[0], "%d", &ahead)
				fmt.Sscanf(fields[1], "%d", &behind)
			}
		}

		return gitStatusMsg{
			isRepo: true, 
			files: gitFiles,
			branch: branch,
			ahead:  ahead,
			behind: behind,
		}
	}
}

// loadPluginCmd 异步加载 WASM 插件
func loadPluginCmd() tea.Cmd {
	return func() tea.Msg {
		// 插件路径 (硬编码示例，实际应从配置读取)
		pluginPath := "plugin.wasm"
		
		if _, err := os.Stat(pluginPath); os.IsNotExist(err) {
			return nil // 插件不存在，静默失败
		}

		manifest := extism.Manifest{
			Wasm: []extism.Wasm{
				extism.WasmFile{Path: pluginPath},
			},
		}

		ctx := context.Background()
		plugin, err := extism.NewPlugin(ctx, manifest, extism.PluginConfig{}, nil)
		if err != nil {
			return pluginLoadedMsg{err: err}
		}

		return pluginLoadedMsg{plugin: plugin}
	}
}

// savePane 保存指定 Pane 的文件到磁盘
func (m *Model) savePane(p *EditorPane) error {
	if p.Filename == "" {
		return fmt.Errorf("no filename specified")
	}

	content := strings.Join(p.Lines, "\n")

	// Intelligent Saver (Restore Tabs)
	// Since we converted Tabs to Spaces on load, we should convert them back logic
	if strings.HasSuffix(p.Filename, "go.mod") || 
	   strings.HasSuffix(p.Filename, "Makefile") || 
	   strings.HasSuffix(p.Filename, ".go") ||
	   strings.HasSuffix(p.Filename, ".mk") {
		// Basic naive conversion: 4 spaces -> Tab
		// This fixes the "go.mod broken" issue and satisfies Makefiles
		content = strings.ReplaceAll(content, "    ", "\t")
	}
	
	return os.WriteFile(p.Filename, []byte(content), 0644)
}

// stageGitFile 暂存文件
func (m *Model) stageGitFile(file string) {
	cmd := exec.Command("git", "add", file)
	cmd.Dir = m.git.RepoPath
	cmd.Run()
}

// unstageGitFile 取消暂存文件
func (m *Model) unstageGitFile(file string) {
	cmd := exec.Command("git", "reset", file)
	cmd.Dir = m.git.RepoPath
	cmd.Run()
}

// commitGit 提交更改
func (m *Model) commitGit(msg string) error {
	cmd := exec.Command("git", "commit", "-m", msg)
	cmd.Dir = m.git.RepoPath
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("提交失败: %v\n%s", err, string(output))
	}
	// 不再同步调用 syncGitStatus，而是由调用方(executeCommand/Update)负责 triggering reload
	return nil
}



// =============================================================================
// Bubble Tea 接口实现
// =============================================================================

// Init 返回初始命令
func (m Model) Init() tea.Cmd {
	// 1. 基本 UI 初始化
	w, h, err := term.GetSize(int(os.Stdout.Fd()))
	var resizeCmd tea.Cmd
	if err == nil {
		resizeCmd = func() tea.Msg {
			return tea.WindowSizeMsg{Width: w, Height: h}
		}
	}

	cmds := []tea.Cmd{
		tea.EnterAltScreen,
		// 2. 并行启动异步加载任务
		loadDirectoryCmd(m.fileTree.rootPath),
		checkGitStatusCmd(),
		loadPluginCmd(),
		// 3. 启动 LSP 客户端
		m.lsp.Start(),
	}
	
	if len(m.tabs) > 0 && len(m.tabs[0].Panes) > 0 && m.tabs[0].Panes[0].Filename != "" {
		cmds = append(cmds, loadFileCmd(m.tabs[0].Panes[0].Filename))
	}
	
	if resizeCmd != nil {
		cmds = append(cmds, resizeCmd)
	}
	
	// 如果之前有正在监听的 push 通道 (虽然 Init 只跑一次，但作为范例)
	if m.pushChan != nil {
		cmds = append(cmds, waitForPushOutput(m.pushChan))
	}

	return tea.Batch(cmds...)
}

// Update 处理消息并更新模型
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	
	// 🔥🔥🔥 独占模式：Git Commit 输入拦截器 🔥🔥🔥
	// 一旦进入 GitCommit 模式，直接在这里拦截所有按键消息！
	// 最高优先级，防止被全局快捷键逻辑抢走按键
	if m.mode == ModeGitCommit {
		switch msg := msg.(type) {
		case tea.KeyMsg:
			switch msg.String() {
			case "esc":
				// 强制退出输入模式
				m.mode = NormalMode
				m.commandInput.Blur()
				m.commandInput.Reset()
				m.focus = FocusGit
				m.statusMsg = "Commit aborted"
				return m, nil
			case "enter":
				// 提交
				val := m.commandInput.Value()
				if val != "" {
					err := m.runGitCommit(val)
					if err != nil {
						m.statusMsg = "Commit Error: " + err.Error()
					} else {
						m.statusMsg = "Committed: " + val
					}
				}
				m.mode = NormalMode
				m.commandInput.Blur()
				m.commandInput.Reset()
				m.focus = FocusGit
				return m, checkGitStatusCmd()
			case "ctrl+c":
				// Ctrl+C 也取消
				m.mode = NormalMode
				m.commandInput.Blur()
				m.commandInput.Reset()
				m.focus = FocusGit
				m.statusMsg = "Commit cancelled"
				return m, nil
			}
			// 其他所有按键（包括 ctrl+h/j/k/l）都直接传给输入框
			var cmd tea.Cmd
			m.commandInput, cmd = m.commandInput.Update(msg)
			return m, cmd
		case tea.WindowSizeMsg:
			// 窗口尺寸变化需要处理
			m.width = msg.Width
			m.height = msg.Height
			m.syncSizes()
			return m, nil
		default:
			// 其他消息（如光标闪烁）传给输入框
			var cmd tea.Cmd
			m.commandInput, cmd = m.commandInput.Update(msg)
			return m, cmd
		}
	}
	
	switch msg := msg.(type) {
	
	// --- LSP 消息处理 ---
	case LSPLogMsg:
		m.statusMsg = string(msg)
		// 如果 LSP 启动了，必须马上发送 initialize 请求
		if strings.HasPrefix(string(msg), "LSP Started") {
			cwd, _ := os.Getwd()
			m.lsp.Send("initialize", InitializeParams{
				ProcessID:    os.Getpid(),
				RootURI:      PathToURI(cwd),
				Capabilities: map[string]interface{}{
					"textDocument": map[string]interface{}{
						"completion": map[string]interface{}{
							"completionItem": map[string]interface{}{
								"snippetSupport": true,
							},
						},
					},
				},
			})
		}
		return m, nil

	case LSPResponseMsg:
		// 处理 LSP 的回复
		
		// 如果是 Initialize 的回复
		if !m.lspInit {
			m.lsp.Notify("initialized", struct{}{})
			m.lspInit = true
			
			// 🔥 重要：同步所有已经打开的文件 🔥
			for _, tab := range m.tabs {
				for _, pane := range tab.Panes {
					if pane.Filename != "" {
						pane.LSPVersion = 1
						m.lsp.Notify("textDocument/didOpen", DidOpenTextDocumentParams{
							TextDocument: TextDocumentItem{
								URI:        PathToURI(pane.Filename),
								LanguageID: DetectLanguageID(pane.Filename),
								Version:    pane.LSPVersion,
								Text:       strings.Join(pane.Lines, "\n"),
							},
						})
					}
				}
			}
			
			m.statusMsg = "LSP Ready! 🚀 (Synced Open Files)"
			return m, nil
		}
		
		// LSP 响应（简化处理，不再使用 LSP 补全）
		m.statusMsg = "LSP Response Received"
		return m, nil
	
	// --- 异步加载完成的消息 ---
	case fileLoadedMsg:
		if msg.err != nil {
			m.statusMsg = fmt.Sprintf("无法读取文件: %v", msg.err)
		} else {
			curTab := m.tabs[m.activeTab]
			currPane := curTab.Panes[curTab.ActivePane]
			currPane.Lines = msg.content
			
			// Update Viewport content as well (joined string)
			// Wait, simple join?
			content := strings.Join(msg.content, "\n")
			currPane.Viewport.SetContent(content)
			
			// 初始化高亮
			m.cachedLexer = lexers.Match(msg.filename)
			if m.cachedLexer == nil {
				m.cachedLexer = lexers.Fallback
			}
			m.cachedLexer = chroma.Coalesce(m.cachedLexer)
			
			// 初始化样式和格式化器
			if m.cachedStyle == nil {
				m.cachedStyle = styles.Get("dracula")
				if m.cachedStyle == nil { m.cachedStyle = styles.Fallback }
			}
			if m.cachedFormatter == nil {
				m.cachedFormatter = formatters.TTY256
			}

			// LSP：同步文件打开状态
			if m.lspInit {
				currPane.LSPVersion = 1
				m.lsp.Notify("textDocument/didOpen", DidOpenTextDocumentParams{
					TextDocument: TextDocumentItem{
						URI:        PathToURI(msg.filename),
						LanguageID: DetectLanguageID(msg.filename),
						Version:    currPane.LSPVersion,
						Text:       strings.Join(msg.content, "\n"),
					},
				})
			}
		}
		return m, nil

	case directoryLoadedMsg:
		m.fileTree.IsLoading = false
		if msg.err != nil {
			m.statusMsg = fmt.Sprintf("无法读取目录: %v", msg.err)
		} else {
			m.fileTree.Entries = msg.entries
		}
		return m, nil

	case gitStatusMsg:
		m.git.IsLoading = false
		if msg.err != nil {
			m.statusMsg = fmt.Sprintf("Git错误: %v", msg.err)
		} else {
			m.git.IsRepo = msg.isRepo
			m.git.Files = msg.files
			m.git.Branch = msg.branch
			m.git.Ahead = msg.ahead
			m.git.Behind = msg.behind
		}
		return m, nil

	case pluginLoadedMsg:
		if msg.err != nil {
			m.pluginError = msg.err
		} else {
			m.plugin = msg.plugin
		}
		return m, nil

	case findFilesMsg:
		// Files loaded, store and apply initial filter
		m.allFiles = msg
		m.filteredFiles = msg // Initially show all
		m.finderCursor = 0
		m.statusMsg = fmt.Sprintf("🔍 Found %d files", len(msg))
		return m, nil

	case tea.WindowSizeMsg:
		// 检查尺寸是否真正改变
		sizeChanged := msg.Width != m.width || msg.Height != m.height
		
		m.width = msg.Width
		m.height = msg.Height
		m.syncSizes() // 立即同步布局尺寸
		
		// 只有在尺寸真正改变时才强制清屏 (避免焦点切换时的闪烁)
		if sizeChanged {
			return m, tea.ClearScreen
		}
		return m, nil

	case tea.KeyMsg:
		// 更新最后输入时间
		m.lastInputTime = time.Now()
		// 清除当前建议，标记为等待新的建议
		m.suggestion = ""
		m.suggestionPending = true
		

		// 处理按键
		newM, cmd := m.handleKeyPress(msg)
		
		// 只有在 Insert 模式下才触发 AI 补全预测
		// 这可以显著减少导航和快捷键操作的延迟
		var batchCmd tea.Cmd
		if m.mode == InsertMode {
			batchCmd = tea.Batch(cmd, startPredictionDebounce())
		} else {
			batchCmd = cmd
		}
		
		return newM, batchCmd

	case tickMsg:
		// 检查是否已经过了去抖动时间，且期间没有新的输入
		if m.suggestionPending && time.Since(m.lastInputTime) >= predictionDebounce {
			m.suggestionPending = false // 停止等待
			m.predictCode()             // 执行预测
		}
		return m, nil
	
	case pushProgressMsg:
		// 实时更新 Git Push 进度
		line := string(msg)
		if strings.TrimSpace(line) != "" {
			m.statusMsg = "GIT: " + line
		}
		// 继续监听下一行
		return m, waitForPushOutput(m.pushChan)
		
	case pushDoneMsg:
		// Push 完成
		if msg.err != nil {
			errStr := msg.err.Error()
			
			// 智能诊断：如果是因为需要认证而失败
			if strings.Contains(errStr, "terminal prompts disabled") || strings.Contains(errStr, "authentication failed") {
				m.statusMsg = "🔑 认证失败! 请在终端手动运行 'git push' 一次以保存凭据。"
			} else {
				if len(errStr) > 50 { errStr = errStr[:47] + "..." }
				m.statusMsg = "❌ Push 失败: " + errStr
			}
		} else {
			m.statusMsg = "✅ Push Complete"
		}
		m.pushChan = nil // 清理通道
		m.git.IsLoading = true
		return m, checkGitStatusCmd()

	case terminalFinishedMsg:
		if msg.err != nil {
			m.statusMsg = "Terminal Error: " + msg.err.Error()
		} else {
			m.statusMsg = "Terminal Session Closed"
		}
		// Force resize sync after returning from full screen terminal
		m.syncSizes()
		return m, tea.ClearScreen



	case stageAllDoneMsg:
		if msg.err != nil {
			m.statusMsg = fmt.Sprintf("❌ Staging 失败: %v", msg.err)
			return m, nil
		}
		// Staging 成功，进入 Git Commit 输入模式
		m.mode = ModeGitCommit
		m.commandInput.Placeholder = "Commit message..."
		m.commandInput.Prompt = "Commit: "
		m.commandInput.Reset()
		m.commandInput.Focus()
		m.statusMsg = "🚀 已暂存! 请输入提交信息:"
		m.focus = FocusCommand
		// 同时后台刷新 Git 状态 (让文件变绿) + 输入框光标闪烁
		return m, tea.Batch(checkGitStatusCmd(), textinput.Blink)
	}
	
	// Default passive component updates (Blinks, Ticks, etc.)
	var cmd tea.Cmd
	switch m.mode {
	case CommandMode, ModeGitCommit:
		m.commandInput, cmd = m.commandInput.Update(msg)
	case FuzzyFindMode:
		m.finderInput, cmd = m.finderInput.Update(msg)
	}

	return m, cmd
}

// handleKeyPress 处理键盘输入
func (m Model) handleKeyPress(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// Ctrl+C: 安全取消 (不再退出程序)
	if msg.Type == tea.KeyCtrlC {
		// 如果在插入模式，回到普通模式
		if m.mode == InsertMode {
			m.mode = NormalMode
			m.statusMsg = "已取消 (Ctrl+C)"
		} else if m.mode == CommandMode {
			m.mode = NormalMode
			m.commandBuffer = ""
			m.statusMsg = "已取消"
		} else {
			m.statusMsg = "按 :q 退出"
		}
		return m, nil
	}

	key := msg.String()
	
    // -------------------------------------------------------------------------
    // 空间导航 (Spatial Navigation)
    // -------------------------------------------------------------------------
    // 布局:
    // Top-Left: File Tree |  Right: Editor
    // Bot-Left: Git Panel |
    
	isCtrlH := msg.Type == tea.KeyCtrlH || key == "ctrl+h" || key == "ctrl+left"
	isCtrlJ := msg.Type == tea.KeyCtrlJ || key == "ctrl+j" || key == "ctrl+down"
	isCtrlK := msg.Type == tea.KeyCtrlK || key == "ctrl+k" || key == "ctrl+up"
	isCtrlL := msg.Type == tea.KeyCtrlL || key == "ctrl+l" || key == "ctrl+right"

    switch m.focus {
    case FocusEditor:
        // Get Active Tab
        curTab := m.tabs[m.activeTab]

        if isCtrlH {
             // Left Navigation
             // 1. If Vertical Split and in Right Pane (1) -> Go to Left Pane (0)
             if curTab.SplitType == VerticalSplit && curTab.ActivePane == 1 {
                 curTab.ActivePane = 0
                 return m, nil
             }

             // 2. Editor -> Left -> Sidebar
             // 优先去 FileTree (Top), 如果没有则去 Git (Bottom)
             if m.showSidebar {
                 m.focus = FocusFileTree
                 m.statusMsg = "Focus: File Tree"
                 return m, nil
             } else if m.showGit {
                 m.focus = FocusGit
                 m.statusMsg = "Focus: Git Panel"
                 return m, nil
             }
        }
        if isCtrlL {
            // Right Navigation
            // If Vertical Split and in Left Pane (0) -> Go to Right Pane (1)
            if curTab.SplitType == VerticalSplit && len(curTab.Panes) > 1 && curTab.ActivePane == 0 {
                curTab.ActivePane = 1
                return m, nil
            }
        }
        if isCtrlJ {
            // Down Navigation
             // If Horizontal Split and in Top Pane (0) -> Go to Bottom Pane (1)
             if curTab.SplitType == HorizontalSplit && len(curTab.Panes) > 1 && curTab.ActivePane == 0 {
                 curTab.ActivePane = 1
                 return m, nil
             }
        }
        if isCtrlK {
            // Up Navigation
             // If Horizontal Split and in Bot Pane (1) -> Go to Top Pane (0)
             if curTab.SplitType == HorizontalSplit && curTab.ActivePane == 1 {
                 curTab.ActivePane = 0
                 return m, nil
             }
        }
        
    case FocusFileTree:
        if isCtrlL {
             // Tree -> Right -> Editor
             m.focus = FocusEditor
             if m.mode != InsertMode { m.mode = NormalMode }
             m.statusMsg = "Focus: Editor"
             return m, nil
        }
        if isCtrlJ {
             // Tree -> Down -> Git
             if m.showGit {
                 m.focus = FocusGit
                 m.statusMsg = "Focus: Git Panel"
                 return m, nil
             }
        }

    case FocusGit:
        if isCtrlL {
              // Git -> Right -> Editor
              m.focus = FocusEditor
              if m.mode != InsertMode { m.mode = NormalMode }
              m.statusMsg = "Focus: Editor"
              return m, nil
        }
        if isCtrlK {
             // Git -> Up -> Tree
             if m.showSidebar { 
                 m.focus = FocusFileTree
                 m.statusMsg = "Focus: File Tree"
                 return m, nil
             }
        }
    }

	// -------------------------------------------------------------------------
	// 模式特定处理
	// -------------------------------------------------------------------------

	// 侧边栏焦点
	if m.focus == FocusFileTree && m.showSidebar {
		return m.handleFileTreeMode(msg)
	}

	if m.focus == FocusGit && m.showGit {
		return m.handleGitMode(msg)
	}



	// 编辑器焦点
	if m.focus == FocusEditor || m.focus == FocusCommand || m.mode == ModeGitCommit { // Command 模式也通常在主区域显示，或者覆盖之
        switch m.mode {
        case NormalMode:
            return m.handleNormalMode(msg)
        case InsertMode:
            return m.handleInsertMode(msg)
        case CommandMode:
            return m.handleCommandMode(msg)
        case FuzzyFindMode:
            return m.handleFuzzyFindMode(msg)
		case WhichKeyMode:
			return m.handleWhichKeyMode(msg)
		case ModeGitCommit:
			return m.handleGitCommitMode(msg)
		case HelpMode: // Handle Help Overlay:
            return m.handleHelpMode(msg)
        }
    }

	return m, nil
}

// handleNormalMode 处理普通模式下的按键
func (m Model) handleNormalMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	curTab := m.tabs[m.activeTab]
	currPane := curTab.Panes[curTab.ActivePane]

	switch msg.String() {
	case "ctrl+\\", "alt+t", "ctrl+t":
		return m, openTerminalCmd()
		
	case "i":
		// 进入插入模式
		m.mode = InsertMode
		m.statusMsg = "-- 插入模式 --"

	case ":":
		// Enter Command Mode
		m.mode = CommandMode
		m.commandBuffer = "" // Clear legacy buffer
		m.commandInput.Prompt = ":"  // 重置为命令模式提示符
		m.commandInput.Placeholder = ""
		m.commandInput.Focus()
		m.commandInput.SetValue("")
		m.statusMsg = ""
		return m, nil
	case "j", "down":
		// 向下移动光标
		if currPane.CursorY < len(currPane.Lines)-1 {
			currPane.CursorY++
			// 确保光标不超出当前行长度
			if currPane.CursorX > len(currPane.Lines[currPane.CursorY]) {
				currPane.CursorX = len(currPane.Lines[currPane.CursorY])
			}
		}
		// Scroll Viewport if needed
		// Viewport scrolling is handled in View(), but ideally here?
		// No, Viewport works by setting offset.
		// If cursorY > viewport.YOffset + Height - 1 -> YOffset++
		// But viewport.Height is dynamic.
		// Let's defer scrolling logic to View() or a updateViewport() helper.
		// Actually, bubbletea viewport has SetYOffset.
		// We can do explicit scrolling:
		if currPane.CursorY >= currPane.Viewport.YOffset + currPane.Viewport.Height {
			currPane.Viewport.SetYOffset(currPane.CursorY - currPane.Viewport.Height + 1)
		}

	case "k", "up":
		// 向上移动光标
		if currPane.CursorY > 0 {
			currPane.CursorY--
			if currPane.CursorX > len(currPane.Lines[currPane.CursorY]) {
				currPane.CursorX = len(currPane.Lines[currPane.CursorY])
			}
		}
		if currPane.CursorY < currPane.Viewport.YOffset {
			currPane.Viewport.SetYOffset(currPane.CursorY)
		}

	case "h", "left":
		// 向左移动光标
		if currPane.CursorX > 0 {
			currPane.CursorX--
		}

	case "H": // Shift+h (Prev Tab)
		m.activeTab--
		if m.activeTab < 0 {
			m.activeTab = len(m.tabs) - 1
		}
		m.syncSizes()

	case "L": // Shift+l (Next Tab)
		m.activeTab++
		if m.activeTab >= len(m.tabs) {
			m.activeTab = 0
		}
		m.syncSizes()

	case "l", "right":
		// 向右移动光标
		if currPane.CursorX < len(currPane.Lines[currPane.CursorY]) {
			currPane.CursorX++
		}

	case "0":
		// 移动到行首
		currPane.CursorX = 0

	case "$":
		// 移动到行尾
		currPane.CursorX = len(currPane.Lines[currPane.CursorY])

	case "tab":
		// 触发 WASM 插件处理
		// Refactor needed: m.callPlugin() -> m.callPlugin(currPane)
		m.callPlugin(currPane)
	
	case "p":
		// 粘贴 (从系统剪贴板)
		text, err := clipboard.ReadAll()
		if err != nil || text == "" {
			m.statusMsg = "ℹ 剪贴板为空"
		} else {
			// Refactor needed: m.pasteText(text) -> m.pasteToPane(currPane, text)
			m.pasteToPane(currPane, text)
			m.statusMsg = "✓ 已粘贴"
		}

	case " ":
		// WhichKey 菜单 (Leader Key)
		m.mode = WhichKeyMode
		m.syncSizes() // Elastic Layout: shrink editor to make room for menu
		m.statusMsg = "⌨ Press a key..."
		return m, nil
	case "ctrl+p":
		// 模糊文件搜索 (Telescope-style finder)
		m.mode = FuzzyFindMode
		m.finderRoot = m.fileTree.rootPath

		// Initialize textinput for typing
		ti := textinput.New()
		ti.Placeholder = m.tr("find.placeholder")
		ti.Focus()
		ti.CharLimit = 256
		ti.Width = 50
		m.finderInput = ti

		// Clear previous state
		m.allFiles = nil
		m.filteredFiles = nil
		m.finderCursor = 0

		m.statusMsg = m.tr("find.scanning")
		return m, findFilesCmd(m.finderRoot)
	}

	return m, nil
}

// handleGitMode 处理 Git 模式下的按键
func (m Model) handleGitMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "j", "down":
		if m.git.Cursor < len(m.git.Files)-1 {
			m.git.Cursor++
		}
	case "k", "up":
		if m.git.Cursor > 0 {
			m.git.Cursor--
		}
	case " ":
		// 暂存/取消暂存
		if len(m.git.Files) > 0 {
			file := m.git.Files[m.git.Cursor]
			if file.Staged {
				m.unstageGitFile(file.Path)
			} else {
				m.stageGitFile(file.Path)
			}
			// 立即触发异步状态刷新
			m.git.IsLoading = true
			return m, checkGitStatusCmd()
		}
	case "c":
		// 手动提交 (不自动 stage，需要先用空格键 stage)
		if !m.git.IsRepo {
			m.statusMsg = "⚠ 不是 Git 仓库"
			return m, nil
		}
		m.mode = ModeGitCommit
		m.commandInput.Placeholder = "Commit message..."
		m.commandInput.Prompt = "Commit: "
		m.commandInput.Reset()
		m.commandInput.Focus()
		m.statusMsg = "📝 请输入提交信息 (仅提交已暂存的文件)"
		return m, textinput.Blink
	

	
	case "r":
		m.git.IsLoading = true
		return m, checkGitStatusCmd()

	case "i":
		// 如果不是 Git 仓库，允许初始化
		if !m.git.IsRepo {
			m.selectingGitRoot = true
			m.focus = FocusFileTree
			m.statusMsg = "Git Init Mode: 请在文件树中选择目录，按 'y' 确认，Esc 取消"
			// 确保侧边栏打开
			m.showSidebar = true
		}

	case "E": // Shift+E: 编辑 .git/config
		if !m.git.IsRepo {
			m.statusMsg = "⚠ 不是 Git 仓库"
			return m, nil
		}
		
		// 构建 .git/config 路径
		configPath := filepath.Join(m.fileTree.rootPath, ".git", "config")
		// 异步加载:
		m.focus = FocusEditor
		m.mode = NormalMode
		m.statusMsg = "📝 编辑 Git 配置 (按 :w 保存)"
		return m, loadFileCmd(configPath)

	case "C":
		// Commit Changes - 先自动暂存所有更改
		if !m.git.IsRepo {
			m.statusMsg = "⚠ 不是 Git 仓库"
			return m, nil
		}
		// 先执行 git add -A，然后进入 commit 模式
		m.statusMsg = "⏳ 暂存更改中..."
		return m, stageAllCmd()

	case "P": // Shift+P: 异步推送到远程 (流式反馈)
		if !m.git.IsRepo {
			m.statusMsg = "⚠ 不是 Git 仓库"
			return m, nil
		}
		m.statusMsg = "🚀 Initiating Push..."
		m.pushChan = make(chan string)
		// 启动后台推送任务 + 启动监听器
		return m, tea.Batch(
			runGitPushStream(m.pushChan),
			waitForPushOutput(m.pushChan),
		)

	case "enter":
		// 查看 Diff
		if len(m.git.Files) == 0 {
			return m, nil
		}
		
		file := m.git.Files[m.git.Cursor]
		var cmd *exec.Cmd
		
		// 根据文件状态决定 diff 命令
		if file.Status == StatusUntracked {
			// Untracked 文件直接显示内容
			// 实际上 git diff 无法显示 untracked，我们直接读取文件
			// 或者 git diff --no-index /dev/null path/to/file (有点复杂)
			// 简单起见，直接读取文件内容
			filepath := filepath.Join(m.fileTree.rootPath, file.Path)
			content, err := os.ReadFile(filepath)
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 无法读取文件: %v", err)
				return m, nil
			}

			curTab := m.tabs[m.activeTab]
			currPane := curTab.Panes[curTab.ActivePane]
			currPane.Lines = strings.Split(string(content), "\n")
			currPane.Filename = file.Path
			// Update Viewport
			currPane.Viewport.SetContent(string(content))
		} else {
			// 已跟踪文件
			args := []string{"diff", "--no-color"}
			if file.Staged {
				args = append(args, "--cached")
			}
			args = append(args, "--", file.Path)
			
			cmd = exec.Command("git", args...)
			cmd.Dir = m.fileTree.rootPath
			output, err := cmd.CombinedOutput()
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ Diff 失败: %v", err)
				return m, nil
			}
			
			text := string(output)
			if text == "" {
				text = "(文件为空或无差异)"
			}
			text = strings.ReplaceAll(text, "\r\n", "\n")
			text = strings.ReplaceAll(text, "\r\n", "\n")
			curTab := m.tabs[m.activeTab]
			currPane := curTab.Panes[curTab.ActivePane]
			currPane.Lines = strings.Split(text, "\n")
			currPane.Filename = file.Path + ".diff"
			currPane.Viewport.SetContent(text)
		}
		
		// 重置光标
		curTab := m.tabs[m.activeTab]
		currPane := curTab.Panes[curTab.ActivePane]
		currPane.CursorX = 0
		currPane.CursorY = 0
		currPane.Viewport.SetYOffset(0)
		
		// 设置 Diff 语法高亮
		m.cachedLexer = lexers.Get("diff")
		if m.cachedLexer == nil {
			m.cachedLexer = lexers.Fallback
		}
		m.cachedLexer = chroma.Coalesce(m.cachedLexer)
		
		// 切换焦点
		m.focus = FocusEditor
		m.mode = NormalMode
		m.statusMsg = fmt.Sprintf("👀 查看 Diff: %s", file.Path)
	}
	return m, nil
}

func (m Model) handleGitCommitMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		// Abort
		m.mode = NormalMode
		m.commandInput.Blur()
		m.commandInput.Reset() // Clear and reset state
		m.statusMsg = "Commit aborted"
		m.focus = FocusGit // Return to Git Panel
		return m, nil

	case "enter":
		// Commit
		msgVal := m.commandInput.Value()
		if msgVal == "" { return m, nil }

		err := m.runGitCommit(msgVal)
		if err != nil {
			m.statusMsg = "Commit Error: " + err.Error()
		} else {
			m.statusMsg = "Committed: " + msgVal
			// Refresh Status immediately
			return m, checkGitStatusCmd()
		}

		m.mode = NormalMode
		m.commandInput.Blur()
		m.commandInput.Reset()
		m.focus = FocusGit // Return to Git Panel
		return m, nil
	}

	// CRITICAL: Propagate events to Input Model
	var cmd tea.Cmd
	m.commandInput, cmd = m.commandInput.Update(msg)
	return m, cmd
}

func (m *Model) runGitCommit(message string) error {
	cmd := exec.Command("git", "commit", "-m", message)
	cmd.Dir = m.fileTree.rootPath // 在项目目录中执行
	return cmd.Run()
}

// handleCommandMode 处理命令模式下的按键（类似 Vim 的 Ex 命令）
func (m Model) handleCommandMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	var cmd tea.Cmd
	
	switch msg.Type {
	case tea.KeyEsc:
		// 取消命令，回到普通模式
		m.mode = NormalMode
		m.commandInput.Blur()
		m.commandInput.SetValue("")
		m.statusMsg = ""
		return m, nil

	case tea.KeyEnter:
		// 执行命令
		rawCmd := m.commandInput.Value()
		cmd := m.executeCommand(rawCmd)
		
		// 重置命令模式状态
		m.mode = NormalMode
		m.commandInput.Blur()
		m.commandInput.SetValue("")
		
		// 恢复焦点 (executeCommand might have changed focus, respect it)
		if m.focus == FocusCommand {
			if m.showGit {
				m.focus = FocusGit
			} else if m.showSidebar {
				m.focus = FocusFileTree
			} else {
				m.focus = FocusEditor
			}
		}
		
		if cmd != nil {
			return m, cmd
		}
		return m, nil
	}
	
	// Pass to textinput
	m.commandInput, cmd = m.commandInput.Update(msg)
	return m, cmd
}

// handleFuzzyFindMode 处理模糊搜索模式下的按键
func (m Model) handleFuzzyFindMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.Type {
	case tea.KeyEsc:
		// Close finder, return to normal
		m.mode = NormalMode
		m.finderInput.Blur()
		m.statusMsg = "Finder closed"
		return m, nil

	case tea.KeyEnter:
		// Open selected file
		if len(m.filteredFiles) > 0 && m.finderCursor < len(m.filteredFiles) {
			item := m.filteredFiles[m.finderCursor]
			fullPath := filepath.Join(m.finderRoot, item.path)

			// Load file into active pane (Old) -> New Tab (New)
			m.createNewTab(fullPath)
			m.mode = NormalMode
			m.finderInput.Blur()
			m.focus = FocusEditor
			m.statusMsg = fmt.Sprintf("Opening: %s", item.path)
			return m, nil
		}
		m.mode = NormalMode
		m.finderInput.Blur()
		return m, nil

	case tea.KeyUp, tea.KeyCtrlK:
		// Move cursor up
		if m.finderCursor > 0 {
			m.finderCursor--
		}
		return m, nil

	case tea.KeyDown, tea.KeyCtrlJ:
		// Move cursor down
		if m.finderCursor < len(m.filteredFiles)-1 {
			m.finderCursor++
		}
		return m, nil
	}

	// Pass to textinput for typing
	var cmd tea.Cmd
	m.finderInput, cmd = m.finderInput.Update(msg)

	// Apply fuzzy filter based on input value
	query := strings.ToLower(m.finderInput.Value())
	if query == "" {
		m.filteredFiles = m.allFiles
	} else {
		var filtered []finderItem
		for _, item := range m.allFiles {
			if strings.Contains(strings.ToLower(item.path), query) {
				filtered = append(filtered, item)
			}
		}
		m.filteredFiles = filtered
	}

	// Reset cursor if out of bounds
	if m.finderCursor >= len(m.filteredFiles) {
		m.finderCursor = 0
	}

	return m, cmd
}



// handleHelpMode 处理帮助页面交互
func (m Model) handleHelpMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc", "q", " ":
		m.mode = NormalMode
		m.syncSizes()
		return m, nil
	}
	var cmd tea.Cmd
	m.helpViewport, cmd = m.helpViewport.Update(msg)
	return m, cmd
}

// handleWhichKeyMode 处理 WhichKey 菜单模式下的按键
func (m Model) handleWhichKeyMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	curTab := m.tabs[m.activeTab]
	currPane := curTab.Panes[curTab.ActivePane]

	switch msg.String() {
	case "esc", "space":
		// Close menu
		m.mode = NormalMode
		m.syncSizes() // Elastic Layout: restore editor to full height
		m.statusMsg = ""
		return m, nil

	case "f":
		// Find Files (Fuzzy Finder)
		m.mode = FuzzyFindMode
		m.finderRoot = m.fileTree.rootPath
		ti := textinput.New()
		ti.Placeholder = "Type to search..."
		ti.Focus()
		ti.CharLimit = 256
		ti.Width = 50
		m.finderInput = ti
		m.allFiles = nil
		m.filteredFiles = nil
		m.finderCursor = 0
		m.statusMsg = "Scanning files..."
		return m, findFilesCmd(m.finderRoot)

	case "e":
		// File Explorer
		m.mode = NormalMode
		m.syncSizes()
		m.showSidebar = true
		m.focus = FocusFileTree
		m.statusMsg = "📂 File Tree"
		return m, nil

	case "g":
		// Git Status
		m.mode = NormalMode
		m.syncSizes()
		m.showGit = true
		m.focus = FocusGit
		m.git.IsLoading = true
		m.statusMsg = "🐙 Git Status"
		return m, checkGitStatusCmd()

	case "w":
		// Save File
		m.mode = NormalMode
		m.syncSizes()
		if err := m.savePane(currPane); err != nil {
			m.statusMsg = "❌ Error: " + err.Error()
		} else {
			m.statusMsg = "💾 Saved: " + currPane.Filename
		}
		return m, nil

	case "q":
		// Quit
		return m, tea.Quit

	case "v":
		// Split Vertical (same logic as :vsp)
		m.mode = NormalMode
		curTab := m.tabs[m.activeTab]
		if len(curTab.Panes) >= 2 {
			m.statusMsg = "⚠ Max 2 panes"
			return m, nil
		}
		newPane := m.createEmptyPane()
		curTab.Panes = append(curTab.Panes, newPane)
		curTab.SplitType = VerticalSplit
		curTab.ActivePane = 1
		m.syncSizes()
		m.statusMsg = "┃ Vertical Split"
		return m, nil

	case "s":
		// Split Horizontal (same logic as :sp)
		m.mode = NormalMode
		curTab := m.tabs[m.activeTab]
		if len(curTab.Panes) >= 2 {
			m.statusMsg = "⚠ Max 2 panes"
			return m, nil
		}
		newPane := m.createEmptyPane()
		curTab.Panes = append(curTab.Panes, newPane)
		curTab.SplitType = HorizontalSplit
		curTab.ActivePane = 1
		m.syncSizes()
		m.statusMsg = "━ Horizontal Split"
		return m, nil



	case "t":
		// Toggle Terminal (System Shell)
		m.mode = NormalMode
		return m, openTerminalCmd()

	case "T":
		// Toggle Line Numbers
		m.mode = NormalMode
		m.syncSizes()
		m.relativeLineNumbers = !m.relativeLineNumbers
		modeName := "Absolute"
		if m.relativeLineNumbers {
			modeName = "Relative"
		}
		m.statusMsg = "🔢 Line Numbers: " + modeName
		return m, nil

	case "p":
		// Paste
		m.mode = NormalMode
		m.syncSizes()
		text, err := clipboard.ReadAll()
		if err != nil || text == "" {
			m.statusMsg = m.tr("msg.clipboard_empty")
		} else {
			m.pasteToPane(currPane, text)
			m.statusMsg = m.tr("msg.pasted")
		}
		return m, nil

	case "l":
		// Toggle Language
		if m.language == LangEN { m.language = LangZH } else { m.language = LangEN }
		m.statusMsg = fmt.Sprintf(m.tr("msg.lang_set"), m.language)
		m.mode = NormalMode
		m.syncSizes() // ✅ 修复：切换语言后重新计算布局
		return m, nil

	case "c":
		// Toggle Completion Feature (开关补全功能)
		m.mode = NormalMode
		m.syncSizes()
		m.completionEnabled = !m.completionEnabled
		if m.completionEnabled {
			m.statusMsg = "✓ 补全功能已启用"
		} else {
			m.showCompletion = false
			m.completions = nil
			m.statusMsg = "✖ 补全功能已禁用"
		}
		return m, nil

	case "?":
		// Enter Help Mode
		m.mode = HelpMode
		content := m.generateHelpContent()
		m.helpViewport.SetContent(content)
		return m, nil

	default:
		// Unknown key - just close menu
		m.mode = NormalMode
		m.syncSizes()
		m.statusMsg = ""
		return m, nil
	}
}




// executeCommand 执行 Ex 命令
func (m *Model) executeCommand(cmdStr string) tea.Cmd {
	cmd := strings.TrimSpace(cmdStr)
	m.mode = NormalMode

	// ---------------------------------------------------------
	// Language Command (:lang zh/en)
	// ---------------------------------------------------------
	if strings.HasPrefix(cmd, "lang") {
		args := strings.Fields(cmd)
		if len(args) > 1 {
			switch args[1] {
			case "zh", "cn":
				m.language = LangZH
			case "en":
				m.language = LangEN
			}
		} else {
			// Toggle if no arg
			if m.language == LangZH {
				m.language = LangEN
			} else {
				m.language = LangZH
			}
		}
		m.statusMsg = fmt.Sprintf(m.tr("msg.lang_set"), m.language)
		return nil
	}

	// ---------------------------------------------------------
	// Tab Commands (:tabnew)
	// ---------------------------------------------------------
	if strings.HasPrefix(cmd, "tabnew") || strings.HasPrefix(cmd, "tabe") {
		args := strings.Fields(cmd)
		path := ""
		if len(args) > 1 {
			path = args[1]
		}
		m.createNewTab(path)
		m.statusMsg = "New Tab Created"
		return nil
	}

	// ---------------------------------------------------------
	// 分屏命令 (:vsp, :sp)
	// ---------------------------------------------------------
	if strings.HasPrefix(cmd, "vsp") || strings.HasPrefix(cmd, "sp") {
		// 限制: 目前只支持 2 个分屏
		curTab := m.tabs[m.activeTab]
		if len(curTab.Panes) >= 2 {
			m.statusMsg = "⚠ Max 2 panes supported in MVP"
			return nil
		}

		args := strings.Fields(cmd)
		var newPane *EditorPane
		var err error

		if len(args) > 1 {
			// 打开新文件
			newPane, err = m.createPaneFromFile(args[1])
			if err != nil {
				m.statusMsg = fmt.Sprintf("Error opening file: %v", err)
				return nil
			}
		} else {
			// 克隆当前文件 (Duplicate view)
			newPane = m.cloneActivePane()
		}

		curTab.Panes = append(curTab.Panes, newPane)
		curTab.ActivePane = 1
		
		if strings.HasPrefix(cmd, "vsp") {
			curTab.SplitType = VerticalSplit
		} else {
			curTab.SplitType = HorizontalSplit
		}
		
		m.syncSizes()
		m.statusMsg = "Split created"
		return nil
	}

	switch cmd {
	case "q", "quit":
		curTab := m.tabs[m.activeTab]
		
		// 1. 如果有多个分屏，只关闭当前分屏
		if len(curTab.Panes) > 1 {
			keepIndex := 0
			if curTab.ActivePane == 0 {
				keepIndex = 1
			}
			curTab.Panes = []*EditorPane{curTab.Panes[keepIndex]}
			curTab.ActivePane = 0
			curTab.SplitType = NoSplit
			m.syncSizes()
			m.statusMsg = "Pane closed"
			return nil
		}
		
		// 2. 如果只有一个分屏，尝试关闭 Tab
		if len(m.tabs) > 1 {
			m.closeActiveTab()
			m.statusMsg = "Tab closed"
			return nil
		}
		
		// 3. 只有一个 Tab 一个分屏，退出程序
		return tea.Quit

	case "w", "write":
		// 保存文件
		curTab := m.tabs[m.activeTab]
		currPane := curTab.Panes[curTab.ActivePane]
		if currPane.Filename == "" {
			m.statusMsg = "⚠ 未指定文件名，使用 :w 文件名 (Save as not impl)"
		} else {
			// 临时重构 saveFile: 需要传参数或者重构 saveFile 使用 activePane
			// 这里我们直接调用 saveFileToPane(currPane)
			err := m.savePane(currPane)
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 保存失败: %v", err)
			} else {
				m.statusMsg = fmt.Sprintf("\"%s\" %d 行已写入", currPane.Filename, len(currPane.Lines))
				if m.showGit {
					return checkGitStatusCmd()
				}
			}
		}

	case "wq", "x":
		curTab := m.tabs[m.activeTab]
		currPane := curTab.Panes[curTab.ActivePane]
		if currPane.Filename != "" {
			if err := m.savePane(currPane); err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 保存失败: %v", err)
				return nil
			}
		} else {
			m.statusMsg = "⚠ 未指定文件名"
			return nil
		}
		// Quit logic (Reuse case q logic? easier to copy since we can't goto case)
		if len(curTab.Panes) > 1 {
			keepIndex := 0
			if curTab.ActivePane == 0 {	keepIndex = 1 }
			curTab.Panes = []*EditorPane{curTab.Panes[keepIndex]}
			curTab.ActivePane = 0
			curTab.SplitType = NoSplit
			m.syncSizes()
			return nil
		}
		if len(m.tabs) > 1 {
			m.closeActiveTab()
			return nil
		}
		return tea.Quit

	// Note: Skipped some cases for brevity, keep rest...
	case "q!":
		return tea.Quit

	case "tree", "e":
		// 切换文件树侧边栏
		m.showSidebar = !m.showSidebar
		m.syncSizes()
		if m.showSidebar {
			if m.fileTree.rootPath == "" {
				m.fileTree.rootPath, _ = os.Getwd()
			}
			m.fileTree.IsLoading = true
			m.fileTree.Entries = []FileEntry{}
			m.fileTree.cursor = 0
			m.focus = FocusFileTree
			m.statusMsg = "焦点: 文件树 | j/k=移动, Enter=打开/进入, a=新建, d=删除, r=重命名"
			return tea.Batch(loadDirectoryCmd(m.fileTree.rootPath), m.forceRefresh())
		} else {
			m.focus = FocusEditor
			m.statusMsg = ""
			return m.forceRefresh()
		}
		
	case "git":
		// 切换 Git 面板
		m.showGit = !m.showGit
		m.syncSizes() // 立即同步布局尺寸
		if m.showGit {
			m.focus = FocusGit
			m.statusMsg = "焦点: Git | Ctrl+H=文件树 Ctrl+L=编辑器"
			m.git.IsLoading = true
			return tea.Batch(checkGitStatusCmd(), m.forceRefresh())
		} else {
			m.focus = FocusEditor
			m.statusMsg = ""
			return m.forceRefresh()
		}

	case "toggle-nu", "tn":
		// 切换行号显示模式 (相对/绝对)
		m.relativeLineNumbers = !m.relativeLineNumbers
		modeName := "Absolute (1, 2, 3...)"
		if m.relativeLineNumbers {
			modeName = "Relative (Vim Hybrid)"
		}
		m.statusMsg = "📐 Line Numbers: " + modeName
		return nil
	case "ai":
		m.statusMsg = "⚛ AI 聊天功能即将推出..."

	case "help":
		m.statusMsg = "命令: :vsp/:sp=分屏 :q=退出 :w=保存 :tree=文件树"

	case "":
		m.statusMsg = ""

	default:
		// Check for specific w filename
		if strings.HasPrefix(cmd, "w ") {
			// Save as... logic
			args := strings.Fields(cmd)
			if len(args) > 1 {
				curTab := m.tabs[m.activeTab]
				currPane := curTab.Panes[curTab.ActivePane]
				currPane.Filename = args[1]
				m.savePane(currPane)
				m.statusMsg = fmt.Sprintf("Saved as \"%s\"", currPane.Filename)
				return nil
			}
		}

		if strings.HasPrefix(cmd, "commit ") {
			// ... existing commit logic ...
			message := strings.TrimPrefix(cmd, "commit ")
			message = strings.TrimSpace(message)
			if message == "" {
				m.statusMsg = "⚠ 提交信息不能为空"
			} else {
				output, err := exec.Command("git", "commit", "-m", message).CombinedOutput()
				if err != nil {
					m.statusMsg = fmt.Sprintf("⚠ 提交失败: %s", strings.TrimSpace(string(output)))
				} else {
					m.statusMsg = fmt.Sprintf("✓ 已提交: %s", message)
					if m.showGit {
						m.focus = FocusGit
					}
					return checkGitStatusCmd()
				}
			}
		} else {
			m.statusMsg = fmt.Sprintf("⚠ 未知命令: %s", cmd)
		}
	}

	return nil
}

// forceRefresh 返回一个模拟的 WindowSizeMsg 以强制重绘
func (m Model) forceRefresh() tea.Cmd {
	return func() tea.Msg {
		return tea.WindowSizeMsg{
			Width:  m.width,
			Height: m.height,
		}
	}
}

// handleFileTreeMode 处理文件树模式下的按键
func (m Model) handleFileTreeMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	// Handle based on current state
	switch m.fileTree.State {
	
	// =========================================================================
	// INPUT MODE: Typing a filename (for Create or Rename)
	// =========================================================================
	case TreeInput:
		switch msg.String() {
		case "esc":
			// Cancel input
			m.fileTree.State = TreeNormal
			m.fileTree.Action = ActionNone
			m.fileTree.Input.Blur()
			m.statusMsg = "已取消"
			return m, nil
			
		case "enter":
			// Execute action
			name := m.fileTree.Input.Value()
			if name == "" {
				m.statusMsg = "⚠ 名称不能为空"
				m.fileTree.State = TreeNormal
				m.fileTree.Input.Blur()
				return m, nil
			}
			
			targetPath := filepath.Join(m.fileTree.rootPath, name)
			
			if m.fileTree.Action == ActionCreate {
				// Smart detection: if ends with /, create directory
				if strings.HasSuffix(name, "/") {
					err := os.MkdirAll(targetPath, 0755)
					if err != nil {
						m.statusMsg = fmt.Sprintf("⚠ 创建目录失败: %v", err)
					} else {
						m.statusMsg = fmt.Sprintf("✓ 已创建目录: %s", name)
					}
				} else {
					// Create file
					file, err := os.Create(targetPath)
					if err != nil {
						m.statusMsg = fmt.Sprintf("⚠ 创建文件失败: %v", err)
					} else {
						file.Close()
						m.statusMsg = fmt.Sprintf("✓ 已创建文件: %s", name)
					}
				}
			} else if m.fileTree.Action == ActionRename {
				oldPath := m.fileTree.Selected
				newPath := filepath.Join(filepath.Dir(oldPath), name)
				err := os.Rename(oldPath, newPath)
				if err != nil {
					m.statusMsg = fmt.Sprintf("⚠ 重命名失败: %v", err)
				} else {
					m.statusMsg = fmt.Sprintf("✓ 已重命名: %s", name)
				}
			}
			
			// Reset state and refresh
			m.fileTree.State = TreeNormal
			m.fileTree.Action = ActionNone
			m.fileTree.Input.Blur()
			m.fileTree.Input.SetValue("")
			m.fileTree.IsLoading = true
			return m, loadDirectoryCmd(m.fileTree.rootPath)
			
		default:
			// Pass to textinput
			var cmd tea.Cmd
			m.fileTree.Input, cmd = m.fileTree.Input.Update(msg)
			return m, cmd
		}
	
	// =========================================================================
	// CONFIRM DELETE MODE: Asking "Are you sure?"
	// =========================================================================
	case TreeConfirmDelete:
		switch msg.String() {
		case "y", "Y":
			// Execute delete
			err := os.RemoveAll(m.fileTree.Selected)
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 删除失败: %v", err)
			} else {
				m.statusMsg = fmt.Sprintf("✓ 已删除: %s", filepath.Base(m.fileTree.Selected))
			}
			m.fileTree.State = TreeNormal
			m.fileTree.Selected = ""
			m.fileTree.IsLoading = true
			return m, loadDirectoryCmd(m.fileTree.rootPath)
			
		case "n", "N", "esc":
			// Cancel
			m.fileTree.State = TreeNormal
			m.fileTree.Selected = ""
			m.statusMsg = "已取消删除"
			return m, nil
		}
		return m, nil
	}
	
	// =========================================================================
	// NORMAL MODE: Navigation and action triggers
	// =========================================================================
	
	// Git init selection mode (existing logic)
	if m.selectingGitRoot {
		switch msg.String() {
		case "y":
			targetDir := m.fileTree.rootPath
			cmd := exec.Command("git", "init", targetDir)
			cmd.Dir = targetDir
			if err := cmd.Run(); err != nil {
				m.statusMsg = fmt.Sprintf("⚠ Git Init 失败: %v", err)
			} else {
				m.statusMsg = fmt.Sprintf("✓ Git 仓库已初始化: %s", targetDir)
				m.git.IsLoading = true
				m.selectingGitRoot = false
				m.focus = FocusGit
				return m, checkGitStatusCmd() 
			}
			return m, nil
		case "esc":
			m.selectingGitRoot = false
			m.focus = FocusGit
			m.statusMsg = "已取消 Git 初始化"
			return m, nil
		}
	}

	switch msg.String() {
	case "j", "down":
		if m.fileTree.cursor < len(m.fileTree.Entries)-1 {
			m.fileTree.cursor++
		}

	case "k", "up":
		if m.fileTree.cursor > 0 {
			m.fileTree.cursor--
		}

	case "enter":
		if len(m.fileTree.Entries) > 0 {
			entry := m.fileTree.Entries[m.fileTree.cursor]
			if entry.isDir {
				m.fileTree.rootPath = entry.path
				m.fileTree.IsLoading = true
				m.fileTree.Entries = []FileEntry{}
				m.fileTree.cursor = 0
				return m, loadDirectoryCmd(entry.path)
			} else {
				// Open in NEW TAB
				m.createNewTab(entry.path)
				m.focus = FocusEditor
				m.mode = NormalMode
				return m, nil
			}
		}

	case "backspace", "-":
		parentDir := filepath.Dir(m.fileTree.rootPath)
		m.fileTree.rootPath = parentDir
		m.fileTree.IsLoading = true
		m.fileTree.Entries = []FileEntry{}
		m.fileTree.cursor = 0
		return m, loadDirectoryCmd(parentDir)

	case "a":
		// Add file/folder (Yazi style)
		m.fileTree.State = TreeInput
		m.fileTree.Action = ActionCreate
		m.fileTree.Input.SetValue("")
		m.fileTree.Input.Focus()
		m.statusMsg = "新建: 输入名称 (以 / 结尾创建目录)"
		return m, nil

	case "r":
		// Rename (Yazi style)
		if len(m.fileTree.Entries) > 0 {
			entry := m.fileTree.Entries[m.fileTree.cursor]
			m.fileTree.State = TreeInput
			m.fileTree.Action = ActionRename
			m.fileTree.Selected = entry.path
			m.fileTree.Input.SetValue(entry.name)
			m.fileTree.Input.Focus()
			m.statusMsg = fmt.Sprintf("重命名: %s", entry.name)
		}
		return m, nil

	case "d":
		// Delete with confirmation (Yazi style)
		if len(m.fileTree.Entries) > 0 {
			entry := m.fileTree.Entries[m.fileTree.cursor]
			m.fileTree.State = TreeConfirmDelete
			m.fileTree.Selected = entry.path
			m.statusMsg = fmt.Sprintf("⚠️ 删除 %s? (y/n)", entry.name)
		}
		return m, nil

	case "esc", "q":
		m.focus = FocusEditor
		m.mode = NormalMode
		m.statusMsg = "Ctrl+H 返回文件树"

	case ":":
		m.mode = CommandMode
		m.commandBuffer = ""
		m.statusMsg = ":"
	}

	if m.selectingGitRoot {
		m.statusMsg = fmt.Sprintf("Navigate to project root, then press 'y' to initialize in: %s", m.fileTree.rootPath)
	}

	return m, nil
}

// handleInsertMode 处理插入模式下的按键
func (m Model) handleInsertMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	curTab := m.tabs[m.activeTab]
	currPane := curTab.Panes[curTab.ActivePane]

	// 获取当前行内容
	line := currPane.Lines[currPane.CursorY]
	runes := []rune(line)
	pos := currPane.CursorX

	switch msg.Type {
	case tea.KeyEsc:
		if m.showCompletion {
			m.showCompletion = false
			m.completions = nil
			return m, nil
		}
		// 退出插入模式
		m.mode = NormalMode
		m.statusMsg = "回到普通模式"
		// 光标左移一位（类似 Vim 行为）
		if currPane.CursorX > 0 {
			currPane.CursorX--
		}

	case tea.KeyEnter:
		if m.showCompletion && len(m.completions) > 0 {
			m.acceptCompletion(currPane)
			return m, nil
		}
		// ✅ Smart Indent：智能缩进
		m.insertNewLineWithIndent(currPane)
		return m, nil

	case tea.KeyTab:
		// Tab 也可以接受补全
		if m.showCompletion && len(m.completions) > 0 {
			m.acceptCompletion(currPane)
			return m, nil
		}
		// 否则插入 4 个空格
		for i := 0; i < 4; i++ {
			m.insertChar(currPane, ' ')
		}
		return m, nil

	case tea.KeyBackspace:
		// ✅ Auto Pairs：成对删除
		if pos > 0 && pos < len(runes) {
			left := runes[pos-1]
			right := runes[pos]
			// 检查是否是一对括号/引号
			isPair := (left == '{' && right == '}') ||
				(left == '[' && right == ']') ||
				(left == '(' && right == ')') ||
				(left == '"' && right == '"') ||
				(left == '\'' && right == '\'') ||
				(left == '`' && right == '`')

			if isPair {
				// 同时删除左右两个字符
				newRunes := append(runes[:pos-1], runes[pos+1:]...)
				currPane.Lines[currPane.CursorY] = string(newRunes)
				currPane.CursorX--
				m.showCompletion = false
				return m, nil
			}
		}
		// 普通删除
		m.deleteChar(currPane)
		m.showCompletion = false

	case tea.KeySpace:
		// 插入空格
		m.insertChar(currPane, ' ')
		m.showCompletion = false
	
	case tea.KeyCtrlV:
		// 粘贴 (从系统剪贴板)
		text, err := clipboard.ReadAll()
		if err != nil || text == "" {
			m.statusMsg = "ℹ 剪贴板为空"
		} else {
			m.pasteToPane(currPane, text)
			m.statusMsg = "✓ 已粘贴"
		}

	case tea.KeyUp, tea.KeyCtrlK:
		if m.showCompletion && len(m.completions) > 0 {
			m.completionIdx = (m.completionIdx - 1 + len(m.completions)) % len(m.completions)
			return m, nil
		}
		// 向上移动光标
		if currPane.CursorY > 0 {
			currPane.CursorY--
			if currPane.CursorX > len(currPane.Lines[currPane.CursorY]) {
				currPane.CursorX = len(currPane.Lines[currPane.CursorY])
			}
		}
		if currPane.CursorY < currPane.Viewport.YOffset {
			currPane.Viewport.SetYOffset(currPane.CursorY)
		}

	case tea.KeyDown, tea.KeyCtrlJ:
		if m.showCompletion && len(m.completions) > 0 {
			m.completionIdx = (m.completionIdx + 1) % len(m.completions)
			return m, nil
		}
		// 向下移动光标
		if currPane.CursorY < len(currPane.Lines)-1 {
			currPane.CursorY++
			if currPane.CursorX > len(currPane.Lines[currPane.CursorY]) {
				currPane.CursorX = len(currPane.Lines[currPane.CursorY])
			}
		}
		if currPane.CursorY >= currPane.Viewport.YOffset + currPane.Viewport.Height {
			currPane.Viewport.SetYOffset(currPane.CursorY - currPane.Viewport.Height + 1)
		}

	case tea.KeyLeft:
		// 向左移动光标
		if currPane.CursorX > 0 {
			currPane.CursorX--
		} else if currPane.CursorY > 0 {
			// 移动到上一行末尾
			currPane.CursorY--
			currPane.CursorX = len(currPane.Lines[currPane.CursorY])
		}

	case tea.KeyRight:
		// 向右移动光标
		if currPane.CursorX < len(currPane.Lines[currPane.CursorY]) {
			currPane.CursorX++
		} else if currPane.CursorY < len(currPane.Lines)-1 {
			// 移动到下一行开头
			currPane.CursorY++
			currPane.CursorX = 0
		}

	default:
		// 处理普通字符输入
		char := msg.String()
		if char != "" && len(char) == 1 {
			ch := rune(char[0])
			
			// ✅ Auto Pairs：自动成对括号/引号
			pairs := map[rune]rune{
				'{': '}', '[': ']', '(': ')', '"': '"', '\'': '\'', '`': '`',
			}
			
			if closer, isPairStart := pairs[ch]; isPairStart {
				// 插入成对字符：左 + 右
				newRunes := make([]rune, 0, len(runes)+2)
				newRunes = append(newRunes, runes[:pos]...)
				newRunes = append(newRunes, ch, closer)
				newRunes = append(newRunes, runes[pos:]...)
				currPane.Lines[currPane.CursorY] = string(newRunes)
				currPane.CursorX++ // 光标在中间
				m.triggerCompletion(currPane)
				return m, nil
			}
			
			// ✅ Auto Pairs：智能跳过闭合符号
			closers := map[rune]bool{'}': true, ']': true, ')': true, '"': true, '\'': true, '`': true}
			if closers[ch] && pos < len(runes) && runes[pos] == ch {
				// 右边已经是这个符号，直接跳过
				currPane.CursorX++
				return m, nil
			}
			
			// 普通字符插入
			m.insertChar(currPane, ch)
			
			// 自动触发补全
			if (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
				(ch >= '0' && ch <= '9') || ch == '_' || ch == '.' {
				m.triggerCompletion(currPane)
			} else {
				m.showCompletion = false
			}
		}
	}
	return m, nil
}

// =============================================================================
// 文本编辑操作
// =============================================================================

// insertChar 在光标位置插入字符 (UTF-8 safe)
func (m *Model) insertChar(p *EditorPane, ch rune) {
	line := p.Lines[p.CursorY]
	runes := []rune(line)

	// Ensure cursor doesn't exceed line length
	if p.CursorX > len(runes) {
		p.CursorX = len(runes)
	}

	// Insert the rune at cursor position
	newRunes := make([]rune, 0, len(runes)+1)
	newRunes = append(newRunes, runes[:p.CursorX]...)
	newRunes = append(newRunes, ch)
	newRunes = append(newRunes, runes[p.CursorX:]...)

	p.Lines[p.CursorY] = string(newRunes)
	p.CursorX++
}

// ... pasteToPane, insertNewLine, deleteChar are already updated ...

// triggerCompletion 触发补全菜单
func (m *Model) triggerCompletion(p *EditorPane) {
	// 检查补全功能是否启用
	if !m.completionEnabled {
		m.showCompletion = false
		return
	}
	
	// 获取光标前的文本作为前缀
	line := p.Lines[p.CursorY]
	runes := []rune(line)
	if p.CursorX > len(runes) {
		return
	}
	
	// 从光标位置向前查找前缀（包括 . 之前的包名）
	prefix := ""
	start := p.CursorX - 1
	for start >= 0 {
		ch := runes[start]
		if ch == '.' || (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || ch == '_' {
			start--
		} else {
			break
		}
	}
	start++
	if start < p.CursorX {
		prefix = string(runes[start:p.CursorX])
	}
	
	// 如果包含 "." 则总是触发（成员补全）
	// 否则至少需要2个字符才触发（避免太频繁）
	hasDot := strings.Contains(prefix, ".")
	if !hasDot && len(prefix) < 2 {
		m.showCompletion = false
		return
	}
	
	// 检测语言
	lang := DetectLanguageID(p.Filename)
	
	// 获取补全列表
	items := GetCompletions(prefix, p.Lines, lang)
	
	if len(items) > 0 {
		m.completions = items
		m.showCompletion = true
		m.completionIdx = 0
		m.completionPrefix = prefix
	} else {
		m.showCompletion = false
	}
}

// acceptCompletion 接受当前选中的补全项
func (m *Model) acceptCompletion(p *EditorPane) {
	if !m.showCompletion || len(m.completions) == 0 {
		return
	}
	
	item := m.completions[m.completionIdx]
	
	// 删除已输入的前缀（. 后面的部分）
	prefixToRemove := m.completionPrefix
	if idx := strings.LastIndex(prefixToRemove, "."); idx >= 0 {
		prefixToRemove = prefixToRemove[idx+1:]
	}
	
	// 删除前缀
	for i := 0; i < len(prefixToRemove); i++ {
		m.deleteCharBackward(p)
	}
	
	// 插入补全文本
	for _, ch := range item.InsertText {
		m.insertChar(p, ch)
	}
	
	// 关闭补全菜单
	m.showCompletion = false
	m.completions = nil
	m.statusMsg = fmt.Sprintf("✅ Inserted: %s", item.Label)
}

// deleteCharBackward 删除光标前一个字符
func (m *Model) deleteCharBackward(p *EditorPane) {
	if p.CursorX > 0 {
		line := p.Lines[p.CursorY]
		runes := []rune(line)
		if p.CursorX <= len(runes) {
			p.Lines[p.CursorY] = string(append(runes[:p.CursorX-1], runes[p.CursorX:]...))
			p.CursorX--
		}
	}
}

// callPlugin 调用 WASM 插件处理当前缓冲区
func (m *Model) callPlugin(p *EditorPane) {
	// 检查插件是否可用
	if m.pluginError != nil {
		m.statusMsg = fmt.Sprintf("⚠ 插件错误: %v", m.pluginError)
		return
	}

	if m.plugin == nil {
		m.statusMsg = "⚠ 插件未加载"
		return
	}

	// 1. 序列化
	bufferContent := strings.Join(p.Lines, "\n")

	// 2. 调用 WASM 函数
	exitCode, output, err := m.plugin.Call("process_command", []byte(bufferContent))
	if err != nil {
		m.statusMsg = fmt.Sprintf("⚠ Plugin call failed: %v", err)
		return
	}

	if exitCode != 0 {
		m.statusMsg = fmt.Sprintf("⚠ Plugin exited with code: %d", exitCode)
		return
	}

	// 3. 更新缓冲区 (假设插件返回新的文件内容)
	// 如果插件只返回修改的部分，这里需要更复杂的逻辑
	// 目前假设它是 "Filter" 模式 (Stdin -> Stdout)
	newContent := string(output)
	
	// 简单替换整个 buffer
	p.Lines = strings.Split(newContent, "\n")
	
	// 重置光标? 或者保持(如果行数变了可能越界)
	if p.CursorY >= len(p.Lines) {
		p.CursorY = len(p.Lines) - 1
	}
	if p.CursorY < 0 { p.CursorY = 0 }
	
	lineLen := len(p.Lines[p.CursorY])
	if p.CursorX > lineLen {
		p.CursorX = lineLen
	}

	m.statusMsg = "✓ Plugin processed buffer"
}

// pasteToPane 在当前光标位置粘贴文本 (支持多行)
func (m *Model) pasteToPane(p *EditorPane, text string) {
	// 处理换行符
	text = strings.ReplaceAll(text, "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	
	pasteLines := strings.Split(text, "\n")
	if len(pasteLines) == 0 {
		return
	}
	
	if len(pasteLines) == 1 {
		// 单行粘贴: 直接插入当前行
		line := p.Lines[p.CursorY]
		newLine := line[:p.CursorX] + pasteLines[0] + line[p.CursorX:]
		p.Lines[p.CursorY] = newLine
		p.CursorX += len(pasteLines[0])
	} else {
		// 多行粘贴
		currentLine := p.Lines[p.CursorY]
		left := currentLine[:p.CursorX]
		right := currentLine[p.CursorX:]
		
		// 更新当前行
		p.Lines[p.CursorY] = left + pasteLines[0]
		
		// 插入中间行
		newLines := make([]string, 0, len(p.Lines)+len(pasteLines)-1)
		newLines = append(newLines, p.Lines[:p.CursorY+1]...)
		newLines = append(newLines, pasteLines[1:len(pasteLines)-1]...)
		
		// 最后一行 + 原来光标后的内容
		lastPasteLine := pasteLines[len(pasteLines)-1] + right
		newLines = append(newLines, lastPasteLine)
		newLines = append(newLines, p.Lines[p.CursorY+1:]...)
		
		p.Lines = newLines
		p.CursorY += len(pasteLines) - 1
		p.CursorX = len(pasteLines[len(pasteLines)-1])
	}
}

// insertNewLine 在当前位置插入新行
func (m *Model) insertNewLine(p *EditorPane) {
	line := p.Lines[p.CursorY]
	// 分割当前行
	left := line[:p.CursorX]
	right := line[p.CursorX:]

	// 更新当前行并插入新行
	p.Lines[p.CursorY] = left
	newLines := make([]string, len(p.Lines)+1)
	copy(newLines[:p.CursorY+1], p.Lines[:p.CursorY+1])
	newLines[p.CursorY+1] = right
	copy(newLines[p.CursorY+2:], p.Lines[p.CursorY+1:])
	p.Lines = newLines

	// 移动光标到新行开头
	p.CursorY++
	p.CursorX = 0
}

// insertNewLineWithIndent 智能缩进换行
func (m *Model) insertNewLineWithIndent(p *EditorPane) {
	line := p.Lines[p.CursorY]
	runes := []rune(line)
	pos := p.CursorX
	if pos > len(runes) {
		pos = len(runes)
	}

	// 1. 提取当前行的缩进（空格和 Tab）
	currentIndent := ""
	for _, ch := range runes {
		if ch == ' ' || ch == '\t' {
			currentIndent += string(ch)
		} else {
			break
		}
	}

	// 2. 检查是否需要增加缩进（光标前是 { [ ( :）
	extraIndent := ""
	if pos > 0 {
		lastChar := runes[pos-1]
		if lastChar == '{' || lastChar == '[' || lastChar == '(' || lastChar == ':' {
			extraIndent = "    " // 4 空格缩进
		}
	}

	// 3. 检查是否是 "分裂模式"（Oreo Mode）：光标在 {} [] () 中间
	isSplitBlock := false
	if pos > 0 && pos < len(runes) {
		prevChar := runes[pos-1]
		nextChar := runes[pos]
		isSplitBlock = (prevChar == '{' && nextChar == '}') ||
			(prevChar == '[' && nextChar == ']') ||
			(prevChar == '(' && nextChar == ')')
	}

	left := string(runes[:pos])
	right := string(runes[pos:])

	if isSplitBlock {
		// 分裂模式：生成三行
		// 第一行：{
		// 第二行：    | (带缩进)
		// 第三行：} (原缩进)
		p.Lines[p.CursorY] = left
		
		// 插入两行
		newLines := make([]string, len(p.Lines)+2)
		copy(newLines[:p.CursorY+1], p.Lines[:p.CursorY+1])
		newLines[p.CursorY+1] = currentIndent + extraIndent // 中间行（光标位置）
		newLines[p.CursorY+2] = currentIndent + right       // 闭合括号行
		copy(newLines[p.CursorY+3:], p.Lines[p.CursorY+1:])
		p.Lines = newLines

		// 光标移到中间行的缩进末尾
		p.CursorY++
		p.CursorX = len(currentIndent) + len(extraIndent)
	} else {
		// 普通换行：继承缩进 + 额外缩进
		p.Lines[p.CursorY] = left
		
		newLine := currentIndent + extraIndent + strings.TrimLeft(right, " \t")
		
		newLines := make([]string, len(p.Lines)+1)
		copy(newLines[:p.CursorY+1], p.Lines[:p.CursorY+1])
		newLines[p.CursorY+1] = newLine
		copy(newLines[p.CursorY+2:], p.Lines[p.CursorY+1:])
		p.Lines = newLines

		// 光标移到新行的缩进末尾
		p.CursorY++
		p.CursorX = len(currentIndent) + len(extraIndent)
	}
}

// deleteChar 删除光标前的字符 (UTF-8 safe, 不会产生乱码)
func (m *Model) deleteChar(p *EditorPane) {
	if p.CursorX > 0 {
		// 使用 rune 切片删除字符（正确处理中文等多字节字符）
		line := p.Lines[p.CursorY]
		runes := []rune(line)

		// Ensure cursor doesn't exceed line length
		if p.CursorX > len(runes) {
			p.CursorX = len(runes)
		}

		// Delete the rune before cursor
		newRunes := make([]rune, 0, len(runes)-1)
		newRunes = append(newRunes, runes[:p.CursorX-1]...)
		newRunes = append(newRunes, runes[p.CursorX:]...)

		p.Lines[p.CursorY] = string(newRunes)
		p.CursorX--
	} else if p.CursorY > 0 {
		// 合并到上一行
		prevLine := p.Lines[p.CursorY-1]
		currLine := p.Lines[p.CursorY]
		// 光标位置是上一行的 rune 长度
		p.CursorX = len([]rune(prevLine))
		p.Lines[p.CursorY-1] = prevLine + currLine

		// 删除当前行
		p.Lines = append(p.Lines[:p.CursorY], p.Lines[p.CursorY+1:]...)
		p.CursorY--
	}
}

// Suppress unused import warning for runewidth (used elsewhere)
var _ = runewidth.StringWidth

// =============================================================================
// WASM 插件调用
// =============================================================================

// Duplicate callPlugin removed.

// tickMsg 用于去抖动计时器
type tickMsg time.Time

// pushProgressMsg 包含一行 Git 输出
type pushProgressMsg string

// pushDoneMsg 表示推送完成
type pushDoneMsg struct{ err error }

// waitForPushOutput 监听推送输出通道
func waitForPushOutput(sub chan string) tea.Cmd {
	return func() tea.Msg {
		data, ok := <-sub
		if !ok {
			return nil // 通道关闭，停止监听
		}
		return pushProgressMsg(data)
	}
}

// stageAllDoneMsg 表示所有文件暂存完成
type stageAllDoneMsg struct{ err error }

// stageAllCmd 异步执行 git add -A
func stageAllCmd() tea.Cmd {
	return func() tea.Msg {
		// 这里我们只需要执行命令，不需要返回输出（除非报错）
		cmd := exec.Command("git", "add", "-A")
		if err := cmd.Run(); err != nil {
			return stageAllDoneMsg{err: err}
		}
		return stageAllDoneMsg{err: nil}
	}
}

// runGitPushStream 在后台运行 git push 并流式传输输出
func runGitPushStream(sub chan string) tea.Cmd {
	return func() tea.Msg {
		// 智能推送策略: 总是尝试设置上游分支
		cmd := exec.Command("git", "push", "-u", "origin", "HEAD")
		// 关键修复: 禁用交互式提示，防止因需要认证而导致界面卡死
		cmd.Env = append(os.Environ(), "GIT_TERMINAL_PROMPT=0")

		stdout, _ := cmd.StdoutPipe()
		stderr, _ := cmd.StderrPipe()
		combinedOutput := io.MultiReader(stdout, stderr)

		if err := cmd.Start(); err != nil {
			sub <- "Error starting cmd: " + err.Error()
			close(sub)
			return pushDoneMsg{err: err}
		}

		// 读取输出并发送到通道
		scanner := bufio.NewScanner(combinedOutput)
		for scanner.Scan() {
			sub <- scanner.Text()
		}

		// 等待命令完成
		err := cmd.Wait()
		close(sub) // 关闭通道通知监听器停止
		return pushDoneMsg{err: err}
	}
}

// startPredictionDebounce 返回一个 Tick 命令，用于触发预测
func startPredictionDebounce() tea.Cmd {
	return tea.Tick(predictionDebounce, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

// predictCode 调用 WASM 预测下一段代码 (Ghost Text)
func (m *Model) predictCode() {
	if m.pluginError != nil || m.plugin == nil {
		return
	}

	// 只发送当前行做上下文 (MVP 简化)
	curTab := m.tabs[m.activeTab]
	currPane := curTab.Panes[curTab.ActivePane]
	if currPane.CursorY >= len(currPane.Lines) { return }
	currentLine := currPane.Lines[currPane.CursorY]
	
	// 如果行为空，不预测
	if strings.TrimSpace(currentLine) == "" {
		m.suggestion = ""
		return 
	}

	// 调用 WASM "predict_code"
	exitCode, output, err := m.plugin.Call("predict_code", []byte(currentLine))
	if err != nil || exitCode != 0 {
		// 忽略预测错误，不显示建议
		return
	}

	prediction := string(output)
	if prediction != "" {
		m.suggestion = prediction
		// 调试信息 (可选)
		// m.statusMsg = fmt.Sprintf("AI建议: %s", prediction)
	}
}

// =============================================================================
// 语法高亮
// =============================================================================

// highlight 使用 Chroma 对内容进行语法高亮
//
// TODO: ViewPort Highlighting 优化
// 当前实现会在每次按键时高亮整个缓冲区，对于大文件可能会有性能问题。
// 未来优化方向：
// 1. 只高亮可见区域（ViewPort）
// 2. 增量高亮（只重新高亮改变的部分）
// 3. 使用缓存避免重复计算
// highlight 使用 Chroma 对内容进行语法高亮
// 已优化：使用 Model 中缓存的 Lexer/Style/Formatter
// highlight and highlightLine removed (obsolete)

// =============================================================================
// 视图渲染
// =============================================================================

// syncSizes 立即同步布局尺寸 (解决 State Lag 问题)
func (m *Model) syncSizes() {
	// Sync Help Viewport
	m.helpViewport.Width = m.width - 6
	m.helpViewport.Height = m.height - 4

	// 侧边栏逻辑
	sidebarWidth := 0
	if m.showSidebar || m.showGit {
		sidebarWidth = 30
	}

	// 编辑器总可用区域
	editorTotalWidth := m.width - sidebarWidth
	if editorTotalWidth < 10 {
		editorTotalWidth = 10
	}

	// 动态高度逻辑: 严谨的垂直空间预算 (Strict Vertical Budgeting)
	availableHeight := m.height

	// 1. Always subtract the Header (Top Tab Bar)
	// We restore HeaderHeight for the Tabs
	availableHeight -= 1 

	// 2. Subtract Bottom Element based on Mode
	if m.mode == WhichKeyMode {
		availableHeight -= WhichKeyHeight
	} else {
		// Normal mode has a status bar
		availableHeight -= StatusBarHeight
		// 补全菜单作为覆盖层，不改变主内容高度
	}



	// Safeguard
	if availableHeight < 0 {
		availableHeight = 0
	}

	editorTotalHeight := availableHeight

	// 更新缓存值
	m.cachedSidebarWidth = sidebarWidth
	m.cachedEditorWidth = editorTotalWidth
	m.cachedContentHeight = editorTotalHeight

	// 更新 FileTree 尺寸
	if m.fileTree.State != TreeNormal {
		// m.fileTree.SetSize(sidebarWidth, editorTotalHeight)
	}

	// 分配 Pane 尺寸 (Active Tab Only)
	if len(m.tabs) == 0 { return }
	activeTab := m.tabs[m.activeTab]
	
	if len(activeTab.Panes) == 0 {
		return
	}

	for i, pane := range activeTab.Panes {
		width := editorTotalWidth
		height := editorTotalHeight

		if activeTab.SplitType == VerticalSplit {
			width = editorTotalWidth / 2
			if len(activeTab.Panes) > 1 {
				width = (editorTotalWidth - 1) / 2
			}
			// Fix parity for last one?
			if i == len(activeTab.Panes)-1 && len(activeTab.Panes) > 1 {
				// remaining = total - (n-1)*width - (n-1)*separator
				// Simple 2 pane logic:
				width = editorTotalWidth - width - 1
			}
		} else if activeTab.SplitType == HorizontalSplit {
			height = editorTotalHeight / 2
			if len(activeTab.Panes) > 1 {
				height = (editorTotalHeight - 1) / 2
			}
			if i == len(activeTab.Panes)-1 && len(activeTab.Panes) > 1 {
				height = editorTotalHeight - height - 1
			}
		}

		pane.Width = width
		pane.Height = height
		pane.Viewport.Width = width
		pane.Viewport.Height = height
	}

}

// calculateSizes 集中计算布局尺寸 (Atomic Layout)
func (m Model) calculateSizes() (int, int, int, int) {
	// 使用缓存值 (syncSizes 已同步)
	if m.cachedSidebarWidth > 0 || m.cachedEditorWidth > 0 {
		return m.cachedSidebarWidth, m.cachedEditorWidth, m.cachedContentHeight, m.cachedContentHeight
	}

	// Fallback: 手动计算 (初始化时)
	statusBar := m.renderStatusBar()
	statusBarHeight := lipgloss.Height(statusBar)

	sidebarWidth := 0
	if m.showSidebar || m.showGit {
		sidebarWidth = 30
	}

	editorWidth := m.width - sidebarWidth
	contentHeight := m.height - statusBarHeight
	if contentHeight < 0 {
		contentHeight = 0
	}

	return sidebarWidth, editorWidth, contentHeight, contentHeight
}

// viewHeader 渲染顶部标题栏
func (m Model) viewHeader() string {
	style := lipgloss.NewStyle().
		Foreground(lipgloss.Color("252")).
		Background(lipgloss.Color("235")). // Dark header background
		Bold(true).
		Padding(0, 1).
		Width(m.width).
		Height(HeaderHeight) // Strict height

	// Simple title
	title := "FuckVim 🚀"
	if len(m.tabs) > 0 {
		curTab := m.tabs[m.activeTab]
		if len(curTab.Panes) > 0 {
			active := curTab.Panes[curTab.ActivePane]
			if active.Filename != "" {
				title += " | " + active.Filename
			}
		}
	}
	// Add some hint
	hint := "Space=Menu"
	
	// Flex layout: Title ...... Hint
	spaces := m.width - lipgloss.Width(title) - lipgloss.Width(hint) - 2 // -2 padding
	if spaces < 1 { spaces = 1 }
	
	content := title + strings.Repeat(" ", spaces) + hint
	return style.Render(content)
}

// viewTabs 渲染顶部标签栏
func (m Model) viewTabs() string {
	var tabs []string
	for i, t := range m.tabs {
		name := fmt.Sprintf(" %d: %s ", i+1, t.Name)
		
		// Styling
		style := lipgloss.NewStyle().
			Foreground(lipgloss.Color("240")).
			Background(lipgloss.Color("235")).
			Padding(0, 1)

		if i == m.activeTab {
			style = lipgloss.NewStyle().
				Foreground(lipgloss.Color("232")). // Dark Text
				Background(lipgloss.Color("205")). // Pink Bg
				Bold(true).
				Padding(0, 1)
		}
		
		tabs = append(tabs, style.Render(name))
	}
	
	// Fill rest of line?
	row := lipgloss.JoinHorizontal(lipgloss.Top, tabs...)
	bg := lipgloss.NewStyle().Background(lipgloss.Color("235")).Width(m.width - lipgloss.Width(row)).Render("")
	
	return lipgloss.JoinHorizontal(lipgloss.Top, row, bg)
}

// View 渲染 UI
func (m Model) View() string {
	
	// 0. Help Overlay (Highest Priority)
	if m.mode == HelpMode {
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("62")).
			Padding(1, 2).
			Width(m.width - 4).
			Height(m.height - 2)
			
		return style.Render(m.helpViewport.View())
	}

	// 1. 确保尺寸同步
	if m.width < 40 || m.height < 10 {
		return "窗口太小，请调整尺寸 (Window too small)"
	}

	// Handle Fuzzy Find modal FIRST (overlay)
	if m.mode == FuzzyFindMode {
		return m.renderFuzzyFinder()
	}

	// 1. Render Header (Tab Bar)
	header := m.viewTabs()

	// 2. Main Content Layout (Sidebar & Editor) follows...

	// 1. 原子化计算布局尺寸
	sidebarWidth, editorWidth, sidebarHeight, editorHeight := m.calculateSizes()

	// 预先计算补全菜单高度，从编辑器底部减去（保持顶部不动）
	completionMenuHeight := 0
	if m.showCompletion && len(m.completions) > 0 {
		maxItems := 5
		if len(m.completions) < maxItems {
			maxItems = len(m.completions)
		}
		completionMenuHeight = maxItems + 2 // 菜单项 + 边框
		editorHeight -= completionMenuHeight
		sidebarHeight -= completionMenuHeight
		if editorHeight < 5 {
			editorHeight = 5
		}
		if sidebarHeight < 5 {
			sidebarHeight = 5
		}
	}

	// 2. 渲染侧边栏 (如果可见)
	var leftPanel string
	if sidebarWidth > 0 {
		if m.showSidebar && m.showGit {
			// 上下分屏
			halfHeight := sidebarHeight / 2
			gitHeight := sidebarHeight - halfHeight
			
			treeView := m.renderSidebar(sidebarWidth, halfHeight)
			gitView := m.renderGit(sidebarWidth, gitHeight)
			
			leftPanel = lipgloss.JoinVertical(lipgloss.Left, treeView, gitView)
		} else if m.showSidebar {
			leftPanel = m.renderSidebar(sidebarWidth, sidebarHeight)
		} else if m.showGit {
			leftPanel = m.renderGit(sidebarWidth, sidebarHeight)
		}
		
		// 强制侧边栏精确尺寸
		leftPanel = lipgloss.Place(sidebarWidth, sidebarHeight, lipgloss.Left, lipgloss.Top, leftPanel)
	}

	// 3. 渲染编辑器 (Split View Logic)
	var editorView string
	editorHasFocus := m.focus == FocusEditor
	
	if len(m.tabs) > 0 {
		curTab := m.tabs[m.activeTab]
		if len(curTab.Panes) == 0 {
			editorView = "" // Should not happen
		} else if len(curTab.Panes) == 1 {
			// Single Pane
			editorView = m.renderPane(curTab.Panes[0], editorWidth, editorHeight, editorHasFocus && curTab.ActivePane == 0)
		} else {
			// Split Pane
			pane0 := m.renderPane(curTab.Panes[0], curTab.Panes[0].Width, curTab.Panes[0].Height, editorHasFocus && curTab.ActivePane == 0)
			pane1 := m.renderPane(curTab.Panes[1], curTab.Panes[1].Width, curTab.Panes[1].Height, editorHasFocus && curTab.ActivePane == 1)

			if curTab.SplitType == VerticalSplit {
				editorView = lipgloss.JoinHorizontal(lipgloss.Top, pane0, pane1)
			} else {
				editorView = lipgloss.JoinVertical(lipgloss.Left, pane0, pane1)
			}
		}
	} else {
		editorView = "No Tabs Open"
	}

	// 强制编辑器精确尺寸
	editorView = lipgloss.Place(editorWidth, editorHeight, lipgloss.Left, lipgloss.Top, editorView)

	// 5. Main Content Assembly
	mainContent := lipgloss.JoinHorizontal(lipgloss.Top, leftPanel, editorView)

	// ---------------------------------------------------------
	// 🚀 补全菜单 (Docked Panel - 底部停靠方案)
	// ---------------------------------------------------------
	var completionPanel string
	if m.showCompletion && len(m.completions) > 0 {
		// 渲染补全菜单项
		var menuLines []string
		maxItems := 5
		
		// 滚动窗口逻辑：让选中项尽量在中间
		displayList := m.completions
		startIdx := 0
		if len(displayList) > maxItems {
			startIdx = m.completionIdx - 2
			if startIdx < 0 {
				startIdx = 0
			}
			endIdx := startIdx + maxItems
			if endIdx > len(displayList) {
				endIdx = len(displayList)
				startIdx = endIdx - maxItems
				if startIdx < 0 {
					startIdx = 0
				}
			}
			displayList = displayList[startIdx : startIdx+maxItems]
			if len(displayList) > maxItems {
				displayList = displayList[:maxItems]
			}
		}

		for i, item := range displayList {
			// 计算实际索引用于高亮判断
			realIdx := startIdx + i
			
			// 图标
			kindIcon := "  "
			switch item.Kind {
			case "func":
				kindIcon = "ƒ "
			case "keyword":
				kindIcon = "▷ "
			case "snippet":
				kindIcon = "✪ "
			case "variable":
				kindIcon = "χ "
			case "module":
				kindIcon = "□ "
			case "struct":
				kindIcon = "◈ "
			}

			// 样式 - 无背景色
			prefix := "  "
			style := lipgloss.NewStyle().Foreground(lipgloss.Color("250"))

			if realIdx == m.completionIdx {
				prefix = "▶ "
				style = lipgloss.NewStyle().
					Foreground(lipgloss.Color("214")). // 橙色高亮
					Bold(true)
			}

			// 格式化行
			label := item.Label
			if len(label) > 25 {
				label = label[:22] + "..."
			}
			lineContent := fmt.Sprintf("%s%s%-25s", prefix, kindIcon, label)
			menuLines = append(menuLines, style.Render(lineContent))
		}

		// 组合菜单 View，加完整边框
		menuContent := lipgloss.JoinVertical(lipgloss.Left, menuLines...)
		completionPanel = lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()). // 完整圆角边框
			BorderForeground(lipgloss.Color("62")).
			Width(m.width - 4).
			Render(menuContent)
	}

	// 6. 渲染底部区域 (Menu or Status Bar or Command Input)
	var bottom string
	if m.mode == WhichKeyMode {
		// Force exact height for menu to prevent overflow or gaps
		// Using WhichKeyHeight which is accounted for in calculateSizes
		style := lipgloss.NewStyle().Height(WhichKeyHeight).MaxHeight(WhichKeyHeight)
		bottom = style.Render(m.viewWhichKey())
	} else if m.mode == CommandMode || m.mode == ModeGitCommit {
		// Command Input Bar (Vim Style) - 不使用 Background 避免光标闪烁时背景也闪
		inputView := m.commandInput.View()
		// 填充到整行宽度
		padding := m.width - lipgloss.Width(inputView)
		if padding > 0 {
			inputView = inputView + strings.Repeat(" ", padding)
		}
		bottom = inputView
	} else {
		// Normal Status Bar (Powerline)
		bottom = m.renderStatusBar()
	}

	// 7. 最终组装：Header + Content + [补全菜单] + StatusBar
	if completionPanel != "" {
		return lipgloss.JoinVertical(lipgloss.Left, header, mainContent, completionPanel, bottom)
	}
	return lipgloss.JoinVertical(lipgloss.Left, header, mainContent, bottom)
}

// viewWhichKey 渲染 WhichKey 菜单 (LazyVim-style Leader Key Menu)
func (m Model) viewWhichKey() string {
	// Styles
	keyStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("205")).
		Bold(true)
	arrowStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("241"))
	descStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("252"))
	titleStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("220")).
		Bold(true)

	// Build rows
	var rows []string
	for _, item := range rootKeys {
		row := fmt.Sprintf("%s %s %s",
			keyStyle.Render(item.Key),
			arrowStyle.Render("→"),
			descStyle.Render(m.tr(item.Desc))) // Translated
		rows = append(rows, row)
	}

	// Split into 2 columns
	mid := (len(rows) + 1) / 2
	col1 := strings.Join(rows[:mid], "\n")
	col2 := ""
	if mid < len(rows) {
		col2 = strings.Join(rows[mid:], "\n")
	}

	// Join columns with gap
	colStyle := lipgloss.NewStyle().Width(m.width/2 - 4)
	body := lipgloss.JoinHorizontal(lipgloss.Top,
		colStyle.Render(col1),
		colStyle.Render(col2))

	// Container with explicit height
	containerStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("62")).
		Padding(1, 2).
		Width(m.width - 4).
		Height(WhichKeyHeight - 2) // ✅ 修复：强制高度确保边框显示

	title := titleStyle.Render("⌨ WhichKey Menu  (Space/Esc to close)")
	content := fmt.Sprintf("%s\n\n%s", title, body)

	return containerStyle.Render(content)
}

// renderFuzzyFinder 渲染模糊搜索弹窗
func (m Model) renderFuzzyFinder() string {
	// Calculate centered popup size
	popupW := m.width * 6 / 10
	popupH := m.height * 6 / 10
	if popupW < 40 {
		popupW = 40
	}
	if popupH < 10 {
		popupH = 10
	}

	// Build content: Title + Input + List
	var content strings.Builder

	// Title
	titleStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("205")).
		Bold(true)
	content.WriteString(titleStyle.Render(m.tr("find.title")))
	content.WriteString("\n\n")

	// Input field
	inputStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("62")).
		Padding(0, 1).
		Width(popupW - 8)
	content.WriteString(inputStyle.Render(m.finderInput.View()))
	content.WriteString("\n\n")

	// Filtered results list
	listHeight := popupH - 10 // Reserve space for title, input, borders
	if listHeight < 3 {
		listHeight = 3
	}

	selectedStyle := lipgloss.NewStyle().
		Background(lipgloss.Color("62")).
		Foreground(lipgloss.Color("230")).
		Bold(true)

	normalStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("252"))

	// Render visible items
	startIdx := 0
	if m.finderCursor >= listHeight {
		startIdx = m.finderCursor - listHeight + 1
	}

	for i := startIdx; i < len(m.filteredFiles) && i < startIdx+listHeight; i++ {
		item := m.filteredFiles[i]
		line := item.path
		if len(line) > popupW-10 {
			line = line[:popupW-13] + "..."
		}

		if i == m.finderCursor {
			content.WriteString("▸ " + selectedStyle.Render(line))
		} else {
			content.WriteString("  " + normalStyle.Render(line))
		}
		content.WriteString("\n")
	}

	// Fill empty lines if fewer items
	for i := len(m.filteredFiles); i < listHeight; i++ {
		content.WriteString("\n")
	}

	// Footer with count
	countStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("241"))
	content.WriteString("\n")
	content.WriteString(countStyle.Render(fmt.Sprintf("%d/%d files", len(m.filteredFiles), len(m.allFiles))))

	// Style the popup
	popupStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("62")).
		Padding(1, 2).
		Width(popupW).
		Height(popupH)

	popupContent := popupStyle.Render(content.String())

	// Center on screen with dimmed background
	return lipgloss.Place(
		m.width, m.height,
		lipgloss.Center, lipgloss.Center,
		popupContent,
		lipgloss.WithWhitespaceChars(" "),
		lipgloss.WithWhitespaceForeground(lipgloss.Color("237")),
	)
}

// renderWindow 渲染通用带边框窗口
func renderWindow(content string, title string, isActive bool, width, height int, isGitSelection bool) string {
	borderColor := lipgloss.Color("240") // Default Gray
	if isActive {
		borderColor = lipgloss.Color("205") // Active Pink
	}
	if isGitSelection {
		borderColor = lipgloss.Color("220") // Selection Yellow
	}

	// --------------------------------------------------------
	// 简化边框渲染：使用 lipgloss 原生边框，手动构建标题行
	// --------------------------------------------------------
	b := lipgloss.RoundedBorder()
	borderSt := lipgloss.NewStyle().Foreground(borderColor)
	titleSt := lipgloss.NewStyle().Foreground(borderColor).Bold(true)

	// 内容区域尺寸 (减去左右边框各1，上下边框各1)
	innerWidth := width - 2
	innerHeight := height - 2
	if innerWidth < 0 { innerWidth = 0 }
	if innerHeight < 0 { innerHeight = 0 }

	// 处理内容：确保精确的行数和宽度
	contentLines := strings.Split(strings.TrimSuffix(content, "\n"), "\n")
	var paddedLines []string
	for i := 0; i < innerHeight; i++ {
		var line string
		if i < len(contentLines) {
			line = contentLines[i]
		} else {
			line = ""
		}
		
		lineWidth := lipgloss.Width(line)
		if lineWidth > innerWidth {
			// 先截断
			line = truncateToWidth(line, innerWidth)
			lineWidth = lipgloss.Width(line)
		}
		// 不管是否截断，都要填充到精确宽度
		if lineWidth < innerWidth {
			line = line + strings.Repeat(" ", innerWidth-lineWidth)
		}
		paddedLines = append(paddedLines, line)
	}

	// 构建完整的窗口 - 预计算容量提升性能
	var result strings.Builder
	estimatedSize := (width + 2) * (height + 1) // 粗略估计
	result.Grow(estimatedSize)

	// 1. 顶部边框 + 标题: ╭─Title────────╮
	leftStr := b.TopLeft + b.Top
	titleW := lipgloss.Width(title)
	dashCount := width - lipgloss.Width(leftStr) - titleW - lipgloss.Width(b.TopRight)
	if dashCount < 0 { dashCount = 0 }
	rightStr := strings.Repeat(b.Top, dashCount) + b.TopRight
	topLine := borderSt.Render(leftStr) + titleSt.Render(title) + borderSt.Render(rightStr)
	result.WriteString(topLine)
	result.WriteString("\n")

	// 2. 中间内容行: │content│
	leftBorder := borderSt.Render(b.Left)
	rightBorder := borderSt.Render(b.Right)
	for _, line := range paddedLines {
		result.WriteString(leftBorder)
		result.WriteString(line)
		result.WriteString(rightBorder)
		result.WriteString("\n")
	}

	// 3. 底部边框: ╰──────────────╯
	bottomLine := borderSt.Render(b.BottomLeft + strings.Repeat(b.Bottom, innerWidth) + b.BottomRight)
	result.WriteString(bottomLine)

	return result.String()
}

// truncateToWidth truncates a string to visual width w, preserving ANSI codes if possible.
func truncateToWidth(s string, w int) string {
	if lipgloss.Width(s) <= w {
		return s
	}
	
	// Convert to runes to handle multi-byte slicing safely
	// BUT slicing string directly is needed for ANSI check? No, ANSI are bytes.
	// But lipgloss.Width handles string.
	// We want to find index k such that Width(s[:k]) <= w and Width(s[:k+1]) > w.
	// Use binary search on BYTE indices.
	// Refinement: use range loop to find character boundaries.
	
	validIndices := make([]int, 0, len(s))
	for i := range s {
		validIndices = append(validIndices, i)
	}
	validIndices = append(validIndices, len(s))
	
	low := 0
	high := len(validIndices) - 1
	bestIdx := 0
	
	for low <= high {
		mid := (low + high) / 2
		byteIdx := validIndices[mid]
		sub := s[:byteIdx]
		width := lipgloss.Width(sub)
		
		if width <= w {
			bestIdx = byteIdx
			low = mid + 1
		} else {
			high = mid - 1
		}
	}
	
	// Append reset code if we truncated (heuristic)
	// Or trust user not to end in weird state.
	// Adding \x1b[0m is safe.
	return s[:bestIdx] + "\x1b[0m"
}

// renderPane 渲染单个编辑器窗格
func (m Model) renderPane(p *EditorPane, width, height int, isActive bool) string {
	// Calculate inner dimensions
	innerWidth := width - 2
	innerHeight := height - 2
	if innerWidth < 0 { innerWidth = 0 }
	if innerHeight < 0 { innerHeight = 0 }
	
	// Update Viewport dimensions for scrolling calculations
	p.Viewport.Width = innerWidth
	p.Viewport.Height = innerHeight
	
	// Ensure YOffset keeps cursor in view
	if p.CursorY < p.Viewport.YOffset {
		p.Viewport.YOffset = p.CursorY
	}
	if p.CursorY >= p.Viewport.YOffset + innerHeight {
		p.Viewport.YOffset = p.CursorY - innerHeight + 1
	}
	if p.Viewport.YOffset < 0 {
		p.Viewport.YOffset = 0
	}
	
	var lines []string
	
	// Code width (after line number)
	codeWidth := innerWidth - 7
	if codeWidth < 1 { codeWidth = 1 }

	// Syntax highlighting setup
	lexer := lexers.Match(p.Filename)
	if lexer == nil { lexer = lexers.Fallback }
	lexer = chroma.Coalesce(lexer)
	style := styles.Get("dracula")
	if style == nil { style = styles.Fallback }
	formatter := formatters.TTY256

	// Render visible lines directly from p.Lines
	startLine := p.Viewport.YOffset
	endLine := startLine + innerHeight
	if endLine > len(p.Lines) {
		endLine = len(p.Lines)
	}

	for lineIdx := startLine; lineIdx < endLine; lineIdx++ {
		rawLine := p.Lines[lineIdx]

		// =============================================
		// Line Number Display (toggleable via :toggle-nu)
		// - Relative mode: current = absolute, others = distance
		// - Absolute mode: all lines show absolute numbers
		// =============================================
		isCursorLine := isActive && lineIdx == p.CursorY
		var lineNumStr string

		if m.relativeLineNumbers {
			// Hybrid Relative Mode (Vim-style)
			if isCursorLine {
				// Current line: show absolute line number
				lineNumStr = fmt.Sprintf("%4d", lineIdx+1)
			} else {
				// Other lines: show relative distance
				relDist := lineIdx - p.CursorY
				if relDist < 0 {
					relDist = -relDist
				}
				lineNumStr = fmt.Sprintf("%4d", relDist)
			}
		} else {
			// Absolute Mode (Standard)
			lineNumStr = fmt.Sprintf("%4d", lineIdx+1)
		}

		// Line number styling
		lineNumStyleToUse := lineNumberStyle
		if isCursorLine {
			lineNumStyleToUse = lipgloss.NewStyle().
				Foreground(lipgloss.Color("220")). // Gold for current line
				Bold(true).
				Width(4).
				Align(lipgloss.Right)
		}
		lineNumStyled := lineNumStyleToUse.Render(lineNumStr)

		var lineContent string

		// Cursor line: render with cursor block, cursor line has subtle background
		if isCursorLine {
			runes := []rune(rawLine)
			cx := p.CursorX
			if cx > len(runes) {
				cx = len(runes)
			}

			if cx == len(runes) {
				// Cursor at EOL
				lineContent = string(runes) + "\x1b[7m \x1b[0m"
			} else {
				before := string(runes[:cx])
				char := string(runes[cx])
				after := string(runes[cx+1:])
				cursorChar := "\x1b[7m" + char + "\x1b[0m"
				lineContent = before + cursorChar + after
			}
		} else {
			// Non-cursor line: apply syntax highlighting
			it, err := lexer.Tokenise(nil, rawLine)
			var highlighted bytes.Buffer
			if err == nil {
				formatter.Format(&highlighted, style, it)
				lineContent = strings.ReplaceAll(highlighted.String(), "\n", "")
			} else {
				lineContent = rawLine
			}
		}

		lines = append(lines, fmt.Sprintf("%s │ %s", lineNumStyled, lineContent))
	}
	
	// Fill empty space if fewer lines than innerHeight
	for len(lines) < innerHeight {
		lineNum := lineNumberStyle.Render("~")
		lines = append(lines, fmt.Sprintf("%s │", lineNum))
	}

	title := fmt.Sprintf("Edit:%s", filepath.Base(p.Filename))
	if p.Filename == "" { title = "[No Name]" }

	return renderWindow(strings.Join(lines, "\n"), title, isActive, width, height, false)
}


// renderSidebar 渲染文件树侧边栏
func (m Model) renderSidebar(width, height int) string {
	var lines []string

	// 内容高度 (reserve 2 for border, 3 for input/confirm if active - border needs 3 lines)
	contentHeight := height - 2
	inputAreaHeight := 0
	if m.fileTree.State == TreeInput || m.fileTree.State == TreeConfirmDelete {
		inputAreaHeight = 3 // top border + content + bottom border
	}
	visibleHeight := contentHeight - inputAreaHeight
	if visibleHeight < 0 { visibleHeight = 0 }

	for i, entry := range m.fileTree.Entries {
		if i >= visibleHeight {
			break
		}

		// 图标
		icon := " 📄 "
		if entry.isDir {
			icon = " 📁 "
		}

		name := entry.name
		if entry.isDir {
			name += "/"
		}
		
		// 动态计算截断长度
		// icon (4 chars) + text
		availableTextWidth := width - 2 - 4 // border(2) - icon(4)
		if availableTextWidth < 5 { availableTextWidth = 5 }

		if len(name) > availableTextWidth {
			name = name[:availableTextWidth-3] + "..."
		}

		line := icon + name

		// 高亮
		if i == m.fileTree.cursor {
			line = treeSelectedStyle.Render(line)
		} else if entry.isDir {
			line = treeDirStyle.Render(line)
		} else {
			line = treeItemStyle.Render(line)
		}

		lines = append(lines, line)
	}

	// 填充空行
	usedLines := len(lines)
	remaining := visibleHeight - usedLines
	for i := 0; i < remaining; i++ {
		lines = append(lines, "")
	}
	
	// Render input box or confirmation at bottom
	if m.fileTree.State == TreeInput {
		// Input box
		inputStyle := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color("39")).
			Width(width - 4)
		inputView := inputStyle.Render(m.fileTree.Input.View())
		lines = append(lines, inputView)
	} else if m.fileTree.State == TreeConfirmDelete {
		// Delete confirmation (red)
		confirmStyle := lipgloss.NewStyle().
			Foreground(lipgloss.Color("196")).
			Bold(true)
		fileName := filepath.Base(m.fileTree.Selected)
		confirmText := confirmStyle.Render(fmt.Sprintf("⚠️ "+m.tr("tree.delete_confirm"), fileName))
		lines = append(lines, confirmText)
	}

	title := fmt.Sprintf("Files:%s", filepath.Base(m.fileTree.rootPath))
	isActive := m.focus == FocusFileTree
	return renderWindow(strings.Join(lines, "\n"), title, isActive, width, height, m.selectingGitRoot)
}

// renderGit 渲染 Git 面板
func (m Model) renderGit(width, height int) string {
	var lines []string

	// 内容高度
	contentHeight := height - 2
	visibleHeight := contentHeight
	if visibleHeight < 0 { visibleHeight = 0 }

	// 如果不是 Git 仓库
	if !m.git.IsRepo {
		lines = append(lines, "")
		lines = append(lines, "  Not a Git Repo")
		lines = append(lines, "")
		lines = append(lines, "  Press 'i' to init")
		
		// 填充剩余行
		for len(lines) < visibleHeight {
			lines = append(lines, "")
		}

		isActive := m.focus == FocusGit
		return renderWindow(strings.Join(lines, "\n"), "Git-NoRepo", isActive, width, height, false)
	}

	if len(m.git.Files) == 0 {
		// Sync Dashboard (Translated)
		output := "\n"
		branchStr := lipgloss.NewStyle().Foreground(lipgloss.Color("205")).Render(m.git.Branch)
		output += fmt.Sprintf("  ✨ Branch: %s\n", branchStr)
		
		output += fmt.Sprintf("  %s\n", m.tr("git.clean"))
		output += fmt.Sprintf("  %s\n\n", m.tr("git.clean_sub"))
		
		if m.git.Ahead == 0 && m.git.Behind == 0 {
			output += "  ✅ Up to date"
		} else {
			if m.git.Ahead > 0 {
				output += fmt.Sprintf("  🚀 准备推送: %d 个提交待上传\n", m.git.Ahead)
			}
			if m.git.Behind > 0 {
				output += fmt.Sprintf("  📥 需拉取: %d 个提交\n", m.git.Behind)
			}
			output += "\n  [ Shift+P ] 推送到远程"
			output += "\n  [ :pull ] 拉取更新"
		}
		
		return renderWindow(output, m.tr("wk.git"), m.focus == FocusGit, width, height, false)
	}

	// Git Repo Content
	for i, file := range m.git.Files {
		if i >= visibleHeight {
			break
		}

		icon := " "
		var style lipgloss.Style
		
		if file.Staged {
			icon = "✓"
			style = gitStagedStyle
		} else {
			switch file.Status {
			case StatusModified:
				icon = "M"
				style = gitModifiedStyle
			case StatusUntracked:
				icon = "?"
				style = gitUntrackedStyle
			case StatusDeleted:
				icon = "D"
				style = gitModifiedStyle
			case StatusAdded:
				icon = "A"
				style = gitStagedStyle
			}
		}

		name := filepath.Base(file.Path)
		// Adjust truncate logic
		availWidth := width - 2 - 4
		if availWidth < 5 { availWidth = 5 }
		if len(name) > availWidth {
			name = name[:availWidth-3] + "..."
		}
		
		line := fmt.Sprintf("%s %s", icon, name)
		if i == m.git.Cursor {
			line = treeSelectedStyle.Render(line)
		} else {
			line = style.Render(line)
		}

		lines = append(lines, line)
	}

	// 填充剩余行
	for len(lines) < visibleHeight {
		lines = append(lines, "")
	}

	// Determine Title State
	title := m.tr("wk.git")
	if len(m.git.Files) > 0 {
		title += " *" // Add helper indicator for dirty
	}

	isActive := m.focus == FocusGit
	return renderWindow(strings.Join(lines, "\n"), title, isActive, width, height, false)
}


// renderLine removed (obsolete)

// getModeInfo Helper for Status Bar Colors
func (m Model) getModeInfo() (string, lipgloss.Color) {
	switch m.mode {
	case InsertMode:
		return m.tr("status.insert"), lipgloss.Color("42") // Green
	case CommandMode, FuzzyFindMode:
		return m.tr("status.command"), lipgloss.Color("208") // Orange
	case WhichKeyMode:
		return "WHICH-KEY", lipgloss.Color("205") // Pink
	case HelpMode: // Add HelpMode case
		return "HELP", lipgloss.Color("63") // Cyan for Help
	default:
		return m.tr("status.normal"), lipgloss.Color("33") // Blue
	}
}

// renderStatusBar 渲染状态栏 (Powerline / Lualine Style)
func (m Model) renderStatusBar() string {
    // 1. Get Data from Active Pane
    if len(m.tabs) == 0 { return "" }
    currentTab := m.tabs[m.activeTab]
    if len(currentTab.Panes) == 0 { return "" }
    pane := currentTab.Panes[currentTab.ActivePane]
    
    // Data points
    modeLabel, modeColor := m.getModeInfo()
    gitBranch := m.git.Branch
    if gitBranch == "" { gitBranch = "master" } // Fallback or empty if not repo
    filename := pane.Filename
    if filename == "" { filename = "[No Name]" }
    
    // Cursor Info
    cursorRow := pane.CursorY + 1
    cursorCol := pane.CursorX + 1 
    
    // 2. Define Styles
    // Colors
    colorGray := lipgloss.Color("237")
    colorLightGray := lipgloss.Color("245")
    colorWhite := lipgloss.Color("255")
    
    // Symbols
    arrow := "\uE0B0" 
    arrowLeft := "\uE0B2"
    
    // Style Builders
    // A. Mode Block
    styleMode := lipgloss.NewStyle().
        Foreground(lipgloss.Color("232")). // Dark Text
        Background(modeColor).
        Bold(true).
        Padding(0, 1)
        
    // B. Git Block
    styleGit := lipgloss.NewStyle().
        Foreground(colorWhite).
        Background(colorGray).
        Padding(0, 1)

    // C. Filename Block (Middle)
    styleFile := lipgloss.NewStyle().
        Foreground(colorLightGray). // Grey text
        Background(lipgloss.Color("235")). // Darker Gray
        Padding(0, 1)
        
    // D. Right Meta Block
    styleMeta := lipgloss.NewStyle().
        Foreground(colorWhite).
        Background(colorGray).
        Padding(0, 1)

    // E. Coordinate Block
    styleCoord := lipgloss.NewStyle().
        Foreground(lipgloss.Color("232")).
        Background(modeColor). // Match Mode color for symmetry
        Bold(true).
        Padding(0, 1)

    // 3. Render Segments with Transitions (The Powerline Trick)
    
    // --- LEFT SIDE ---
    // Mode -> Arrow(ModeColor to Gray) -> Git
    segMode := styleMode.Render(modeLabel)
    arrow1 := lipgloss.NewStyle().Foreground(modeColor).Background(colorGray).Render(arrow)
    
    // Git -> Arrow(Gray to DarkGray) -> File
    segGit := styleGit.Render(" " + gitBranch)
    arrow2 := lipgloss.NewStyle().Foreground(colorGray).Background(lipgloss.Color("235")).Render(arrow)
    
    segFile := styleFile.Render(filename)
    // End of left side arrow (DarkGray to Transparent/Black)
    // Assuming default terminal bg (Color 0 or Transparent)
    // But lipgloss Default Background is usually Terminal BG.
    // If we want transparency, maybe no background?
    // Let's assume Black ("234" or "0") matches terminal mostly.
    arrow3 := lipgloss.NewStyle().Foreground(lipgloss.Color("235")).Render(arrow) 

    // --- RIGHT SIDE ---
    
    // Transparent -> Gray
    arrowR1 := lipgloss.NewStyle().Foreground(colorGray).Render(arrowLeft)
    segType := styleMeta.Render("UTF-8 | Go") // Hardcoded for MVP, dynamic later
    
    // Gray -> ModeColor
    arrowR2 := lipgloss.NewStyle().Foreground(modeColor).Background(colorGray).Render(arrowLeft)
    segCoord := styleCoord.Render(fmt.Sprintf("Ln %d, Col %d", cursorRow, cursorCol))

    // 4. Spacer (Push right side to the edge)
    leftBlock := lipgloss.JoinHorizontal(lipgloss.Bottom, segMode, arrow1, segGit, arrow2, segFile, arrow3)
    rightBlock := lipgloss.JoinHorizontal(lipgloss.Bottom, arrowR1, segType, arrowR2, segCoord)
    
    // Calculate available width
    w := lipgloss.Width
    availableWidth := m.width - w(leftBlock) - w(rightBlock)
    if availableWidth < 0 { availableWidth = 0 }
    
    // 如果有状态消息，在中间显示
    var spacer string
    if m.statusMsg != "" {
        statusStyle := lipgloss.NewStyle().
            Foreground(lipgloss.Color("220")). // 黄色
            Bold(true)
        statusText := " " + m.statusMsg + " "
        statusWidth := w(statusText)
        if statusWidth < availableWidth {
            leftPad := (availableWidth - statusWidth) / 2
            rightPad := availableWidth - statusWidth - leftPad
            spacer = strings.Repeat(" ", leftPad) + statusStyle.Render(statusText) + strings.Repeat(" ", rightPad)
        } else {
            // 状态消息太长，截断
            spacer = statusStyle.Render(truncateToWidth(statusText, availableWidth))
        }
    } else {
        spacer = lipgloss.NewStyle().Width(availableWidth).Render("")
    }
    
    // 5. Final Join
    return lipgloss.JoinHorizontal(lipgloss.Top, leftBlock, spacer, rightBlock)
}


// =============================================================================
// 主函数
// =============================================================================

func main() {
	// 创建初始模型
	initModel := initialModel()
	
	// 创建 Bubble Tea 程序
	p := tea.NewProgram(
		initModel,
		tea.WithAltScreen(), // 使用备用屏幕（退出时恢复原终端内容）
	)
	
	// 设置全局 Program，让 LSP 协程能发消息回来
	globalProgram = p

	// 运行程序
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "错误: %v\n", err)
		os.Exit(1)
	}
	
	// 清理 LSP 客户端
	if initModel.lsp != nil {
		initModel.lsp.Stop()
	}
}
