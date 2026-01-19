package main

import (
	"sort"
	"strings"
	"unicode"
)

// completion.go - 简单实用的静态补全系统

// CompletionItemSimple 简化的补全项
type CompletionItemSimple struct {
	Label    string // 显示的文本
	InsertText string // 实际插入的文本（可包含模板）
	Detail   string // 描述
	Kind     string // 类型：func, var, keyword, snippet
}

// GetCompletions 获取补全列表
// prefix: 当前输入的前缀 (如 "fmt." 或 "Pri")
// lines: 当前文件的所有行
// language: 语言类型
func GetCompletions(prefix string, lines []string, language string) []CompletionItemSimple {
	var results []CompletionItemSimple
	
	// 检查是否是成员访问 (如 fmt.xxx, console.xxx)
	if idx := strings.LastIndex(prefix, "."); idx >= 0 {
		pkg := prefix[:idx]
		memberPrefix := ""
		if idx+1 < len(prefix) {
			memberPrefix = prefix[idx+1:]
		}
		
		// 根据语言选择包补全数据
		var pkgCompletions map[string][]CompletionItemSimple
		switch language {
		case "python":
			pkgCompletions = pythonModuleCompletions
		case "javascript", "typescript", "javascriptreact", "typescriptreact":
			pkgCompletions = jsModuleCompletions
		case "rust":
			pkgCompletions = rustModuleCompletions
		default:
			pkgCompletions = goPackageCompletions
		}
		
		// 查找包的成员补全
		if members, ok := pkgCompletions[pkg]; ok {
			for _, item := range members {
				if memberPrefix == "" || strings.HasPrefix(strings.ToLower(item.Label), strings.ToLower(memberPrefix)) {
					results = append(results, item)
				}
			}
		}
		return results
	}
	
	// 普通补全：关键字 + 代码片段 + 文件内标识符
	lowerPrefix := strings.ToLower(prefix)
	
	// 1. 语言关键字
	var keywords []CompletionItemSimple
	var snippets []CompletionItemSimple
	var pkgCompletions map[string][]CompletionItemSimple
	
	switch language {
	case "python":
		keywords = pythonKeywords
		snippets = pythonSnippets
		pkgCompletions = pythonModuleCompletions
	case "javascript", "javascriptreact":
		keywords = jsKeywords
		snippets = jsSnippets
		pkgCompletions = jsModuleCompletions
	case "typescript", "typescriptreact":
		keywords = tsKeywords
		snippets = tsSnippets
		pkgCompletions = jsModuleCompletions
	case "rust":
		keywords = rustKeywords
		snippets = rustSnippets
		pkgCompletions = rustModuleCompletions
	case "c", "cpp":
		keywords = cKeywords
		snippets = cSnippets
		pkgCompletions = nil
	case "java":
		keywords = javaKeywords
		snippets = javaSnippets
		pkgCompletions = javaModuleCompletions
	default:
		keywords = goKeywords
		snippets = goSnippets
		pkgCompletions = goPackageCompletions
	}
	
	for _, item := range keywords {
		if strings.HasPrefix(strings.ToLower(item.Label), lowerPrefix) {
			results = append(results, item)
		}
	}
	
	// 2. 常用代码片段
	for _, item := range snippets {
		if strings.HasPrefix(strings.ToLower(item.Label), lowerPrefix) {
			results = append(results, item)
		}
	}
	
	// 3. 常用包/模块名
	if pkgCompletions != nil {
		for pkg := range pkgCompletions {
			if strings.HasPrefix(strings.ToLower(pkg), lowerPrefix) {
				results = append(results, CompletionItemSimple{
					Label:  pkg,
					InsertText: pkg,
					Detail: "module",
					Kind:   "module",
				})
			}
		}
	}
	
	// 4. 文件内标识符
	identifiers := extractIdentifiers(lines)
	for _, id := range identifiers {
		if id != prefix && strings.HasPrefix(strings.ToLower(id), lowerPrefix) {
			results = append(results, CompletionItemSimple{
				Label:  id,
				InsertText: id,
				Detail: "identifier",
				Kind:   "variable",
			})
		}
	}
	
	// 去重
	seen := make(map[string]bool)
	var unique []CompletionItemSimple
	for _, item := range results {
		if !seen[item.Label] {
			seen[item.Label] = true
			unique = append(unique, item)
		}
	}
	
	// 按类型和名称排序
	sort.Slice(unique, func(i, j int) bool {
		// 优先级：snippet > keyword > func > variable
		priority := map[string]int{"snippet": 0, "keyword": 1, "func": 2, "module": 3, "variable": 4}
		pi, pj := priority[unique[i].Kind], priority[unique[j].Kind]
		if pi != pj {
			return pi < pj
		}
		return unique[i].Label < unique[j].Label
	})
	
	// 限制最多 15 个
	if len(unique) > 15 {
		unique = unique[:15]
	}
	
	return unique
}

// extractIdentifiers 从代码中提取标识符
func extractIdentifiers(lines []string) []string {
	seen := make(map[string]bool)
	var result []string
	
	for _, line := range lines {
		// 简单的标识符提取：找所有字母开头的单词
		words := extractWords(line)
		for _, word := range words {
			if len(word) >= 2 && !isKeyword(word) && !seen[word] {
				seen[word] = true
				result = append(result, word)
			}
		}
	}
	
	return result
}

func extractWords(s string) []string {
	var words []string
	var current strings.Builder
	
	for _, r := range s {
		if unicode.IsLetter(r) || unicode.IsDigit(r) || r == '_' {
			current.WriteRune(r)
		} else {
			if current.Len() > 0 {
				word := current.String()
				// 必须以字母开头
				if unicode.IsLetter(rune(word[0])) {
					words = append(words, word)
				}
				current.Reset()
			}
		}
	}
	
	if current.Len() > 0 {
		word := current.String()
		if unicode.IsLetter(rune(word[0])) {
			words = append(words, word)
		}
	}
	
	return words
}

func isKeyword(word string) bool {
	keywords := map[string]bool{
		"break": true, "case": true, "chan": true, "const": true, "continue": true,
		"default": true, "defer": true, "else": true, "fallthrough": true, "for": true,
		"func": true, "go": true, "goto": true, "if": true, "import": true,
		"interface": true, "map": true, "package": true, "range": true, "return": true,
		"select": true, "struct": true, "switch": true, "type": true, "var": true,
	}
	return keywords[word]
}

// =============================================================================
// Go 语言补全数据
// =============================================================================

var goKeywords = []CompletionItemSimple{
	{Label: "func", InsertText: "func ", Detail: "function declaration", Kind: "keyword"},
	{Label: "var", InsertText: "var ", Detail: "variable declaration", Kind: "keyword"},
	{Label: "const", InsertText: "const ", Detail: "constant declaration", Kind: "keyword"},
	{Label: "type", InsertText: "type ", Detail: "type declaration", Kind: "keyword"},
	{Label: "struct", InsertText: "struct {\n\t\n}", Detail: "struct type", Kind: "keyword"},
	{Label: "interface", InsertText: "interface {\n\t\n}", Detail: "interface type", Kind: "keyword"},
	{Label: "if", InsertText: "if ", Detail: "if statement", Kind: "keyword"},
	{Label: "else", InsertText: "else ", Detail: "else clause", Kind: "keyword"},
	{Label: "for", InsertText: "for ", Detail: "for loop", Kind: "keyword"},
	{Label: "range", InsertText: "range ", Detail: "range iteration", Kind: "keyword"},
	{Label: "switch", InsertText: "switch ", Detail: "switch statement", Kind: "keyword"},
	{Label: "case", InsertText: "case ", Detail: "case clause", Kind: "keyword"},
	{Label: "default", InsertText: "default:", Detail: "default clause", Kind: "keyword"},
	{Label: "return", InsertText: "return ", Detail: "return statement", Kind: "keyword"},
	{Label: "defer", InsertText: "defer ", Detail: "defer statement", Kind: "keyword"},
	{Label: "go", InsertText: "go ", Detail: "goroutine", Kind: "keyword"},
	{Label: "select", InsertText: "select {\n\t\n}", Detail: "select statement", Kind: "keyword"},
	{Label: "chan", InsertText: "chan ", Detail: "channel type", Kind: "keyword"},
	{Label: "map", InsertText: "map[", Detail: "map type", Kind: "keyword"},
	{Label: "make", InsertText: "make()", Detail: "make builtin", Kind: "func"},
	{Label: "new", InsertText: "new()", Detail: "new builtin", Kind: "func"},
	{Label: "append", InsertText: "append()", Detail: "append builtin", Kind: "func"},
	{Label: "len", InsertText: "len()", Detail: "length builtin", Kind: "func"},
	{Label: "cap", InsertText: "cap()", Detail: "capacity builtin", Kind: "func"},
	{Label: "copy", InsertText: "copy()", Detail: "copy builtin", Kind: "func"},
	{Label: "delete", InsertText: "delete()", Detail: "delete builtin", Kind: "func"},
	{Label: "panic", InsertText: "panic()", Detail: "panic builtin", Kind: "func"},
	{Label: "recover", InsertText: "recover()", Detail: "recover builtin", Kind: "func"},
	{Label: "nil", InsertText: "nil", Detail: "nil value", Kind: "keyword"},
	{Label: "true", InsertText: "true", Detail: "boolean true", Kind: "keyword"},
	{Label: "false", InsertText: "false", Detail: "boolean false", Kind: "keyword"},
	{Label: "iota", InsertText: "iota", Detail: "iota constant", Kind: "keyword"},
}

var goSnippets = []CompletionItemSimple{
	{Label: "iferr", InsertText: "if err != nil {\n\treturn err\n}", Detail: "if err != nil", Kind: "snippet"},
	{Label: "ifnil", InsertText: "if x == nil {\n\t\n}", Detail: "if nil check", Kind: "snippet"},
	{Label: "fori", InsertText: "for i := 0; i < n; i++ {\n\t\n}", Detail: "for i loop", Kind: "snippet"},
	{Label: "forr", InsertText: "for _, v := range  {\n\t\n}", Detail: "for range loop", Kind: "snippet"},
	{Label: "funcm", InsertText: "func (m *) () {\n\t\n}", Detail: "method declaration", Kind: "snippet"},
	{Label: "main", InsertText: "func main() {\n\t\n}", Detail: "main function", Kind: "snippet"},
	{Label: "init", InsertText: "func init() {\n\t\n}", Detail: "init function", Kind: "snippet"},
	{Label: "test", InsertText: "func Test(t *testing.T) {\n\t\n}", Detail: "test function", Kind: "snippet"},
	{Label: "bench", InsertText: "func Benchmark(b *testing.B) {\n\tfor i := 0; i < b.N; i++ {\n\t\t\n\t}\n}", Detail: "benchmark function", Kind: "snippet"},
	{Label: "goroutine", InsertText: "go func() {\n\t\n}()", Detail: "anonymous goroutine", Kind: "snippet"},
}

