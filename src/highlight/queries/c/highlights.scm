;; C语言语法高亮规则

;; 关键字
[
  "if"
  "else"
  "switch"
  "case"
  "default"
  "break"
  "continue"
  "return"
  "while"
  "for"
  "do"
  "goto"
  "sizeof"
] @keyword

;; 类型关键字
[
  "void"
  "char"
  "short"
  "int"
  "long"
  "float"
  "double"
  "signed"
  "unsigned"
  "const"
  "volatile"
  "extern"
  "static"
  "auto"
  "register"
  "struct"
  "union"
  "enum"
  "typedef"
] @type

;; 预处理指令
[
  "#include"
  "#define"
  "#ifdef"
  "#ifndef"
  "#if"
  "#else"
  "#elif"
  "#endif"
  "#pragma"
  "#error"
  "#warning"
] @preproc

;; 字符串
(string_literal) @string
(system_lib_string) @string

;; 字符
(char_literal) @character

;; 数字
[
  (number_literal)
  (int_literal)
  (float_literal)
] @number

;; 布尔值
((identifier) @boolean
 (#match? @boolean "^(true|false)$"))

;; NULL值
((identifier) @constant.builtin
 (#eq? @constant.builtin "NULL"))

;; 注释
(comment) @comment

;; 函数定义
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))

;; 函数调用
(call_expression
  function: (identifier) @function.call)

;; 变量声明
(declaration
  declarator: (identifier) @variable)

;; 结构体字段
(field_declaration
  declarator: (field_identifier) @field)

;; 预处理器引入头文件
(preproc_include
  path: (system_lib_string) @include)
(preproc_include
  path: (string_literal) @include)

;; 参数
(parameter_declaration
  declarator: (identifier) @parameter)

;; 操作符
[
  "="
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "+"
  "-"
  "*"
  "/"
  "%"
  "+="
  "-="
  "*="
  "/="
  "%="
  "++"
  "--"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  "&&"
  "||"
  "!"
  "."
  "->"
] @operator

;; 分隔符
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  ";"
  ","
] @punctuation