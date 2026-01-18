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

	"github.com/charmbracelet/bubbles/viewport"
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

// FileTreeModel 文件树组件
type FileTreeModel struct {
	rootPath   string
	cursor     int
	offset     int // 滚动偏移量
	Entries    []FileEntry
	IsLoading  bool // 是否正在加载
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
}

// SplitType 分屏类型
type SplitType int

const (
	NoSplit SplitType = iota
	VerticalSplit
	HorizontalSplit
)

// Model 是 Bubble Tea 的核心状态结构
type Model struct {
	// 多窗口系统
	panes      []*EditorPane
	activePane int
	splitType  SplitType

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

	m := Model{
		panes:      []*EditorPane{initialPane},
		activePane: 0,
		splitType:  NoSplit,
		
		mode:      NormalMode,
		statusMsg: "欢迎使用 FuckVim! 按 'i' 插入, :vsp 分屏, :q 退出",
		width:     80,
		height:    24,
		fileTree: FileTreeModel{
			rootPath:  cwd,
			IsLoading: true,
		},
		git: GitModel{
			IsLoading: true,
		},
	}

	return m
}

// createPaneFromFile 创建新窗格 (如果文件不存在则为空缓冲)
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
		content = string(bytes)
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
	curr := m.panes[m.activePane]
	
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

// -----------------------------------------------------------------------------
// 异步加载命令 (Async Loader Commands)
// -----------------------------------------------------------------------------

