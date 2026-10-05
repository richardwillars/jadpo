use jadpo_diagnostics::{Diagnostic, SourceSpan};
use std::path::{Path, PathBuf};

mod ast;
mod parser;

pub use ast::*;
pub use parser::{parse, ParseResult};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

impl SourceFile {
    pub fn new(path: PathBuf, text: String) -> Self {
        Self { path, text }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    Identifier,
    IntegerLiteral,
    DecimalLiteral,
    StringLiteral,
    BooleanLiteral,
    NoneLiteral,
    RoutePath,

    Type,
    Enum,
    Entity,
    Value,
    Input,
    Output,
    Failure,
    Function,
    Action,
    Fixture,
    Test,
    Route,
    Config,
    Module,
    Import,
    Persist,
    Code,
    Kind,
    Message,
    Public,
    Internal,
    Optional,
    Required,
    Many,
    Missing,
    Fails,
    Var,
    Mut,
    Return,
    Reject,
    Attempt,
    Call,
    Async,
    Await,
    If,
    Else,
    Match,
    Success,
    Propagate,
    Assert,
    Advance,
    And,
    Or,
    Not,
    Auth,
    Path,
    Headers,
    Explicitly,
    Run,
    Deadline,
    Min,
    Max,
    MinLength,
    MaxLength,
    Pattern,
    Format,
    Throw,
    Create,
    Query,
    Where,
    OrderBy,
    Asc,
    Desc,
    Update,
    Set,
    Empty,
    Conflict,
    Constraint,
    Identity,
    Unique,
    Index,
    References,
    As,
    OnDelete,
    Restrict,
    Cascade,
    SetNull,
    Inverse,
    Via,
    Include,
    Into,
    Limit,
    Offset,

    Get,
    Post,
    Put,
    Patch,
    Delete,

    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftAngle,
    RightAngle,
    Colon,
    Comma,
    Dot,
    Question,
    Equal,
    EqualEqual,
    BangEqual,
    LessEqual,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Arrow,
    FatArrow,

    Eof,
}

impl TokenKind {
    pub const fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::LineComment)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: TextRange,
}