// 常用包的成员补全
var goPackageCompletions = map[string][]CompletionItemSimple{
	"fmt": {
		{Label: "Println", InsertText: "Println()", Detail: "func(a ...any) (n int, err error)", Kind: "func"},
		{Label: "Printf", InsertText: "Printf()", Detail: "func(format string, a ...any) (n int, err error)", Kind: "func"},
		{Label: "Print", InsertText: "Print()", Detail: "func(a ...any) (n int, err error)", Kind: "func"},
		{Label: "Sprintf", InsertText: "Sprintf()", Detail: "func(format string, a ...any) string", Kind: "func"},
		{Label: "Errorf", InsertText: "Errorf()", Detail: "func(format string, a ...any) error", Kind: "func"},
		{Label: "Fprintf", InsertText: "Fprintf()", Detail: "func(w io.Writer, format string, a ...any)", Kind: "func"},
		{Label: "Scanln", InsertText: "Scanln()", Detail: "func(a ...any) (n int, err error)", Kind: "func"},
		{Label: "Sscanf", InsertText: "Sscanf()", Detail: "func(str string, format string, a ...any)", Kind: "func"},
	},
	"strings": {
		{Label: "Contains", InsertText: "Contains()", Detail: "func(s, substr string) bool", Kind: "func"},
		{Label: "HasPrefix", InsertText: "HasPrefix()", Detail: "func(s, prefix string) bool", Kind: "func"},
		{Label: "HasSuffix", InsertText: "HasSuffix()", Detail: "func(s, suffix string) bool", Kind: "func"},
		{Label: "Split", InsertText: "Split()", Detail: "func(s, sep string) []string", Kind: "func"},
		{Label: "Join", InsertText: "Join()", Detail: "func(elems []string, sep string) string", Kind: "func"},
		{Label: "Replace", InsertText: "Replace()", Detail: "func(s, old, new string, n int) string", Kind: "func"},
		{Label: "ReplaceAll", InsertText: "ReplaceAll()", Detail: "func(s, old, new string) string", Kind: "func"},
		{Label: "ToLower", InsertText: "ToLower()", Detail: "func(s string) string", Kind: "func"},
		{Label: "ToUpper", InsertText: "ToUpper()", Detail: "func(s string) string", Kind: "func"},
		{Label: "TrimSpace", InsertText: "TrimSpace()", Detail: "func(s string) string", Kind: "func"},
		{Label: "Trim", InsertText: "Trim()", Detail: "func(s, cutset string) string", Kind: "func"},
		{Label: "Index", InsertText: "Index()", Detail: "func(s, substr string) int", Kind: "func"},
		{Label: "Builder", InsertText: "Builder{}", Detail: "type Builder struct", Kind: "struct"},
	},
	"os": {
		{Label: "Open", InsertText: "Open()", Detail: "func(name string) (*File, error)", Kind: "func"},
		{Label: "Create", InsertText: "Create()", Detail: "func(name string) (*File, error)", Kind: "func"},
		{Label: "ReadFile", InsertText: "ReadFile()", Detail: "func(name string) ([]byte, error)", Kind: "func"},
		{Label: "WriteFile", InsertText: "WriteFile()", Detail: "func(name string, data []byte, perm FileMode)", Kind: "func"},
		{Label: "Remove", InsertText: "Remove()", Detail: "func(name string) error", Kind: "func"},
		{Label: "RemoveAll", InsertText: "RemoveAll()", Detail: "func(path string) error", Kind: "func"},
		{Label: "Mkdir", InsertText: "Mkdir()", Detail: "func(name string, perm FileMode) error", Kind: "func"},
		{Label: "MkdirAll", InsertText: "MkdirAll()", Detail: "func(path string, perm FileMode) error", Kind: "func"},
		{Label: "Getenv", InsertText: "Getenv()", Detail: "func(key string) string", Kind: "func"},
		{Label: "Setenv", InsertText: "Setenv()", Detail: "func(key, value string) error", Kind: "func"},
		{Label: "Exit", InsertText: "Exit()", Detail: "func(code int)", Kind: "func"},
		{Label: "Args", InsertText: "Args", Detail: "var Args []string", Kind: "variable"},
		{Label: "Stdin", InsertText: "Stdin", Detail: "var Stdin *File", Kind: "variable"},
		{Label: "Stdout", InsertText: "Stdout", Detail: "var Stdout *File", Kind: "variable"},
		{Label: "Stderr", InsertText: "Stderr", Detail: "var Stderr *File", Kind: "variable"},
	},
	"io": {
		{Label: "Copy", InsertText: "Copy()", Detail: "func(dst Writer, src Reader) (int64, error)", Kind: "func"},
		{Label: "ReadAll", InsertText: "ReadAll()", Detail: "func(r Reader) ([]byte, error)", Kind: "func"},
		{Label: "WriteString", InsertText: "WriteString()", Detail: "func(w Writer, s string) (int, error)", Kind: "func"},
		{Label: "EOF", InsertText: "EOF", Detail: "var EOF error", Kind: "variable"},
		{Label: "Reader", InsertText: "Reader", Detail: "interface Reader", Kind: "interface"},
		{Label: "Writer", InsertText: "Writer", Detail: "interface Writer", Kind: "interface"},
	},
	"path": {
		{Label: "Join", InsertText: "Join()", Detail: "func(elem ...string) string", Kind: "func"},
		{Label: "Base", InsertText: "Base()", Detail: "func(path string) string", Kind: "func"},
		{Label: "Dir", InsertText: "Dir()", Detail: "func(path string) string", Kind: "func"},
		{Label: "Ext", InsertText: "Ext()", Detail: "func(path string) string", Kind: "func"},
	},
	"filepath": {
		{Label: "Join", InsertText: "Join()", Detail: "func(elem ...string) string", Kind: "func"},
		{Label: "Abs", InsertText: "Abs()", Detail: "func(path string) (string, error)", Kind: "func"},
		{Label: "Base", InsertText: "Base()", Detail: "func(path string) string", Kind: "func"},
		{Label: "Dir", InsertText: "Dir()", Detail: "func(path string) string", Kind: "func"},
		{Label: "Ext", InsertText: "Ext()", Detail: "func(path string) string", Kind: "func"},
		{Label: "Walk", InsertText: "Walk()", Detail: "func(root string, fn WalkFunc) error", Kind: "func"},
		{Label: "Glob", InsertText: "Glob()", Detail: "func(pattern string) ([]string, error)", Kind: "func"},
	},
	"json": {
		{Label: "Marshal", InsertText: "Marshal()", Detail: "func(v any) ([]byte, error)", Kind: "func"},
		{Label: "Unmarshal", InsertText: "Unmarshal()", Detail: "func(data []byte, v any) error", Kind: "func"},
		{Label: "NewEncoder", InsertText: "NewEncoder()", Detail: "func(w io.Writer) *Encoder", Kind: "func"},
		{Label: "NewDecoder", InsertText: "NewDecoder()", Detail: "func(r io.Reader) *Decoder", Kind: "func"},
	},
	"time": {
		{Label: "Now", InsertText: "Now()", Detail: "func() Time", Kind: "func"},
		{Label: "Sleep", InsertText: "Sleep()", Detail: "func(d Duration)", Kind: "func"},
		{Label: "Since", InsertText: "Since()", Detail: "func(t Time) Duration", Kind: "func"},
		{Label: "Until", InsertText: "Until()", Detail: "func(t Time) Duration", Kind: "func"},
		{Label: "Parse", InsertText: "Parse()", Detail: "func(layout, value string) (Time, error)", Kind: "func"},
		{Label: "Second", InsertText: "Second", Detail: "const Second Duration", Kind: "variable"},
		{Label: "Minute", InsertText: "Minute", Detail: "const Minute Duration", Kind: "variable"},
		{Label: "Hour", InsertText: "Hour", Detail: "const Hour Duration", Kind: "variable"},
		{Label: "Millisecond", InsertText: "Millisecond", Detail: "const Millisecond Duration", Kind: "variable"},
	},
	"errors": {
		{Label: "New", InsertText: "New()", Detail: "func(text string) error", Kind: "func"},
		{Label: "Is", InsertText: "Is()", Detail: "func(err, target error) bool", Kind: "func"},
		{Label: "As", InsertText: "As()", Detail: "func(err error, target any) bool", Kind: "func"},
		{Label: "Unwrap", InsertText: "Unwrap()", Detail: "func(err error) error", Kind: "func"},
	},
	"context": {
		{Label: "Background", InsertText: "Background()", Detail: "func() Context", Kind: "func"},
		{Label: "TODO", InsertText: "TODO()", Detail: "func() Context", Kind: "func"},
		{Label: "WithCancel", InsertText: "WithCancel()", Detail: "func(parent Context) (Context, CancelFunc)", Kind: "func"},
		{Label: "WithTimeout", InsertText: "WithTimeout()", Detail: "func(parent Context, timeout Duration)", Kind: "func"},
		{Label: "WithValue", InsertText: "WithValue()", Detail: "func(parent Context, key, val any) Context", Kind: "func"},
	},
	"sync": {
		{Label: "Mutex", InsertText: "Mutex{}", Detail: "type Mutex struct", Kind: "struct"},
		{Label: "RWMutex", InsertText: "RWMutex{}", Detail: "type RWMutex struct", Kind: "struct"},
		{Label: "WaitGroup", InsertText: "WaitGroup{}", Detail: "type WaitGroup struct", Kind: "struct"},
		{Label: "Once", InsertText: "Once{}", Detail: "type Once struct", Kind: "struct"},
		{Label: "Map", InsertText: "Map{}", Detail: "type Map struct", Kind: "struct"},
	},
	"log": {
		{Label: "Println", InsertText: "Println()", Detail: "func(v ...any)", Kind: "func"},
		{Label: "Printf", InsertText: "Printf()", Detail: "func(format string, v ...any)", Kind: "func"},
		{Label: "Fatal", InsertText: "Fatal()", Detail: "func(v ...any)", Kind: "func"},
		{Label: "Fatalf", InsertText: "Fatalf()", Detail: "func(format string, v ...any)", Kind: "func"},
		{Label: "Panic", InsertText: "Panic()", Detail: "func(v ...any)", Kind: "func"},
	},
	"http": {
		{Label: "Get", InsertText: "Get()", Detail: "func(url string) (*Response, error)", Kind: "func"},
		{Label: "Post", InsertText: "Post()", Detail: "func(url, contentType string, body io.Reader)", Kind: "func"},
		{Label: "ListenAndServe", InsertText: "ListenAndServe()", Detail: "func(addr string, handler Handler) error", Kind: "func"},
		{Label: "HandleFunc", InsertText: "HandleFunc()", Detail: "func(pattern string, handler func)", Kind: "func"},
		{Label: "NewRequest", InsertText: "NewRequest()", Detail: "func(method, url string, body io.Reader)", Kind: "func"},
		{Label: "StatusOK", InsertText: "StatusOK", Detail: "const StatusOK = 200", Kind: "variable"},
	},
	"strconv": {
		{Label: "Atoi", InsertText: "Atoi()", Detail: "func(s string) (int, error)", Kind: "func"},
		{Label: "Itoa", InsertText: "Itoa()", Detail: "func(i int) string", Kind: "func"},
		{Label: "ParseInt", InsertText: "ParseInt()", Detail: "func(s string, base, bitSize int) (int64, error)", Kind: "func"},
		{Label: "ParseFloat", InsertText: "ParseFloat()", Detail: "func(s string, bitSize int) (float64, error)", Kind: "func"},
		{Label: "FormatInt", InsertText: "FormatInt()", Detail: "func(i int64, base int) string", Kind: "func"},
	},
	"regexp": {
		{Label: "Compile", InsertText: "Compile()", Detail: "func(expr string) (*Regexp, error)", Kind: "func"},
		{Label: "MustCompile", InsertText: "MustCompile()", Detail: "func(str string) *Regexp", Kind: "func"},
		{Label: "MatchString", InsertText: "MatchString()", Detail: "func(pattern, s string) (bool, error)", Kind: "func"},
	},
	"sort": {
		{Label: "Strings", InsertText: "Strings()", Detail: "func(x []string)", Kind: "func"},
		{Label: "Ints", InsertText: "Ints()", Detail: "func(x []int)", Kind: "func"},
		{Label: "Slice", InsertText: "Slice()", Detail: "func(x any, less func(i, j int) bool)", Kind: "func"},
		{Label: "Search", InsertText: "Search()", Detail: "func(n int, f func(int) bool) int", Kind: "func"},
	},
	"bytes": {
		{Label: "Buffer", InsertText: "Buffer{}", Detail: "type Buffer struct", Kind: "struct"},
		{Label: "Contains", InsertText: "Contains()", Detail: "func(b, subslice []byte) bool", Kind: "func"},
		{Label: "Equal", InsertText: "Equal()", Detail: "func(a, b []byte) bool", Kind: "func"},
		{Label: "Split", InsertText: "Split()", Detail: "func(s, sep []byte) [][]byte", Kind: "func"},
		{Label: "Join", InsertText: "Join()", Detail: "func(s [][]byte, sep []byte) []byte", Kind: "func"},
	},
	"bufio": {
		{Label: "NewReader", InsertText: "NewReader()", Detail: "func(rd io.Reader) *Reader", Kind: "func"},
		{Label: "NewWriter", InsertText: "NewWriter()", Detail: "func(w io.Writer) *Writer", Kind: "func"},
		{Label: "NewScanner", InsertText: "NewScanner()", Detail: "func(r io.Reader) *Scanner", Kind: "func"},
	},
	"exec": {
		{Label: "Command", InsertText: "Command()", Detail: "func(name string, arg ...string) *Cmd", Kind: "func"},
		{Label: "LookPath", InsertText: "LookPath()", Detail: "func(file string) (string, error)", Kind: "func"},
	},
	"reflect": {
		{Label: "TypeOf", InsertText: "TypeOf()", Detail: "func(i any) Type", Kind: "func"},
		{Label: "ValueOf", InsertText: "ValueOf()", Detail: "func(i any) Value", Kind: "func"},
	},
	"testing": {
		{Label: "T", InsertText: "T", Detail: "type T struct", Kind: "struct"},
		{Label: "B", InsertText: "B", Detail: "type B struct", Kind: "struct"},
	},
}

