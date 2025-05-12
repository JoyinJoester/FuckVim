;; Keywords
"as" @keyword
"async" @keyword
"await" @keyword
"break" @keyword
"const" @keyword
"continue" @keyword
"crate" @keyword
"dyn" @keyword
"else" @keyword
"enum" @keyword
"extern" @keyword
"fn" @keyword
"for" @keyword
"if" @keyword
"impl" @keyword
"in" @keyword
"let" @keyword
"loop" @keyword
"macro_rules!" @keyword
"match" @keyword
"mod" @keyword
"move" @keyword
"mut" @keyword
"pub" @keyword
"ref" @keyword
"return" @keyword
"self" @keyword
"static" @keyword
"struct" @keyword
"super" @keyword
"trait" @keyword
"type" @keyword
"union" @keyword
"unsafe" @keyword
"use" @keyword
"where" @keyword
"while" @keyword
"yield" @keyword

;; Attributes
(attribute_item) @attribute
(inner_attribute_item) @attribute

;; Operators
"!" @operator
"!=" @operator
"%" @operator
"%=" @operator
"&" @operator
"&&" @operator
"&=" @operator
"*" @operator
"*=" @operator
"+" @operator
"+=" @operator
"-" @operator
"-=" @operator
"->" @operator
"." @operator
".." @operator
"..=" @operator
"/" @operator
"/=" @operator
":" @operator
"::" @operator
";" @operator
"<<" @operator
"<<=" @operator
"=" @operator
"==" @operator
"=>" @operator
">" @operator
">=" @operator
">>" @operator
">>=" @operator
"@" @operator
"^" @operator
"^=" @operator
"|" @operator
"|=" @operator
"||" @operator

;; Delimiters
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket

;; Types
(type_identifier) @type
(primitive_type) @type.builtin
(field_type) @type

;; Functions
(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(call_expression function: (identifier) @function.call)
(call_expression function: (field_expression field: (field_identifier) @function.call))
(generic_function 
  function: (identifier) @function.call)
(generic_function 
  function: (field_expression field: (field_identifier) @function.call))
(macro_invocation macro: (identifier) @function.macro)

;; Variables
(identifier) @variable
(self) @variable.builtin
(shorthand_field_initializer (identifier) @variable)
(field_identifier) @property
(field_expression field: (field_identifier) @property)

;; Parameters
(parameter pattern: (identifier) @variable.parameter)
(closure_parameters (identifier) @variable.parameter)

;; Comments
(line_comment) @comment
(block_comment) @comment

;; Strings
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

;; Numbers
(integer_literal) @number
(float_literal) @number

;; Boolean
(boolean_literal) @constant.builtin

;; Lifetime
(lifetime (identifier) @label)

;; Special
((identifier) @variable.builtin
 (#match? @variable.builtin "^(Some|None|Ok|Err)$"))

;; Error
(ERROR) @error