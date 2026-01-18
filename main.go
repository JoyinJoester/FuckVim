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
	"sort"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	extism "github.com/extism/go-sdk"
	"golang.org/x/term"
	"github.com/atotto/clipboard" // 系统剪贴板支持

	// Chroma 语法高亮库
	"github.com/alecthomas/chroma/v2"
	"github.com/alecthomas/chroma/v2/formatters"
	"github.com/alecthomas/chroma/v2/lexers"
	"github.com/alecthomas/chroma/v2/styles"
)

// =============================================================================
// 常量定义
// =============================================================================

const (
	// WASM插件路径 - 相对于执行目录
	pluginPath = "plugin.wasm"

	// 预测去抖动时间 - 用户停止输入多久后触发AI预测
	predictionDebounce = 500 * time.Millisecond
)

// Mode 表示编辑器模式
type Mode int

const (
	NormalMode   Mode = iota // 普通模式 - 浏览和导航
	InsertMode               // 插入模式 - 输入文本
	CommandMode              // 命令模式 - 输入 Ex 命令 (:q, :w, etc.)
	FileTreeMode             // 文件树模式 - 浏览文件系统
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
	default:
		return "UNKNOWN"
	}
}

// FileTree 文件树组件
type FileTree struct {
	entries   []FileEntry // 文件列表
	cursor    int         // 当前选中的索引
	rootPath  string      // 根目录路径
}

// FileEntry 文件条目
type FileEntry struct {
	name  string
	path  string
	isDir bool
}

// Focus 表示当前焦点位置
type Focus int

