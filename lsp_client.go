package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"net/textproto"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"

	tea "github.com/charmbracelet/bubbletea"
)

// lsp_client.go - LSP 客户端核心引擎

type LSPClient struct {
	cmd       *exec.Cmd
	stdin     io.WriteCloser
	stdout    io.ReadCloser
	requestID int
	mu        sync.Mutex
	isReady   bool
}

// 消息定义：用于 Bubble Tea 通信
type LSPResponseMsg struct {
	ID     int
	Result json.RawMessage // 原始 JSON，由接收方解析
}

type LSPLogMsg string

func NewLSPClient() *LSPClient {
	return &LSPClient{}
}

// 启动 gopls
func (c *LSPClient) Start() tea.Cmd {
	return func() tea.Msg {
		// 1. 查找 gopls - 尝试多个位置
		var path string
		var err error
		
		// 先尝试 PATH
		path, err = exec.LookPath("gopls")
		if err != nil {
			// 尝试常见的 Go bin 目录
			homeDir, _ := os.UserHomeDir()
			candidates := []string{
				filepath.Join(homeDir, "go", "bin", "gopls"),
				"/usr/local/go/bin/gopls",
				"/root/go/bin/gopls",
				filepath.Join(os.Getenv("GOPATH"), "bin", "gopls"),
			}
			
			for _, candidate := range candidates {
				if _, statErr := os.Stat(candidate); statErr == nil {
					path = candidate
					break
				}
			}
			
			if path == "" {
				return LSPLogMsg("Error: gopls not found. Run: go install golang.org/x/tools/gopls@latest")
			}
		}

		// 2. 启动进程
		cmd := exec.Command(path)
		stdin, _ := cmd.StdinPipe()
		stdout, _ := cmd.StdoutPipe()

		if err := cmd.Start(); err != nil {
			return LSPLogMsg("Error starting gopls: " + err.Error())
		}

		c.cmd = cmd
		c.stdin = stdin
		c.stdout = stdout

		// 3. 开始读取循环（异步）
		go c.readLoop()

		return LSPLogMsg("LSP Started (gopls)")
	}
}

// 发送请求的底层逻辑
func (c *LSPClient) Send(method string, params interface{}) int {
	c.mu.Lock()
	c.requestID++
	id := c.requestID
	c.mu.Unlock()

	req := BaseRequest{
		JSONRPC: "2.0",
		ID:      id,
		Method:  method,
		Params:  params,
	}

	body, _ := json.Marshal(req)
	// LSP 协议头：必须带 Content-Length
	msg := fmt.Sprintf("Content-Length: %d\r\n\r\n%s", len(body), body)

	if c.stdin != nil {
		c.stdin.Write([]byte(msg))
	}
	return id
}

// 发送通知（不需要回复）
func (c *LSPClient) Notify(method string, params interface{}) {
	req := BaseNotification{
		JSONRPC: "2.0",
		Method:  method,
		Params:  params,
	}
	body, _ := json.Marshal(req)
	msg := fmt.Sprintf("Content-Length: %d\r\n\r\n%s", len(body), body)
	if c.stdin != nil {
		c.stdin.Write([]byte(msg))
	}
}

// 读取循环：解析 LSP 协议头
func (c *LSPClient) readLoop() {
	reader := bufio.NewReader(c.stdout)
	tp := textproto.NewReader(reader)

	for {
		// 1. 读取 Header (Content-Length: 123)
		headers, err := tp.ReadMIMEHeader()
		if err != nil {
			return // 进程挂了
		}

		lengthStr := headers.Get("Content-Length")
		if lengthStr == "" {
			continue
		}
		length, _ := strconv.Atoi(lengthStr)

		// 2. 读取 Body
		body := make([]byte, length)
		_, err = io.ReadFull(reader, body)
		if err != nil {
			return
		}

		// 3. 解析基础响应，看看是哪个 ID 的回复
		var baseResp BaseResponse
		json.Unmarshal(body, &baseResp)

		// 4. 发送给主程序
		if globalProgram != nil {
			// 这里的 Result 需要再次解析
			resJSON, _ := json.Marshal(baseResp.Result)
			globalProgram.Send(LSPResponseMsg{ID: baseResp.ID, Result: resJSON})
		}
	}
}

// Stop 停止 LSP 客户端
func (c *LSPClient) Stop() {
	if c.stdin != nil {
		c.stdin.Close()
	}
	if c.cmd != nil && c.cmd.Process != nil {
		c.cmd.Process.Kill()
	}
}

// 辅助工具：文件路径转 URI
func PathToURI(path string) string {
	abs, _ := filepath.Abs(path)
	// Windows 下需要处理反斜杠
	abs = filepath.ToSlash(abs)
	// 如果是 Windows，前面加 /
	if !strings.HasPrefix(abs, "/") {
		abs = "/" + abs
	}
	return "file://" + abs
}

// 根据文件扩展名检测语言 ID
func DetectLanguageID(filename string) string {
	ext := strings.ToLower(filepath.Ext(filename))
	switch ext {
	case ".go":
		return "go"
	case ".py":
		return "python"
	case ".js":
		return "javascript"
	case ".ts":
		return "typescript"
	case ".rs":
		return "rust"
	case ".c", ".h":
		return "c"
	case ".cpp", ".cc", ".cxx", ".hpp":
		return "cpp"
	case ".java":
		return "java"
	case ".rb":
		return "ruby"
	case ".lua":
		return "lua"
	case ".md":
		return "markdown"
	case ".json":
		return "json"
	case ".yaml", ".yml":
		return "yaml"
	case ".html":
		return "html"
	case ".css":
		return "css"
	default:
		return "plaintext"
	}
}