// =============================================================================
// Python 语言补全数据
// =============================================================================

var pythonKeywords = []CompletionItemSimple{
	{Label: "def", InsertText: "def ", Detail: "function definition", Kind: "keyword"},
	{Label: "class", InsertText: "class ", Detail: "class definition", Kind: "keyword"},
	{Label: "if", InsertText: "if ", Detail: "if statement", Kind: "keyword"},
	{Label: "elif", InsertText: "elif ", Detail: "elif clause", Kind: "keyword"},
	{Label: "else", InsertText: "else:", Detail: "else clause", Kind: "keyword"},
	{Label: "for", InsertText: "for ", Detail: "for loop", Kind: "keyword"},
	{Label: "while", InsertText: "while ", Detail: "while loop", Kind: "keyword"},
	{Label: "try", InsertText: "try:", Detail: "try block", Kind: "keyword"},
	{Label: "except", InsertText: "except ", Detail: "except clause", Kind: "keyword"},
	{Label: "finally", InsertText: "finally:", Detail: "finally clause", Kind: "keyword"},
	{Label: "with", InsertText: "with ", Detail: "with statement", Kind: "keyword"},
	{Label: "as", InsertText: "as ", Detail: "as keyword", Kind: "keyword"},
	{Label: "import", InsertText: "import ", Detail: "import statement", Kind: "keyword"},
	{Label: "from", InsertText: "from ", Detail: "from import", Kind: "keyword"},
	{Label: "return", InsertText: "return ", Detail: "return statement", Kind: "keyword"},
	{Label: "yield", InsertText: "yield ", Detail: "yield expression", Kind: "keyword"},
	{Label: "lambda", InsertText: "lambda ", Detail: "lambda expression", Kind: "keyword"},
	{Label: "async", InsertText: "async ", Detail: "async definition", Kind: "keyword"},
	{Label: "await", InsertText: "await ", Detail: "await expression", Kind: "keyword"},
	{Label: "pass", InsertText: "pass", Detail: "pass statement", Kind: "keyword"},
	{Label: "break", InsertText: "break", Detail: "break statement", Kind: "keyword"},
	{Label: "continue", InsertText: "continue", Detail: "continue statement", Kind: "keyword"},
	{Label: "raise", InsertText: "raise ", Detail: "raise exception", Kind: "keyword"},
	{Label: "assert", InsertText: "assert ", Detail: "assert statement", Kind: "keyword"},
	{Label: "global", InsertText: "global ", Detail: "global declaration", Kind: "keyword"},
	{Label: "nonlocal", InsertText: "nonlocal ", Detail: "nonlocal declaration", Kind: "keyword"},
	{Label: "True", InsertText: "True", Detail: "boolean True", Kind: "keyword"},
	{Label: "False", InsertText: "False", Detail: "boolean False", Kind: "keyword"},
	{Label: "None", InsertText: "None", Detail: "None value", Kind: "keyword"},
	{Label: "and", InsertText: "and ", Detail: "logical and", Kind: "keyword"},
	{Label: "or", InsertText: "or ", Detail: "logical or", Kind: "keyword"},
	{Label: "not", InsertText: "not ", Detail: "logical not", Kind: "keyword"},
	{Label: "in", InsertText: "in ", Detail: "in operator", Kind: "keyword"},
	{Label: "is", InsertText: "is ", Detail: "is operator", Kind: "keyword"},
	// 内置函数
	{Label: "print", InsertText: "print()", Detail: "print(*args)", Kind: "func"},
	{Label: "len", InsertText: "len()", Detail: "len(s)", Kind: "func"},
	{Label: "range", InsertText: "range()", Detail: "range(stop)", Kind: "func"},
	{Label: "str", InsertText: "str()", Detail: "str(object)", Kind: "func"},
	{Label: "int", InsertText: "int()", Detail: "int(x)", Kind: "func"},
	{Label: "float", InsertText: "float()", Detail: "float(x)", Kind: "func"},
	{Label: "list", InsertText: "list()", Detail: "list(iterable)", Kind: "func"},
	{Label: "dict", InsertText: "dict()", Detail: "dict(**kwargs)", Kind: "func"},
	{Label: "set", InsertText: "set()", Detail: "set(iterable)", Kind: "func"},
	{Label: "tuple", InsertText: "tuple()", Detail: "tuple(iterable)", Kind: "func"},
	{Label: "type", InsertText: "type()", Detail: "type(object)", Kind: "func"},
	{Label: "isinstance", InsertText: "isinstance()", Detail: "isinstance(obj, class)", Kind: "func"},
	{Label: "open", InsertText: "open()", Detail: "open(file, mode)", Kind: "func"},
	{Label: "input", InsertText: "input()", Detail: "input(prompt)", Kind: "func"},
	{Label: "enumerate", InsertText: "enumerate()", Detail: "enumerate(iterable)", Kind: "func"},
	{Label: "zip", InsertText: "zip()", Detail: "zip(*iterables)", Kind: "func"},
	{Label: "map", InsertText: "map()", Detail: "map(func, iterable)", Kind: "func"},
	{Label: "filter", InsertText: "filter()", Detail: "filter(func, iterable)", Kind: "func"},
	{Label: "sorted", InsertText: "sorted()", Detail: "sorted(iterable)", Kind: "func"},
	{Label: "reversed", InsertText: "reversed()", Detail: "reversed(seq)", Kind: "func"},
	{Label: "sum", InsertText: "sum()", Detail: "sum(iterable)", Kind: "func"},
	{Label: "max", InsertText: "max()", Detail: "max(iterable)", Kind: "func"},
	{Label: "min", InsertText: "min()", Detail: "min(iterable)", Kind: "func"},
	{Label: "abs", InsertText: "abs()", Detail: "abs(x)", Kind: "func"},
	{Label: "round", InsertText: "round()", Detail: "round(number)", Kind: "func"},
	{Label: "hasattr", InsertText: "hasattr()", Detail: "hasattr(obj, name)", Kind: "func"},
	{Label: "getattr", InsertText: "getattr()", Detail: "getattr(obj, name)", Kind: "func"},
	{Label: "setattr", InsertText: "setattr()", Detail: "setattr(obj, name, value)", Kind: "func"},
	{Label: "super", InsertText: "super()", Detail: "super()", Kind: "func"},
}

