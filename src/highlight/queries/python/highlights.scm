;; Keywords
"and" @keyword
"as" @keyword
"assert" @keyword
"async" @keyword
"await" @keyword
"break" @keyword
"class" @keyword
"continue" @keyword
"def" @keyword
"del" @keyword
"elif" @keyword
"else" @keyword
"except" @keyword
"finally" @keyword
"for" @keyword
"from" @keyword
"global" @keyword
"if" @keyword
"import" @keyword
"in" @keyword
"is" @keyword
"lambda" @keyword
"nonlocal" @keyword
"not" @keyword
"or" @keyword
"pass" @keyword
"raise" @keyword
"return" @keyword
"try" @keyword
"while" @keyword
"with" @keyword
"yield" @keyword

;; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function.call)
(call function: (attribute attribute: (identifier) @function.call))

;; Methods
(call function: (attribute object: (identifier) @variable
                          attribute: (identifier) @method.call))

;; Variables
(identifier) @variable

;; Parameters
(parameters (identifier) @variable.parameter)
(default_parameter name: (identifier) @variable.parameter)
(typed_parameter name: (identifier) @variable.parameter)

;; Properties and attributes
(attribute object: (identifier) @variable
           attribute: (identifier) @property)

;; Types
(type (identifier) @type)
((identifier) @type
 (#match? @type "^[A-Z]"))

;; Builtins
((identifier) @variable.builtin
 (#match? @variable.builtin "^(__.*__|self|cls)$"))

;; Comments
(comment) @comment

;; Strings
(string) @string
(formatted_string) @string

;; Numbers
(integer) @number
(float) @number

;; Boolean
(true) @constant.builtin
(false) @constant.builtin
(none) @constant.builtin

;; Operators
"+" @operator
"-" @operator
"*" @operator
"/" @operator
"%" @operator
"==" @operator
"!=" @operator
"<" @operator
">" @operator
"<=" @operator
">=" @operator
"=" @operator
"-=" @operator
"+=" @operator
"*=" @operator
"/=" @operator
"%=" @operator
"and" @operator
"or" @operator
"not" @operator
"in" @operator
"is" @operator

;; Punctuation
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
"," @punctuation.delimiter
"." @punctuation.delimiter
":" @punctuation.delimiter
";" @punctuation.delimiter

;; Decorators
(decorator "@" @attribute
           name: (identifier) @attribute)

;; Error
(ERROR) @error