impl Token {
    pub fn text(self, source: &str) -> &str {
        &source[self.range.start..self.range.end]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(source_name: &Path, source: &str) -> LexResult {
    Lexer::new(source_name, source).run()
}

struct Lexer<'source> {
    source_name: String,
    source: &'source str,
    offset: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'source> Lexer<'source> {
    fn new(source_name: &Path, source: &'source str) -> Self {
        Self {
            source_name: source_name.to_string_lossy().into_owned(),
            source,
            offset: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> LexResult {
        while self.offset < self.source.len() {
            let start = self.offset;
            let character = self.current_character().expect("offset is in bounds");

            match character {
                character if character.is_whitespace() => self.lex_whitespace(start),
                '/' if self.remaining().starts_with("//") => self.lex_comment(start),
                '/' if self.route_path_position() => self.lex_path(start),
                '/' => self.single(TokenKind::Slash, start),
                '"' => self.lex_string(start),
                '-' if self.remaining().starts_with("->") => {
                    self.offset += 2;
                    self.push(TokenKind::Arrow, start);
                }
                '-' if self
                    .next_character()
                    .is_some_and(|next| next.is_ascii_digit())
                    && !self.subtraction_position() =>
                {
                    self.lex_number(start)
                }
                '-' => self.single(TokenKind::Minus, start),
                '+' => self.single(TokenKind::Plus, start),
                '*' => self.single(TokenKind::Star, start),
                '%' => self.single(TokenKind::Percent, start),
                character if character.is_ascii_digit() => self.lex_number(start),
                character if is_identifier_start(character) => self.lex_identifier(start),
                '{' => self.single(TokenKind::LeftBrace, start),
                '}' => self.single(TokenKind::RightBrace, start),
                '(' => self.single(TokenKind::LeftParen, start),
                ')' => self.single(TokenKind::RightParen, start),
                '[' => self.single(TokenKind::LeftBracket, start),
                ']' => self.single(TokenKind::RightBracket, start),
                '<' if self.remaining().starts_with("<=") => {
                    self.offset += 2;
                    self.push(TokenKind::LessEqual, start);
                }
                '<' => self.single(TokenKind::LeftAngle, start),
                '>' if self.remaining().starts_with(">=") => {
                    self.offset += 2;
                    self.push(TokenKind::GreaterEqual, start);
                }
                '>' => self.single(TokenKind::RightAngle, start),
                ':' => self.single(TokenKind::Colon, start),
                ',' => self.single(TokenKind::Comma, start),
                '.' => self.single(TokenKind::Dot, start),
                '?' => self.single(TokenKind::Question, start),
                '=' if self.remaining().starts_with("==") => {
                    self.offset += 2;
                    self.push(TokenKind::EqualEqual, start);
                }
                '=' if self.remaining().starts_with("=>") => {
                    self.offset += 2;
                    self.push(TokenKind::FatArrow, start);
                }
                '=' => self.single(TokenKind::Equal, start),
                '!' if self.remaining().starts_with("!=") => {
                    self.offset += 2;
                    self.push(TokenKind::BangEqual, start);
                }
                _ => self.unexpected_character(start, character),
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            range: TextRange::new(self.source.len(), self.source.len()),
        });

        LexResult {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn lex_whitespace(&mut self, start: usize) {
        self.advance_while(char::is_whitespace);
        self.push(TokenKind::Whitespace, start);
    }

    fn lex_comment(&mut self, start: usize) {
        self.offset += 2;
        self.advance_while(|character| character != '\n');
        self.push(TokenKind::LineComment, start);
    }

    fn lex_path(&mut self, start: usize) {
        self.advance_while(|character| !character.is_whitespace());
        self.push(TokenKind::RoutePath, start);
    }

    fn route_path_position(&self) -> bool {
        self.tokens
            .iter()
            .rev()
            .find(|token| !token.kind.is_trivia())
            .is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Get
                        | TokenKind::Post
                        | TokenKind::Put
                        | TokenKind::Patch
                        | TokenKind::Delete
                )
            })
    }

    fn lex_string(&mut self, start: usize) {
        self.offset += 1;
        let mut terminated = false;

        while let Some(character) = self.current_character() {
            match character {
                '"' => {
                    self.offset += 1;
                    terminated = true;
                    break;
                }
                '\\' => {
                    let escape_start = self.offset;
                    self.offset += 1;
                    match self.current_character() {
                        Some('"' | '\\' | 'n' | 'r' | 't') => {
                            self.advance_character();
                        }
                        Some(_) => {
                            self.advance_character();
                            self.diagnostics.push(self.diagnostic(
                                "SYN_INVALID_ESCAPE",
                                escape_start,
                                self.offset,
                            ));
                        }
                        None => break,
                    }
                }
                '\n' | '\r' => break,
                _ => self.advance_character(),
            }
        }

        if !terminated {
            self.diagnostics
                .push(self.diagnostic("SYN_UNTERMINATED_STRING", start, self.offset));
        }

        self.push(TokenKind::StringLiteral, start);
    }

    fn lex_number(&mut self, start: usize) {
        if self.current_character() == Some('-') {
            self.offset += 1;
        }

        self.advance_while(|character| character.is_ascii_digit());
        let mut kind = TokenKind::IntegerLiteral;

        if self.current_character() == Some('.')
            && self
                .next_character()
                .is_some_and(|next| next.is_ascii_digit())
        {
            kind = TokenKind::DecimalLiteral;
            self.offset += 1;
            self.advance_while(|character| character.is_ascii_digit());
        }

        self.push(kind, start);
    }

    fn subtraction_position(&self) -> bool {
        let mut previous = self
            .tokens
            .iter()
            .rev()
            .filter(|token| !token.kind.is_trivia());
        let Some(token) = previous.next() else {
            return false;
        };
        // Retain signed literal tokens at value starts (including declaration
        // settings), but separate subtraction after an expression operand.
        // Ignore trivia so `value-2`, `value -2` and `value - 2` agree.
        matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Config
                | TokenKind::Input
                | TokenKind::Output
                | TokenKind::Value
                | TokenKind::Path
                | TokenKind::IntegerLiteral
                | TokenKind::DecimalLiteral
                | TokenKind::StringLiteral
                | TokenKind::BooleanLiteral
                | TokenKind::NoneLiteral
                | TokenKind::RightParen
                | TokenKind::RightBracket
                | TokenKind::RightBrace
        ) || previous
            .next()
            .is_some_and(|token| token.kind == TokenKind::Dot)
    }

    fn lex_identifier(&mut self, start: usize) {
        self.advance_while(is_identifier_continue);
        let text = &self.source[start..self.offset];
        self.push(keyword_kind(text), start);
    }

    fn unexpected_character(&mut self, start: usize, _character: char) {
        self.advance_character();
        self.diagnostics
            .push(self.diagnostic("SYN_UNEXPECTED_CHARACTER", start, self.offset));
    }

    fn single(&mut self, kind: TokenKind, start: usize) {
        self.advance_character();
        self.push(kind, start);
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            range: TextRange::new(start, self.offset),
        });
    }