var pythonSnippets = []CompletionItemSimple{
	{Label: "defmain", InsertText: "def main():\n    ", Detail: "main function", Kind: "snippet"},
	{Label: "ifmain", InsertText: "if __name__ == \"__main__\":\n    main()", Detail: "if __name__ == \"__main__\"", Kind: "snippet"},
	{Label: "tryex", InsertText: "try:\n    \nexcept Exception as e:\n    ", Detail: "try/except block", Kind: "snippet"},
	{Label: "withopen", InsertText: "with open(filename, 'r') as f:\n    ", Detail: "with open file", Kind: "snippet"},
	{Label: "fori", InsertText: "for i in range():\n    ", Detail: "for i in range", Kind: "snippet"},
	{Label: "forin", InsertText: "for item in :\n    ", Detail: "for item in iterable", Kind: "snippet"},
	{Label: "listcomp", InsertText: "[x for x in ]", Detail: "list comprehension", Kind: "snippet"},
	{Label: "dictcomp", InsertText: "{k: v for k, v in }", Detail: "dict comprehension", Kind: "snippet"},
	{Label: "classdef", InsertText: "class ClassName:\n    def __init__(self):\n        ", Detail: "class definition", Kind: "snippet"},
	{Label: "property", InsertText: "@property\ndef (self):\n    return self._", Detail: "@property decorator", Kind: "snippet"},
	{Label: "staticmethod", InsertText: "@staticmethod\ndef ():\n    ", Detail: "@staticmethod", Kind: "snippet"},
	{Label: "classmethod", InsertText: "@classmethod\ndef (cls):\n    ", Detail: "@classmethod", Kind: "snippet"},
	{Label: "asyncdef", InsertText: "async def ():\n    ", Detail: "async function", Kind: "snippet"},
}

var pythonModuleCompletions = map[string][]CompletionItemSimple{
	"os": {
		{Label: "path", InsertText: "path", Detail: "os.path module", Kind: "module"},
		{Label: "getcwd", InsertText: "getcwd()", Detail: "get current directory", Kind: "func"},
		{Label: "listdir", InsertText: "listdir()", Detail: "list directory", Kind: "func"},
		{Label: "mkdir", InsertText: "mkdir()", Detail: "create directory", Kind: "func"},
		{Label: "makedirs", InsertText: "makedirs()", Detail: "create directories", Kind: "func"},
		{Label: "remove", InsertText: "remove()", Detail: "remove file", Kind: "func"},
		{Label: "rmdir", InsertText: "rmdir()", Detail: "remove directory", Kind: "func"},
		{Label: "rename", InsertText: "rename()", Detail: "rename file", Kind: "func"},
		{Label: "environ", InsertText: "environ", Detail: "environment variables", Kind: "variable"},
		{Label: "getenv", InsertText: "getenv()", Detail: "get environment variable", Kind: "func"},
	},
	"sys": {
		{Label: "argv", InsertText: "argv", Detail: "command line arguments", Kind: "variable"},
		{Label: "path", InsertText: "path", Detail: "module search path", Kind: "variable"},
		{Label: "exit", InsertText: "exit()", Detail: "exit program", Kind: "func"},
		{Label: "stdin", InsertText: "stdin", Detail: "standard input", Kind: "variable"},
		{Label: "stdout", InsertText: "stdout", Detail: "standard output", Kind: "variable"},
		{Label: "stderr", InsertText: "stderr", Detail: "standard error", Kind: "variable"},
		{Label: "version", InsertText: "version", Detail: "Python version", Kind: "variable"},
	},
	"json": {
		{Label: "dumps", InsertText: "dumps()", Detail: "serialize to JSON string", Kind: "func"},
		{Label: "loads", InsertText: "loads()", Detail: "parse JSON string", Kind: "func"},
		{Label: "dump", InsertText: "dump()", Detail: "serialize to file", Kind: "func"},
		{Label: "load", InsertText: "load()", Detail: "parse from file", Kind: "func"},
	},
	"re": {
		{Label: "match", InsertText: "match()", Detail: "match at beginning", Kind: "func"},
		{Label: "search", InsertText: "search()", Detail: "search pattern", Kind: "func"},
		{Label: "findall", InsertText: "findall()", Detail: "find all matches", Kind: "func"},
		{Label: "sub", InsertText: "sub()", Detail: "substitute pattern", Kind: "func"},
		{Label: "compile", InsertText: "compile()", Detail: "compile pattern", Kind: "func"},
		{Label: "split", InsertText: "split()", Detail: "split by pattern", Kind: "func"},
	},
	"datetime": {
		{Label: "datetime", InsertText: "datetime", Detail: "datetime class", Kind: "struct"},
		{Label: "date", InsertText: "date", Detail: "date class", Kind: "struct"},
		{Label: "time", InsertText: "time", Detail: "time class", Kind: "struct"},
		{Label: "timedelta", InsertText: "timedelta()", Detail: "time difference", Kind: "func"},
		{Label: "now", InsertText: "now()", Detail: "current datetime", Kind: "func"},
		{Label: "today", InsertText: "today()", Detail: "current date", Kind: "func"},
	},
	"collections": {
		{Label: "defaultdict", InsertText: "defaultdict()", Detail: "dict with default", Kind: "func"},
		{Label: "Counter", InsertText: "Counter()", Detail: "count elements", Kind: "func"},
		{Label: "deque", InsertText: "deque()", Detail: "double-ended queue", Kind: "func"},
		{Label: "namedtuple", InsertText: "namedtuple()", Detail: "named tuple", Kind: "func"},
		{Label: "OrderedDict", InsertText: "OrderedDict()", Detail: "ordered dictionary", Kind: "func"},
	},
	"itertools": {
		{Label: "chain", InsertText: "chain()", Detail: "chain iterables", Kind: "func"},
		{Label: "cycle", InsertText: "cycle()", Detail: "cycle iterator", Kind: "func"},
		{Label: "repeat", InsertText: "repeat()", Detail: "repeat value", Kind: "func"},
		{Label: "combinations", InsertText: "combinations()", Detail: "r-length combinations", Kind: "func"},
		{Label: "permutations", InsertText: "permutations()", Detail: "r-length permutations", Kind: "func"},
		{Label: "product", InsertText: "product()", Detail: "cartesian product", Kind: "func"},
	},
	"pathlib": {
		{Label: "Path", InsertText: "Path()", Detail: "Path object", Kind: "func"},
	},
	"typing": {
		{Label: "List", InsertText: "List", Detail: "List type hint", Kind: "struct"},
		{Label: "Dict", InsertText: "Dict", Detail: "Dict type hint", Kind: "struct"},
		{Label: "Optional", InsertText: "Optional", Detail: "Optional type hint", Kind: "struct"},
		{Label: "Union", InsertText: "Union", Detail: "Union type hint", Kind: "struct"},
		{Label: "Tuple", InsertText: "Tuple", Detail: "Tuple type hint", Kind: "struct"},
		{Label: "Callable", InsertText: "Callable", Detail: "Callable type hint", Kind: "struct"},
		{Label: "Any", InsertText: "Any", Detail: "Any type hint", Kind: "struct"},
	},
	"functools": {
		{Label: "partial", InsertText: "partial()", Detail: "partial function", Kind: "func"},
		{Label: "reduce", InsertText: "reduce()", Detail: "reduce iterable", Kind: "func"},
		{Label: "lru_cache", InsertText: "lru_cache()", Detail: "LRU cache decorator", Kind: "func"},
		{Label: "wraps", InsertText: "wraps()", Detail: "wrapper decorator", Kind: "func"},
	},
	"random": {
		{Label: "random", InsertText: "random()", Detail: "random float [0, 1)", Kind: "func"},
		{Label: "randint", InsertText: "randint()", Detail: "random integer", Kind: "func"},
		{Label: "choice", InsertText: "choice()", Detail: "random choice", Kind: "func"},
		{Label: "shuffle", InsertText: "shuffle()", Detail: "shuffle list", Kind: "func"},
		{Label: "sample", InsertText: "sample()", Detail: "random sample", Kind: "func"},
	},
	"math": {
		{Label: "sqrt", InsertText: "sqrt()", Detail: "square root", Kind: "func"},
		{Label: "pow", InsertText: "pow()", Detail: "power", Kind: "func"},
		{Label: "floor", InsertText: "floor()", Detail: "floor division", Kind: "func"},
		{Label: "ceil", InsertText: "ceil()", Detail: "ceiling", Kind: "func"},
		{Label: "sin", InsertText: "sin()", Detail: "sine", Kind: "func"},
		{Label: "cos", InsertText: "cos()", Detail: "cosine", Kind: "func"},
		{Label: "pi", InsertText: "pi", Detail: "pi constant", Kind: "variable"},
		{Label: "e", InsertText: "e", Detail: "e constant", Kind: "variable"},
	},
	"self": {
		// Common instance attributes/methods patterns
		{Label: "__init__", InsertText: "__init__", Detail: "constructor", Kind: "func"},
		{Label: "__str__", InsertText: "__str__", Detail: "string representation", Kind: "func"},
		{Label: "__repr__", InsertText: "__repr__", Detail: "repr representation", Kind: "func"},
	},
}

// =============================================================================
// JavaScript/TypeScript 语言补全数据
// =============================================================================