const (
	FocusEditor   Focus = iota // 编辑器获得焦点
	FocusFileTree              // 文件树获得焦点
	FocusGit                   // Git 面板获得焦点
	FocusCommand               // 命令行获得焦点
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

// Model 是 Bubble Tea 的核心状态结构
type Model struct {
	// 文本缓冲区 - 每行一个字符串
	lines []string

	// 光标位置
	cursorY int // 当前行 (0-indexed)
	cursorX int // 当前列 (0-indexed)

	// 编辑器模式
	mode Mode

	// 命令缓冲区 (用于 :command 模式)
	commandBuffer string

	// 状态/消息显示
	statusMsg string

	// AI Ghost Text 建议
	suggestion       string // 当前显示的建议文本
	suggestionPending bool   // 是否正在等待预测（去抖动中）
	lastInputTime    time.Time // 最后一次输入的时间

	// 当前文件名 (用于语法高亮检测)
	filename string

	// WASM 插件实例
	plugin *extism.Plugin

	// 插件加载错误（如果有）
	pluginError error

	// 终端尺寸
	width  int
	height int

	// 文件树侧边栏
	showSidebar bool
	fileTree    FileTree

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
}

// =============================================================================
// 初始化
// =============================================================================

// initialModel 创建初始模型状态
func initialModel() Model {
	// 默认文件名（用于语法高亮检测）
	filename := ""
	if len(os.Args) > 1 {
		filename = os.Args[1]
	}

	m := Model{
		// 初始化空缓冲区，至少有一行
		lines:     []string{""},
		cursorY:   0,
		cursorX:   0,
		mode:      NormalMode,
		filename:  filename,
		statusMsg: "欢迎使用 FuckVim! 按 'i' 插入, :w 保存, :q 退出",
		width:     80,
		height:    24,
	}

	// 如果指定了文件名，尝试加载文件内容
	if filename != "" {
		m.loadFile()
	}

	// 初始化文件树 (加载当前目录)
	cwd, _ := os.Getwd()
	m.fileTree.rootPath = cwd
	m.loadFileTree(cwd)

	// 初始化 Git 状态
	m.syncGitStatus()

	// 加载 WASM 插件
	m.loadPlugin()

	return m
}

// loadFile 从磁盘加载文件内容
func (m *Model) loadFile() {
	if m.filename == "" {
		return
	}

	content, err := os.ReadFile(m.filename)
	if err != nil {
		if os.IsNotExist(err) {
			// 文件不存在，创建新文件
			m.statusMsg = fmt.Sprintf("[新文件] %s", m.filename)
			m.lines = []string{""}
		} else {
			m.statusMsg = fmt.Sprintf("⚠ 读取文件失败: %v", err)
		}
		return
	}

	// 将文件内容分割成行
	text := string(content)
	// 处理不同的换行符
	text = strings.ReplaceAll(text, "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	
	m.lines = strings.Split(text, "\n")
	
	// 确保至少有一行
	if len(m.lines) == 0 {
		m.lines = []string{""}
	}

	// ----------------------------------------------------
	// 初始化语法高亮缓存
	// ----------------------------------------------------
	// 1. Lexer
	m.cachedLexer = lexers.Match(m.filename)
	if m.cachedLexer == nil {
		m.cachedLexer = lexers.Analyse(text)
	}
	if m.cachedLexer == nil {
		m.cachedLexer = lexers.Fallback
	}
	m.cachedLexer = chroma.Coalesce(m.cachedLexer)

	// 2. Style
	m.cachedStyle = styles.Get("dracula")
	if m.cachedStyle == nil {
		m.cachedStyle = styles.Fallback
	}

	// 3. Formatter
	m.cachedFormatter = formatters.TTY256

	m.statusMsg = fmt.Sprintf("\"%s\" %d 行已读取", m.filename, len(m.lines))
}

// saveFile 保存文件到磁盘
func (m *Model) saveFile() error {
	if m.filename == "" {
		return fmt.Errorf("未指定文件名")
	}

	content := strings.Join(m.lines, "\n")
	err := os.WriteFile(m.filename, []byte(content), 0644)
	if err != nil {
		return err
	}

	return nil
}

// loadFileTree 加载指定路径的文件列表
func (m *Model) loadFileTree(path string) {
	m.fileTree.entries = []FileEntry{}
	m.fileTree.cursor = 0
	
	// 更新当前浏览路径
	m.fileTree.rootPath = path

	// 读取目录内容
	entries, err := os.ReadDir(path)
	if err != nil {
		m.statusMsg = fmt.Sprintf("⚠ 读取目录失败: %v", err)
		return
	}

	// 分离目录和文件
	var dirs, files []FileEntry
	for _, entry := range entries {
		// 跳过隐藏文件（以.开头）
		if strings.HasPrefix(entry.Name(), ".") {
			continue
		}

		fe := FileEntry{
			name:  entry.Name(),
			path:  filepath.Join(path, entry.Name()),
			isDir: entry.IsDir(),
		}

		if entry.IsDir() {
			dirs = append(dirs, fe)
		} else {
			files = append(files, fe)
		}
	}

	// 排序：目录在前，文件在后，各自按名称排序
	sort.Slice(dirs, func(i, j int) bool { return dirs[i].name < dirs[j].name })
	sort.Slice(files, func(i, j int) bool { return files[i].name < files[j].name })

	m.fileTree.entries = append(dirs, files...)
}

// ... syncGitStatus ...
// syncGitStatus 同步 Git 仓库状态
func (m *Model) syncGitStatus() {
	// 获取当前工作目录
	cwd, err := os.Getwd()
	if err != nil {
		return
	}
	m.git.RepoPath = cwd

	// 执行 git status --porcelain
	cmd := exec.Command("git", "status", "--porcelain")
	cmd.Dir = cwd
	output, err := cmd.Output()
	if err != nil {
		// 假如错误码是 128 (不是 git 仓库) 或其他错误
		m.git.IsRepo = false
		m.git.Files = []GitFile{}
		return
	}

	m.git.IsRepo = true
	// 解析输出
	var gitFiles []GitFile
	lines := strings.Split(string(output), "\n")
	for _, line := range lines {
		if len(line) < 4 {
			continue
		}

		// XY PATH
		// X = index stats, Y = worktree status
		x := line[0]
		y := line[1]
		file := strings.TrimSpace(line[3:])

		// 简单的状态映射 logic
		var status GitStatus
		staged := false

		if x != ' ' && x != '?' {
			staged = true
			if x == 'M' { status = StatusModified }
			if x == 'A' { status = StatusAdded }
			if x == 'D' { status = StatusDeleted }
		} else {
			if y == 'M' { status = StatusModified }
			if y == 'D' { status = StatusDeleted }
			if y == '?' { status = StatusUntracked }
		}

		// 如果既有暂存又有修改（部分暂存），在这个简单模型中优先显示暂存
		// 实际上我们可能需要更复杂的状态，但 MVP 够用了

		gitFiles = append(gitFiles, GitFile{
			Path:   file,
			Status: status,
			Staged: staged,
		})
	}

	m.git.Files = gitFiles
}

// stageGitFile 暂存文件
func (m *Model) stageGitFile(file string) {
	cmd := exec.Command("git", "add", file)
	cmd.Dir = m.git.RepoPath
	cmd.Run()
	m.syncGitStatus()
}

// unstageGitFile 取消暂存文件
func (m *Model) unstageGitFile(file string) {
	cmd := exec.Command("git", "reset", file)
	cmd.Dir = m.git.RepoPath
	cmd.Run()
	m.syncGitStatus()
}

// commitGit 提交更改
func (m *Model) commitGit(msg string) error {
	cmd := exec.Command("git", "commit", "-m", msg)
	cmd.Dir = m.git.RepoPath
	output, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("提交失败: %v\n%s", err, string(output))
	}
	m.syncGitStatus()
	return nil
}

// loadPlugin 加载 Extism WASM 插件
//
// 为什么在这里加载？
// 我们在初始化时加载一次，而不是每次调用时加载，以避免性能开销。
// 插件实例在整个编辑器生命周期中复用。
func (m *Model) loadPlugin() {
	// 检查插件文件是否存在
	if _, err := os.Stat(pluginPath); os.IsNotExist(err) {
		m.pluginError = fmt.Errorf("插件文件不存在: %s (请先运行 'make build-plugin')", pluginPath)
		return
	}

	// 创建 Extism 插件清单
	// Wasm 字段指定要加载的 WASM 模块
	manifest := extism.Manifest{
		Wasm: []extism.Wasm{
			extism.WasmFile{Path: pluginPath},
		},
	}

	// 创建插件实例
	// context.Background() 用于插件的生命周期管理
	// extism.PluginConfig{} 使用默认配置
	ctx := context.Background()
	plugin, err := extism.NewPlugin(ctx, manifest, extism.PluginConfig{}, nil)
	if err != nil {
		m.pluginError = fmt.Errorf("加载插件失败: %w", err)
		return
	}

	m.plugin = plugin
}

// =============================================================================
// Bubble Tea 接口实现
// =============================================================================

// Init 返回初始命令
func (m Model) Init() tea.Cmd {
	// 1. 尝试获取真实终端尺寸
	w, h, err := term.GetSize(int(os.Stdout.Fd()))
	var resizeCmd tea.Cmd
	if err == nil {
		resizeCmd = func() tea.Msg {
			return tea.WindowSizeMsg{Width: w, Height: h}
		}
	}

	// 2. 组合命令: 清屏 + 进入AltScreen + 强制发送尺寸
	// 这样可以确保启动时布局正确，不会默认成 80x24
	cmds := []tea.Cmd{tea.ClearScreen, tea.EnterAltScreen}
	if resizeCmd != nil {
		cmds = append(cmds, resizeCmd)
	}
	return tea.Batch(cmds...)
}

// Update 处理消息并更新模型
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
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
			if len(errStr) > 50 { errStr = errStr[:47] + "..." }
			m.statusMsg = "❌ Push 失败: " + errStr
		} else {
			m.statusMsg = "✅ Push Complete"
		}
		m.pushChan = nil // 清理通道
		m.syncGitStatus()
		return m, nil
	}

	return m, nil
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
        if isCtrlH {
             // Editor -> Left -> Sidebar
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
             // 侧边栏都关闭，不做操作
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
	if m.focus == FocusEditor || m.focus == FocusCommand { // Command 模式也通常在主区域显示，或者覆盖之
        switch m.mode {
        case NormalMode:
            return m.handleNormalMode(msg)
        case InsertMode:
            return m.handleInsertMode(msg)
        case CommandMode:
            return m.handleCommandMode(msg)
        }
    }

	return m, nil
}