// 消息定义
type fileLoadedMsg struct {
	content []string
	err     error
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
		lines := strings.Split(text, "\n")
		if len(lines) == 0 {
			lines = []string{""}
		}
		return fileLoadedMsg{content: lines}
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
	err := os.WriteFile(p.Filename, []byte(content), 0644)
	if err != nil {
		return err
	}
	return nil
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
	}
	
	if len(m.panes) > 0 && m.panes[0].Filename != "" {
		cmds = append(cmds, loadFileCmd(m.panes[0].Filename))
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
	switch msg := msg.(type) {
	
	// --- 异步加载完成的消息 ---
	case fileLoadedMsg:
		if msg.err != nil {
			m.statusMsg = fmt.Sprintf("无法读取文件: %v", msg.err)
		} else {
			currPane := m.panes[m.activePane]
			currPane.Lines = msg.content
			
			// Update Viewport content as well (joined string)
			// Wait, simple join?
			content := strings.Join(msg.content, "\n")
			currPane.Viewport.SetContent(content)
			
			// 初始化高亮
			m.cachedLexer = lexers.Match(currPane.Filename)
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

	case stageAllDoneMsg:
		if msg.err != nil {
			m.statusMsg = fmt.Sprintf("❌ Staging 失败: %v", msg.err)
			return m, nil
		}
		// Staging 成功，进入提交模式
		m.mode = CommandMode
		m.commandBuffer = "commit "
		m.statusMsg = "🚀 已暂存(0s)! 请输入提交信息:"
		m.focus = FocusCommand
		// 同时后台刷新 Git 状态 (让文件变绿)
		return m, checkGitStatusCmd()
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
             // Left Navigation
             // 1. If Vertical Split and in Right Pane (1) -> Go to Left Pane (0)
             if m.splitType == VerticalSplit && m.activePane == 1 {
                 m.activePane = 0
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
            if m.splitType == VerticalSplit && len(m.panes) > 1 && m.activePane == 0 {
                m.activePane = 1
                return m, nil
            }
        }
        if isCtrlJ {
            // Down Navigation
             // If Horizontal Split and in Top Pane (0) -> Go to Bottom Pane (1)
             if m.splitType == HorizontalSplit && len(m.panes) > 1 && m.activePane == 0 {
                 m.activePane = 1
                 return m, nil
             }
        }
        if isCtrlK {
            // Up Navigation
             // If Horizontal Split and in Bot Pane (1) -> Go to Top Pane (0)
             if m.splitType == HorizontalSplit && m.activePane == 1 {
                 m.activePane = 0
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
	currPane := m.panes[m.activePane]

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
		// 手动提交: 先用空格键 stage 单个文件，然后 c 提交
		m.mode = CommandMode
		m.commandBuffer = "commit "
		m.statusMsg = "请输入提交信息: :commit <msg>"
		m.focus = FocusCommand
	
	case "C": // Shift+C: 智能提交 (Stage All + Commit)
		// 1. Auto-Stage 所有文件 (异步)
		m.statusMsg = "🚀 Staging changes..."
		return m, stageAllCmd()
	
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

			currPane := m.panes[m.activePane]
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
			currPane := m.panes[m.activePane]
			currPane.Lines = strings.Split(text, "\n")
			currPane.Filename = file.Path + ".diff"
			currPane.Viewport.SetContent(text)
		}
		
		// 重置光标
		m.panes[m.activePane].CursorX = 0
		m.panes[m.activePane].CursorY = 0
		m.panes[m.activePane].Viewport.SetYOffset(0)
		
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

	// ---------------------------------------------------------
	// 分屏命令 (:vsp, :sp)
	// ---------------------------------------------------------
	if strings.HasPrefix(cmd, "vsp") || strings.HasPrefix(cmd, "sp") {
		// 限制: 目前只支持 2 个分屏
		if len(m.panes) >= 2 {
			m.statusMsg = "⚠ Max 2 panes supported in MVP"
			return nil
		}

		args := strings.Fields(cmd)
		var newPane *EditorPane
		var err error

		if len(args) > 1 {
			// 打开指定文件 (存在或新建)
			path := args[1]
			newPane, err = m.createPaneFromFile(path)
			if err != nil {
				m.statusMsg = fmt.Sprintf("⚠ Error opening file: %v", err)
				return nil
			}
		} else {
			// 克隆当前 Pane
			newPane = m.cloneActivePane()
		}

		// 添加 Pane
		m.panes = append(m.panes, newPane)
		m.activePane = 1 // 切换到新 Pane
		
		if strings.HasPrefix(cmd, "vsp") {
			m.splitType = VerticalSplit
		} else {
			m.splitType = HorizontalSplit
		}

		m.syncSizes()
		m.statusMsg = "Split created"
		return nil
	}

	switch cmd {
	case "q", "quit":
		// 如果有多个分屏，只关闭当前分屏
		if len(m.panes) > 1 {
			// Remove current pane
			// Since only 2 panes, if we close one, we revert to single pane
			// Keep the *other* pane
			keepIndex := 0
			if m.activePane == 0 {
				keepIndex = 1
			}
			m.panes = []*EditorPane{m.panes[keepIndex]}
			m.activePane = 0
			m.splitType = NoSplit
			m.syncSizes()
			m.statusMsg = "Pane closed"
			return nil
		}
		// 只有一个分屏，退出程序
		return tea.Quit

	case "w", "write":
		// 保存文件
		currPane := m.panes[m.activePane]
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
		currPane := m.panes[m.activePane]
		if currPane.Filename != "" {
			if err := m.savePane(currPane); err != nil {
				m.statusMsg = fmt.Sprintf("⚠ 保存失败: %v", err)
				return nil
			}
		}
		
		// Close logic (duplicate of :q)
		if len(m.panes) > 1 {
			keepIndex := 0
			if m.activePane == 0 { keepIndex = 1 }
			m.panes = []*EditorPane{m.panes[keepIndex]}
			m.activePane = 0
			m.splitType = NoSplit
			m.syncSizes()
			return nil
		}
		return tea.Quit
	// Note: Skipped some cases for brevity, keep rest...
	case "q!":
		return tea.Quit

	case "tree", "e":
		// ... existing logic ...
	// ... (rest of cases need careful check if they used m.filename etc)

	// Since we are replacing the whole switch block or parts, let's keep it safe.
	// Actually, replace the whole function content for clarity? No, replace specific blocks.
	// But :vsp is prefix.
	// I will replace only the top part and 'q', 'w' logic.

    // ... Copying existing rest cases ...
		// m.syncSizes() called below
		if m.showSidebar {
			if m.fileTree.rootPath == "" {
				m.fileTree.rootPath, _ = os.Getwd()
			}
			m.fileTree.IsLoading = true
			m.fileTree.Entries = []FileEntry{}
			m.fileTree.cursor = 0
			m.focus = FocusFileTree
			m.statusMsg = "焦点: 文件树 | j/k=移动, Enter=打开/进入, Backspace=返回上一级"
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
				currPane := m.panes[m.activePane]
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
				m.git.IsLoading = true
				m.selectingGitRoot = false
				m.focus = FocusGit
				return m, checkGitStatusCmd() 
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
		if m.fileTree.cursor < len(m.fileTree.Entries)-1 {
			m.fileTree.cursor++
		}

	case "k", "up":
		// 向上移动
		if m.fileTree.cursor > 0 {
			m.fileTree.cursor--
		}

	case "enter":
		// 打开选中的文件或目录
		if len(m.fileTree.Entries) > 0 {
			entry := m.fileTree.Entries[m.fileTree.cursor]
			if entry.isDir {
				// 进入目录 (异步)
				m.fileTree.rootPath = entry.path
				m.fileTree.IsLoading = true
				m.fileTree.Entries = []FileEntry{} // 清空旧列表
				m.fileTree.cursor = 0
				return m, loadDirectoryCmd(entry.path)
			} else {
				// 文件：加载到编辑器 (异步)
				m.panes[m.activePane].Filename = entry.path
				// 切换焦点到编辑器，但保持侧边栏可见！
				m.focus = FocusEditor
				m.mode = NormalMode
				return m, loadFileCmd(entry.path)
			}
		}

	case "backspace", "-":
		// 返回上一级目录 (异步)
		parentDir := filepath.Dir(m.fileTree.rootPath)
		m.fileTree.rootPath = parentDir
		m.fileTree.IsLoading = true
		m.fileTree.Entries = []FileEntry{} // 清空旧列表
		m.fileTree.cursor = 0
		return m, loadDirectoryCmd(parentDir)

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
	currPane := m.panes[m.activePane]

	switch msg.Type {
	case tea.KeyEsc:
		// 退出插入模式
		m.mode = NormalMode
		m.statusMsg = "回到普通模式"
		// 光标左移一位（类似 Vim 行为）
		if currPane.CursorX > 0 {
			currPane.CursorX--
		}

	case tea.KeyEnter:
		// 插入新行
		m.insertNewLine(currPane)

	case tea.KeyBackspace:
		// 删除字符
		m.deleteChar(currPane)

	case tea.KeySpace:
		// 插入空格
		m.insertChar(currPane, ' ')
	
	case tea.KeyCtrlV:
		// 粘贴 (从系统剪贴板)
		text, err := clipboard.ReadAll()
		if err != nil || text == "" {
			m.statusMsg = "ℹ 剪贴板为空"
		} else {
			m.pasteToPane(currPane, text)
			m.statusMsg = "✓ 已粘贴"
		}

	case tea.KeyUp:
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

	case tea.KeyDown:
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

	case tea.KeyTab:
		// 如果有 AI 建议，按 Tab 接受建议
		if m.suggestion != "" {
			// 将建议的字符串逐个字符插入
			for _, ch := range m.suggestion {
				m.insertChar(currPane, ch)
			}
			m.suggestion = ""
			m.statusMsg = "✓ 已接受 AI 建议"
			return m, nil
		}

		// 否则插入制表符（4个空格）
		for i := 0; i < 4; i++ {
			m.insertChar(currPane, ' ')
		}

	default:
		// 插入普通字符
		if len(msg.String()) == 1 {
			m.insertChar(currPane, rune(msg.String()[0]))
		}
	}

	return m, nil
}

// =============================================================================
// 文本编辑操作
// =============================================================================

// insertChar 在光标位置插入字符
func (m *Model) insertChar(p *EditorPane, ch rune) {
	line := p.Lines[p.CursorY]
	newLine := line[:p.CursorX] + string(ch) + line[p.CursorX:]
	p.Lines[p.CursorY] = newLine
	p.CursorX++
}

// ... pasteToPane, insertNewLine, deleteChar are already updated ...

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

// deleteChar 删除光标前的字符
func (m *Model) deleteChar(p *EditorPane) {
	if p.CursorX > 0 {
		// 删除当前行中的字符
		line := p.Lines[p.CursorY]
		p.Lines[p.CursorY] = line[:p.CursorX-1] + line[p.CursorX:]
		p.CursorX--
	} else if p.CursorY > 0 {
		// 合并到上一行
		prevLine := p.Lines[p.CursorY-1]
		currLine := p.Lines[p.CursorY]
		p.CursorX = len(prevLine)
		p.Lines[p.CursorY-1] = prevLine + currLine

		// 删除当前行
		p.Lines = append(p.Lines[:p.CursorY], p.Lines[p.CursorY+1:]...)
		p.CursorY--
	}
}

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
	currPane := m.panes[m.activePane]
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

	// 动态高度逻辑: 实时渲染状态栏以获取其实际高度
	statusBar := m.renderStatusBar()
	statusBarHeight := lipgloss.Height(statusBar)

	editorTotalHeight := m.height - statusBarHeight
	if editorTotalHeight < 0 {
		editorTotalHeight = 0
	}

	// 更新缓存值
	m.cachedSidebarWidth = sidebarWidth
	m.cachedEditorWidth = editorTotalWidth
	m.cachedContentHeight = editorTotalHeight

	// 分配 Pane 尺寸
	if len(m.panes) == 0 {
		return
	}

	for i, pane := range m.panes {
		width := editorTotalWidth
		height := editorTotalHeight

		if m.splitType == VerticalSplit {
			width = editorTotalWidth / 2
			// 修正: 如果是左边的 Pane，且总宽是奇数，或者右边有边框?
			// 简单起见，均分，中间加个边框? View 渲染时再处理边框占位
			// 这里假设 Viewport 占满分配的区域
			// 如果有 2 个 Pane，每个占一半。
			// 为了给中间竖线留位置，宽度 -1
			if len(m.panes) > 1 {
				width = (editorTotalWidth - 1) / 2
			}
		} else if m.splitType == HorizontalSplit {
			if len(m.panes) > 1 {
				height = (editorTotalHeight - 1) / 2
			}
		}

		pane.Viewport.Width = width
		pane.Viewport.Height = height
		
		// 如果只有一个 Pane，确保利用剩余的像素 (奇数情况)
		// 实际上 Viewport 不严格要求像素完美对齐，因为 lipgloss.Place 会处理
		// 但为了滚动准确，高度最好准确
		if i == 1 {
			if m.splitType == VerticalSplit {
				width = editorTotalWidth - m.panes[0].Viewport.Width - 1
			} else if m.splitType == HorizontalSplit {
				height = editorTotalHeight - m.panes[0].Viewport.Height - 1
			}
			pane.Viewport.Width = width
			pane.Viewport.Height = height
		}
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

	// 3. 渲染编辑器 (Split View Logic)
	var editorView string
	
	if len(m.panes) == 0 {
		editorView = "" // Should not happen
	} else if len(m.panes) == 1 {
		// Single Pane
		editorView = m.renderPane(m.panes[0], editorWidth, editorHeight, m.activePane == 0)
	} else {
		// Split Pane
		pane0 := m.renderPane(m.panes[0], m.panes[0].Viewport.Width, m.panes[0].Viewport.Height, m.activePane == 0)
		pane1 := m.renderPane(m.panes[1], m.panes[1].Viewport.Width, m.panes[1].Viewport.Height, m.activePane == 1)

		if m.splitType == VerticalSplit {
			// Add border in between? renderPane already has border.
			editorView = lipgloss.JoinHorizontal(lipgloss.Top, pane0, pane1)
		} else {
			editorView = lipgloss.JoinVertical(lipgloss.Left, pane0, pane1)
		}
	}

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
// renderPane 渲染单个编辑器窗格
func (m Model) renderPane(p *EditorPane, width, height int, isActive bool) string {
	// 1. Sync Viewport Content
	// 注意: 每次渲染都 SetContent 可能有性能损耗，但在 TUI 中通常可以接受
	// 这样保证 Viewport 的滚动逻辑基于最新内容
	rawContent := strings.Join(p.Lines, "\n")
	p.Viewport.SetContent(rawContent)
	p.Viewport.Width = width - 2 // reserved for border
	p.Viewport.Height = height - 2
	
	// 2. 获取可视区域 (Bubble Tea Viewport 处理了滚动)
	visibleContent := p.Viewport.View()
	visibleLines := strings.Split(visibleContent, "\n")

	var lines []string

	// 实际可用内容宽高 (减去边框)
	contentWidth := width - 2
	// contentHeight := height - 2 
	
	// 行号区域宽度
	codeWidth := contentWidth - 7
	if codeWidth < 1 { codeWidth = 1 }

	// 语法高亮 (简单处理: 每帧匹配，以后优化到 EditorPane.Lexer)
	lexer := lexers.Match(p.Filename)
	if lexer == nil { lexer = lexers.Fallback }
	lexer = chroma.Coalesce(lexer)
	style := styles.Get("dracula")
	if style == nil { style = styles.Fallback }
	formatter := formatters.TTY256

	// Render loop
	// P.Viewport.View() returns ONLY the visible lines.
	// We need to calculate the starting line number based on YOffset.
	startLine := p.Viewport.YOffset
	
	for i, lineContent := range visibleLines {
		// 避免超出高度 (Viewport 有时会多返回一行?)
		if i >= height-2 { break }

		realLineNum := startLine + i + 1
		lineNumStr := fmt.Sprintf("%d", realLineNum)
		// Style line number
		lineNumStyled := lineNumberStyle.Render(lineNumStr)
		
		// Highlight line content
		// 对单行高亮有个问题: 上下文丢失。但为了 MVP...
		// 更好的做法是全高亮然后 Viewport 截取。
		// 但 Viewport 目前只存纯文本? 
		// SetContent 可以存 ANSI 字符串。
		// 如果 SetContent 存了高亮后的 ANSI 字符串，Viewport.View() 就会返回带颜色的。
		// 让我们尝试在 SetContent 之前高亮整个文件? 
		// 对于大文件太慢。
		// MVP: 单行高亮。
		
		it, err := lexer.Tokenise(nil, lineContent)
		var highlighted bytes.Buffer
		if err == nil {
			formatter.Format(&highlighted, style, it)
			lineContent = highlighted.String()
		}

		// 移除换行
		lineContent = strings.ReplaceAll(lineContent, "\n", "")
		
		// 截断
		// lineStyle := lipgloss.NewStyle().Width(codeWidth).MaxWidth(codeWidth)
		// renderedContent := lineStyle.Render(lineContent)
		// ANSI 字符处理比较麻烦，这里暂时不做硬截断，依赖 Viewport 的宽? 
		// Viewport 会处理换行吗? 
		// 如果 Viewport 只是 Text，它不做 wrapping (除非 SetStyle)。
		
		lines = append(lines, fmt.Sprintf("%s │ %s", lineNumStyled, lineContent))
	}
	
	// Fill empty space
	for len(lines) < height-2 {
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

	// 内容高度
	contentHeight := height - 2
	visibleHeight := contentHeight
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
		// Sync Dashboard
		output := "\n"
		output += fmt.Sprintf("  ✨ On branch: %s\n", lipgloss.NewStyle().Foreground(lipgloss.Color("205")).Render(m.git.Branch))
		output += "  Working Tree Clean\n\n"
		
		if m.git.Ahead == 0 && m.git.Behind == 0 {
			output += "  ✅ Up to date with remote"
		} else {
			if m.git.Ahead > 0 {
				output += fmt.Sprintf("  🚀 Ahead: %d commit(s)\n", m.git.Ahead)
			}
			if m.git.Behind > 0 {
				output += fmt.Sprintf("  ⬇️ Behind: %d commit(s)\n", m.git.Behind)
			}
			output += "\n  (Press 'Shift+P' to Push)"
		}
		
		// 填充空白行以保持布局一致 (可选)
		// 这里我们直接返回 lipgloss 渲染结果，renderWindow 会处理边框，
		// 但高度填充需要自己做吗？ renderWindow 接受 content string.
		// 为了垂直对齐，我们可以 append plain newlines to output
		
		return renderWindow(output, "Git-Clean", m.focus == FocusGit, width, height, false)
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



// renderLine removed (obsolete)

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
	var cx, cy int
	if len(m.panes) > m.activePane {
		cx = m.panes[m.activePane].CursorX + 1
		cy = m.panes[m.activePane].CursorY + 1
	}
	position := fmt.Sprintf(" Ln %d, Col %d ", cy, cx)

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
	
	// 不再强制截断 msg 到单行剩余宽度，
	// 而是允许其在样式器中自动折行（或由 renderStatusBar 的调用者根据 Width 限制）
	// 但为了保持左右对齐的视觉效果，我们仍然计算中间部分的填充
	
	middleContent := fmt.Sprintf(" %s", msg)
	middlePart := statusBarStyle.Width(availWidth).Render(middleContent)

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