var jsKeywords = []CompletionItemSimple{
	{Label: "const", InsertText: "const ", Detail: "constant declaration", Kind: "keyword"},
	{Label: "let", InsertText: "let ", Detail: "variable declaration", Kind: "keyword"},
	{Label: "var", InsertText: "var ", Detail: "variable declaration (legacy)", Kind: "keyword"},
	{Label: "function", InsertText: "function ", Detail: "function declaration", Kind: "keyword"},
	{Label: "class", InsertText: "class ", Detail: "class declaration", Kind: "keyword"},
	{Label: "if", InsertText: "if (", Detail: "if statement", Kind: "keyword"},
	{Label: "else", InsertText: "else ", Detail: "else clause", Kind: "keyword"},
	{Label: "for", InsertText: "for (", Detail: "for loop", Kind: "keyword"},
	{Label: "while", InsertText: "while (", Detail: "while loop", Kind: "keyword"},
	{Label: "do", InsertText: "do {", Detail: "do-while loop", Kind: "keyword"},
	{Label: "switch", InsertText: "switch (", Detail: "switch statement", Kind: "keyword"},
	{Label: "case", InsertText: "case ", Detail: "case clause", Kind: "keyword"},
	{Label: "default", InsertText: "default:", Detail: "default clause", Kind: "keyword"},
	{Label: "break", InsertText: "break;", Detail: "break statement", Kind: "keyword"},
	{Label: "continue", InsertText: "continue;", Detail: "continue statement", Kind: "keyword"},
	{Label: "return", InsertText: "return ", Detail: "return statement", Kind: "keyword"},
	{Label: "try", InsertText: "try {", Detail: "try block", Kind: "keyword"},
	{Label: "catch", InsertText: "catch (", Detail: "catch clause", Kind: "keyword"},
	{Label: "finally", InsertText: "finally {", Detail: "finally clause", Kind: "keyword"},
	{Label: "throw", InsertText: "throw ", Detail: "throw statement", Kind: "keyword"},
	{Label: "new", InsertText: "new ", Detail: "new operator", Kind: "keyword"},
	{Label: "this", InsertText: "this", Detail: "this reference", Kind: "keyword"},
	{Label: "super", InsertText: "super", Detail: "super reference", Kind: "keyword"},
	{Label: "extends", InsertText: "extends ", Detail: "class inheritance", Kind: "keyword"},
	{Label: "import", InsertText: "import ", Detail: "import statement", Kind: "keyword"},
	{Label: "export", InsertText: "export ", Detail: "export statement", Kind: "keyword"},
	{Label: "from", InsertText: "from ", Detail: "from clause", Kind: "keyword"},
	{Label: "async", InsertText: "async ", Detail: "async function", Kind: "keyword"},
	{Label: "await", InsertText: "await ", Detail: "await expression", Kind: "keyword"},
	{Label: "typeof", InsertText: "typeof ", Detail: "typeof operator", Kind: "keyword"},
	{Label: "instanceof", InsertText: "instanceof ", Detail: "instanceof operator", Kind: "keyword"},
	{Label: "true", InsertText: "true", Detail: "boolean true", Kind: "keyword"},
	{Label: "false", InsertText: "false", Detail: "boolean false", Kind: "keyword"},
	{Label: "null", InsertText: "null", Detail: "null value", Kind: "keyword"},
	{Label: "undefined", InsertText: "undefined", Detail: "undefined value", Kind: "keyword"},
}

var tsKeywords = append(jsKeywords, []CompletionItemSimple{
	{Label: "interface", InsertText: "interface ", Detail: "interface declaration", Kind: "keyword"},
	{Label: "type", InsertText: "type ", Detail: "type alias", Kind: "keyword"},
	{Label: "enum", InsertText: "enum ", Detail: "enum declaration", Kind: "keyword"},
	{Label: "implements", InsertText: "implements ", Detail: "implements clause", Kind: "keyword"},
	{Label: "public", InsertText: "public ", Detail: "public modifier", Kind: "keyword"},
	{Label: "private", InsertText: "private ", Detail: "private modifier", Kind: "keyword"},
	{Label: "protected", InsertText: "protected ", Detail: "protected modifier", Kind: "keyword"},
	{Label: "readonly", InsertText: "readonly ", Detail: "readonly modifier", Kind: "keyword"},
	{Label: "abstract", InsertText: "abstract ", Detail: "abstract modifier", Kind: "keyword"},
	{Label: "static", InsertText: "static ", Detail: "static modifier", Kind: "keyword"},
	{Label: "as", InsertText: "as ", Detail: "type assertion", Kind: "keyword"},
	{Label: "is", InsertText: "is ", Detail: "type guard", Kind: "keyword"},
	{Label: "keyof", InsertText: "keyof ", Detail: "keyof operator", Kind: "keyword"},
	{Label: "never", InsertText: "never", Detail: "never type", Kind: "keyword"},
	{Label: "unknown", InsertText: "unknown", Detail: "unknown type", Kind: "keyword"},
	{Label: "any", InsertText: "any", Detail: "any type", Kind: "keyword"},
	{Label: "void", InsertText: "void", Detail: "void type", Kind: "keyword"},
	{Label: "string", InsertText: "string", Detail: "string type", Kind: "keyword"},
	{Label: "number", InsertText: "number", Detail: "number type", Kind: "keyword"},
	{Label: "boolean", InsertText: "boolean", Detail: "boolean type", Kind: "keyword"},
	{Label: "object", InsertText: "object", Detail: "object type", Kind: "keyword"},
}...)

var jsSnippets = []CompletionItemSimple{
	{Label: "log", InsertText: "console.log()", Detail: "console.log", Kind: "snippet"},
	{Label: "arrow", InsertText: "() => {\n  \n}", Detail: "arrow function", Kind: "snippet"},
	{Label: "asyncfn", InsertText: "async function () {\n  \n}", Detail: "async function", Kind: "snippet"},
	{Label: "asyncarrow", InsertText: "async () => {\n  \n}", Detail: "async arrow function", Kind: "snippet"},
	{Label: "fori", InsertText: "for (let i = 0; i < ; i++) {\n  \n}", Detail: "for i loop", Kind: "snippet"},
	{Label: "forof", InsertText: "for (const item of ) {\n  \n}", Detail: "for...of loop", Kind: "snippet"},
	{Label: "forin", InsertText: "for (const key in ) {\n  \n}", Detail: "for...in loop", Kind: "snippet"},
	{Label: "foreach", InsertText: ".forEach((item) => {\n  \n})", Detail: "forEach loop", Kind: "snippet"},
	{Label: "map", InsertText: ".map((item) => )", Detail: "array map", Kind: "snippet"},
	{Label: "filter", InsertText: ".filter((item) => )", Detail: "array filter", Kind: "snippet"},
	{Label: "reduce", InsertText: ".reduce((acc, item) => , initial)", Detail: "array reduce", Kind: "snippet"},
	{Label: "trycatch", InsertText: "try {\n  \n} catch (error) {\n  \n}", Detail: "try/catch block", Kind: "snippet"},
	{Label: "promise", InsertText: "new Promise((resolve, reject) => {\n  \n})", Detail: "new Promise", Kind: "snippet"},
	{Label: "import", InsertText: "import {  } from '';", Detail: "import statement", Kind: "snippet"},
	{Label: "export", InsertText: "export { }", Detail: "export statement", Kind: "snippet"},
	{Label: "exportdefault", InsertText: "export default ", Detail: "export default", Kind: "snippet"},
	{Label: "classdef", InsertText: "class  {\n  constructor() {\n    \n  }\n}", Detail: "class definition", Kind: "snippet"},
	{Label: "ternary", InsertText: " ?  : ", Detail: "ternary operator", Kind: "snippet"},
	{Label: "destruct", InsertText: "const {  } = ", Detail: "destructuring", Kind: "snippet"},
	{Label: "spread", InsertText: "...[]", Detail: "spread operator", Kind: "snippet"},
}

var tsSnippets = append(jsSnippets, []CompletionItemSimple{
	{Label: "interface", InsertText: "interface  {\n  \n}", Detail: "interface definition", Kind: "snippet"},
	{Label: "typeAlias", InsertText: "type  = ", Detail: "type alias", Kind: "snippet"},
	{Label: "enum", InsertText: "enum  {\n  \n}", Detail: "enum definition", Kind: "snippet"},
	{Label: "generic", InsertText: "<T>", Detail: "generic type parameter", Kind: "snippet"},
}...)