    fn diagnostic(&self, code: &'static str, start: usize, end: usize) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(code);
        diagnostic.primary = Some(SourceSpan {
            source: self.source_name.clone(),
            start,
            end,
        });
        diagnostic
    }

    fn remaining(&self) -> &'source str {
        &self.source[self.offset..]
    }

    fn current_character(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn next_character(&self) -> Option<char> {
        let mut characters = self.remaining().chars();
        characters.next()?;
        characters.next()
    }

    fn advance_character(&mut self) {
        if let Some(character) = self.current_character() {
            self.offset += character.len_utf8();
        }
    }

    fn advance_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(character) = self.current_character() {
            if !predicate(character) {
                break;
            }
            self.offset += character.len_utf8();
        }
    }
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn keyword_kind(text: &str) -> TokenKind {
    match text {
        "type" => TokenKind::Type,
        "enum" => TokenKind::Enum,
        "entity" => TokenKind::Entity,
        "value" => TokenKind::Value,
        "input" => TokenKind::Input,
        "output" => TokenKind::Output,
        "failure" => TokenKind::Failure,
        "function" => TokenKind::Function,
        "action" => TokenKind::Action,
        "fixture" => TokenKind::Fixture,
        "test" => TokenKind::Test,
        "route" => TokenKind::Route,
        "config" => TokenKind::Config,
        "module" => TokenKind::Module,
        "import" => TokenKind::Import,
        "persist" => TokenKind::Persist,
        "code" => TokenKind::Code,
        "kind" => TokenKind::Kind,
        "message" => TokenKind::Message,
        "public" => TokenKind::Public,
        "internal" => TokenKind::Internal,
        "optional" => TokenKind::Optional,
        "required" => TokenKind::Required,
        "many" => TokenKind::Many,
        "missing" => TokenKind::Missing,
        "fails" => TokenKind::Fails,
        "var" => TokenKind::Var,
        "mut" => TokenKind::Mut,
        "return" => TokenKind::Return,
        "reject" => TokenKind::Reject,
        "attempt" => TokenKind::Attempt,
        "call" => TokenKind::Call,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "match" => TokenKind::Match,
        "success" => TokenKind::Success,
        "propagate" => TokenKind::Propagate,
        "assert" => TokenKind::Assert,
        "advance" => TokenKind::Advance,
        "and" => TokenKind::And,
        "or" => TokenKind::Or,
        "not" => TokenKind::Not,
        "auth" => TokenKind::Auth,
        "path" => TokenKind::Path,
        "headers" => TokenKind::Headers,
        "explicitly" => TokenKind::Explicitly,
        "run" => TokenKind::Run,
        "deadline" => TokenKind::Deadline,
        "min" => TokenKind::Min,
        "max" => TokenKind::Max,
        "min_length" => TokenKind::MinLength,
        "max_length" => TokenKind::MaxLength,
        "pattern" => TokenKind::Pattern,
        "format" => TokenKind::Format,
        "throw" => TokenKind::Throw,
        "create" => TokenKind::Create,
        "query" => TokenKind::Query,
        "where" => TokenKind::Where,
        "order_by" => TokenKind::OrderBy,
        "asc" => TokenKind::Asc,
        "desc" => TokenKind::Desc,
        "update" => TokenKind::Update,
        "set" => TokenKind::Set,
        "patch" => TokenKind::Patch,
        "empty" => TokenKind::Empty,
        "conflict" => TokenKind::Conflict,
        "constraint" => TokenKind::Constraint,
        "identity" => TokenKind::Identity,
        "unique" => TokenKind::Unique,
        "index" => TokenKind::Index,
        "references" => TokenKind::References,
        "as" => TokenKind::As,
        "on_delete" => TokenKind::OnDelete,
        "restrict" => TokenKind::Restrict,
        "cascade" => TokenKind::Cascade,
        "set_null" => TokenKind::SetNull,
        "inverse" => TokenKind::Inverse,
        "via" => TokenKind::Via,
        "include" => TokenKind::Include,
        "into" => TokenKind::Into,
        "limit" => TokenKind::Limit,
        "offset" => TokenKind::Offset,
        "delete" => TokenKind::Delete,
        "true" | "false" => TokenKind::BooleanLiteral,
        "none" => TokenKind::NoneLiteral,
        "GET" => TokenKind::Get,
        "POST" => TokenKind::Post,
        "PUT" => TokenKind::Put,
        "PATCH" => TokenKind::Patch,
        "DELETE" => TokenKind::Delete,
        _ => TokenKind::Identifier,
    }
}

