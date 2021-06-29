mod imp;

use super::parser::TokenKind;
use super::token::{Position, Range, Token, TokenStream};

/// A transition in the lexer's state machine.
pub enum Transition {
    /// An end state was reached so the token can be generated.
    Done(Token),
    /// Go to the next state.
    /// This is useful for implementing back edges in the state machine
    /// as rust has no guaranteed tail call elimination.
    Next(fn(&mut Lexer) -> Transition),
}

enum EmitMode {
    Parser,
    Trivia,
    Invalid,
}

#[derive(Debug, Clone, Default)]
struct State {
    /// Information for the current offset.
    cursor: Cursor,
    /// Information for the current lexeme start.
    start: Start,
    /// The byte width of the last read character.
    width: usize,
    /// The current line in the file.
    line: u32,
}

#[derive(Debug, Clone, Default)]
struct Cursor {
    /// The current byte offset.
    byte: usize,
    /// The current unicode code point offset.
    character: u32,
}

#[derive(Debug, Clone, Default)]
struct Start {
    /// The byte offset for the start of current lexeme.
    byte: usize,
    /// The unicode code point offset for the start of current lexeme.
    character: u32,
    /// The unicode code point offset for the line of current lexeme.
    character_line: u32,
    /// The start position of the current lexeme.
    pos: Position,
}

/// A lexer that tokenizes a `String`.
#[derive(Debug, Default)]
pub struct Lexer {
    /// The input string.
    input: String,
    /// Write all tokens to a buffer if true.
    log: bool,
    /// The current state of the lexer.
    state: State,
    /// The last token that was generated.
    current: Token,
    /// Buffer used for peeking
    lookahead: std::collections::VecDeque<Token>,
    /// Buffer of all scanned tokens (except invalid ones).
    buffer: Vec<Token>,
    /// Buffer of all invalid tokens.
    invalid: Vec<Token>,
    /// The current trivia token.
    trivia: Option<Token>,
}

impl Lexer {
    /// Creates a new `Lexer` that tokenizes the given `String`.
    #[allow(dead_code)]
    pub fn new(input: String, log: bool) -> Lexer {
        Lexer {
            input,
            log,
            ..Default::default()
        }
    }