var jsModuleCompletions = map[string][]CompletionItemSimple{
	"console": {
		{Label: "log", InsertText: "log()", Detail: "log message", Kind: "func"},
		{Label: "error", InsertText: "error()", Detail: "log error", Kind: "func"},
		{Label: "warn", InsertText: "warn()", Detail: "log warning", Kind: "func"},
		{Label: "info", InsertText: "info()", Detail: "log info", Kind: "func"},
		{Label: "debug", InsertText: "debug()", Detail: "log debug", Kind: "func"},
		{Label: "table", InsertText: "table()", Detail: "log as table", Kind: "func"},
		{Label: "time", InsertText: "time()", Detail: "start timer", Kind: "func"},
		{Label: "timeEnd", InsertText: "timeEnd()", Detail: "end timer", Kind: "func"},
		{Label: "clear", InsertText: "clear()", Detail: "clear console", Kind: "func"},
		{Label: "dir", InsertText: "dir()", Detail: "display object", Kind: "func"},
	},
	"Math": {
		{Label: "floor", InsertText: "floor()", Detail: "round down", Kind: "func"},
		{Label: "ceil", InsertText: "ceil()", Detail: "round up", Kind: "func"},
		{Label: "round", InsertText: "round()", Detail: "round", Kind: "func"},
		{Label: "random", InsertText: "random()", Detail: "random number", Kind: "func"},
		{Label: "max", InsertText: "max()", Detail: "maximum value", Kind: "func"},
		{Label: "min", InsertText: "min()", Detail: "minimum value", Kind: "func"},
		{Label: "abs", InsertText: "abs()", Detail: "absolute value", Kind: "func"},
		{Label: "sqrt", InsertText: "sqrt()", Detail: "square root", Kind: "func"},
		{Label: "pow", InsertText: "pow()", Detail: "power", Kind: "func"},
		{Label: "PI", InsertText: "PI", Detail: "pi constant", Kind: "variable"},
		{Label: "E", InsertText: "E", Detail: "e constant", Kind: "variable"},
	},
	"JSON": {
		{Label: "parse", InsertText: "parse()", Detail: "parse JSON string", Kind: "func"},
		{Label: "stringify", InsertText: "stringify()", Detail: "convert to JSON", Kind: "func"},
	},
	"Object": {
		{Label: "keys", InsertText: "keys()", Detail: "get object keys", Kind: "func"},
		{Label: "values", InsertText: "values()", Detail: "get object values", Kind: "func"},
		{Label: "entries", InsertText: "entries()", Detail: "get key-value pairs", Kind: "func"},
		{Label: "assign", InsertText: "assign()", Detail: "merge objects", Kind: "func"},
		{Label: "freeze", InsertText: "freeze()", Detail: "freeze object", Kind: "func"},
		{Label: "fromEntries", InsertText: "fromEntries()", Detail: "object from entries", Kind: "func"},
	},
	"Array": {
		{Label: "isArray", InsertText: "isArray()", Detail: "check if array", Kind: "func"},
		{Label: "from", InsertText: "from()", Detail: "create array", Kind: "func"},
		{Label: "of", InsertText: "of()", Detail: "create from args", Kind: "func"},
	},
	"Promise": {
		{Label: "all", InsertText: "all()", Detail: "wait for all", Kind: "func"},
		{Label: "race", InsertText: "race()", Detail: "wait for first", Kind: "func"},
		{Label: "resolve", InsertText: "resolve()", Detail: "create resolved", Kind: "func"},
		{Label: "reject", InsertText: "reject()", Detail: "create rejected", Kind: "func"},
		{Label: "allSettled", InsertText: "allSettled()", Detail: "wait for all settled", Kind: "func"},
		{Label: "any", InsertText: "any()", Detail: "wait for any", Kind: "func"},
	},
	"Date": {
		{Label: "now", InsertText: "now()", Detail: "current timestamp", Kind: "func"},
		{Label: "parse", InsertText: "parse()", Detail: "parse date string", Kind: "func"},
	},
	"String": {
		{Label: "fromCharCode", InsertText: "fromCharCode()", Detail: "char from code", Kind: "func"},
		{Label: "fromCodePoint", InsertText: "fromCodePoint()", Detail: "char from code point", Kind: "func"},
	},
	"Number": {
		{Label: "parseInt", InsertText: "parseInt()", Detail: "parse integer", Kind: "func"},
		{Label: "parseFloat", InsertText: "parseFloat()", Detail: "parse float", Kind: "func"},
		{Label: "isNaN", InsertText: "isNaN()", Detail: "check if NaN", Kind: "func"},
		{Label: "isFinite", InsertText: "isFinite()", Detail: "check if finite", Kind: "func"},
		{Label: "isInteger", InsertText: "isInteger()", Detail: "check if integer", Kind: "func"},
	},
	"document": {
		{Label: "getElementById", InsertText: "getElementById()", Detail: "get element by ID", Kind: "func"},
		{Label: "querySelector", InsertText: "querySelector()", Detail: "query selector", Kind: "func"},
		{Label: "querySelectorAll", InsertText: "querySelectorAll()", Detail: "query all", Kind: "func"},
		{Label: "createElement", InsertText: "createElement()", Detail: "create element", Kind: "func"},
		{Label: "createTextNode", InsertText: "createTextNode()", Detail: "create text node", Kind: "func"},
		{Label: "addEventListener", InsertText: "addEventListener()", Detail: "add event listener", Kind: "func"},
		{Label: "body", InsertText: "body", Detail: "document body", Kind: "variable"},
		{Label: "head", InsertText: "head", Detail: "document head", Kind: "variable"},
	},
	"window": {
		{Label: "alert", InsertText: "alert()", Detail: "show alert", Kind: "func"},
		{Label: "confirm", InsertText: "confirm()", Detail: "show confirm", Kind: "func"},
		{Label: "prompt", InsertText: "prompt()", Detail: "show prompt", Kind: "func"},
		{Label: "setTimeout", InsertText: "setTimeout()", Detail: "set timeout", Kind: "func"},
		{Label: "setInterval", InsertText: "setInterval()", Detail: "set interval", Kind: "func"},
		{Label: "clearTimeout", InsertText: "clearTimeout()", Detail: "clear timeout", Kind: "func"},
		{Label: "clearInterval", InsertText: "clearInterval()", Detail: "clear interval", Kind: "func"},
		{Label: "fetch", InsertText: "fetch()", Detail: "fetch API", Kind: "func"},
		{Label: "location", InsertText: "location", Detail: "window location", Kind: "variable"},
		{Label: "localStorage", InsertText: "localStorage", Detail: "local storage", Kind: "variable"},
		{Label: "sessionStorage", InsertText: "sessionStorage", Detail: "session storage", Kind: "variable"},
	},
	"localStorage": {
		{Label: "getItem", InsertText: "getItem()", Detail: "get item", Kind: "func"},
		{Label: "setItem", InsertText: "setItem()", Detail: "set item", Kind: "func"},
		{Label: "removeItem", InsertText: "removeItem()", Detail: "remove item", Kind: "func"},
		{Label: "clear", InsertText: "clear()", Detail: "clear storage", Kind: "func"},
	},
	"fs": {
		{Label: "readFileSync", InsertText: "readFileSync()", Detail: "read file sync", Kind: "func"},
		{Label: "writeFileSync", InsertText: "writeFileSync()", Detail: "write file sync", Kind: "func"},
		{Label: "readFile", InsertText: "readFile()", Detail: "read file async", Kind: "func"},
		{Label: "writeFile", InsertText: "writeFile()", Detail: "write file async", Kind: "func"},
		{Label: "existsSync", InsertText: "existsSync()", Detail: "check if exists", Kind: "func"},
		{Label: "mkdirSync", InsertText: "mkdirSync()", Detail: "create directory", Kind: "func"},
		{Label: "readdirSync", InsertText: "readdirSync()", Detail: "read directory", Kind: "func"},
		{Label: "unlinkSync", InsertText: "unlinkSync()", Detail: "delete file", Kind: "func"},
	},
	"path": {
		{Label: "join", InsertText: "join()", Detail: "join paths", Kind: "func"},
		{Label: "resolve", InsertText: "resolve()", Detail: "resolve path", Kind: "func"},
		{Label: "dirname", InsertText: "dirname()", Detail: "directory name", Kind: "func"},
		{Label: "basename", InsertText: "basename()", Detail: "base name", Kind: "func"},
		{Label: "extname", InsertText: "extname()", Detail: "extension", Kind: "func"},
	},
	"process": {
		{Label: "env", InsertText: "env", Detail: "environment variables", Kind: "variable"},
		{Label: "argv", InsertText: "argv", Detail: "command arguments", Kind: "variable"},
		{Label: "cwd", InsertText: "cwd()", Detail: "current directory", Kind: "func"},
		{Label: "exit", InsertText: "exit()", Detail: "exit process", Kind: "func"},
		{Label: "nextTick", InsertText: "nextTick()", Detail: "next tick", Kind: "func"},
	},
}

// =============================================================================
// Rust 语言补全数据
// =============================================================================

var rustKeywords = []CompletionItemSimple{
	{Label: "fn", InsertText: "fn ", Detail: "function definition", Kind: "keyword"},
	{Label: "let", InsertText: "let ", Detail: "variable binding", Kind: "keyword"},
	{Label: "mut", InsertText: "mut ", Detail: "mutable binding", Kind: "keyword"},
	{Label: "const", InsertText: "const ", Detail: "constant", Kind: "keyword"},
	{Label: "static", InsertText: "static ", Detail: "static variable", Kind: "keyword"},
	{Label: "struct", InsertText: "struct ", Detail: "struct definition", Kind: "keyword"},
	{Label: "enum", InsertText: "enum ", Detail: "enum definition", Kind: "keyword"},
	{Label: "impl", InsertText: "impl ", Detail: "implementation", Kind: "keyword"},
	{Label: "trait", InsertText: "trait ", Detail: "trait definition", Kind: "keyword"},
	{Label: "type", InsertText: "type ", Detail: "type alias", Kind: "keyword"},
	{Label: "mod", InsertText: "mod ", Detail: "module", Kind: "keyword"},
	{Label: "use", InsertText: "use ", Detail: "use statement", Kind: "keyword"},
	{Label: "pub", InsertText: "pub ", Detail: "public visibility", Kind: "keyword"},
	{Label: "crate", InsertText: "crate", Detail: "crate root", Kind: "keyword"},
	{Label: "self", InsertText: "self", Detail: "self reference", Kind: "keyword"},
	{Label: "super", InsertText: "super", Detail: "parent module", Kind: "keyword"},
	{Label: "if", InsertText: "if ", Detail: "if expression", Kind: "keyword"},
	{Label: "else", InsertText: "else ", Detail: "else clause", Kind: "keyword"},
	{Label: "match", InsertText: "match ", Detail: "match expression", Kind: "keyword"},
	{Label: "for", InsertText: "for ", Detail: "for loop", Kind: "keyword"},
	{Label: "while", InsertText: "while ", Detail: "while loop", Kind: "keyword"},
	{Label: "loop", InsertText: "loop ", Detail: "infinite loop", Kind: "keyword"},
	{Label: "break", InsertText: "break", Detail: "break statement", Kind: "keyword"},
	{Label: "continue", InsertText: "continue", Detail: "continue statement", Kind: "keyword"},
	{Label: "return", InsertText: "return ", Detail: "return statement", Kind: "keyword"},
	{Label: "where", InsertText: "where ", Detail: "where clause", Kind: "keyword"},
	{Label: "async", InsertText: "async ", Detail: "async function", Kind: "keyword"},
	{Label: "await", InsertText: "await", Detail: "await expression", Kind: "keyword"},
	{Label: "move", InsertText: "move ", Detail: "move closure", Kind: "keyword"},
	{Label: "ref", InsertText: "ref ", Detail: "reference pattern", Kind: "keyword"},
	{Label: "unsafe", InsertText: "unsafe ", Detail: "unsafe block", Kind: "keyword"},
	{Label: "true", InsertText: "true", Detail: "boolean true", Kind: "keyword"},
	{Label: "false", InsertText: "false", Detail: "boolean false", Kind: "keyword"},
	{Label: "Some", InsertText: "Some()", Detail: "Option::Some", Kind: "func"},
	{Label: "None", InsertText: "None", Detail: "Option::None", Kind: "keyword"},
	{Label: "Ok", InsertText: "Ok()", Detail: "Result::Ok", Kind: "func"},
	{Label: "Err", InsertText: "Err()", Detail: "Result::Err", Kind: "func"},
	// 常用宏
	{Label: "println!", InsertText: "println!()", Detail: "print with newline", Kind: "func"},
	{Label: "print!", InsertText: "print!()", Detail: "print", Kind: "func"},
	{Label: "format!", InsertText: "format!()", Detail: "format string", Kind: "func"},
	{Label: "vec!", InsertText: "vec![]", Detail: "create vector", Kind: "func"},
	{Label: "panic!", InsertText: "panic!()", Detail: "panic macro", Kind: "func"},
	{Label: "assert!", InsertText: "assert!()", Detail: "assertion", Kind: "func"},
	{Label: "assert_eq!", InsertText: "assert_eq!()", Detail: "equality assertion", Kind: "func"},
	{Label: "dbg!", InsertText: "dbg!()", Detail: "debug print", Kind: "func"},
	{Label: "todo!", InsertText: "todo!()", Detail: "todo placeholder", Kind: "func"},
	{Label: "unimplemented!", InsertText: "unimplemented!()", Detail: "unimplemented", Kind: "func"},
}