// handleNormalMode 处理普通模式下的按键
func (m Model) handleNormalMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "i":
		// 进入插入模式
		m.mode = InsertMode
		m.statusMsg = "-- 插入模式 --"

	case ":":
		// 进入命令模式 (Vim 风格)
		m.mode = CommandMode
		m.commandBuffer = ""
		m.statusMsg = ":"

	case "j", "down":
		// 向下移动光标
		if m.cursorY < len(m.lines)-1 {
			m.cursorY++
			// 确保光标不超出当前行长度
			if m.cursorX > len(m.lines[m.cursorY]) {
				m.cursorX = len(m.lines[m.cursorY])
			}
		}

	case "k", "up":
		// 向上移动光标
		if m.cursorY > 0 {
			m.cursorY--
			if m.cursorX > len(m.lines[m.cursorY]) {
				m.cursorX = len(m.lines[m.cursorY])
			}
		}

	case "h", "left":
		// 向左移动光标
		if m.cursorX > 0 {
			m.cursorX--
		}

	case "l", "right":
		// 向右移动光标
		if m.cursorX < len(m.lines[m.cursorY]) {
			m.cursorX++
		}

	case "0":
		// 移动到行首
		m.cursorX = 0

	case "$":
		// 移动到行尾
		m.cursorX = len(m.lines[m.cursorY])

	case "tab":
		// 触发 WASM 插件处理 - 核心功能！
		m.callPlugin()
	
	case "p":
		// 粘贴 (从系统剪贴板)
		text, err := clipboard.ReadAll()
		if err != nil || text == "" {
			m.statusMsg = "ℹ 剪贴板为空"
		} else {
			m.pasteText(text)
			m.statusMsg = "✓ 已粘贴"
		}
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
		}
	case "c":
		// 手动提交: 先用空格键 stage 单个文件，然后 c 提交
		m.mode = CommandMode
		m.commandBuffer = "commit "
		m.statusMsg = "请输入提交信息: :commit <msg>"
		m.focus = FocusCommand
	
	case "C": // Shift+C: 智能提交 (Stage All + Commit)
		// 1. Auto-Stage 所有文件
		exec.Command("git", "add", "-A").Run()
		
		// 2. 刷新状态 (让用户看到所有文件变绿)
		m.syncGitStatus()
		
		// 3. 进入提交消息输入
		m.mode = CommandMode
		m.commandBuffer = "commit "
		m.statusMsg = "🚀 已暂存所有文件! 请输入提交信息:"
		m.focus = FocusCommand
	
	case "r":
		m.syncGitStatus()

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
		content, err := os.ReadFile(configPath)
		if err != nil {
			m.statusMsg = fmt.Sprintf("⚠ 读取配置失败: %v", err)
			return m, nil
		}
		
		// 加载到编辑器
		text := string(content)
		text = strings.ReplaceAll(text, "\r\n", "\n")
		text = strings.ReplaceAll(text, "\r", "\n")
		m.lines = strings.Split(text, "\n")
		if len(m.lines) == 0 {
			m.lines = []string{""}
		}
		m.filename = configPath
		m.cursorX = 0
		m.cursorY = 0
		
		// 重新初始化语法高亮 (INI 格式)
		m.cachedLexer = lexers.Match(m.filename)
		if m.cachedLexer == nil {
			m.cachedLexer = lexers.Fallback
		}
		m.cachedLexer = chroma.Coalesce(m.cachedLexer)
		
		// 切换焦点到编辑器
		m.focus = FocusEditor
		m.mode = NormalMode
		m.statusMsg = "📝 编辑 Git 配置 (按 :w 保存)"

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
		m.statusMsg = "Diff 功能暂未实现"
	}
	return m, nil
}

// handleCommandMode 处理命令模式下的按键（类似 Vim 的 Ex 命令）
func (m Model) handleCommandMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.Type {
	case tea.KeyEsc:
		// 取消命令，回到普通模式
		m.mode = NormalMode
		m.commandBuffer = ""
		m.statusMsg = ""

	case tea.KeyEnter:
		// 执行命令
		cmd := m.executeCommand()
		
		// 重置命令模式状态
		m.mode = NormalMode
		m.commandBuffer = ""
		
		// 恢复焦点到合适的窗口 (只在仍是 FocusCommand 时)
		// 如果 executeCommand 已经设置了焦点，不要覆盖它
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

	case tea.KeyBackspace:
		// 删除命令缓冲区中的字符
		if len(m.commandBuffer) > 0 {
			m.commandBuffer = m.commandBuffer[:len(m.commandBuffer)-1]
			m.statusMsg = ":" + m.commandBuffer
		} else {
			// 缓冲区已空，回到普通模式
			m.mode = NormalMode
			m.statusMsg = ""
		}

	default:
		// 添加字符到命令缓冲区
		if len(msg.String()) == 1 {
			m.commandBuffer += msg.String()
			m.statusMsg = ":" + m.commandBuffer
		}
	}

	return m, nil
}