#[cfg(test)]
mod tests {
    use super::{lex, TokenKind};
    use std::fs;
    use std::path::Path;

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("syntax crate should be inside the repository")
    }

    #[test]
    fn lexes_the_jadpo_seed_without_diagnostics() {
        let path = repository_root().join("examples/jadpo-seed/app.jadpo");
        let source = fs::read_to_string(&path).expect("seed should be readable");
        let result = lex(&path, &source);

        assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
        assert!(result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Type));
        assert!(result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Route));
        assert!(result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::RoutePath));
        assert_eq!(
            result.tokens.last().map(|token| token.kind),
            Some(TokenKind::Eof)
        );
    }

    #[test]
    fn lexes_all_fixtures_with_only_the_expected_lexical_diagnostics() {
        let root = repository_root().join("tests/compile");
        let mut paths = Vec::new();
        collect_sources(&root, &mut paths);

        assert_eq!(paths.len(), 216);
        for path in paths {
            let source = fs::read_to_string(&path).expect("fixture should be readable");
            let result = lex(&path, &source);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let expected = match file_name {
                "75_invalid_string_escape.jadpo" => &["SYN_INVALID_ESCAPE"][..],
                "76_unexpected_character.jadpo" => &["SYN_UNEXPECTED_CHARACTER"][..],
                _ => &[],
            };
            assert_eq!(
                result
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code)
                    .collect::<Vec<_>>(),
                expected,
                "{}: {:#?}",
                path.display(),
                result.diagnostics
            );
        }
    }

    #[test]
    fn token_ranges_slice_the_original_source() {
        let source = "return Email(\"person@example.com\")";
        let result = lex(Path::new("inline.jadpo"), source);
        let email = result
            .tokens
            .iter()
            .find(|token| token.kind == TokenKind::Identifier)
            .expect("Email identifier should exist");
        let string = result
            .tokens
            .iter()
            .find(|token| token.kind == TokenKind::StringLiteral)
            .expect("string literal should exist");

        assert_eq!(email.text(source), "Email");
        assert_eq!(string.text(source), "\"person@example.com\"");
    }

    #[test]
    fn reports_invalid_escape_and_continues() {
        let source = "\"bad\\q\" type";
        let result = lex(Path::new("inline.jadpo"), source);

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "SYN_INVALID_ESCAPE");
        assert!(result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Type));
    }

    #[test]
    fn reports_unterminated_strings() {
        let source = "\"unfinished\nvalue";
        let result = lex(Path::new("inline.jadpo"), source);

        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "SYN_UNTERMINATED_STRING");
    }

    fn collect_sources(path: &Path, output: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(path).expect("fixture directory should be readable") {
            let entry = entry.expect("fixture entry should be readable");
            let entry_path = entry.path();
            if entry_path.is_dir() {
                collect_sources(&entry_path, output);
            } else if entry_path
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("jadpo")
            {
                output.push(entry_path);
            }
        }
    }
}