var rustSnippets = []CompletionItemSimple{
	{Label: "fnmain", InsertText: "fn main() {\n    \n}", Detail: "main function", Kind: "snippet"},
	{Label: "fntest", InsertText: "#[test]\nfn test_() {\n    \n}", Detail: "test function", Kind: "snippet"},
	{Label: "impl", InsertText: "impl  {\n    \n}", Detail: "implementation block", Kind: "snippet"},
	{Label: "implfor", InsertText: "impl  for  {\n    \n}", Detail: "trait implementation", Kind: "snippet"},
	{Label: "structdef", InsertText: "struct  {\n    \n}", Detail: "struct definition", Kind: "snippet"},
	{Label: "enumdef", InsertText: "enum  {\n    \n}", Detail: "enum definition", Kind: "snippet"},
	{Label: "matchexpr", InsertText: "match  {\n    _ => \n}", Detail: "match expression", Kind: "snippet"},
	{Label: "iflet", InsertText: "if let Some() =  {\n    \n}", Detail: "if let Some", Kind: "snippet"},
	{Label: "whilelet", InsertText: "while let Some() =  {\n    \n}", Detail: "while let Some", Kind: "snippet"},
	{Label: "derive", InsertText: "#[derive()]", Detail: "derive macro", Kind: "snippet"},
	{Label: "closure", InsertText: "|x| ", Detail: "closure", Kind: "snippet"},
	{Label: "asyncfn", InsertText: "async fn () {\n    \n}", Detail: "async function", Kind: "snippet"},
}

var rustModuleCompletions = map[string][]CompletionItemSimple{
	"Vec": {
		{Label: "new", InsertText: "new()", Detail: "create empty Vec", Kind: "func"},
		{Label: "with_capacity", InsertText: "with_capacity()", Detail: "create with capacity", Kind: "func"},
		{Label: "push", InsertText: "push()", Detail: "add element", Kind: "func"},
		{Label: "pop", InsertText: "pop()", Detail: "remove last element", Kind: "func"},
		{Label: "len", InsertText: "len()", Detail: "get length", Kind: "func"},
		{Label: "is_empty", InsertText: "is_empty()", Detail: "check if empty", Kind: "func"},
		{Label: "iter", InsertText: "iter()", Detail: "get iterator", Kind: "func"},
		{Label: "iter_mut", InsertText: "iter_mut()", Detail: "get mutable iterator", Kind: "func"},
	},
	"String": {
		{Label: "new", InsertText: "new()", Detail: "create empty String", Kind: "func"},
		{Label: "from", InsertText: "from()", Detail: "create from str", Kind: "func"},
		{Label: "push_str", InsertText: "push_str()", Detail: "append string", Kind: "func"},
		{Label: "push", InsertText: "push()", Detail: "append char", Kind: "func"},
		{Label: "len", InsertText: "len()", Detail: "get length", Kind: "func"},
		{Label: "is_empty", InsertText: "is_empty()", Detail: "check if empty", Kind: "func"},
		{Label: "chars", InsertText: "chars()", Detail: "get char iterator", Kind: "func"},
		{Label: "as_str", InsertText: "as_str()", Detail: "as string slice", Kind: "func"},
	},
	"Option": {
		{Label: "Some", InsertText: "Some()", Detail: "Some variant", Kind: "func"},
		{Label: "None", InsertText: "None", Detail: "None variant", Kind: "variable"},
		{Label: "unwrap", InsertText: "unwrap()", Detail: "unwrap value", Kind: "func"},
		{Label: "unwrap_or", InsertText: "unwrap_or()", Detail: "unwrap with default", Kind: "func"},
		{Label: "is_some", InsertText: "is_some()", Detail: "check if Some", Kind: "func"},
		{Label: "is_none", InsertText: "is_none()", Detail: "check if None", Kind: "func"},
		{Label: "map", InsertText: "map()", Detail: "transform value", Kind: "func"},
		{Label: "and_then", InsertText: "and_then()", Detail: "chain options", Kind: "func"},
	},
	"Result": {
		{Label: "Ok", InsertText: "Ok()", Detail: "Ok variant", Kind: "func"},
		{Label: "Err", InsertText: "Err()", Detail: "Err variant", Kind: "func"},
		{Label: "unwrap", InsertText: "unwrap()", Detail: "unwrap value", Kind: "func"},
		{Label: "expect", InsertText: "expect()", Detail: "unwrap with message", Kind: "func"},
		{Label: "is_ok", InsertText: "is_ok()", Detail: "check if Ok", Kind: "func"},
		{Label: "is_err", InsertText: "is_err()", Detail: "check if Err", Kind: "func"},
		{Label: "map", InsertText: "map()", Detail: "transform value", Kind: "func"},
		{Label: "map_err", InsertText: "map_err()", Detail: "transform error", Kind: "func"},
	},
	"HashMap": {
		{Label: "new", InsertText: "new()", Detail: "create empty HashMap", Kind: "func"},
		{Label: "insert", InsertText: "insert()", Detail: "insert key-value", Kind: "func"},
		{Label: "get", InsertText: "get()", Detail: "get value by key", Kind: "func"},
		{Label: "remove", InsertText: "remove()", Detail: "remove by key", Kind: "func"},
		{Label: "contains_key", InsertText: "contains_key()", Detail: "check if key exists", Kind: "func"},
		{Label: "keys", InsertText: "keys()", Detail: "get keys iterator", Kind: "func"},
		{Label: "values", InsertText: "values()", Detail: "get values iterator", Kind: "func"},
	},
	"fs": {
		{Label: "read_to_string", InsertText: "read_to_string()", Detail: "read file to String", Kind: "func"},
		{Label: "write", InsertText: "write()", Detail: "write to file", Kind: "func"},
		{Label: "read", InsertText: "read()", Detail: "read file to bytes", Kind: "func"},
		{Label: "create", InsertText: "create()", Detail: "create file", Kind: "func"},
		{Label: "remove_file", InsertText: "remove_file()", Detail: "delete file", Kind: "func"},
		{Label: "create_dir", InsertText: "create_dir()", Detail: "create directory", Kind: "func"},
		{Label: "read_dir", InsertText: "read_dir()", Detail: "read directory", Kind: "func"},
	},
	"io": {
		{Label: "stdin", InsertText: "stdin()", Detail: "standard input", Kind: "func"},
		{Label: "stdout", InsertText: "stdout()", Detail: "standard output", Kind: "func"},
		{Label: "stderr", InsertText: "stderr()", Detail: "standard error", Kind: "func"},
		{Label: "Read", InsertText: "Read", Detail: "Read trait", Kind: "struct"},
		{Label: "Write", InsertText: "Write", Detail: "Write trait", Kind: "struct"},
	},
	"std": {
		{Label: "io", InsertText: "io", Detail: "I/O module", Kind: "module"},
		{Label: "fs", InsertText: "fs", Detail: "filesystem module", Kind: "module"},
		{Label: "env", InsertText: "env", Detail: "environment module", Kind: "module"},
		{Label: "collections", InsertText: "collections", Detail: "collections module", Kind: "module"},
		{Label: "thread", InsertText: "thread", Detail: "threading module", Kind: "module"},
		{Label: "sync", InsertText: "sync", Detail: "synchronization module", Kind: "module"},
	},
}

// =============================================================================
// C/C++ 语言补全数据
// =============================================================================

var cKeywords = []CompletionItemSimple{
	{Label: "int", InsertText: "int ", Detail: "integer type", Kind: "keyword"},
	{Label: "char", InsertText: "char ", Detail: "character type", Kind: "keyword"},
	{Label: "float", InsertText: "float ", Detail: "float type", Kind: "keyword"},
	{Label: "double", InsertText: "double ", Detail: "double type", Kind: "keyword"},
	{Label: "void", InsertText: "void ", Detail: "void type", Kind: "keyword"},
	{Label: "long", InsertText: "long ", Detail: "long type", Kind: "keyword"},
	{Label: "short", InsertText: "short ", Detail: "short type", Kind: "keyword"},
	{Label: "unsigned", InsertText: "unsigned ", Detail: "unsigned modifier", Kind: "keyword"},
	{Label: "signed", InsertText: "signed ", Detail: "signed modifier", Kind: "keyword"},
	{Label: "const", InsertText: "const ", Detail: "constant modifier", Kind: "keyword"},
	{Label: "static", InsertText: "static ", Detail: "static modifier", Kind: "keyword"},
	{Label: "extern", InsertText: "extern ", Detail: "extern modifier", Kind: "keyword"},
	{Label: "struct", InsertText: "struct ", Detail: "struct definition", Kind: "keyword"},
	{Label: "union", InsertText: "union ", Detail: "union definition", Kind: "keyword"},
	{Label: "enum", InsertText: "enum ", Detail: "enum definition", Kind: "keyword"},
	{Label: "typedef", InsertText: "typedef ", Detail: "type definition", Kind: "keyword"},
	{Label: "if", InsertText: "if (", Detail: "if statement", Kind: "keyword"},
	{Label: "else", InsertText: "else ", Detail: "else clause", Kind: "keyword"},
	{Label: "for", InsertText: "for (", Detail: "for loop", Kind: "keyword"},
	{Label: "while", InsertText: "while (", Detail: "while loop", Kind: "keyword"},
	{Label: "do", InsertText: "do {", Detail: "do-while loop", Kind: "keyword"},
	{Label: "switch", InsertText: "switch (", Detail: "switch statement", Kind: "keyword"},
	{Label: "case", InsertText: "case ", Detail: "case clause", Kind: "keyword"},
	{Label: "default", InsertText: "default:", Detail: "default clause", Kind: "keyword"},
	{Label: "break", InsertText: "break;", Detail: "break statement", Kind: "keyword"},
	{Label: "continue", InsertText: "continue;", Detail: "continue statement", Kind: "keyword"},
	{Label: "return", InsertText: "return ", Detail: "return statement", Kind: "keyword"},
	{Label: "sizeof", InsertText: "sizeof()", Detail: "sizeof operator", Kind: "func"},
	{Label: "NULL", InsertText: "NULL", Detail: "null pointer", Kind: "keyword"},
	{Label: "#include", InsertText: "#include ", Detail: "include directive", Kind: "keyword"},
	{Label: "#define", InsertText: "#define ", Detail: "define macro", Kind: "keyword"},
	{Label: "#ifdef", InsertText: "#ifdef ", Detail: "ifdef directive", Kind: "keyword"},
	{Label: "#ifndef", InsertText: "#ifndef ", Detail: "ifndef directive", Kind: "keyword"},
	{Label: "#endif", InsertText: "#endif", Detail: "endif directive", Kind: "keyword"},
	// 常用函数
	{Label: "printf", InsertText: "printf()", Detail: "print formatted", Kind: "func"},
	{Label: "scanf", InsertText: "scanf()", Detail: "scan formatted", Kind: "func"},
	{Label: "malloc", InsertText: "malloc()", Detail: "allocate memory", Kind: "func"},
	{Label: "free", InsertText: "free()", Detail: "free memory", Kind: "func"},
	{Label: "strlen", InsertText: "strlen()", Detail: "string length", Kind: "func"},
	{Label: "strcpy", InsertText: "strcpy()", Detail: "copy string", Kind: "func"},
	{Label: "strcmp", InsertText: "strcmp()", Detail: "compare strings", Kind: "func"},
	{Label: "memcpy", InsertText: "memcpy()", Detail: "copy memory", Kind: "func"},
	{Label: "memset", InsertText: "memset()", Detail: "set memory", Kind: "func"},
}