// executeCommand 执行 Ex 命令
func (m *Model) executeCommand() tea.Cmd {
	cmd := strings.TrimSpace(m.commandBuffer)
	m.commandBuffer = ""
	m.mode = NormalMode

	switch cmd {
	case "q", "quit":
		// 退出编辑器
		return tea.Quit

	case "w", "write":
		// 保存文件
		if m.filename == "" {
			m.statusMsg = "⚠ 未指定文件名，使用 :w 文件名"
		} else {
			err := m.saveFile()
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 保存失败: %v", err)
			} else {
				m.statusMsg = fmt.Sprintf("\"%s\" %d 行已写入", m.filename, len(m.lines))
				// 保存后自动刷新 Git 状态
				if m.showGit {
					m.syncGitStatus()
				}
			}
		}

	case "wq", "x":
		// 保存并退出
		if m.filename != "" {
			err := m.saveFile()
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 保存失败: %v", err)
				return nil
			}
		}
		return tea.Quit

	case "q!":
		// 强制退出（不保存）
		return tea.Quit

	case "tree", "e":
		// 切换文件树侧边栏
		m.showSidebar = !m.showSidebar
		m.syncSizes() // 立即同步布局尺寸
		if m.showSidebar {
			// 如果 rootPath 为空，使用当前目录
			if m.fileTree.rootPath == "" {
				m.fileTree.rootPath, _ = os.Getwd()
			}
			m.loadFileTree(m.fileTree.rootPath)
			m.focus = FocusFileTree
			m.statusMsg = "焦点: 文件树 | j/k=移动, Enter=打开/进入, Backspace=返回上一级"
		} else {
			m.focus = FocusEditor
			m.statusMsg = ""
		}
		return m.forceRefresh() // 模拟 Resize 事件以强制修正布局
		
	case "git":
		// 切换 Git 面板
		m.showGit = !m.showGit
		m.syncSizes() // 立即同步布局尺寸
		if m.showGit {
			m.syncGitStatus()
			m.focus = FocusGit
			m.statusMsg = "焦点: Git | Ctrl+H=文件树 Ctrl+L=编辑器"
		} else {
			m.focus = FocusEditor
			m.statusMsg = ""
		}
		return m.forceRefresh() // 模拟 Resize 事件以强制修正布局

	case "ai":
		// AI 聊天占位
		m.statusMsg = "⚛ AI 聊天功能即将推出..."

	case "help":
		m.statusMsg = "命令: :q=退出 :w=保存 :tree=文件树 :ai=AI聊天"

	case "":
		// 空命令，什么都不做
		m.statusMsg = ""

	default:
		// 检查是否是 commit 命令 (格式: "commit <message>")
		if strings.HasPrefix(cmd, "commit ") {
			message := strings.TrimPrefix(cmd, "commit ")
			message = strings.TrimSpace(message)
			if message == "" {
				m.statusMsg = "⚠ 提交信息不能为空"
			} else {
				// 执行 git commit
				output, err := exec.Command("git", "commit", "-m", message).CombinedOutput()
				if err != nil {
					m.statusMsg = fmt.Sprintf("⚠ 提交失败: %s", strings.TrimSpace(string(output)))
				} else {
					m.statusMsg = fmt.Sprintf("✓ 已提交: %s", message)
					// 刷新 Git 状态
					m.syncGitStatus()
					// 如果 Git 面板打开，返回焦点
					if m.showGit {
						m.focus = FocusGit
					}
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
	// 如果正在选择 Git 初始化目录
	if m.selectingGitRoot {
		switch msg.String() {
		case "y":
			// 确认在此目录 (rootPath) 初始化
			targetDir := m.fileTree.rootPath
			
			// run git init
			cmd := exec.Command("git", "init", targetDir)
			cmd.Dir = targetDir
			if err := cmd.Run(); err != nil {
				m.statusMsg = fmt.Sprintf("⚠ Git Init 失败: %v", err)
			} else {
				m.statusMsg = fmt.Sprintf("✓ Git 仓库已初始化: %s", targetDir)
				// 刷新并重置
				m.syncGitStatus()
				m.selectingGitRoot = false
				m.focus = FocusGit
			}
			return m, nil
		
		case "esc":
			// 取消
			m.selectingGitRoot = false
			m.focus = FocusGit
			m.statusMsg = "已取消 Git 初始化"
			return m, nil
		}
		// 允许继续导航 (j/k/enter/backspace) 以便选择文件夹
		// Fallthrough to normal navigation
	}

	switch msg.String() {
	case "j", "down":
		// 向下移动
		if m.fileTree.cursor < len(m.fileTree.entries)-1 {
			m.fileTree.cursor++
		}

	case "k", "up":
		// 向上移动
		if m.fileTree.cursor > 0 {
			m.fileTree.cursor--
		}

	case "enter":
		// 打开选中的文件或目录
		if len(m.fileTree.entries) > 0 {
			entry := m.fileTree.entries[m.fileTree.cursor]
			if entry.isDir {
				// 进入目录
				m.loadFileTree(entry.path)
			} else {
				// 文件：加载到编辑器
				m.filename = entry.path
				m.loadFile()
				// 切换焦点到编辑器，但保持侧边栏可见！
				m.focus = FocusEditor
				m.mode = NormalMode
			}
		}

	case "backspace", "-":
		// 返回上一级目录
		parentDir := filepath.Dir(m.fileTree.rootPath)
		m.loadFileTree(parentDir)

	case "esc", "q":
		// 切换焦点到编辑器（不关闭侧边栏）
		m.focus = FocusEditor
		m.mode = NormalMode
		m.statusMsg = "Ctrl+H 返回文件树"

	case ":":
		// 进入命令模式
		m.mode = CommandMode
		m.commandBuffer = ""
		m.statusMsg = ":"
	}

	// 如果仍然在选择模式，确保提示信息是最新的（覆盖上面的状态）
	if m.selectingGitRoot {
		m.statusMsg = fmt.Sprintf("Navigate to project root, then press 'y' to initialize in: %s", m.fileTree.rootPath)
	}

	return m, nil
}

// handleInsertMode 处理插入模式下的按键
func (m Model) handleInsertMode(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.Type {
	case tea.KeyEsc:
		// 退出插入模式
		m.mode = NormalMode
		m.statusMsg = "回到普通模式"
		// 光标左移一位（类似 Vim 行为）
		if m.cursorX > 0 {
			m.cursorX--
		}

	case tea.KeyEnter:
		// 插入新行
		m.insertNewLine()

	case tea.KeyBackspace:
		// 删除字符
		m.deleteChar()

	case tea.KeySpace:
		// 插入空格
		m.insertChar(' ')
	
	case tea.KeyCtrlV:
		// 粘贴 (从系统剪贴板)
		text, err := clipboard.ReadAll()
		if err != nil || text == "" {
			m.statusMsg = "ℹ 剪贴板为空"
		} else {
			m.pasteText(text)
			m.statusMsg = "✓ 已粘贴"
		}

	case tea.KeyUp:
		// 向上移动光标
		if m.cursorY > 0 {
			m.cursorY--
			if m.cursorX > len(m.lines[m.cursorY]) {
				m.cursorX = len(m.lines[m.cursorY])
			}
		}

	case tea.KeyDown:
		// 向下移动光标
		if m.cursorY < len(m.lines)-1 {
			m.cursorY++
			if m.cursorX > len(m.lines[m.cursorY]) {
				m.cursorX = len(m.lines[m.cursorY])
			}
		}

	case tea.KeyLeft:
		// 向左移动光标
		if m.cursorX > 0 {
			m.cursorX--
		} else if m.cursorY > 0 {
			// 移动到上一行末尾
			m.cursorY--
			m.cursorX = len(m.lines[m.cursorY])
		}

	case tea.KeyRight:
		// 向右移动光标
		if m.cursorX < len(m.lines[m.cursorY]) {
			m.cursorX++
		} else if m.cursorY < len(m.lines)-1 {
			// 移动到下一行开头
			m.cursorY++
			m.cursorX = 0
		}

	case tea.KeyTab:
		// 如果有 AI 建议，按 Tab 接受建议
		if m.suggestion != "" {
			// 将建议的字符串逐个字符插入
			// TODO: 更高效的插入方式
			for _, ch := range m.suggestion {
				m.insertChar(ch)
			}
			m.suggestion = ""
			m.statusMsg = "✓ 已接受 AI 建议"
			return m, nil
		}

		// 否则插入制表符（4个空格）
		for i := 0; i < 4; i++ {
			m.insertChar(' ')
		}

	default:
		// 插入普通字符
		if len(msg.String()) == 1 {
			m.insertChar(rune(msg.String()[0]))
		}
	}

	return m, nil
}

// =============================================================================
// 文本编辑操作
// =============================================================================

// insertChar 在当前光标位置插入字符
func (m *Model) insertChar(ch rune) {
	line := m.lines[m.cursorY]
	// 在光标位置插入字符
	newLine := line[:m.cursorX] + string(ch) + line[m.cursorX:]
	m.lines[m.cursorY] = newLine
	m.cursorX++
}

// pasteText 在当前光标位置粘贴文本 (支持多行)
func (m *Model) pasteText(text string) {
	// 处理换行符
	text = strings.ReplaceAll(text, "\r\n", "\n")
	text = strings.ReplaceAll(text, "\r", "\n")
	
	pasteLines := strings.Split(text, "\n")
	if len(pasteLines) == 0 {
		return
	}
	
	if len(pasteLines) == 1 {
		// 单行粘贴: 直接插入当前行
		line := m.lines[m.cursorY]
		newLine := line[:m.cursorX] + pasteLines[0] + line[m.cursorX:]
		m.lines[m.cursorY] = newLine
		m.cursorX += len(pasteLines[0])
	} else {
		// 多行粘贴
		currentLine := m.lines[m.cursorY]
		left := currentLine[:m.cursorX]
		right := currentLine[m.cursorX:]
		
		// 更新当前行
		m.lines[m.cursorY] = left + pasteLines[0]
		
		// 插入中间行
		newLines := make([]string, 0, len(m.lines)+len(pasteLines)-1)
		newLines = append(newLines, m.lines[:m.cursorY+1]...)
		newLines = append(newLines, pasteLines[1:len(pasteLines)-1]...)
		
		// 最后一行 + 原来光标后的内容
		lastPasteLine := pasteLines[len(pasteLines)-1] + right
		newLines = append(newLines, lastPasteLine)
		newLines = append(newLines, m.lines[m.cursorY+1:]...)
		
		m.lines = newLines
		m.cursorY += len(pasteLines) - 1
		m.cursorX = len(pasteLines[len(pasteLines)-1])
	}
}

// insertNewLine 在当前位置插入新行
func (m *Model) insertNewLine() {
	line := m.lines[m.cursorY]
	// 分割当前行
	left := line[:m.cursorX]
	right := line[m.cursorX:]

	// 更新当前行并插入新行
	m.lines[m.cursorY] = left
	newLines := make([]string, len(m.lines)+1)
	copy(newLines[:m.cursorY+1], m.lines[:m.cursorY+1])
	newLines[m.cursorY+1] = right
	copy(newLines[m.cursorY+2:], m.lines[m.cursorY+1:])
	m.lines = newLines

	// 移动光标到新行开头
	m.cursorY++
	m.cursorX = 0
}

// deleteChar 删除光标前的字符
func (m *Model) deleteChar() {
	if m.cursorX > 0 {
		// 删除当前行中的字符
		line := m.lines[m.cursorY]
		m.lines[m.cursorY] = line[:m.cursorX-1] + line[m.cursorX:]
		m.cursorX--
	} else if m.cursorY > 0 {
		// 合并到上一行
		prevLine := m.lines[m.cursorY-1]
		currLine := m.lines[m.cursorY]
		m.cursorX = len(prevLine)
		m.lines[m.cursorY-1] = prevLine + currLine

		// 删除当前行
		m.lines = append(m.lines[:m.cursorY], m.lines[m.cursorY+1:]...)
		m.cursorY--
	}
}

// =============================================================================
// WASM 插件调用
// =============================================================================

// callPlugin 调用 WASM 插件处理当前缓冲区
//
// 这是 Go Host <-> Rust WASM 通信的核心！
//
// 流程:
// 1. 序列化缓冲区为单个字符串
// 2. 调用 WASM 的 process_command 函数
// 3. 反序列化返回结果并更新缓冲区
func (m *Model) callPlugin() {
	// 检查插件是否可用
	if m.pluginError != nil {
		m.statusMsg = fmt.Sprintf("⚠ 插件错误: %v", m.pluginError)
		return
	}

	if m.plugin == nil {
		m.statusMsg = "⚠ 插件未加载"
		return
	}

	// 1. 序列化: 将 lines 切片转换为单个换行分隔的字符串
	//    这是因为 WASM 函数只能接收和返回简单类型（字符串/字节）
	bufferContent := strings.Join(m.lines, "\n")

	// 2. 调用 WASM 函数
	//    "process_command" 是 Rust 中用 #[plugin_fn] 导出的函数名
	//    我们传入整个缓冲区，让 Rust 处理
	exitCode, output, err := m.plugin.Call("process_command", []byte(bufferContent))
	if err != nil {
		m.statusMsg = fmt.Sprintf("⚠ 插件调用失败: %v", err)
		return
	}

	if exitCode != 0 {
		m.statusMsg = fmt.Sprintf("⚠ 插件返回错误码: %d", exitCode)
		return
	}

	// 3. 反序列化: 将返回的字符串分割回 lines 切片
	resultStr := string(output)
	m.lines = strings.Split(resultStr, "\n")

	// 确保至少有一行
	if len(m.lines) == 0 {
		m.lines = []string{""}
	}

	// 调整光标位置以防越界
	if m.cursorY >= len(m.lines) {
		m.cursorY = len(m.lines) - 1
	}
	if m.cursorX > len(m.lines[m.cursorY]) {
		m.cursorX = len(m.lines[m.cursorY])
	}

	m.statusMsg = "✓ AI处理完成！(用 ;; 前缀的行已被转换)"
}

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

// runGitPushStream 在后台运行 git push 并流式传输输出
func runGitPushStream(sub chan string) tea.Cmd {
	return func() tea.Msg {
		// 智能推送策略: 总是尝试设置上游分支
		// git push -u origin HEAD 将当前分支推送到 origin 上的同名分支并建立关联
		// 这解决了 "fatal: The current branch master has no upstream branch" 问题
		cmd := exec.Command("git", "push", "-u", "origin", "HEAD")
		
		stdout, _ := cmd.StdoutPipe()
		stderr, _ := cmd.StderrPipe()
		
		if err := cmd.Start(); err != nil {
			sub <- "Error starting cmd: " + err.Error()
			close(sub)
			return pushDoneMsg{err: err}
		}

		// 在 Goroutine 中读取输出
		go func() {
			defer close(sub)
			
			// 组合 stdout 和 stderr
			scanner := bufio.NewScanner(io.MultiReader(stdout, stderr))
			for scanner.Scan() {
				text := scanner.Text()
				// 发送每行输出到通道
				sub <- text
			}
			
			// 等待命令完成
			cmd.Wait()
		}()
		
		// 注意: 这个 Cmd 本身只负责启动 Goroutine，
		// 真正的完成信号由 Update 中的通道关闭或额外逻辑处理?
		// Bubble Tea 的模型里，Cmd 通常返回 Msg。
		// 这里我们用 Goroutine 发消息到 channel，Update 监听 channel。
		// 但怎么知道结束了呢？
		// 当 channel 关闭时，waitForPushOutput 返回 nil。
		// 但我们需要发送最后的 pushDoneMsg。
		
		// 改进策略: 让 runGitPushStream 阻塞等待 cmd 完成并返回 pushDoneMsg？
		// 不行，那样会阻塞 UI (如果没放进 goroutine)。
		// 其实 Cmd 函数本身是在后台运行的吗？不，Cmd 函数是同步调用的，返回 Msg。
		// Bubble Tea 运行时会在 goroutine 中执行 Cmd。
		
		// 正确做法:
		// runGitPushStream 应该是一个 Cmd，它执行整个 push 过程 (阻塞)，
		// 在过程中往 channel 发送数据。
		// 最后返回 pushDoneMsg。
		
		// 重新实现 runGitPushStream (阻塞式):
		
		combinedOutput := io.MultiReader(stdout, stderr)
		scanner := bufio.NewScanner(combinedOutput)
		for scanner.Scan() {
			sub <- scanner.Text()
		}
		
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
	currentLine := m.lines[m.cursorY]
	
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
func (m Model) highlight(content string) string {
	// 如果没有缓存 (e.g. 新文件未加载完毕), 使用 fallback
	if m.cachedLexer == nil {
		return content
	}

	// 执行词法分析 (使用缓存的 lexer)
	iterator, err := m.cachedLexer.Tokenise(nil, content)
	if err != nil {
		return content
	}

	// 格式化输出 (使用缓存的 formatter & style)
	var buf bytes.Buffer
	if err := m.cachedFormatter.Format(&buf, m.cachedStyle, iterator); err != nil {
		return content
	}

	return buf.String()
}


// highlightLine 高亮单行内容
func (m Model) highlightLine(line string) string {
	// 对单行进行高亮处理
	highlighted := m.highlight(line)
	// 移除末尾的换行符（如果有）
	return strings.TrimSuffix(highlighted, "\n")
}

// =============================================================================
// 视图渲染
// =============================================================================

// syncSizes 立即同步布局尺寸 (解决 State Lag 问题)
func (m *Model) syncSizes() {
	// 侧边栏逻辑
	sidebarWidth := 0
	if m.showSidebar || m.showGit {
		sidebarWidth = 30
	}

	// 编辑器逻辑: 剩余宽度完全分配给编辑器
	editorWidth := m.width - sidebarWidth
	if editorWidth < 10 { editorWidth = 10 }

	// 高度逻辑: 减去底部状态栏 (1行)
	contentHeight := m.height - 1
	if contentHeight < 0 { contentHeight = 0 }

	// 更新缓存值
	m.cachedSidebarWidth = sidebarWidth
	m.cachedEditorWidth = editorWidth
	m.cachedContentHeight = contentHeight
}

// calculateSizes 集中计算布局尺寸 (Atomic Layout)
func (m Model) calculateSizes() (int, int, int, int) {
	// 使用缓存值 (syncSizes 已同步)
	if m.cachedSidebarWidth > 0 || m.cachedEditorWidth > 0 {
		return m.cachedSidebarWidth, m.cachedEditorWidth, m.cachedContentHeight, m.cachedContentHeight
	}

	// Fallback: 手动计算 (初始化时)
	totalWidth := m.width
	totalHeight := m.height

	sidebarWidth := 0
	if m.showSidebar || m.showGit {
		sidebarWidth = 30
	}

	editorWidth := totalWidth - sidebarWidth
    
	contentHeight := totalHeight - 1
	if contentHeight < 0 { contentHeight = 0 }

	return sidebarWidth, editorWidth, contentHeight, contentHeight
}

// View 渲染 UI
func (m Model) View() string {
	// 如果终端尺寸太小，显示提示
	if m.width < 40 || m.height < 10 {
		return "窗口太小，请调整尺寸 (Window too small)"
	}

	// 1. 原子化计算布局尺寸
	sidebarWidth, editorWidth, sidebarHeight, editorHeight := m.calculateSizes()

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

	// 3. 渲染编辑器
	editorView := m.renderEditor(editorWidth, editorHeight)
	// 强制编辑器精确尺寸
	editorView = lipgloss.Place(editorWidth, editorHeight, lipgloss.Left, lipgloss.Top, editorView)

	// 4. 合成主布局
	var mainContent string
	if leftPanel != "" {
		mainContent = lipgloss.JoinHorizontal(lipgloss.Top, leftPanel, editorView)
	} else {
		mainContent = editorView
	}

	// 5. 渲染状态栏 (底部)
	statusBar := m.renderStatusBar()

	// 6. Final assembly
	frame := lipgloss.JoinVertical(lipgloss.Left, mainContent, statusBar)

	// 7. Full-Frame Lock: 强制最终输出为精确尺寸
	// 这保证每次渲染的字符串结构完全一致，终端可以正确地原地覆盖像素
	return lipgloss.Place(m.width, m.height, lipgloss.Left, lipgloss.Top, frame)
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
		// 使用 lipgloss.Width 正确计算带 ANSI 码的宽度
		lineWidth := lipgloss.Width(line)
		if lineWidth < innerWidth {
			line = line + strings.Repeat(" ", innerWidth-lineWidth)
		}
		// 注意：不在此处截断，因为截断带 ANSI 码的字符串可能破坏转义序列
		// 依赖上层渲染函数控制内容宽度
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

// renderEditor 渲染编辑器区域
func (m Model) renderEditor(width, height int) string {
	var lines []string

	// 实际可用内容宽高 (减去边框)
	contentWidth := width - 2
	contentHeight := height - 2 // Border top/bottom take 1 each

	// 行号区域宽度 (4 char + " │ " 3 char = 7)
	// 实际代码区域宽度
	codeWidth := contentWidth - 7
	if codeWidth < 1 { codeWidth = 1 }

	for i := 0; i < contentHeight; i++ {
		if i < len(m.lines) {
			// 渲染实际行
			lineNum := lineNumberStyle.Render(fmt.Sprintf("%d", i+1))
			lineContent := m.renderLine(i)
			
			// 移除内容中可能存在的换行符
			lineContent = strings.ReplaceAll(lineContent, "\n", "")
			
			// 强制截断/填充
			lineStyle := lipgloss.NewStyle().Width(codeWidth).MaxWidth(codeWidth)
			renderedContent := lineStyle.Render(lineContent)
			
			// 再次处理换行（lipgloss可能引入）
			if strings.Contains(renderedContent, "\n") {
				renderedContent = strings.Split(renderedContent, "\n")[0]
			}

			lines = append(lines, fmt.Sprintf("%s │ %s", lineNum, renderedContent))
		} else {
			// 空行
			lineNum := lineNumberStyle.Render("~")
			lines = append(lines, fmt.Sprintf("%s │", lineNum))
		}
	}

	title := fmt.Sprintf("Edit:%s", filepath.Base(m.filename))
	if m.filename == "" { title = "[No Name]" }

	isActive := m.focus == FocusEditor
	return renderWindow(strings.Join(lines, "\n"), title, isActive, width, height, false)
}

// renderSidebar 渲染文件树侧边栏
func (m Model) renderSidebar(width, height int) string {
	var lines []string

	// 内容高度
	contentHeight := height - 2
	visibleHeight := contentHeight
	if visibleHeight < 0 { visibleHeight = 0 }

	for i, entry := range m.fileTree.entries {
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
	title := "Git-Clean"
	if len(m.git.Files) > 0 {
		title = "Git-Dirty"
	}

	isActive := m.focus == FocusGit
	return renderWindow(strings.Join(lines, "\n"), title, isActive, width, height, false)
}



// renderLine 渲染单行文本，包含光标显示和语法高亮
func (m Model) renderLine(lineIndex int) string {
	line := m.lines[lineIndex]

	// 如果不是当前行，应用语法高亮后直接返回
	if lineIndex != m.cursorY {
		return m.highlightLine(line)
	}

	// 当前行需要显示光标
	// 注意：光标行暂时不应用语法高亮，因为 ANSI 转义码会影响光标位置计算
	// TODO: 未来可以实现更智能的光标行高亮
	if m.cursorX >= len(line) {
		// 光标在行尾
		
		// 如果有建议，显示在光标后
		suggestion := ""
		if m.suggestion != "" {
			suggestion = suggestionStyle.Render(m.suggestion)
		}
		
		return cursorLineStyle.Render(line + "█" + suggestion)
	}

	// 光标在行中间 - 高亮光标位置的字符
	before := line[:m.cursorX]
	cursor := string(line[m.cursorX])
	after := line[m.cursorX+1:]

	// 使用反色显示光标
	cursorStyle := lipgloss.NewStyle().
		Background(lipgloss.Color("230")).
		Foreground(lipgloss.Color("0"))

	// 如果有建议，显示在行尾 (简化处理，或者跟在光标后？题目说 after cursor)
	// 这里我们把它加在整行最后，因为通常是补全行尾
	// 如果需要紧跟光标，需要改逻辑插入到 after 中
	// 但鉴于我们的 mock 逻辑是基于 ends_with，只有光标在行尾时才会有建议
	// 所以这里如果光标在中间，理论上 suggestion 应该为空（除非我们改了 predict 逻辑）
	// 不过为了健壮性，我们还是加上
	suggestion := ""
	if m.suggestion != "" {
		suggestion = suggestionStyle.Render(m.suggestion)
	}

	return cursorLineStyle.Render(before + cursorStyle.Render(cursor) + after + suggestion)
}

// renderStatusBar 渲染状态栏
func (m Model) renderStatusBar() string {
	// 模式指示器
	var modeIndicator string
	
	// 优先显示焦点状态
	if m.focus == FocusGit {
		modeIndicator = gitHeaderStyle.Background(lipgloss.Color("205")).Foreground(lipgloss.Color("230")).Padding(0, 1).Render(" GIT ")
	} else if m.focus == FocusFileTree {
		modeIndicator = treeModeStyle.Render(" TREE ")
	} else {
		// 编辑器或全局模式
		switch m.mode {
		case NormalMode:
			modeIndicator = normalModeStyle.Render(" NORMAL ")
		case InsertMode:
			modeIndicator = insertModeStyle.Render(" INSERT ")
		case CommandMode:
			modeIndicator = commandModeStyle.Render(" COMMAND ")
		default:
			modeIndicator = normalModeStyle.Render(" NORMAL ")
		}
	}

	// 位置信息
	position := fmt.Sprintf(" Ln %d, Col %d ", m.cursorY+1, m.cursorX+1)

	// 插件状态
	pluginStatus := " WASM: OK "
	if m.pluginError != nil {
		pluginStatus = " WASM: ERR "
	}

	// 计算中间部分 (包含消息)
	leftPart := modeIndicator
	rightPart := statusBarStyle.Render(pluginStatus) + statusBarStyle.Render(position)
	
	// Available width for middle
	availWidth := m.width - lipgloss.Width(leftPart) - lipgloss.Width(rightPart)
	if availWidth < 0 { availWidth = 0 }

	// Msg styling
	msg := m.statusMsg
	if len(msg) > availWidth-2 {
		msg = msg[:availWidth-2] + "..."
	}
	// Fill remaining with space
	padding := availWidth - lipgloss.Width(msg) - 1 // -1 for safe margin
	if padding < 0 { padding = 0 }
	
	middleContent := fmt.Sprintf(" %s%s", msg, strings.Repeat(" ", padding))
	middlePart := statusBarStyle.Render(middleContent)

	return leftPart + middlePart + rightPart
}

// =============================================================================
// 主函数
// =============================================================================

func main() {
	// 创建 Bubble Tea 程序
	p := tea.NewProgram(
		initialModel(),
		tea.WithAltScreen(), // 使用备用屏幕（退出时恢复原终端内容）
	)

	// 运行程序
	if _, err := p.Run(); err != nil {
		fmt.Fprintf(os.Stderr, "错误: %v\n", err)
		os.Exit(1)
	}
}
