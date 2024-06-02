# lelwel
[![Crates.io](https://img.shields.io/crates/v/lelwel)](https://crates.io/crates/lelwel)
[![MIT/Apache 2.0](https://img.shields.io/crates/l/lelwel)](./LICENSE-MIT)
[![Crates.io](https://img.shields.io/crates/d/lelwel)](https://crates.io/crates/lelwel)
[![Rust](https://img.shields.io/github/actions/workflow/status/0x2a-42/lelwel/rust.yml)](https://github.com/0x2a-42/lelwel/actions)
[![Playground](https://img.shields.io/badge/playground-8A2BE2)](https://0x2a-42.github.io/playground.html)

## Table of Contents
* [Introduction](#introduction)
* [Grammar Examples](#grammar-examples)
* [Quickstart](#quickstart)
* [Grammar Specification](#grammar-specification)
* [License](#license)

## Introduction

[Lelwel](https://en.wikipedia.org/wiki/Lelwel_hartebeest) (**L**anguage for **E**xtended **L**L(1) parsing **W**ith **E**rror resilience and **L**ossless syntax trees) generates recursive descent parsers for Rust using [LL(1) grammars](https://en.wikipedia.org/wiki/LL_grammar) with extensions for direct left recursion, operator precedence, semantic predicates (which also enable arbitrary lookahead), and semantic actions (which allow to deal with semantic context sensitivity, e.g. type / variable name ambiguity in C).

The parser creates a homogeneous, lossless, concrete syntax tree (CST) that can be used to construct an abstract syntax tree (AST).
Certain patterns are detected to avoid CST nodes for rules that only forward to other rules.
Bindings can be defined in regexes to rename the CST node for certain parses.

The error recovery and tree construction is inspired by Alex Kladov's (matklad) [Resilient LL Parsing Tutorial](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html).
Lelwel uses a (to my knowledge) novel heuristic to automatically calculate the recovery sets, by using the follow sets of the dominators in the directed graph induced by the grammar.

Lelwel is written as a library.
It is used by the CLI tool `llw`, the language server `lelwel-ls`, and can be included as a build dependency in order to be called from a `build.rs` file.
There is a plugin for [Neovim](https://github.com/0x2a-42/nvim-lelwel) that uses the language server.

By default the generated parser uses [Logos](https://github.com/maciejhirsz/logos) for lexing and [Codespan](https://github.com/brendanzab/codespan) for diagnostics, however this is not mandatory.

#### Why Yet Another Parser Generator?
* **Error Resilience:** The generated parser may provide similar error resilience as handwritten parsers.
* **Lossless Syntax Tree:** Language tooling such as language servers or formatters require all the information about the source code including whitespaces and comments.
* **Language Server:** Get instant feedback when your grammar contains conflicts or errors.
* **Easy to Debug:** The generated parser is easy to understand and can be debugged with standard tools.

#### Why LL(1) and not a more general CFL or PEG parser?
* **Error Resillience:** It seems to be the case that LL parsers are better suited than LR parsers for generating meaningful syntax trees from incomplete source code.
* **Runtime Complexity:** More general parsers such as GLR/GLL or ALL(*) can have a runtime complexity of $O(n^3)$ or $O(n^4)$ respectively for certain grammars. With LL(1) parsers you are guaranteed to have linear runtime complexity as long as your semantic actions and predicates have a constant runtime complexity.
* **Ambiguity:** The decision problem of whether an arbitrary context free grammar is ambiguous is undecidable. Warnings of a general parser generator therefore may contain false positives. In the worst case ambiguities may be found at runtime.
The PEG formalism just defines ambiguity away, which may cause the parser to parse a different language than you think.

## Grammar Examples
The [parser for lelwel grammar files](src/frontend/lelwel.llw) (\*.llw) is itself generated by lelwel.
There are also examples for [C without a preprocessor](examples/c/src/c.llw) (actually resolves ambiguity with semantic context information, unlike examples for ANTLR4 and Tree-sitter),  [Lua](examples/lua/src/lua.llw), [arithmetic expressions](examples/calc/src/calc.llw), [JSON](examples/json/src/json.llw), and [Oberon-0](examples/oberon0/src/oberon0.llw).

You can try out examples in the [Lelwel Playground](https://0x2a-42.github.io/playground.html).

The [following example](examples/l) shows a grammar for the toy language "L" introduced by the [Resilient LL Parsing Tutorial](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html#Introducing-L).

```antlr
token Fn='fn' Let='let' Return='return' True='true' False='false';
token Arrow='->' LPar='(' RPar=')' Comma=',' Colon=':' LBrace='{' RBrace='}'
      Semi=';' Asn='=' Plus='+' Minus='-' Star='*' Slash='/';
token Name='<name>' Int='<int>';
token Whitespace;

skip Whitespace;

start file;

file: fn*;
fn: 'fn' Name param_list ['->' type_expr] block;
param_list: '(' [param (?1 ',' param)* [',']] ')';
param: Name ':' type_expr;
type_expr: Name;
block: '{' stmt* '}';
stmt:
  stmt_expr
| stmt_let
| block
| stmt_return
;
stmt_expr: expr ';';
stmt_let: 'let' Name '=' expr ';';
stmt_return: 'return' [expr] ';';
expr: expr_bin;
expr_bin:
  expr_bin ('*' | '/') expr_bin
| expr_bin ('+' | '-') expr_bin
| expr_call
;
expr_call:
  expr_call arg_list
| expr_literal
| expr_name
| expr_paren
;
arg_list: '(' [expr (?1 ',' expr)* [',']] ')';
expr_literal: Int | 'true' | 'false';
expr_name: Name;
expr_paren: '(' expr ')';
```

## Quickstart
1. Write a grammar file and place it in the `src` directory of your crate.
   Optionally you can install the CLI or language server to validate your grammar file: `cargo install --features=cli,lsp lelwel`.
1. Add the following to your `Cargo.toml` and  `build.rs` files.
   ```toml
   [dependencies]
   logos = "0.14.0"
   codespan-reporting = "0.11.1"

   [build-dependencies]
   lelwel = "0.6.1"
   ```
   ```rust
   fn main() {
      lelwel::build("src/your_grammar.llw");
   }
   ```
1. Start a build. This will create a `parser.rs` file next to your grammar file.
   The `parser.rs` file is supposed to be manually edited to implement the lexer and it includes the actual parser `generated.rs`, which is written to the Cargo `OUT_DIR`.
   If you change the grammar after the `parser.rs` file has been generated, it may be required to manually update the `Token` enum or the `Parser` impl for semantic predicates and actions.
1. Use the parser module with the following minimal `main.rs` file for printing the CST and diagnostics.
   ```rust
   mod parser;

   use codespan_reporting::files::SimpleFile;
   use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
   use codespan_reporting::term::{self, Config};
   use logos::Logos;
   use parser::*;

   fn main() -> std::io::Result<()> {
       let args: Vec<String> = std::env::args().collect();
       if args.len() != 2 {
           std::process::exit(1);
       }
       let source = std::fs::read_to_string(&args[1])?;
       let mut diags = vec![];
       let (tokens, ranges) = tokenize(Token::lexer(&source), &mut diags);
       let cst = Parser::parse(&source, tokens, ranges, &mut diags);
       println!("{cst}");
       let writer = StandardStream::stderr(ColorChoice::Auto);
       let config = Config::default();
       let file = SimpleFile::new(&args[1], &source);
       for diag in diags.iter() {
           term::emit(&mut writer.lock(), &config, &file, diag).unwrap();
       }
       Ok(())
   }
   ```

## Grammar Specification

Lelwel grammars are based on the formalism of [context free grammars (CFG)](https://en.wikipedia.org/wiki/Context-free_grammar) and more specifically [LL(1) grammars](https://en.wikipedia.org/wiki/LL_grammar).
There are certain extensions to the classical grammar syntax such as constructs similar to those from EBNF.

A grammar file consists of top level definitions which are independent of their order.

### Token List
A token list definition introduces a list of tokens (terminals) to the grammar.
It starts with the `token` keyword, ends with a `;` and contains a list of token names and corresponding token symbols.

A token name must start with a capital letter.
The token symbol is optional and delimited by single quotation marks.
It is used in error messages and the generator of the `parser.rs` file.
In a regex a token can be referenced by its name or symbol.

> [!TIP]
> If the token symbol string starts with `<` and ends with `>`, the token is interpreted as a class of tokens for which the symbol is only a description.
> This influences how error messages and lexer rules are generated by default in `parser.rs`.

#### Example
```antlr
token MyKeyword='my_keyword' Int='<integer literal>' True='true' False='false';
```

### Rule
A grammar rule must start with a lower case letter.
A regular expression is used to specify the right hand side of the rule.

The following special regex patterns for rules are recognized.
- **Left Recursive:**

  A rule that has direct left recursion.
  The rule must consist of a top level alternation.
  There may be multiple recursive and non-recursive branches.
  A node in the syntax tree is only created if a recursive branch is taken.

  **Example:**
  ```antlr
  call_expr:
    call_expr arg_list
  | primary_expr
  ;
  ```
- **Operator Precedence:**

  A rule that parses binary expressions.
  The rule must consist of a top level alternation.
  Exactly one branch contains a reference to a different rule.
  All other branches must contain left and right recursive concatenations with 3 elements.
  The middle element of the concatenations must be a reference to one token or an alternation of tokens.
  The order of the top level alternation branch decides the operator precedence.
  A node in the syntax tree is only created if a recursive branch is taken.

  **Example:**
  ```antlr
  bin_expr:
    bin_expr ('*' | '/') bin_expr
  | bin_expr ('+' | '-') bin_expr
  | primary_expr
  ;
  ```
- **Unconditional Forwarding:**

  A rule that only forwards to other rules.
  The rule either consist of a single reference to another rule or a top level alternation where each branch references a single rule.
  No node in the syntax tree is created for such rules.

  **Example:**
  ```antlr
  stmt:
    for_stmt
  | while_stmt
  | expr_stmt
  | return_stmt
  ;
  ```
- **Conditional Forwarding:**

  A rule that consists of a concatenation where the first element is a reference to a rule and following elements may be empty.
  A node in the syntax tree is only created if the maybe empty elements are used.

  **Example:**
  ```antlr
  expr_list:
    expr (',' expr)*
  ;
  ```
- **Right Recursive Forwarding:**

  The rule must consist of a top level alternation with at least one right recursive branch and at least one branch that references a rule.
  A node in the syntax tree is only created if a right recursive branch is taken.

  **Example:**
  ```antlr
  unary_expr:
    ('+' | '-') unary_expr
  | primary_expr
  ;
  ```
- **Maybe Empty:**

  A rule that may derive to the empty word.
  A node is only created if the derivation is not empty.

  **Example:**
  ```antlr
  generic_args:
    ['<' type_list '>']
  ;
  ```

### Regular Expressions
Regular expressions are built from the following syntactic constructs.
- **Grouping**: `(...)`
- **Identifier**: `rule_name` or `TokenName`
- **Symbol**: `'token symbol'`
- **Concatenation**: `A B` which is `A` followed by `B`
- **Alternation**: `A | B` which is either `A` or `B`
- **Optional**: `[A]` which is either `A` or nothing
- **Star Repetition**: `A*` which is a repetition of 0 or more `A`
- **Plus Repetition**: `A+` which is a repetition of 1 or more `A`
- **Semantic Predicate**: `?1` which is the semantic predicate number 1
- **Semantic Action**: `#1` which is the semantic action number 1
- **Binding**: `@new_node_name` renames the syntax tree node
- **Node Marker**: `<1` marker with index 1 to create new node
- **Node Creation**: `1>new_node_name` insert node at position of marker with index 1

### Start
A `start` definition specifies the start rule of the grammar.
There must be exactly one start definition in a grammar.
The start rule must not be referenced in a regex.

#### Example
```antlr
start translation_unit;
```

### Skip
A `skip` definition allows to specify a list of tokens, which are ignored by the parser.
These tokens will however still be part of the syntax tree.
#### Example
```antlr
skip Whitespace Comment;
```

### Right
A `right` definition allows to specify a list of tokens, which are handled as right associative operators in operator precedence rules.
#### Example
```antlr
right '^' '=';
```

## License
Lelwel, its examples, and its generated code are licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