var cSnippets = []CompletionItemSimple{
	{Label: "main", InsertText: "int main(int argc, char *argv[]) {\n    \n    return 0;\n}", Detail: "main function", Kind: "snippet"},
	{Label: "fori", InsertText: "for (int i = 0; i < ; i++) {\n    \n}", Detail: "for i loop", Kind: "snippet"},
	{Label: "forj", InsertText: "for (int j = 0; j < ; j++) {\n    \n}", Detail: "for j loop", Kind: "snippet"},
	{Label: "whileloop", InsertText: "while () {\n    \n}", Detail: "while loop", Kind: "snippet"},
	{Label: "dowhile", InsertText: "do {\n    \n} while ();", Detail: "do-while loop", Kind: "snippet"},
	{Label: "structdef", InsertText: "struct  {\n    \n};", Detail: "struct definition", Kind: "snippet"},
	{Label: "ifdef", InsertText: "#ifdef \n\n#endif", Detail: "ifdef block", Kind: "snippet"},
	{Label: "ifndef", InsertText: "#ifndef \n#define \n\n#endif", Detail: "include guard", Kind: "snippet"},
	{Label: "includestdio", InsertText: "#include <stdio.h>", Detail: "include stdio.h", Kind: "snippet"},
	{Label: "includestdlib", InsertText: "#include <stdlib.h>", Detail: "include stdlib.h", Kind: "snippet"},
	{Label: "includestring", InsertText: "#include <string.h>", Detail: "include string.h", Kind: "snippet"},
}

// =============================================================================
// Java 语言补全数据
// =============================================================================

var javaKeywords = []CompletionItemSimple{
	{Label: "public", InsertText: "public ", Detail: "public modifier", Kind: "keyword"},
	{Label: "private", InsertText: "private ", Detail: "private modifier", Kind: "keyword"},
	{Label: "protected", InsertText: "protected ", Detail: "protected modifier", Kind: "keyword"},
	{Label: "static", InsertText: "static ", Detail: "static modifier", Kind: "keyword"},
	{Label: "final", InsertText: "final ", Detail: "final modifier", Kind: "keyword"},
	{Label: "abstract", InsertText: "abstract ", Detail: "abstract modifier", Kind: "keyword"},
	{Label: "class", InsertText: "class ", Detail: "class declaration", Kind: "keyword"},
	{Label: "interface", InsertText: "interface ", Detail: "interface declaration", Kind: "keyword"},
	{Label: "enum", InsertText: "enum ", Detail: "enum declaration", Kind: "keyword"},
	{Label: "extends", InsertText: "extends ", Detail: "extends clause", Kind: "keyword"},
	{Label: "implements", InsertText: "implements ", Detail: "implements clause", Kind: "keyword"},
	{Label: "new", InsertText: "new ", Detail: "new operator", Kind: "keyword"},
	{Label: "this", InsertText: "this", Detail: "this reference", Kind: "keyword"},
	{Label: "super", InsertText: "super", Detail: "super reference", Kind: "keyword"},
	{Label: "if", InsertText: "if (", Detail: "if statement", Kind: "keyword"},
	{Label: "else", InsertText: "else ", Detail: "else clause", Kind: "keyword"},
	{Label: "for", InsertText: "for (", Detail: "for loop", Kind: "keyword"},
	{Label: "while", InsertText: "while (", Detail: "while loop", Kind: "keyword"},
	{Label: "do", InsertText: "do {", Detail: "do-while loop", Kind: "keyword"},
	{Label: "switch", InsertText: "switch (", Detail: "switch statement", Kind: "keyword"},
	{Label: "case", InsertText: "case ", Detail: "case clause", Kind: "keyword"},
	{Label: "default", InsertText: "default:", Detail: "default clause", Kind: "keyword"},
	{Label: "break", InsertText: "break;", Detail: "break statement", Kind: "keyword"},
	{Label: "continue", InsertText: "continue;", Detail: "continue statement", Kind: "keyword"},
	{Label: "return", InsertText: "return ", Detail: "return statement", Kind: "keyword"},
	{Label: "try", InsertText: "try {", Detail: "try block", Kind: "keyword"},
	{Label: "catch", InsertText: "catch (", Detail: "catch clause", Kind: "keyword"},
	{Label: "finally", InsertText: "finally {", Detail: "finally clause", Kind: "keyword"},
	{Label: "throw", InsertText: "throw ", Detail: "throw statement", Kind: "keyword"},
	{Label: "throws", InsertText: "throws ", Detail: "throws clause", Kind: "keyword"},
	{Label: "import", InsertText: "import ", Detail: "import statement", Kind: "keyword"},
	{Label: "package", InsertText: "package ", Detail: "package declaration", Kind: "keyword"},
	{Label: "void", InsertText: "void", Detail: "void type", Kind: "keyword"},
	{Label: "int", InsertText: "int", Detail: "int type", Kind: "keyword"},
	{Label: "long", InsertText: "long", Detail: "long type", Kind: "keyword"},
	{Label: "double", InsertText: "double", Detail: "double type", Kind: "keyword"},
	{Label: "float", InsertText: "float", Detail: "float type", Kind: "keyword"},
	{Label: "boolean", InsertText: "boolean", Detail: "boolean type", Kind: "keyword"},
	{Label: "char", InsertText: "char", Detail: "char type", Kind: "keyword"},
	{Label: "byte", InsertText: "byte", Detail: "byte type", Kind: "keyword"},
	{Label: "short", InsertText: "short", Detail: "short type", Kind: "keyword"},
	{Label: "String", InsertText: "String", Detail: "String class", Kind: "struct"},
	{Label: "null", InsertText: "null", Detail: "null value", Kind: "keyword"},
	{Label: "true", InsertText: "true", Detail: "boolean true", Kind: "keyword"},
	{Label: "false", InsertText: "false", Detail: "boolean false", Kind: "keyword"},
}

var javaSnippets = []CompletionItemSimple{
	{Label: "main", InsertText: "public static void main(String[] args) {\n    \n}", Detail: "main method", Kind: "snippet"},
	{Label: "sout", InsertText: "System.out.println()", Detail: "print line", Kind: "snippet"},
	{Label: "soutf", InsertText: "System.out.printf()", Detail: "print formatted", Kind: "snippet"},
	{Label: "fori", InsertText: "for (int i = 0; i < ; i++) {\n    \n}", Detail: "for i loop", Kind: "snippet"},
	{Label: "foreach", InsertText: "for ( item : ) {\n    \n}", Detail: "for-each loop", Kind: "snippet"},
	{Label: "trycatch", InsertText: "try {\n    \n} catch (Exception e) {\n    e.printStackTrace();\n}", Detail: "try-catch block", Kind: "snippet"},
	{Label: "classdef", InsertText: "public class  {\n    \n}", Detail: "class definition", Kind: "snippet"},
	{Label: "interfacedef", InsertText: "public interface  {\n    \n}", Detail: "interface definition", Kind: "snippet"},
	{Label: "getter", InsertText: "public  get() {\n    return this.;\n}", Detail: "getter method", Kind: "snippet"},
	{Label: "setter", InsertText: "public void set( value) {\n    this. = value;\n}", Detail: "setter method", Kind: "snippet"},
}

var javaModuleCompletions = map[string][]CompletionItemSimple{
	"System": {
		{Label: "out", InsertText: "out", Detail: "standard output", Kind: "variable"},
		{Label: "err", InsertText: "err", Detail: "standard error", Kind: "variable"},
		{Label: "in", InsertText: "in", Detail: "standard input", Kind: "variable"},
		{Label: "exit", InsertText: "exit()", Detail: "exit program", Kind: "func"},
		{Label: "currentTimeMillis", InsertText: "currentTimeMillis()", Detail: "current time", Kind: "func"},
		{Label: "getenv", InsertText: "getenv()", Detail: "get environment", Kind: "func"},
	},
	"out": {
		{Label: "println", InsertText: "println()", Detail: "print line", Kind: "func"},
		{Label: "print", InsertText: "print()", Detail: "print", Kind: "func"},
		{Label: "printf", InsertText: "printf()", Detail: "print formatted", Kind: "func"},
	},
	"Arrays": {
		{Label: "sort", InsertText: "sort()", Detail: "sort array", Kind: "func"},
		{Label: "asList", InsertText: "asList()", Detail: "array to list", Kind: "func"},
		{Label: "binarySearch", InsertText: "binarySearch()", Detail: "binary search", Kind: "func"},
		{Label: "fill", InsertText: "fill()", Detail: "fill array", Kind: "func"},
		{Label: "copyOf", InsertText: "copyOf()", Detail: "copy array", Kind: "func"},
		{Label: "toString", InsertText: "toString()", Detail: "array to string", Kind: "func"},
	},
	"Collections": {
		{Label: "sort", InsertText: "sort()", Detail: "sort collection", Kind: "func"},
		{Label: "reverse", InsertText: "reverse()", Detail: "reverse list", Kind: "func"},
		{Label: "shuffle", InsertText: "shuffle()", Detail: "shuffle list", Kind: "func"},
		{Label: "max", InsertText: "max()", Detail: "find maximum", Kind: "func"},
		{Label: "min", InsertText: "min()", Detail: "find minimum", Kind: "func"},
		{Label: "emptyList", InsertText: "emptyList()", Detail: "empty list", Kind: "func"},
	},
	"Math": {
		{Label: "abs", InsertText: "abs()", Detail: "absolute value", Kind: "func"},
		{Label: "max", InsertText: "max()", Detail: "maximum", Kind: "func"},
		{Label: "min", InsertText: "min()", Detail: "minimum", Kind: "func"},
		{Label: "pow", InsertText: "pow()", Detail: "power", Kind: "func"},
		{Label: "sqrt", InsertText: "sqrt()", Detail: "square root", Kind: "func"},
		{Label: "random", InsertText: "random()", Detail: "random number", Kind: "func"},
		{Label: "floor", InsertText: "floor()", Detail: "floor", Kind: "func"},
		{Label: "ceil", InsertText: "ceil()", Detail: "ceiling", Kind: "func"},
		{Label: "round", InsertText: "round()", Detail: "round", Kind: "func"},
		{Label: "PI", InsertText: "PI", Detail: "pi constant", Kind: "variable"},
	},
}