    /// Gets an iterator of invalid tokens.
    #[allow(dead_code)]
    pub fn invalid_iter(&self) -> std::slice::Iter<'_, Token> {
        self.invalid.iter()
    }

    /// Gets an iterator of buffered tokens.
    #[allow(dead_code)]
    pub fn buffer_iter(&self) -> std::slice::Iter<'_, Token> {
        self.buffer.iter()
    }

    /// Find the next token.
    fn tokenize(&mut self) {
        self.trivia = None;
        let mut trans = Transition::Next(Self::state_start);
        loop {
            match trans {
                Transition::Next(func) => {
                    trans = func(self);
                }
                Transition::Done(token) => {
                    self.current = token;
                    return;
                }
            }
        }
    }

    /// Consumes the next character from the input.
    fn consume(&mut self) -> Option<char> {
        if self.state.cursor.byte >= self.input.len() {
            self.state.width = 0;
            None
        } else {
            let current = self
                .input
                .get(self.state.cursor.byte..)
                .unwrap()
                .chars()
                .next();
            self.state.width = current.unwrap().len_utf8();
            self.state.cursor.byte += self.state.width;
            self.state.cursor.character += 1;
            current
        }
    }

    /// Ignores all read characters.
    #[allow(dead_code)]
    fn ignore(&mut self) -> Transition {
        self.state.start.byte = self.state.cursor.byte;
        self.state.start.character = self.state.cursor.character;
        self.state.start.pos = Position::new(
            self.state.line,
            self.state.start.character - self.state.start.character_line,
        );
        Transition::Next(Self::state_start)
    }

    /// Restores the previously read character.
    #[allow(dead_code)]
    fn backup(&mut self) {
        self.state.cursor.byte -= self.state.width;
        self.state.cursor.character -= 1;
    }

    /// Restores the character.
    #[allow(dead_code)]
    fn undo(&mut self, c: char) {
        self.state.cursor.byte -= c.len_utf8();
        self.state.cursor.character -= 1;
    }

    /// Gets the current lexeme.
    #[allow(dead_code)]
    fn get(&mut self, byte_start: usize, end: usize) -> &str {
        &self.input[self.state.start.byte + byte_start..self.state.cursor.byte - end]
    }

    /// Gets the last read character.
    #[allow(dead_code)]
    fn get_char(&mut self) -> Option<char> {
        self.input[self.state.cursor.byte - self.state.width..self.state.cursor.byte]
            .chars()
            .next()
    }

    /// Accepts the next character if it evaluates the predicate to true.
    #[allow(dead_code)]
    fn accept<F: FnOnce(char) -> bool>(&mut self, pred: F) -> bool {
        if let Some(c) = self.consume() {
            if pred(c) {
                return true;
            }
            self.backup();
        }
        false
    }

    /// Accepts the next character if it is contained in the valid slice.
    #[allow(dead_code)]
    fn accept_oneof(&mut self, valid: &str) -> bool {
        if let Some(c) = self.consume() {
            if valid.contains(c) {
                return true;
            }
            self.backup();
        }
        false
    }

    /// Accepts the next character if it is the valid character.
    #[allow(dead_code)]
    fn accept_char(&mut self, valid: char) -> bool {
        if let Some(c) = self.consume() {
            if c == valid {
                return true;
            }
            self.backup();
        }
        false
    }

    /// Accepts all characters until one is not contained in the valid slice.
    #[allow(dead_code)]
    fn accept_star<F: FnOnce(char) -> bool + Copy>(&mut self, pred: F) {
        while let Some(c) = self.consume() {
            if !pred(c) {
                self.backup();
                break;
            }
        }
    }

    /// Accepts all characters until one is not contained in the valid slice.
    #[allow(dead_code)]
    fn accept_plus<F: FnOnce(char) -> bool + Copy>(&mut self, pred: F) -> bool {
        if self.accept(pred) {
            self.accept_star(pred);
            true
        } else {
            false
        }
    }

    /// Accepts given number of characters.
    #[allow(dead_code)]
    fn accept_count<F: FnOnce(char) -> bool + Copy>(&mut self, pred: F, count: usize) -> bool {
        let cursor = self.state.cursor.clone();
        for _ in 0..count {
            if !self.accept(pred) {
                self.state.cursor = cursor;
                return false;
            }
        }
        true
    }

    /// Log the token in the token buffer.
    fn log(&mut self, token: &Token) {
        if self.log {
            self.buffer.push(token.clone());
        }
    }

    /// Finishes lexing and emits a token for the parser in the specified channel.
    #[allow(dead_code)]
    fn emit_with_mode(&mut self, kind: TokenKind, mode: EmitMode) -> Transition {
        let end = Position::new(
            self.state.line,
            self.state.cursor.character - self.state.start.character_line,
        );
        let token = Token::new(kind, Range::new(self.state.start.pos, end));
        self.state.start.byte = self.state.cursor.byte;
        self.state.start.character = self.state.cursor.character;
        self.state.start.pos = Position::new(
            self.state.line,
            self.state.start.character - self.state.start.character_line,
        );
        match mode {
            EmitMode::Parser => {
                self.log(&token);
                Transition::Done(token)
            }
            EmitMode::Trivia => {
                self.log(&token);
                self.trivia = Some(token);
                Transition::Next(Self::state_start)
            }
            EmitMode::Invalid => {
                self.invalid.push(token);
                Transition::Next(Self::state_start)
            }
        }
    }

    /// Finishes lexing and emits a token for the parser.
    #[allow(dead_code)]
    fn emit(&mut self, kind: TokenKind) -> Transition {
        self.emit_with_mode(kind, EmitMode::Parser)
    }

    /// Finishes lexing and continue in start state.
    #[allow(dead_code)]
    fn emit_trivia(&mut self, kind: TokenKind) -> Transition {
        self.emit_with_mode(kind, EmitMode::Trivia)
    }

    /// Finishes lexing, logs invalid token, and continue in start state.
    #[allow(dead_code)]
    fn emit_invalid(&mut self) -> Transition {
        self.emit_with_mode(TokenKind::Invalid, EmitMode::Invalid)
    }

    /// Continue lexing at the start state.
    #[allow(dead_code)]
    fn emit_continue(&mut self) -> Transition {
        self.ignore();
        Transition::Next(Self::state_start)
    }

    /// Advances to next line.
    #[allow(dead_code)]
    fn line(&mut self) {
        self.state.line += 1;
        self.state.start.character_line = self.state.cursor.character;
    }
}

impl TokenStream for Lexer {
    fn current(&self) -> Token {
        self.current.clone()
    }
    fn peek(&mut self, offset: usize) -> Token {
        let old_current = self.current.clone();
        while offset > self.lookahead.len() {
            self.tokenize();
            self.lookahead.push_back(self.current.clone());
        }
        self.current = old_current;
        self.lookahead[offset - 1].clone()
    }
    fn advance(&mut self) {
        if let Some(token) = self.lookahead.pop_front() {
            self.current = token;
        } else {
            self.tokenize();
        }
    }
    fn trivia(&mut self) -> Option<Token> {
        self.trivia.clone()
    }
    fn finalize(&mut self) {
        self.state.cursor.byte = self.input.len();
        let end = Position::new(
            self.state.line,
            self.state.cursor.character - self.state.start.character_line,
        );
        self.current = Token::new(TokenKind::EOF, Range::new(self.state.start.pos, end));
    }
}

impl Iterator for Lexer {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.tokenize();
        let tok = self.current();
        if tok.kind != TokenKind::EOF {
            Some(tok)
        } else {
            None
        }
    }
}
