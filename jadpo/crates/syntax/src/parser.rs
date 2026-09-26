use crate::ast::*;
use crate::{lex, TextRange, Token, TokenKind};
use jadpo_diagnostics::{Diagnostic, DiagnosticFact, SourceSpan, TextEdit};
use std::collections::BTreeSet;
use std::path::Path;

pub type ParseResult = ParsedSyntax;

pub fn parse(source_name: &Path, source: &str) -> ParseResult {
    let lexed = lex(source_name, source);
    Parser::new(source_name, source, lexed.tokens, lexed.diagnostics).parse_file()
}

struct Parser<'source> {
    source_name: String,
    source: &'source str,
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

struct ParsedRecordBody {
    fields: Vec<FieldDeclaration>,
    inverses: Vec<InverseDeclaration>,
    persistence_constraints: Vec<PersistenceConstraintDeclaration>,
    end: usize,
}

impl<'source> Parser<'source> {
    fn new(
        source_name: &Path,
        source: &'source str,
        tokens: Vec<Token>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        let mut parser = Self {
            source_name: source_name.to_string_lossy().into_owned(),
            source,
            tokens,
            cursor: 0,
            diagnostics,
        };
        parser.skip_trivia();
        parser
    }

    fn parse_file(mut self) -> ParseResult {
        let module = if self.at(TokenKind::Module) {
            self.parse_module_declaration()
        } else {
            None
        };
        let mut imports = Vec::new();
        while self.at(TokenKind::Import) {
            if let Some(import) = self.parse_import_declaration() {
                imports.push(import);
            }
        }
        if module.is_none() && !imports.is_empty() {
            self.error_at("SYN_IMPORT_REQUIRES_MODULE", imports[0].range);
        }
        let mut declarations = Vec::new();
        let mut exports = Vec::new();

        while !self.at(TokenKind::Eof) {
            let before = self.cursor;
            let diagnostics_before = self.diagnostics.len();
            let public = if self.at(TokenKind::Public) {
                self.bump();
                true
            } else {
                false
            };
            if let Some(declaration) = self.parse_declaration() {
                if public {
                    if let Some(name) = declaration_name(&declaration) {
                        exports.push(name.clone());
                    } else {
                        self.error_at("SYN_ROUTE_EXPORT_INVALID", declaration.range());
                    }
                }
                declarations.push(declaration);
            } else {
                if self.diagnostics.len() == diagnostics_before {
                    let token = self.current();
                    self.diagnostic_at(
                        Diagnostic::error("SYN_EXPECTED_DECLARATION").with_fact(
                            DiagnosticFact::FoundValue(diagnostic_token_label(token, self.source)),
                        ),
                        token.range,
                    );
                }
                self.recover_declaration();
            }

            if self.cursor == before {
                self.bump();
            }
        }

        ParseResult {
            source_name: self.source_name,
            source_text: self.source.to_owned(),
            file: SyntaxFile {
                module,
                imports,
                exports,
                declarations,
                range: TextRange::new(0, self.source.len()),
            },
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn parse_module_declaration(&mut self) -> Option<ModuleDeclaration> {
        let start = self
            .expect(TokenKind::Module, "expected `module`")?
            .range
            .start;
        let path = self.parse_qualified_name()?;
        let end = path.last()?.range.end;
        Some(ModuleDeclaration {
            path,
            range: TextRange::new(start, end),
        })
    }

    fn parse_import_declaration(&mut self) -> Option<ImportDeclaration> {
        let start = self
            .expect(TokenKind::Import, "expected `import`")?
            .range
            .start;
        let module = self.parse_qualified_name()?;
        self.expect(
            TokenKind::LeftBrace,
            "expected `{` before selectively imported names",
        )?;
        let mut names = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            names.push(self.expect_name("expected imported declaration name")?);
            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        if names.is_empty() {
            self.error_current("SYN_EMPTY_IMPORT");
        }
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after imported names")?
            .range
            .end;
        Some(ImportDeclaration {
            module,
            names,
            range: TextRange::new(start, end),
        })
    }

    fn parse_declaration(&mut self) -> Option<Declaration> {
        match self.current_kind() {
            TokenKind::Type => self.parse_type_declaration().map(Declaration::Type),
            TokenKind::Enum => self.parse_enum_declaration().map(Declaration::Enum),
            TokenKind::Entity => self
                .parse_record_declaration(RecordKind::Entity)
                .map(Declaration::Record),
            TokenKind::Value => self
                .parse_record_declaration(RecordKind::Value)
                .map(Declaration::Record),
            TokenKind::Input => self
                .parse_record_declaration(RecordKind::Input)
                .map(Declaration::Record),
            TokenKind::Output => self
                .parse_record_declaration(RecordKind::Output)
                .map(Declaration::Record),
            TokenKind::Failure => self.parse_failure_declaration().map(Declaration::Failure),
            TokenKind::Function => self
                .parse_callable_declaration(CallableKind::Function)
                .map(Declaration::Callable),
            TokenKind::Action => self
                .parse_callable_declaration(CallableKind::Action)
                .map(Declaration::Callable),
            TokenKind::Test => self.parse_test_declaration().map(Declaration::Test),
            TokenKind::Route => self.parse_route_declaration().map(Declaration::Route),
            _ => None,
        }
    }

    fn parse_test_declaration(&mut self) -> Option<TestDeclaration> {
        let start = self.expect(TokenKind::Test, "expected `test`")?.range.start;
        let name = self.parse_literal_of(TokenKind::StringLiteral)?;
        let body = self.parse_block()?;
        Some(TestDeclaration {
            range: TextRange::new(start, body.range.end),
            name,
            body,
        })
    }

    fn parse_enum_declaration(&mut self) -> Option<EnumDeclaration> {
        let start = self.expect(TokenKind::Enum, "expected `enum`")?.range.start;
        let name = self.expect_name("expected an enum name")?;
        self.expect(TokenKind::LeftBrace, "expected `{` before enum variants")?;
        let mut variants = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            if let Some(name) = self.expect_name("expected an enum variant") {
                let (fields, end) = if self.at(TokenKind::LeftBrace) {
                    self.parse_field_block()?
                } else {
                    (Vec::new(), name.range.end)
                };
                variants.push(EnumVariantDeclaration {
                    range: TextRange::new(name.range.start, end),
                    name,
                    fields,
                });
            }
            if self.at(TokenKind::Comma) {
                self.bump();
            }
            if self.cursor == before {
                self.bump();
            }
        }
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after enum variants")?
            .range
            .end;
        if variants.is_empty() {
            self.error_at("SYN_ENUM_EMPTY", name.range);
        }
        Some(EnumDeclaration {
            name,
            variants,
            range: TextRange::new(start, end),
        })
    }

    fn parse_type_declaration(&mut self) -> Option<TypeDeclaration> {
        let start = self.expect(TokenKind::Type, "expected `type`")?.range.start;
        let name = self.expect_name("expected a type name")?;
        self.expect(TokenKind::Equal, "expected `=` after type name")?;
        if self.at(TokenKind::LeftBrace) {
            let diagnostic = Diagnostic::error("SYN_TYPE_PARENT_REQUIRED").with_note(
                "use `type User = Text { ... }`, or use `entity User { ... }` for fields",
            );
            self.diagnostic_at(diagnostic, self.current().range);
            return None;
        }
        let parent = self.parse_type_reference()?;
        let (constraints, end) = self.parse_constraint_block()?;

        Some(TypeDeclaration {
            name,
            parent,
            constraints,
            range: TextRange::new(start, end),
        })
    }

    fn parse_record_declaration(&mut self, kind: RecordKind) -> Option<RecordDeclaration> {
        let keyword = match kind {
            RecordKind::Entity => TokenKind::Entity,
            RecordKind::Value => TokenKind::Value,
            RecordKind::Input => TokenKind::Input,
            RecordKind::Output => TokenKind::Output,
        };
        let start = self
            .expect(keyword, "expected record declaration")?
            .range
            .start;
        let name = self.expect_name("expected a record name")?;
        let body = self.parse_record_body(kind)?;

        Some(RecordDeclaration {
            kind,
            name,
            fields: body.fields,
            inverses: body.inverses,
            persistence_constraints: body.persistence_constraints,
            range: TextRange::new(start, body.end),
        })
    }

    fn parse_record_body(&mut self, kind: RecordKind) -> Option<ParsedRecordBody> {
        self.expect(TokenKind::LeftBrace, "expected `{` before fields")?;
        let mut fields = Vec::new();
        let mut inverses = Vec::new();
        let mut persistence_constraints = Vec::new();

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            let diagnostics_before = self.diagnostics.len();
            if self.at(TokenKind::Inverse) {
                if kind != RecordKind::Entity {
                    self.error_current("SYN_INVERSE_NON_ENTITY");
                }
                if let Some(inverse) = self.parse_inverse_declaration() {
                    inverses.push(inverse);
                }
            } else if self.at(TokenKind::Constraint) {
                if kind != RecordKind::Entity {
                    self.error_current("SYN_CONSTRAINT_NON_ENTITY");
                }
                if let Some(constraint) = self.parse_persistence_constraint() {
                    persistence_constraints.push(constraint);
                }
            } else if let Some(field) = self.parse_field_declaration() {
                fields.push(field);
            } else {
                if self.diagnostics.len() == diagnostics_before {
                    self.error_current("SYN_EXPECTED_FIELD");
                }
                self.recover_until(&[TokenKind::RightBrace]);
            }
            if self.cursor == before {
                self.bump();
            }
        }

        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after fields")?
            .range
            .end;
        Some(ParsedRecordBody {
            fields,
            inverses,
            persistence_constraints,
            end,
        })
    }

    fn parse_persistence_constraint(&mut self) -> Option<PersistenceConstraintDeclaration> {
        let start = self
            .expect(TokenKind::Constraint, "expected `constraint`")?
            .range
            .start;
        let name = self.expect_contextual_name("expected constraint name")?;
        self.expect(TokenKind::Colon, "expected `:` after constraint name")?;
        self.expect(TokenKind::Unique, "expected `unique` constraint kind")?;
        self.expect(
            TokenKind::LeftParen,
            "expected `(` before constraint fields",
        )?;
        let mut fields = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            fields.push(self.expect_contextual_name("expected constrained field name")?);
            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        let end = self
            .expect(
                TokenKind::RightParen,
                "expected `)` after constraint fields",
            )?
            .range
            .end;
        Some(PersistenceConstraintDeclaration {
            name,
            kind: PersistenceConstraintKind::Unique,
            fields,
            range: TextRange::new(start, end),
        })
    }

    fn parse_inverse_declaration(&mut self) -> Option<InverseDeclaration> {
        let start = self
            .expect(TokenKind::Inverse, "expected `inverse`")?
            .range
            .start;
        let name = self.expect_contextual_name("expected inverse relationship name")?;
        self.expect(
            TokenKind::Colon,
            "expected `:` after inverse relationship name",
        )?;
        let cardinality = match self.current_kind() {
            TokenKind::Many => {
                self.bump();
                InverseCardinality::Many
            }
            TokenKind::Optional => {
                self.bump();
                InverseCardinality::Optional
            }
            _ => {
                self.error_current("SYN_EXPECTED_INVERSE_CARDINALITY");
                return None;
            }
        };
        let target = self.expect_name("expected child entity after inverse cardinality")?;
        self.expect(TokenKind::Via, "expected `via` after inverse target")?;
        let via = self.parse_type_reference()?;
        let end = via.range.end;
        Some(InverseDeclaration {
            name,
            cardinality,
            target,
            via,
            range: TextRange::new(start, end),
        })
    }

    fn parse_failure_declaration(&mut self) -> Option<FailureDeclaration> {
        let start = self
            .expect(TokenKind::Failure, "expected `failure`")?
            .range
            .start;
        let name = self.expect_name("expected a failure name")?;
        self.expect(TokenKind::LeftBrace, "expected `{` after failure name")?;

        let mut kind = None;
        let mut code = None;
        let mut message = None;
        let mut public_fields = Vec::new();
        let mut internal_fields = Vec::new();

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            match self.current_kind() {
                TokenKind::Kind => {
                    self.bump();
                    kind = self.expect_name("expected a standard failure kind after `kind`");
                }
                TokenKind::Code => {
                    self.bump();
                    code = self.parse_literal_of(TokenKind::StringLiteral);
                }
                TokenKind::Message => {
                    self.bump();
                    message = self.parse_literal_of(TokenKind::StringLiteral);
                }
                TokenKind::Public => {
                    self.bump();
                    if let Some((fields, _)) = self.parse_field_block() {
                        public_fields = fields;
                    }
                }
                TokenKind::Internal => {
                    self.bump();
                    if let Some((fields, _)) = self.parse_field_block() {
                        internal_fields = fields;
                    }
                }
                _ => {
                    self.error_current("SYN_EXPECTED_FAILURE_ITEM");
                    self.recover_until(&[
                        TokenKind::Kind,
                        TokenKind::Code,
                        TokenKind::Message,
                        TokenKind::Public,
                        TokenKind::Internal,
                        TokenKind::RightBrace,
                    ]);
                }
            }
            if self.cursor == before {
                self.bump();
            }
        }

        let end = self
            .expect(
                TokenKind::RightBrace,
                "expected `}` after failure declaration",
            )?
            .range
            .end;

        if code.is_none() {
            self.error_at("SYN_FAILURE_CODE_REQUIRED", name.range);
        }
        if kind.is_none() {
            self.error_at("SYN_FAILURE_KIND_REQUIRED", name.range);
        }

        Some(FailureDeclaration {
            name,
            kind: kind.unwrap_or_else(|| Name {
                text: "InternalFault".to_owned(),
                range: TextRange::new(start, start),
            }),
            code,
            message,
            public_fields,
            internal_fields,
            range: TextRange::new(start, end),
        })
    }

    fn parse_callable_declaration(&mut self, kind: CallableKind) -> Option<CallableDeclaration> {
        let keyword = match kind {
            CallableKind::Function => TokenKind::Function,
            CallableKind::Action => TokenKind::Action,
        };
        let start = self
            .expect(keyword, "expected callable declaration")?
            .range
            .start;
        let name = self.expect_name("expected a callable name")?;
        let parameters = self.parse_parameters()?;
        let return_start = self
            .expect(TokenKind::Arrow, "expected `->` and a return type")?
            .range
            .start;
        let return_type = self.parse_type_reference()?;
        let return_annotation_range = TextRange::new(return_start, return_type.range.end);

        let mut failures = Vec::new();
        let mut failures_range = None;
        if self.at(TokenKind::Fails) {
            let failures_start = self.bump().range.start;
            loop {
                if let Some(failure) = self.expect_name("expected failure name after `fails`") {
                    failures.push(failure);
                }
                if !self.at(TokenKind::Comma) {
                    break;
                }
                self.bump();
            }
            failures_range = failures
                .last()
                .map(|failure| TextRange::new(failures_start, failure.range.end));
        }

        let body = self.parse_block()?;
        let end = body.range.end;
        Some(CallableDeclaration {
            kind,
            name,
            parameters,
            return_type,
            return_annotation_range,
            failures,
            failures_range,
            body,
            range: TextRange::new(start, end),
        })
    }

    fn parse_parameters(&mut self) -> Option<Vec<Parameter>> {
        self.expect(TokenKind::LeftParen, "expected `(` before parameters")?;
        let mut parameters = Vec::new();

        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            let start = self.current().range.start;
            let Some(name) = self.expect_contextual_name("expected parameter name") else {
                self.recover_until(&[TokenKind::Comma, TokenKind::RightParen]);
                if self.at(TokenKind::Comma) {
                    self.bump();
                }
                continue;
            };
            self.expect(TokenKind::Colon, "expected `:` after parameter name")?;
            let parameter_type = self.parse_type_reference()?;
            let end = parameter_type.range.end;
            parameters.push(Parameter {
                name,
                parameter_type,
                range: TextRange::new(start, end),
            });

            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }

        self.expect(TokenKind::RightParen, "expected `)` after parameters")?;
        Some(parameters)
    }

    fn parse_block(&mut self) -> Option<Block> {
        let start = self
            .expect(TokenKind::LeftBrace, "expected `{` to start block")?
            .range
            .start;
        let mut statements = Vec::new();

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            } else {
                self.error_current("SYN_EXPECTED_STATEMENT");
                self.recover_statement();
            }
            if self.cursor == before {
                self.bump();
            }
        }

        let end = self
            .expect(TokenKind::RightBrace, "expected `}` to close block")?
            .range
            .end;
        Some(Block {
            statements,
            range: TextRange::new(start, end),
        })
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_kind() {
            TokenKind::Var => self.parse_binding_statement().map(Statement::Binding),
            TokenKind::Identifier | TokenKind::Input | TokenKind::Output | TokenKind::Value
                if self.next_significant_kind() == Some(TokenKind::Equal) =>
            {
                self.parse_assignment_statement().map(Statement::Assignment)
            }
            TokenKind::Return => self.parse_return_statement().map(Statement::Return),
            TokenKind::Reject => self.parse_reject_statement().map(Statement::Reject),
            TokenKind::If => self.parse_if_statement().map(Statement::If),
            TokenKind::Match => self.parse_match_statement().map(Statement::Match),
            TokenKind::Assert => self.parse_assert_statement().map(Statement::Assert),
            TokenKind::Throw => self.parse_unsupported_throw().map(Statement::Unsupported),
            _ => None,
        }
    }

    fn parse_assert_statement(&mut self) -> Option<AssertStatement> {
        let start = self
            .expect(TokenKind::Assert, "expected `assert`")?
            .range
            .start;
        let condition = self.parse_expression();
        Some(AssertStatement {
            range: TextRange::new(start, condition.range().end),
            condition,
        })
    }

    fn parse_assignment_statement(&mut self) -> Option<AssignmentStatement> {
        let target = self.expect_contextual_name("expected assignment target")?;
        let start = target.range.start;
        self.expect(TokenKind::Equal, "expected `=` in assignment")?;
        let value = self.parse_expression();
        let end = value.range().end;
        Some(AssignmentStatement {
            target,
            value,
            range: TextRange::new(start, end),
        })
    }

    fn parse_binding_statement(&mut self) -> Option<BindingStatement> {
        let start = self.expect(TokenKind::Var, "expected `var`")?.range.start;
        let mutable = if self.at(TokenKind::Mut) {
            self.bump();
            true
        } else {
            false
        };
        let name = self.expect_contextual_name("expected binding name")?;
        let annotation = if self.at(TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_reference()?)
        } else {
            None
        };
        self.expect(TokenKind::Equal, "expected `=` in binding")?;
        let value = self.parse_expression();
        let end = value.range().end;
        Some(BindingStatement {
            mutable,
            name,
            annotation,
            value,
            range: TextRange::new(start, end),
        })
    }

    fn parse_return_statement(&mut self) -> Option<ReturnStatement> {
        let start = self
            .expect(TokenKind::Return, "expected `return`")?
            .range
            .start;
        let value = self.parse_expression();
        let end = value.range().end;
        Some(ReturnStatement {
            value,
            range: TextRange::new(start, end),
        })
    }

    fn parse_reject_statement(&mut self) -> Option<RejectStatement> {
        let start = self
            .expect(TokenKind::Reject, "expected `reject`")?
            .range
            .start;
        let failure = self.expect_name("expected failure name after `reject`")?;
        let mut values = Vec::new();
        let mut end = failure.range.end;

        if self.at(TokenKind::LeftBrace) {
            let parsed = self.parse_object_body()?;
            values = parsed.0;
            end = parsed.1;
        }

        Some(RejectStatement {
            failure,
            values,
            range: TextRange::new(start, end),
        })
    }

    fn parse_if_statement(&mut self) -> Option<IfStatement> {
        let start = self.expect(TokenKind::If, "expected `if`")?.range.start;
        let condition = self.parse_expression();
        let then_block = self.parse_block()?;
        let else_block = if self.at(TokenKind::Else) {
            self.bump();
            Some(self.parse_block()?)
        } else {
            None
        };
        let end = else_block
            .as_ref()
            .map_or(then_block.range.end, |block| block.range.end);

        Some(IfStatement {
            condition,
            then_block,
            else_block,
            range: TextRange::new(start, end),
        })
    }

    fn parse_match_statement(&mut self) -> Option<MatchStatement> {
        let start = self
            .expect(TokenKind::Match, "expected `match`")?
            .range
            .start;
        let subject = self.parse_match_expression();
        self.expect(TokenKind::LeftBrace, "expected `{` before match arms")?;
        let mut arms = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let arm_start = self.current().range.start;
            let pattern = self.parse_match_pattern()?;
            self.expect(TokenKind::FatArrow, "expected `=>` after match pattern")?;
            let body = self.parse_block()?;
            arms.push(MatchArm {
                range: TextRange::new(arm_start, body.range.end),
                pattern,
                body,
            });
        }
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after match arms")?
            .range
            .end;
        Some(MatchStatement {
            subject,
            arms,
            range: TextRange::new(start, end),
        })
    }

    fn parse_match_expression(&mut self) -> Expression {
        self.parse_binary_expression(0, false)
    }

    fn parse_match_pattern(&mut self) -> Option<MatchPattern> {
        match self.current_kind() {
            TokenKind::StringLiteral
            | TokenKind::IntegerLiteral
            | TokenKind::DecimalLiteral
            | TokenKind::BooleanLiteral
            | TokenKind::NoneLiteral => self.parse_any_literal().map(MatchPattern::Literal),
            TokenKind::Identifier if self.current().text(self.source) == "_" => {
                let token = self.bump();
                Some(MatchPattern::Wildcard(Name {
                    text: "_".to_owned(),
                    range: token.range,
                }))
            }
            TokenKind::Identifier
            | TokenKind::Input
            | TokenKind::Output
            | TokenKind::Value
            | TokenKind::Path => {
                let path = self.parse_qualified_name()?;
                let range = TextRange::new(path.first()?.range.start, path.last()?.range.end);
                let target = NameExpression { path, range };
                if target.path.len() == 1
                    && target.path[0].text == "some"
                    && self.at(TokenKind::LeftParen)
                {
                    self.bump();
                    let binding =
                        self.expect_contextual_name("expected binding in `some(...)` pattern")?;
                    let end = self
                        .expect(TokenKind::RightParen, "expected `)` after optional binding")?
                        .range
                        .end;
                    Some(MatchPattern::OptionalSome(OptionalSomePattern {
                        binding,
                        range: TextRange::new(range.start, end),
                    }))
                } else if self.at(TokenKind::LeftBrace) {
                    self.bump();
                    let mut bindings = Vec::new();
                    while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
                        bindings
                            .push(self.expect_contextual_name("expected variant field binding")?);
                        if self.at(TokenKind::Comma) {
                            self.bump();
                        }
                    }
                    let end = self
                        .expect(
                            TokenKind::RightBrace,
                            "expected `}` after variant field bindings",
                        )?
                        .range
                        .end;
                    Some(MatchPattern::Variant(VariantPattern {
                        target,
                        bindings,
                        range: TextRange::new(range.start, end),
                    }))
                } else {
                    Some(MatchPattern::Name(target))
                }
            }
            _ => {
                self.error_current("SYN_MATCH_PATTERN");
                None
            }
        }
    }

    fn parse_unsupported_throw(&mut self) -> Option<UnsupportedStatement> {
        let start = self
            .expect(TokenKind::Throw, "expected `throw`")?
            .range
            .start;
        let expression = self.parse_expression();
        let range = TextRange::new(start, expression.range().end);
        self.error_at("SYN_UNSUPPORTED_THROW", range);
        Some(UnsupportedStatement {
            keyword: "throw".to_owned(),
            range,
        })
    }

    fn parse_expression(&mut self) -> Expression {
        self.parse_binary_expression(0, true)
    }

    fn parse_binary_expression(
        &mut self,
        minimum_precedence: u8,
        allow_construction: bool,
    ) -> Expression {
        let mut expression = self.parse_prefix_expression(allow_construction);
        while let Some((operator, precedence)) = self.current_binary_operator() {
            if precedence < minimum_precedence {
                break;
            }
            self.bump();
            let right = self.parse_binary_expression(precedence + 1, allow_construction);
            let range = TextRange::new(expression.range().start, right.range().end);
            expression = Expression::Binary(BinaryExpression {
                left: Box::new(expression),
                operator,
                right: Box::new(right),
                range,
            });
        }

        expression
    }

    fn parse_prefix_expression(&mut self, allow_construction: bool) -> Expression {
        if self.at(TokenKind::Attempt) {
            let start = self.bump().range.start;
            let value = self.parse_prefix_expression(allow_construction);
            return Expression::Attempt(AttemptExpression {
                range: TextRange::new(start, value.range().end),
                value: Box::new(value),
            });
        }
        if self.at(TokenKind::Not) || self.at(TokenKind::Minus) {
            let token = self.bump();
            let operator = if token.kind == TokenKind::Not {
                UnaryOperator::Not
            } else {
                UnaryOperator::Negate
            };
            let value = self.parse_prefix_expression(allow_construction);
            return Expression::Unary(UnaryExpression {
                range: TextRange::new(token.range.start, value.range().end),
                operator,
                value: Box::new(value),
            });
        }
        match self.current_kind() {
            TokenKind::Create => self.parse_create_expression(),
            TokenKind::Query => self.parse_query_expression(),
            TokenKind::Update => self.parse_update_expression(),
            TokenKind::Delete => self.parse_delete_expression(),
            TokenKind::StringLiteral
            | TokenKind::IntegerLiteral
            | TokenKind::DecimalLiteral
            | TokenKind::BooleanLiteral
            | TokenKind::NoneLiteral => self
                .parse_any_literal()
                .map(Expression::Literal)
                .unwrap_or_else(|| Expression::Missing(self.current().range)),
            TokenKind::Identifier
            | TokenKind::Input
            | TokenKind::Output
            | TokenKind::Value
            | TokenKind::Path => self.parse_named_expression_with_construction(allow_construction),
            TokenKind::LeftParen => {
                let start = self.bump().range.start;
                let value = self.parse_expression();
                let end = self
                    .expect(TokenKind::RightParen, "expected `)` after expression")
                    .map_or(value.range().end, |token| token.range.end);
                Expression::Grouped(GroupedExpression {
                    value: Box::new(value),
                    range: TextRange::new(start, end),
                })
            }
            _ => {
                let range = self.current().range;
                self.error_current("SYN_EXPECTED_EXPRESSION");
                if !self.at(TokenKind::Eof) {
                    self.bump();
                }
                Expression::Missing(range)
            }
        }
    }

    fn current_binary_operator(&self) -> Option<(BinaryOperator, u8)> {
        Some(match self.current_kind() {
            TokenKind::Or => (BinaryOperator::Or, 1),
            TokenKind::And => (BinaryOperator::And, 2),
            TokenKind::EqualEqual => (BinaryOperator::Equal, 3),
            TokenKind::BangEqual => (BinaryOperator::NotEqual, 3),
            TokenKind::LeftAngle => (BinaryOperator::Less, 4),
            TokenKind::LessEqual => (BinaryOperator::LessEqual, 4),
            TokenKind::RightAngle => (BinaryOperator::Greater, 4),
            TokenKind::GreaterEqual => (BinaryOperator::GreaterEqual, 4),
            TokenKind::Plus => (BinaryOperator::Add, 5),
            TokenKind::Minus => (BinaryOperator::Subtract, 5),
            TokenKind::Star => (BinaryOperator::Multiply, 6),
            TokenKind::Slash => (BinaryOperator::Divide, 6),
            TokenKind::Percent => (BinaryOperator::Remainder, 6),
            _ => return None,
        })
    }

    fn parse_create_expression(&mut self) -> Expression {
        let start = self.bump().range.start;
        let Some(path) = self.parse_qualified_name() else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let target_range = TextRange::new(
            path.first().map_or(start, |name| name.range.start),
            path.last().map_or(start, |name| name.range.end),
        );
        let target = NameExpression {
            path,
            range: target_range,
        };
        let (fields, fields_end) = self.parse_object_body().unwrap_or_default();
        let conflicts = self.parse_conflict_bindings();
        let end = conflicts
            .last()
            .map_or(fields_end, |binding| binding.range.end);
        Expression::Create(CreateExpression {
            target,
            fields,
            conflicts,
            range: TextRange::new(start, end),
        })
    }

    fn parse_query_expression(&mut self) -> Expression {
        let start = self.bump().range.start;
        let cardinality = match self.current_kind() {
            TokenKind::Optional => {
                self.bump();
                QueryCardinality::Optional
            }
            TokenKind::Required => {
                self.bump();
                QueryCardinality::Required
            }
            TokenKind::Many => {
                self.bump();
                QueryCardinality::Many
            }
            _ => {
                let range = self.current().range;
                let diagnostic = self.expected_diagnostic(
                    "SYN_UNEXPECTED_TOKEN",
                    "one of `optional`, `required`, or `many` after `query`",
                );
                self.diagnostic_at(diagnostic, range);
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
        };
        let Some(path) = self.parse_qualified_name() else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let target_range = TextRange::new(
            path.first().map_or(start, |name| name.range.start),
            path.last().map_or(start, |name| name.range.end),
        );
        let target = NameExpression {
            path,
            range: target_range,
        };
        if self
            .expect(TokenKind::LeftBrace, "expected `{` after query target")
            .is_none()
        {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        }
        if self
            .expect(TokenKind::Where, "expected `where` in query")
            .is_none()
        {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        }
        if self
            .expect(TokenKind::Colon, "expected `:` after `where`")
            .is_none()
        {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        }
        let Some(field) = self.expect_contextual_name("expected entity field after `where`") else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        if self
            .expect(TokenKind::EqualEqual, "expected `==` in query predicate")
            .is_none()
        {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        }
        let value = Box::new(self.parse_expression());
        let order = if cardinality == QueryCardinality::Many {
            let Some(order_token) = self.expect(
                TokenKind::OrderBy,
                "expected `order_by` in many-result query",
            ) else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            let order_start = order_token.range.start;
            if self
                .expect(TokenKind::Colon, "expected `:` after `order_by`")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            let Some(field) = self.expect_contextual_name("expected entity field after `order_by`")
            else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            let direction = match self.current_kind() {
                TokenKind::Asc => QueryOrderDirection::Ascending,
                TokenKind::Desc => QueryOrderDirection::Descending,
                _ => {
                    self.error_current("SYN_EXPECTED_ORDER_DIRECTION");
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                }
            };
            let order_end = self.bump().range.end;
            Some(QueryOrder {
                field,
                direction,
                range: TextRange::new(order_start, order_end),
            })
        } else {
            None
        };
        let pagination = if cardinality == QueryCardinality::Many && self.at(TokenKind::Limit) {
            let Some(pagination) = self.parse_query_pagination() else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            Some(pagination)
        } else {
            None
        };
        let mut includes = Vec::new();
        while self.at(TokenKind::Include) {
            let include_start = self.bump().range.start;
            if self
                .expect(TokenKind::Colon, "expected `:` after `include`")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            let Some(relationship) =
                self.expect_contextual_name("expected inverse relationship after `include`")
            else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            let nested_relationship = if self.at(TokenKind::Dot) {
                self.bump();
                let Some(nested) =
                    self.expect_contextual_name("expected nested relationship name after `.`")
                else {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                };
                Some(nested)
            } else {
                None
            };
            let include_cardinality = match self.current_kind() {
                TokenKind::Required => {
                    self.bump();
                    QueryIncludeCardinality::Required
                }
                TokenKind::Optional => {
                    self.bump();
                    QueryIncludeCardinality::Optional
                }
                _ => QueryIncludeCardinality::Many,
            };
            if self
                .expect(
                    TokenKind::Into,
                    "expected `into` and a named output shape after included relationship",
                )
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            if self
                .expect(TokenKind::Colon, "expected `:` after `into`")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            let Some(result) = self.parse_type_reference() else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            let (order, pagination, include_end) = if include_cardinality
                == QueryIncludeCardinality::Many
            {
                if self
                    .expect(
                        TokenKind::OrderBy,
                        "expected `order_by` for included to-many relationship",
                    )
                    .is_none()
                {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                }
                if self
                    .expect(TokenKind::Colon, "expected `:` after `order_by`")
                    .is_none()
                {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                }
                let Some(order_field) =
                    self.expect_contextual_name("expected child field after `order_by`")
                else {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                };
                let direction = match self.current_kind() {
                    TokenKind::Asc => QueryOrderDirection::Ascending,
                    TokenKind::Desc => QueryOrderDirection::Descending,
                    _ => {
                        self.error_current("SYN_EXPECTED_ORDER_DIRECTION");
                        return Expression::Missing(TextRange::new(
                            start,
                            self.current().range.end,
                        ));
                    }
                };
                let order_end = self.bump().range.end;
                let Some(pagination) = self.parse_query_pagination() else {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                };
                (
                    QueryOrder {
                        field: order_field,
                        direction,
                        range: TextRange::new(include_start, order_end),
                    },
                    pagination.clone(),
                    pagination.range.end,
                )
            } else {
                let synthetic = TextRange::new(result.range.end, result.range.end);
                (
                    QueryOrder {
                        field: relationship.clone(),
                        direction: QueryOrderDirection::Ascending,
                        range: synthetic,
                    },
                    QueryPagination {
                        limit: Box::new(Expression::Literal(Literal {
                            kind: LiteralKind::Integer,
                            text: "1".to_owned(),
                            range: synthetic,
                        })),
                        offset: Box::new(Expression::Literal(Literal {
                            kind: LiteralKind::Integer,
                            text: "0".to_owned(),
                            range: synthetic,
                        })),
                        range: synthetic,
                    },
                    result.range.end,
                )
            };
            includes.push(QueryInclude {
                relationship,
                nested_relationship,
                cardinality: include_cardinality,
                result,
                order,
                pagination,
                range: TextRange::new(include_start, include_end),
            });
        }
        let missing = if cardinality == QueryCardinality::Required {
            if self
                .expect(TokenKind::Missing, "expected `missing` in required query")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            if self
                .expect(TokenKind::Colon, "expected `:` after `missing`")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            self.parse_failure_binding()
        } else {
            None
        };
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after query predicate")
            .map_or(value.range().end, |token| token.range.end);
        Expression::Query(QueryExpression {
            cardinality,
            target,
            field,
            value,
            order,
            pagination,
            includes,
            missing,
            range: TextRange::new(start, end),
        })
    }

    fn parse_query_pagination(&mut self) -> Option<QueryPagination> {
        let start = self
            .expect(
                TokenKind::Limit,
                "expected explicit `limit` for a to-many load",
            )?
            .range
            .start;
        self.expect(TokenKind::Colon, "expected `:` after `limit`")?;
        let limit = Box::new(self.parse_expression());
        self.expect(TokenKind::Offset, "expected `offset` after query limit")?;
        self.expect(TokenKind::Colon, "expected `:` after `offset`")?;
        let offset = Box::new(self.parse_expression());
        let end = offset.range().end;
        Some(QueryPagination {
            limit,
            offset,
            range: TextRange::new(start, end),
        })
    }

    fn parse_failure_binding(&mut self) -> Option<RejectStatement> {
        let failure = self.expect_name("expected failure name after `missing:`")?;
        let start = failure.range.start;
        let mut values = Vec::new();
        let mut end = failure.range.end;

        if self.at(TokenKind::LeftBrace) {
            let parsed = self.parse_object_body()?;
            values = parsed.0;
            end = parsed.1;
        }

        Some(RejectStatement {
            failure,
            values,
            range: TextRange::new(start, end),
        })
    }

    fn parse_update_expression(&mut self) -> Expression {
        let start = self.bump().range.start;
        let Some(target) = self.parse_required_mutation_target(start, "update") else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let Some((field, value)) = self.parse_mutation_predicate(start, "update") else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let (changes, conditional_changes, patch, empty) = if self.at(TokenKind::Set) {
            self.bump();
            if self
                .expect(TokenKind::Colon, "expected `:` after `set`")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            let Some((changes, _)) = self.parse_object_body() else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            (changes, Vec::new(), None, None)
        } else if self.at(TokenKind::Patch) {
            self.bump();
            if self
                .expect(TokenKind::Colon, "expected `:` after `patch`")
                .is_none()
            {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            }
            let Some(path) = self.parse_qualified_name() else {
                self.error_current("SYN_EXPECTED_PATCH_INPUT");
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            let patch = NameExpression {
                range: TextRange::new(
                    path.first().map_or(start, |name| name.range.start),
                    path.last().map_or(start, |name| name.range.end),
                ),
                path,
            };
            let Some(empty) = self.parse_named_failure_binding("empty", TokenKind::Empty) else {
                return Expression::Missing(TextRange::new(start, self.current().range.end));
            };
            let (changes, conditional_changes) = if self.at(TokenKind::Set) {
                self.bump();
                if self
                    .expect(TokenKind::Colon, "expected `:` after `set`")
                    .is_none()
                {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                }
                let Some(changes) = self.parse_patch_set_body() else {
                    return Expression::Missing(TextRange::new(start, self.current().range.end));
                };
                changes
            } else {
                (Vec::new(), Vec::new())
            };
            (changes, conditional_changes, Some(patch), Some(empty))
        } else {
            self.error_current("SYN_EXPECTED_UPDATE_BODY");
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let Some(missing) = self.parse_named_failure_binding("missing", TokenKind::Missing) else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let conflicts = self.parse_conflict_bindings();
        if conflicts.is_empty() {
            self.error_current("SYN_MUTATION_CONFLICT_REQUIRED");
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        }
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after update")
            .map_or_else(
                || conflicts.last().unwrap().range.end,
                |token| token.range.end,
            );
        Expression::Update(UpdateExpression {
            target,
            field,
            value,
            changes,
            conditional_changes,
            patch,
            empty,
            missing,
            conflicts,
            range: TextRange::new(start, end),
        })
    }

    fn parse_patch_set_body(
        &mut self,
    ) -> Option<(Vec<FieldInitialiser>, Vec<PatchConditionalChange>)> {
        self.expect(TokenKind::LeftBrace, "expected `{`")?;
        let mut changes = Vec::new();
        let mut conditional_changes = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            let Some(change) = self.parse_field_initialiser() else {
                self.error_current("SYN_EXPECTED_FIELD_INITIALISER");
                self.recover_until(&[TokenKind::RightBrace]);
                continue;
            };
            if self.current().text(self.source) == "when" {
                self.bump();
                let Some(path) = self.parse_qualified_name() else {
                    self.error_current("SYN_EXPECTED_SUPPLIED_FIELD");
                    return None;
                };
                if self.current().text(self.source) != "supplied" {
                    self.error_current("SYN_EXPECTED_SUPPLIED");
                    return None;
                }
                let supplied_end = self.bump().range.end;
                let supplied = NameExpression {
                    range: TextRange::new(path[0].range.start, path.last()?.range.end),
                    path,
                };
                conditional_changes.push(PatchConditionalChange {
                    range: TextRange::new(change.range.start, supplied_end),
                    change,
                    supplied,
                });
            } else {
                changes.push(change);
            }
            if self.cursor == before {
                self.bump();
            }
        }
        self.expect(
            TokenKind::RightBrace,
            "expected `}` after patch-derived values",
        )?;
        Some((changes, conditional_changes))
    }

    fn parse_delete_expression(&mut self) -> Expression {
        let start = self.bump().range.start;
        let Some(target) = self.parse_required_mutation_target(start, "delete") else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let Some((field, value)) = self.parse_mutation_predicate(start, "delete") else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let Some(missing) = self.parse_named_failure_binding("missing", TokenKind::Missing) else {
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        };
        let conflicts = self.parse_conflict_bindings();
        if conflicts.is_empty() {
            self.error_current("SYN_MUTATION_CONFLICT_REQUIRED");
            return Expression::Missing(TextRange::new(start, self.current().range.end));
        }
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after delete")
            .map_or_else(
                || conflicts.last().unwrap().range.end,
                |token| token.range.end,
            );
        Expression::Delete(DeleteExpression {
            target,
            field,
            value,
            missing,
            conflicts,
            range: TextRange::new(start, end),
        })
    }

    fn parse_required_mutation_target(
        &mut self,
        start: usize,
        operation: &str,
    ) -> Option<NameExpression> {
        self.expect(
            TokenKind::Required,
            &format!("expected `required` after `{operation}`"),
        )?;
        let path = self.parse_qualified_name()?;
        let range = TextRange::new(
            path.first().map_or(start, |name| name.range.start),
            path.last().map_or(start, |name| name.range.end),
        );
        self.expect(
            TokenKind::LeftBrace,
            &format!("expected `{{` after {operation} target"),
        )?;
        Some(NameExpression { path, range })
    }

    fn parse_mutation_predicate(
        &mut self,
        _start: usize,
        operation: &str,
    ) -> Option<(Name, Box<Expression>)> {
        self.expect(
            TokenKind::Where,
            &format!("expected `where` in {operation}"),
        )?;
        self.expect(TokenKind::Colon, "expected `:` after `where`")?;
        let field = self.expect_contextual_name("expected entity field after `where`")?;
        self.expect(TokenKind::EqualEqual, "expected `==` in mutation predicate")?;
        Some((field, Box::new(self.parse_expression())))
    }

    fn parse_named_failure_binding(
        &mut self,
        label: &str,
        kind: TokenKind,
    ) -> Option<RejectStatement> {
        self.expect(kind, &format!("expected `{label}` failure binding"))?;
        self.expect(TokenKind::Colon, &format!("expected `:` after `{label}`"))?;
        self.parse_failure_binding()
    }

    fn parse_conflict_bindings(&mut self) -> Vec<ConflictBinding> {
        let mut bindings = Vec::new();
        while self.at(TokenKind::Conflict) {
            let start = self.bump().range.start;
            let constraint = if self.at(TokenKind::Colon) {
                None
            } else {
                let path = self.parse_qualified_name();
                path.map(|path| {
                    let range = TextRange::new(
                        path.first().unwrap().range.start,
                        path.last().unwrap().range.end,
                    );
                    NameExpression { path, range }
                })
            };
            if self
                .expect(TokenKind::Colon, "expected `:` after conflict binding")
                .is_none()
            {
                break;
            }
            let Some(rejection) = self.parse_failure_binding() else {
                break;
            };
            let end = rejection.range.end;
            bindings.push(ConflictBinding {
                constraint,
                rejection,
                range: TextRange::new(start, end),
            });
        }
        bindings
    }

    fn parse_named_expression(&mut self) -> Expression {
        self.parse_named_expression_with_construction(true)
    }

    fn parse_named_expression_with_construction(&mut self, allow_construction: bool) -> Expression {
        let Some(path) = self.parse_qualified_name() else {
            return Expression::Missing(self.current().range);
        };
        let name_range = TextRange::new(
            path.first().map_or(0, |name| name.range.start),
            path.last().map_or(0, |name| name.range.end),
        );
        let target = NameExpression {
            path,
            range: name_range,
        };

        if self.at(TokenKind::LeftParen) {
            let (arguments, end) = self.parse_arguments().unwrap_or_default();
            Expression::Invocation(InvocationExpression {
                callee: target,
                arguments,
                range: TextRange::new(name_range.start, end),
            })
        } else if allow_construction && self.at(TokenKind::LeftBrace) {
            let (fields, end) = self.parse_object_body().unwrap_or_default();
            Expression::Construction(ConstructionExpression {
                target,
                fields,
                range: TextRange::new(name_range.start, end),
            })
        } else {
            Expression::Name(target)
        }
    }

    fn parse_arguments(&mut self) -> Option<(Vec<Expression>, usize)> {
        self.expect(TokenKind::LeftParen, "expected `(`")?;
        let mut arguments = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            arguments.push(self.parse_expression());
            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        let end = self
            .expect(TokenKind::RightParen, "expected `)` after arguments")?
            .range
            .end;
        Some((arguments, end))
    }

    fn parse_object_body(&mut self) -> Option<(Vec<FieldInitialiser>, usize)> {
        self.expect(TokenKind::LeftBrace, "expected `{`")?;
        let mut fields = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            if let Some(field) = self.parse_field_initialiser() {
                fields.push(field);
            } else {
                self.error_current("SYN_EXPECTED_FIELD_INITIALISER");
                self.recover_until(&[TokenKind::RightBrace]);
            }
            if self.cursor == before {
                self.bump();
            }
        }
        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after object values")?
            .range
            .end;
        Some((fields, end))
    }

    fn parse_field_initialiser(&mut self) -> Option<FieldInitialiser> {
        let start = self.current().range.start;
        let name = self.expect_contextual_name("expected field name")?;
        self.expect(TokenKind::Colon, "expected `:` after field name")?;
        let value = self.parse_expression();
        let end = value.range().end;
        Some(FieldInitialiser {
            name,
            value,
            range: TextRange::new(start, end),
        })
    }

    fn parse_type_reference(&mut self) -> Option<TypeReference> {
        let path = self.parse_qualified_name()?;
        let start = path.first()?.range.start;
        let mut arguments = Vec::new();

        if self.at(TokenKind::LeftAngle) {
            self.bump();
            while !self.at(TokenKind::RightAngle) && !self.at(TokenKind::Eof) {
                arguments.push(self.parse_type_reference()?);
                if self.at(TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect(TokenKind::RightAngle, "expected `>` after type arguments")?;
        }

        let nullable = if self.at(TokenKind::Question) {
            self.bump();
            true
        } else {
            false
        };
        let end = self.previous_significant_end();

        Some(TypeReference {
            path,
            arguments,
            nullable,
            range: TextRange::new(start, end),
        })
    }

    fn parse_qualified_name(&mut self) -> Option<Vec<Name>> {
        let mut path = vec![self.expect_contextual_name("expected name")?];
        while self.at(TokenKind::Dot) {
            self.bump();
            path.push(self.expect_contextual_name("expected name after `.`")?);
        }
        Some(path)
    }

    fn parse_field_block(&mut self) -> Option<(Vec<FieldDeclaration>, usize)> {
        self.expect(TokenKind::LeftBrace, "expected `{` before fields")?;
        let mut fields = Vec::new();

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            let diagnostics_before = self.diagnostics.len();
            if let Some(field) = self.parse_field_declaration() {
                fields.push(field);
            } else {
                if self.diagnostics.len() == diagnostics_before {
                    self.error_current("SYN_EXPECTED_FIELD");
                }
                self.recover_until(&[TokenKind::RightBrace]);
            }
            if self.cursor == before {
                self.bump();
            }
        }

        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after fields")?
            .range
            .end;
        Some((fields, end))
    }

    fn parse_field_declaration(&mut self) -> Option<FieldDeclaration> {
        let start = self.current().range.start;
        let name = self.expect_contextual_name("expected field name")?;
        self.expect(TokenKind::Colon, "expected `:` after field name")?;
        let field_type = self.parse_type_reference()?;
        let constraints = if self.at(TokenKind::LeftBrace) {
            self.parse_constraint_block()?.0
        } else {
            Vec::new()
        };
        let mut persistence = Vec::new();
        while let Some(modifier) = match self.current_kind() {
            TokenKind::Identity => Some(PersistenceModifier::Identity),
            TokenKind::Unique => Some(PersistenceModifier::Unique),
            TokenKind::Index => Some(PersistenceModifier::Index),
            _ => None,
        } {
            let range = self.current().range;
            self.bump();
            if persistence.contains(&modifier) {
                self.error_at("SYN_DUPLICATE_FIELD_MODIFIER", range);
            } else {
                persistence.push(modifier);
            }
        }
        let reference = if self.at(TokenKind::References) {
            let reference_start = self.bump().range.start;
            let target = self.parse_type_reference()?;
            let relationship = if self.at(TokenKind::As) {
                self.bump();
                Some(self.expect_contextual_name("expected relationship name after `as`")?)
            } else {
                None
            };
            self.expect(
                TokenKind::OnDelete,
                "expected `on_delete` after relationship target",
            )?;
            let on_delete = match self.current_kind() {
                TokenKind::Restrict => ReferenceDeleteAction::Restrict,
                TokenKind::Cascade => ReferenceDeleteAction::Cascade,
                TokenKind::SetNull => ReferenceDeleteAction::SetNull,
                _ => {
                    self.error_current("SYN_EXPECTED_DELETE_ACTION");
                    return None;
                }
            };
            let reference_end = self.bump().range.end;
            Some(ReferenceDeclaration {
                target,
                relationship,
                on_delete,
                range: TextRange::new(reference_start, reference_end),
            })
        } else {
            None
        };
        let optional = if self.at(TokenKind::Optional) {
            self.bump();
            true
        } else {
            false
        };
        let end = self.previous_significant_end();

        Some(FieldDeclaration {
            name,
            field_type,
            constraints,
            persistence,
            reference,
            optional,
            range: TextRange::new(start, end),
        })
    }

    fn parse_constraint_block(&mut self) -> Option<(Vec<Constraint>, usize)> {
        self.expect(TokenKind::LeftBrace, "expected `{` before constraints")?;
        let mut constraints = Vec::new();

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            if let Some(constraint) = self.parse_constraint() {
                constraints.push(constraint);
            } else {
                self.error_current("SYN_EXPECTED_CONSTRAINT");
                self.recover_until(&[
                    TokenKind::Min,
                    TokenKind::Max,
                    TokenKind::MinLength,
                    TokenKind::MaxLength,
                    TokenKind::Pattern,
                    TokenKind::Format,
                    TokenKind::RightBrace,
                ]);
            }
            if self.cursor == before {
                self.bump();
            }
        }

        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after constraints")?
            .range
            .end;
        Some((constraints, end))
    }

    fn parse_constraint(&mut self) -> Option<Constraint> {
        let start = self.current().range.start;
        let kind = match self.current_kind() {
            TokenKind::Min => ConstraintKind::Min,
            TokenKind::Max => ConstraintKind::Max,
            TokenKind::MinLength => ConstraintKind::MinLength,
            TokenKind::MaxLength => ConstraintKind::MaxLength,
            TokenKind::Pattern => ConstraintKind::Pattern,
            TokenKind::Format => ConstraintKind::Format,
            _ => return None,
        };
        self.bump();

        let value = match kind {
            ConstraintKind::Min | ConstraintKind::Max => self.parse_number_literal(),
            ConstraintKind::MinLength | ConstraintKind::MaxLength => {
                self.parse_literal_of(TokenKind::IntegerLiteral)
            }
            ConstraintKind::Pattern => self.parse_literal_of(TokenKind::StringLiteral),
            ConstraintKind::Format => self.parse_name_literal(),
        }
        .unwrap_or_else(|| {
            self.error_current("SYN_EXPECTED_CONSTRAINT_VALUE");
            Literal {
                kind: LiteralKind::String,
                text: String::new(),
                range: self.current().range,
            }
        });
        let end = value.range.end;
        Some(Constraint {
            kind,
            value,
            range: TextRange::new(start, end),
        })
    }

    fn parse_number_literal(&mut self) -> Option<Literal> {
        match self.current_kind() {
            TokenKind::IntegerLiteral | TokenKind::DecimalLiteral => self.parse_any_literal(),
            _ => None,
        }
    }

    fn parse_name_literal(&mut self) -> Option<Literal> {
        let token = self.expect_name_token("expected constraint format name")?;
        Some(Literal {
            kind: LiteralKind::String,
            text: token.text(self.source).to_owned(),
            range: token.range,
        })
    }

    fn parse_literal_of(&mut self, expected: TokenKind) -> Option<Literal> {
        if !self.at(expected) {
            self.error_current("SYN_EXPECTED_LITERAL");
            return None;
        }
        self.parse_any_literal()
    }

    fn parse_any_literal(&mut self) -> Option<Literal> {
        let token = self.current();
        let kind = match token.kind {
            TokenKind::StringLiteral => LiteralKind::String,
            TokenKind::IntegerLiteral => LiteralKind::Integer,
            TokenKind::DecimalLiteral => LiteralKind::Decimal,
            TokenKind::BooleanLiteral => LiteralKind::Boolean,
            TokenKind::NoneLiteral => LiteralKind::None,
            _ => return None,
        };
        self.bump();
        Some(Literal {
            kind,
            text: token.text(self.source).to_owned(),
            range: token.range,
        })
    }

    fn parse_route_declaration(&mut self) -> Option<RouteDeclaration> {
        let start = self
            .expect(TokenKind::Route, "expected `route`")?
            .range
            .start;
        let method = match self.current_kind() {
            TokenKind::Get => HttpMethod::Get,
            TokenKind::Post => HttpMethod::Post,
            TokenKind::Put => HttpMethod::Put,
            TokenKind::Patch => HttpMethod::Patch,
            TokenKind::Delete => HttpMethod::Delete,
            _ => {
                self.error_current("SYN_EXPECTED_HTTP_METHOD");
                return None;
            }
        };
        self.bump();
        let path_token = self.expect(TokenKind::RoutePath, "expected route path")?;
        let path = path_token.text(self.source).to_owned();
        self.expect(TokenKind::LeftBrace, "expected `{` after route path")?;

        let mut public = false;
        let mut auth_seen = false;
        let mut path_fields = Vec::new();
        let mut path_seen = false;
        let mut input = None;
        let mut output = None;
        let mut run = None;
        let mut inline_action = None;

        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.cursor;
            match self.current_kind() {
                TokenKind::Auth => {
                    let item = self.bump();
                    if auth_seen {
                        self.error_at("ROUTE_ITEM_DUPLICATE", item.range);
                    }
                    auth_seen = true;
                    let colon_end = self.expect_route_item_colon(item.range);
                    if self.at(TokenKind::NoneLiteral) {
                        self.bump();
                        public = true;
                    } else {
                        let invalid = self.current();
                        let value_is_missing = matches!(
                            invalid.kind,
                            TokenKind::Auth
                                | TokenKind::Path
                                | TokenKind::Input
                                | TokenKind::Output
                                | TokenKind::Run
                                | TokenKind::Action
                                | TokenKind::RightBrace
                                | TokenKind::Eof
                        );
                        let found = if value_is_missing {
                            "missing value".to_owned()
                        } else if invalid.kind == TokenKind::Identifier {
                            invalid.text(self.source).to_owned()
                        } else {
                            "supplied token".to_owned()
                        };
                        let invalid_range = if value_is_missing {
                            TextRange::new(colon_end, colon_end)
                        } else {
                            invalid.range
                        };
                        let route = format!("{} {}", http_method_name(method), path);
                        let (remove_start, remove_end) = route_item_removal_range(
                            self.source,
                            item.range.start,
                            if value_is_missing {
                                colon_end
                            } else {
                                invalid.range.end
                            },
                        );
                        let mut diagnostic = Diagnostic::error("ROUTE_AUTH_VALUE_INVALID")
                            .with_fact(DiagnosticFact::Route(route.clone()))
                            .with_fact(DiagnosticFact::FoundValue(found));
                        diagnostic.impact.affected.push(route);
                        diagnostic.alternatives[0].edits.push(TextEdit {
                            source: self.source_name.clone(),
                            start: remove_start,
                            end: remove_end,
                            replacement: String::new(),
                        });
                        diagnostic.alternatives[1].edits.push(TextEdit {
                            source: self.source_name.clone(),
                            start: invalid_range.start,
                            end: invalid_range.end,
                            replacement: if value_is_missing {
                                " none".to_owned()
                            } else {
                                "none".to_owned()
                            },
                        });
                        self.diagnostic_at(diagnostic, invalid_range);
                        if !value_is_missing {
                            self.bump();
                        }
                    }
                }
                TokenKind::Path => {
                    let item = self.bump();
                    if path_seen {
                        self.error_at("ROUTE_ITEM_DUPLICATE", item.range);
                    }
                    path_seen = true;
                    self.expect_route_item_colon(item.range);
                    if let Some((fields, _)) = self.parse_field_block() {
                        for field in &fields {
                            if field.field_type.nullable
                                || field.optional
                                || !field.constraints.is_empty()
                                || !field.persistence.is_empty()
                                || field.reference.is_some()
                            {
                                self.error_at("ROUTE_PATH_FIELD_MODIFIER_INVALID", field.range);
                            }
                        }
                        path_fields = fields;
                    }
                }
                TokenKind::Input => {
                    let item = self.bump();
                    if input.is_some() {
                        self.error_at("ROUTE_ITEM_DUPLICATE", item.range);
                    }
                    self.expect_route_item_colon(item.range);
                    input = self.parse_type_reference();
                }
                TokenKind::Output => {
                    let item = self.bump();
                    if output.is_some() {
                        self.error_at("ROUTE_ITEM_DUPLICATE", item.range);
                    }
                    self.expect_route_item_colon(item.range);
                    output = self.parse_type_reference();
                }
                TokenKind::Run => {
                    let item = self.bump();
                    if run.is_some() {
                        self.error_at("ROUTE_ITEM_DUPLICATE", item.range);
                    }
                    self.expect_route_item_colon(item.range);
                    let expression = self.parse_named_expression();
                    match expression {
                        Expression::Invocation(invocation) => run = Some(invocation),
                        other => self.error_at("SYN_EXPECTED_INVOCATION", other.range()),
                    }
                }
                TokenKind::Action => {
                    let item = self.bump();
                    if inline_action.is_some() {
                        self.error_at("ROUTE_ITEM_DUPLICATE", item.range);
                    }
                    self.expect_route_item_colon(item.range);
                    let action_start = item.range.start;
                    let mut failures = Vec::new();
                    let mut failures_range = None;
                    if self.at(TokenKind::Fails) {
                        let failures_start = self.bump().range.start;
                        loop {
                            if let Some(failure) =
                                self.expect_name("expected problem name after `fails`")
                            {
                                failures.push(failure);
                            }
                            if !self.at(TokenKind::Comma) {
                                break;
                            }
                            self.bump();
                        }
                        failures_range = failures
                            .last()
                            .map(|failure| TextRange::new(failures_start, failure.range.end));
                    }
                    if let Some(body) = self.parse_block() {
                        inline_action = Some(InlineAction {
                            range: TextRange::new(action_start, body.range.end),
                            failures,
                            failures_range,
                            body,
                        });
                    }
                }
                _ => {
                    self.error_current("SYN_EXPECTED_ROUTE_ITEM");
                    self.recover_until(&[
                        TokenKind::Auth,
                        TokenKind::Path,
                        TokenKind::Input,
                        TokenKind::Output,
                        TokenKind::Run,
                        TokenKind::Action,
                        TokenKind::RightBrace,
                    ]);
                }
            }
            if self.cursor == before {
                self.bump();
            }
        }

        let end = self
            .expect(TokenKind::RightBrace, "expected `}` after route")?
            .range
            .end;

        if run.is_some() == inline_action.is_some() {
            let (code, _message) = if run.is_some() {
                (
                    "ROUTE_BEHAVIOUR_CONFLICT",
                    "route must contain either `run:` or an inline `action`, not both",
                )
            } else {
                (
                    "ROUTE_BEHAVIOUR_REQUIRED",
                    "route must contain exactly one of `run:` or an inline `action`",
                )
            };
            self.error_at(code, TextRange::new(start, path_token.range.end));
        }

        let mut placeholders = Vec::new();
        let mut remaining = path.as_str();
        while let Some(open) = remaining.find('{') {
            let after_open = &remaining[open + 1..];
            let Some(close) = after_open.find('}') else {
                self.error_at("ROUTE_PATH_PLACEHOLDER_INVALID", path_token.range);
                break;
            };
            let name = &after_open[..close];
            if name.is_empty()
                || !name.chars().enumerate().all(|(index, character)| {
                    if index == 0 {
                        character.is_ascii_alphabetic() || character == '_'
                    } else {
                        character.is_ascii_alphanumeric() || character == '_'
                    }
                })
            {
                self.error_at("ROUTE_PATH_PLACEHOLDER_INVALID", path_token.range);
            } else {
                placeholders.push(name.to_owned());
            }
            remaining = &after_open[close + 1..];
        }
        let mut unique_placeholders = BTreeSet::new();
        for placeholder in &placeholders {
            if !unique_placeholders.insert(placeholder.clone()) {
                self.error_at("ROUTE_PATH_PLACEHOLDER_DUPLICATE", path_token.range);
            }
        }
        let declared = path_fields
            .iter()
            .map(|field| field.name.text.clone())
            .collect::<BTreeSet<_>>();
        for placeholder in &unique_placeholders {
            if !declared.contains(placeholder) {
                self.error_at("ROUTE_PATH_BINDING_MISSING", path_token.range);
            }
        }
        let mut seen_bindings = BTreeSet::new();
        for field in &path_fields {
            if !seen_bindings.insert(field.name.text.clone()) {
                self.error_at("ROUTE_PATH_BINDING_DUPLICATE", field.range);
            } else if !unique_placeholders.contains(&field.name.text) {
                self.error_at("ROUTE_PATH_BINDING_EXTRA", field.range);
            }
        }

        Some(RouteDeclaration {
            method,
            path,
            path_range: path_token.range,
            public,
            path_fields,
            input,
            output,
            run,
            inline_action,
            range: TextRange::new(start, end),
        })
    }

    fn expect_name(&mut self, message: &str) -> Option<Name> {
        let token = self.expect(TokenKind::Identifier, message)?;
        Some(Name {
            text: token.text(self.source).to_owned(),
            range: token.range,
        })
    }

    fn expect_contextual_name(&mut self, message: &str) -> Option<Name> {
        let token = self.expect_name_token(message)?;
        Some(Name {
            text: token.text(self.source).to_owned(),
            range: token.range,
        })
    }

    fn expect_name_token(&mut self, _message: &str) -> Option<Token> {
        if matches!(
            self.current_kind(),
            TokenKind::Identifier
                | TokenKind::Input
                | TokenKind::Output
                | TokenKind::Value
                | TokenKind::Path
        ) {
            Some(self.bump())
        } else {
            self.error_current("SYN_EXPECTED_NAME");
            None
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            let range = self.current().range;
            let diagnostic = self.expected_diagnostic("SYN_UNEXPECTED_TOKEN", message);
            self.diagnostic_at(diagnostic, range);
            None
        }
    }

    fn expected_diagnostic(&self, code: &'static str, message: &str) -> Diagnostic {
        let expected = message
            .strip_prefix("expected ")
            .unwrap_or(message)
            .to_owned();
        Diagnostic::error(code)
            .with_fact(DiagnosticFact::Expected(expected))
            .with_fact(DiagnosticFact::FoundValue(diagnostic_token_label(
                self.current(),
                self.source,
            )))
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current_kind() == kind
    }

    fn current_kind(&self) -> TokenKind {
        self.current().kind
    }

    fn current(&self) -> Token {
        self.tokens[self.cursor]
    }

    fn next_significant_kind(&self) -> Option<TokenKind> {
        self.tokens[self.cursor + 1..]
            .iter()
            .find(|token| !token.kind.is_trivia())
            .map(|token| token.kind)
    }

    fn bump(&mut self) -> Token {
        let token = self.current();
        if token.kind != TokenKind::Eof {
            self.cursor += 1;
            self.skip_trivia();
        }
        token
    }

    fn skip_trivia(&mut self) {
        while self
            .tokens
            .get(self.cursor)
            .is_some_and(|token| token.kind.is_trivia())
        {
            self.cursor += 1;
        }
    }

    fn previous_significant_end(&self) -> usize {
        self.tokens[..self.cursor]
            .iter()
            .rev()
            .find(|token| !token.kind.is_trivia())
            .map_or(0, |token| token.range.end)
    }

    fn error_current(&mut self, code: &'static str) {
        self.error_at(code, self.current().range);
    }

    fn expect_route_item_colon(&mut self, item_range: TextRange) -> usize {
        if self.at(TokenKind::Colon) {
            return self.bump().range.end;
        }
        let mut diagnostic = Diagnostic::error("ROUTE_ITEM_COLON_REQUIRED").with_edit(TextEdit {
            source: self.source_name.clone(),
            start: item_range.end,
            end: item_range.end,
            replacement: ":".to_owned(),
        });
        diagnostic.primary = Some(SourceSpan {
            source: self.source_name.clone(),
            start: item_range.start,
            end: item_range.end,
        });
        self.diagnostics.push(diagnostic);
        item_range.end
    }

    fn error_at(&mut self, code: &'static str, range: TextRange) {
        self.diagnostic_at(Diagnostic::error(code), range);
    }

    fn diagnostic_at(&mut self, mut diagnostic: Diagnostic, range: TextRange) {
        diagnostic.primary = Some(SourceSpan {
            source: self.source_name.clone(),
            start: range.start,
            end: range.end,
        });
        self.diagnostics.push(diagnostic);
    }

    fn recover_declaration(&mut self) {
        self.recover_until(&[
            TokenKind::Type,
            TokenKind::Enum,
            TokenKind::Entity,
            TokenKind::Value,
            TokenKind::Input,
            TokenKind::Output,
            TokenKind::Failure,
            TokenKind::Function,
            TokenKind::Action,
            TokenKind::Test,
            TokenKind::Route,
            TokenKind::Eof,
        ]);
    }

    fn recover_statement(&mut self) {
        self.recover_until(&[
            TokenKind::Var,
            TokenKind::Return,
            TokenKind::Reject,
            TokenKind::If,
            TokenKind::Match,
            TokenKind::Assert,
            TokenKind::Throw,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]);
    }

    fn recover_until(&mut self, kinds: &[TokenKind]) {
        while !kinds.contains(&self.current_kind()) && !self.at(TokenKind::Eof) {
            self.bump();
        }
    }
}

fn http_method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn diagnostic_token_label(token: Token, source: &str) -> String {
    let text = token.text(source);
    if !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return text.to_owned();
    }
    match token.kind {
        TokenKind::StringLiteral => "string literal".to_owned(),
        TokenKind::IntegerLiteral | TokenKind::DecimalLiteral => "number literal".to_owned(),
        TokenKind::RoutePath => "route path".to_owned(),
        TokenKind::LeftBrace => "opening brace".to_owned(),
        TokenKind::RightBrace => "closing brace".to_owned(),
        TokenKind::Colon => "colon".to_owned(),
        TokenKind::Comma => "comma".to_owned(),
        TokenKind::Eof => "end of file".to_owned(),
        _ => "this token".to_owned(),
    }
}

fn route_item_removal_range(source: &str, item_start: usize, value_end: usize) -> (usize, usize) {
    let line_start = source[..item_start]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let line_end = source[value_end..]
        .find('\n')
        .map_or(source.len(), |offset| value_end + offset + 1);
    let value_line_end = line_end.saturating_sub(usize::from(
        line_end > 0 && source.as_bytes()[line_end - 1] == b'\n',
    ));
    if source[line_start..item_start].trim().is_empty()
        && source[value_end..value_line_end].trim().is_empty()
    {
        (line_start, line_end)
    } else {
        (item_start, value_end)
    }
}

fn declaration_name(declaration: &Declaration) -> Option<&Name> {
    match declaration {
        Declaration::Type(declaration) => Some(&declaration.name),
        Declaration::Enum(declaration) => Some(&declaration.name),
        Declaration::Record(declaration) => Some(&declaration.name),
        Declaration::Failure(declaration) => Some(&declaration.name),
        Declaration::Callable(declaration) => Some(&declaration.name),
        Declaration::Test(_) => None,
        Declaration::Route(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::{
        CallableKind, Declaration, HttpMethod, PersistenceConstraintKind, PersistenceModifier,
        QueryOrderDirection, RecordKind, ReferenceDeleteAction, Statement,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("syntax crate should be inside the repository")
    }

    #[test]
    fn parses_the_jadpo_seed_without_diagnostics() {
        let path = repository_root().join("examples/jadpo-seed/app.jadpo");
        let source = fs::read_to_string(&path).expect("seed should be readable");
        let parsed = parse(&path, &source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let outline = parsed
            .file
            .declarations
            .iter()
            .map(declaration_outline)
            .collect::<Vec<_>>();
        assert_eq!(
            outline,
            vec![
                "type Email",
                "type InviteCode",
                "entity Customer",
                "input RegisterCustomer",
                "output RegistrationAccepted",
                "failure InviteCodeRejected",
                "action register_customer",
                "route POST /registrations",
                "route GET /registrations/{email}",
            ]
        );
    }

    #[test]
    fn explains_when_record_fields_are_written_as_a_type_alias() {
        let source = "type User = {\n    id: UUID\n}\n";
        let parsed = parse(Path::new("inline.jadpo"), source);

        assert_eq!(parsed.diagnostics.len(), 1);
        assert_eq!(parsed.diagnostics[0].code, "SYN_TYPE_PARENT_REQUIRED");
        assert_eq!(parsed.diagnostics[0].primary.as_ref().unwrap().start, 12);
        assert!(parsed.diagnostics[0].notes[0].contains("entity User"));
    }

    #[test]
    fn parses_module_headers_selective_imports_and_public_declarations() {
        let source = r#"
module todo.api
import todo.domain { Todo, TodoTitle }

public input CreateTodo { title: TodoTitle }
output PrivateResult { todo: Todo }
"#;
        let parsed = parse(Path::new("api.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        assert_eq!(
            parsed
                .file
                .module
                .as_ref()
                .unwrap()
                .path
                .iter()
                .map(|part| part.text.as_str())
                .collect::<Vec<_>>(),
            vec!["todo", "api"]
        );
        assert_eq!(parsed.file.imports.len(), 1);
        assert_eq!(
            parsed.file.imports[0]
                .names
                .iter()
                .map(|name| name.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Todo", "TodoTitle"]
        );
        assert_eq!(
            parsed
                .file
                .exports
                .iter()
                .map(|name| name.text.as_str())
                .collect::<Vec<_>>(),
            vec!["CreateTodo"]
        );
    }

    #[test]
    fn requires_colons_between_route_items_and_values() {
        let source = "route GET /health { auth public explicitly output Health run health() }";
        let parsed = parse(Path::new("inline.jadpo"), source);

        assert!(parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUTE_ITEM_COLON_REQUIRED"));
        assert!(parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUTE_AUTH_VALUE_INVALID"));
    }

    #[test]
    fn invalid_route_auth_value_is_one_human_owned_root_diagnostic() {
        let source = r#"input RegisterCustomer { email: Text }
output RegistrationAccepted { accepted: Bool }
action register_customer(input: RegisterCustomer) -> RegistrationAccepted {
    return RegistrationAccepted { accepted: true }
}
route POST /registrations {
    auth: nonke
    input: RegisterCustomer
    output: RegistrationAccepted
    run: register_customer(input)
}
"#;
        let parsed = parse(Path::new("auth-value.jadpo"), source);

        assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.code, "ROUTE_AUTH_VALUE_INVALID");
        assert_eq!(
            diagnostic.message,
            "Route authentication value `nonke` is not valid"
        );
        assert_eq!(diagnostic.decision_owner.as_str(), "human");
        assert_eq!(diagnostic.alternatives.len(), 2);
        assert!(diagnostic
            .alternatives
            .iter()
            .all(|choice| !choice.preferred && choice.edits.len() == 1));
        let primary = diagnostic.primary.as_ref().expect("primary range");
        assert_eq!(&source[primary.start..primary.end], "nonke");

        let route = parsed
            .file
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                Declaration::Route(route) => Some(route),
                _ => None,
            })
            .expect("the route should survive parser recovery");
        assert!(route.input.is_some());
        assert!(route.output.is_some());
        assert!(route.run.is_some());
        assert!(!route.public);
    }

    #[test]
    fn requires_colons_for_block_valued_route_items_with_repairs() {
        let source = "output Health { ok: Bool } route GET /health/{id} { path { id: Uuid } output: Health action { return Health { ok: true } } }";
        let parsed = parse(Path::new("route-blocks.jadpo"), source);
        let diagnostics = parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "ROUTE_ITEM_COLON_REQUIRED")
            .collect::<Vec<_>>();

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[0].recommended_next_step.edits[0].replacement,
            ":"
        );
        assert_eq!(
            diagnostics[1].recommended_next_step.edits[0].replacement,
            ":"
        );
    }

    #[test]
    fn required_queries_require_a_colon_separated_missing_failure() {
        let source = "entity Customer { id: Uuid } action find(id: Customer.id) -> Customer { return query required Customer { where: id == id missing CustomerNotFound } }";
        let parsed = parse(Path::new("required-query.jadpo"), source);

        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SYN_UNEXPECTED_TOKEN"
                && diagnostic.message == "Expected `:` after `missing`"
        }));
    }

    #[test]
    fn queries_require_a_colon_after_where() {
        let source = "entity Customer { id: Uuid } action find(id: Customer.id) -> Customer? { return query optional Customer { where id == id } }";
        let parsed = parse(Path::new("query-where.jadpo"), source);

        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SYN_UNEXPECTED_TOKEN"
                && diagnostic.message == "Expected `:` after `where`"
        }));
    }

    #[test]
    fn query_clauses_require_colon_separated_values() {
        let cases = [
            (
                "query many Customer { where: id == id order_by id asc }",
                "expected `:` after `order_by`",
            ),
            (
                "query many Customer { where: id == id order_by: id asc limit 2 offset: 0 }",
                "expected `:` after `limit`",
            ),
            (
                "query many Customer { where: id == id order_by: id asc limit: 2 offset 0 }",
                "expected `:` after `offset`",
            ),
            (
                "query required Customer { where: id == id include todos into: CustomerTodos order_by: id asc limit: 2 offset: 0 missing: Missing }",
                "expected `:` after `include`",
            ),
            (
                "query required Customer { where: id == id include: todos into CustomerTodos order_by: id asc limit: 2 offset: 0 missing: Missing }",
                "expected `:` after `into`",
            ),
        ];

        for (query, expected_syntax) in cases {
            let source = format!(
                "entity Customer {{ id: Uuid }} action find(id: Customer.id) -> Customer? {{ return {query} }}"
            );
            let parsed = parse(Path::new("query-clause.jadpo"), &source);
            assert!(
                parsed.diagnostics.iter().any(|diagnostic| diagnostic.code
                    == "SYN_UNEXPECTED_TOKEN"
                    && diagnostic.message == expected_syntax.replacen("expected", "Expected", 1)),
                "missing diagnostic for {expected_syntax:?} in {:#?}",
                parsed.diagnostics
            );
        }
    }

    #[test]
    fn mutations_require_colon_separated_items() {
        let source = "entity Customer { id: Uuid } failure Missing { kind NotFound code \"missing\" } failure Clashed { kind Conflict code \"clashed\" } action change(id: Customer.id) -> Customer fails Missing, Clashed { return update required Customer { where: id == id set { id: id } missing: Missing conflict: Clashed } }";
        let parsed = parse(Path::new("mutation.jadpo"), source);

        assert!(parsed.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SYN_UNEXPECTED_TOKEN"
                && diagnostic.message == "Expected `:` after `set`"
        }));
    }

    #[test]
    fn parses_entity_persistence_modifiers_after_the_field_type() {
        let source = "entity Account { id: Uuid identity email: Text unique owner: Text index }";
        let parsed = parse(Path::new("entity-modifiers.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Record(account) = &parsed.file.declarations[0] else {
            panic!("expected entity declaration");
        };
        assert_eq!(
            account.fields[0].persistence,
            vec![PersistenceModifier::Identity]
        );
        assert_eq!(
            account.fields[1].persistence,
            vec![PersistenceModifier::Unique]
        );
        assert_eq!(
            account.fields[2].persistence,
            vec![PersistenceModifier::Index]
        );
    }

    #[test]
    fn parses_named_compound_constraints_and_precise_conflicts() {
        let source = "entity Membership { id: Uuid identity tenant: Text email: Text constraint tenant_email: unique(tenant, email) } failure Exists { kind Conflict code \"exists\" } action add(id: Membership.id, tenant: Membership.tenant, email: Membership.email) -> Membership fails Exists { return create Membership { id: id tenant: tenant email: email } conflict Membership.tenant_email: Exists }";
        let parsed = parse(Path::new("compound-constraint.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Record(entity) = &parsed.file.declarations[0] else {
            panic!("expected entity declaration");
        };
        assert_eq!(entity.persistence_constraints.len(), 1);
        assert_eq!(
            entity.persistence_constraints[0].kind,
            PersistenceConstraintKind::Unique
        );
        assert_eq!(
            entity.persistence_constraints[0]
                .fields
                .iter()
                .map(|field| field.text.as_str())
                .collect::<Vec<_>>(),
            vec!["tenant", "email"]
        );
        let Declaration::Callable(callable) = &parsed.file.declarations[2] else {
            panic!("expected action declaration");
        };
        let Statement::Return(statement) = &callable.body.statements[0] else {
            panic!("expected return statement");
        };
        let crate::Expression::Create(create) = &statement.value else {
            panic!("expected create expression");
        };
        assert_eq!(
            create.conflicts[0]
                .constraint
                .as_ref()
                .map(|constraint| constraint.path[1].text.as_str()),
            Some("tenant_email")
        );
    }

    #[test]
    fn parses_a_colon_separated_create_conflict_binding() {
        let source = "entity Account { id: Uuid identity } failure AccountConflict { kind Conflict code \"account_conflict\" } action add(id: Account.id) -> Account fails AccountConflict { return create Account { id: id } conflict: AccountConflict }";
        let parsed = parse(Path::new("create-conflict.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Callable(callable) = &parsed.file.declarations[2] else {
            panic!("expected action declaration");
        };
        let Statement::Return(statement) = &callable.body.statements[0] else {
            panic!("expected return statement");
        };
        let crate::Expression::Create(create) = &statement.value else {
            panic!("expected create expression");
        };
        assert_eq!(
            create
                .conflicts
                .first()
                .map(|binding| binding.rejection.failure.text.as_str()),
            Some("AccountConflict")
        );
    }

    #[test]
    fn parses_an_explicit_entity_reference_and_delete_lifecycle() {
        let source = "entity User { id: Uuid identity } entity Todo { id: Uuid identity owner_id: User.id references User.id as owner on_delete cascade }";
        let parsed = parse(Path::new("relationship.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Record(todo) = &parsed.file.declarations[1] else {
            panic!("expected todo entity");
        };
        let reference = todo.fields[1]
            .reference
            .as_ref()
            .expect("owner field should carry relationship metadata");
        assert_eq!(
            reference
                .target
                .path
                .iter()
                .map(|part| part.text.as_str())
                .collect::<Vec<_>>(),
            vec!["User", "id"]
        );
        assert_eq!(
            reference
                .relationship
                .as_ref()
                .map(|name| name.text.as_str()),
            Some("owner")
        );
        assert_eq!(reference.on_delete, ReferenceDeleteAction::Cascade);
    }

    #[test]
    fn parses_a_deterministically_ordered_many_query() {
        let source = "entity User { id: Uuid identity } entity Todo { id: Uuid identity owner_id: User.id references User.id on_delete cascade } action list(user_id: User.id) -> List<Todo> { return query many Todo { where: owner_id == user_id order_by: id asc } }";
        let parsed = parse(Path::new("many-query.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Callable(callable) = &parsed.file.declarations[2] else {
            panic!("expected action declaration");
        };
        let Statement::Return(statement) = &callable.body.statements[0] else {
            panic!("expected return statement");
        };
        let crate::Expression::Query(query) = &statement.value else {
            panic!("expected query expression");
        };
        assert_eq!(query.cardinality, crate::QueryCardinality::Many);
        assert_eq!(query.order.as_ref().unwrap().field.text, "id");
    }

    #[test]
    fn parses_an_inverse_and_explicit_batched_include() {
        let source = r#"
entity User {
    id: Uuid identity
    inverse todos: many Todo via Todo.owner_id
    inverse notes: many Note via Note.owner_id
}
entity Todo { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
entity Note { id: Uuid identity owner_id: User.id references User.id on_delete cascade }
output UserActivity { parent: User todos: List<Todo> notes: List<Note> }
failure UserNotFound { kind NotFound code "user_not_found" }
action load(user_id: User.id) -> UserActivity fails UserNotFound {
    return query required User {
        where: id == user_id
        include: todos into: UserActivity order_by: id asc limit: 100 offset: 0
        include: notes into: UserActivity order_by: id desc limit: 50 offset: 10
        missing: UserNotFound
    }
}
"#;
        let parsed = parse(Path::new("inverse-query.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Record(user) = &parsed.file.declarations[0] else {
            panic!("expected user entity");
        };
        assert_eq!(user.inverses[0].name.text, "todos");
        assert_eq!(user.inverses[0].target.text, "Todo");
        let Declaration::Callable(callable) = &parsed.file.declarations[5] else {
            panic!("expected load action");
        };
        let Statement::Return(statement) = &callable.body.statements[0] else {
            panic!("expected return statement");
        };
        let crate::Expression::Query(query) = &statement.value else {
            panic!("expected query expression");
        };
        let include = query.includes.first().expect("include should be parsed");
        assert_eq!(include.relationship.text, "todos");
        assert_eq!(include.result.path[0].text, "UserActivity");
        assert_eq!(include.order.field.text, "id");
        let crate::Expression::Literal(limit) = include.pagination.limit.as_ref() else {
            panic!("expected literal limit");
        };
        assert_eq!(limit.text, "100");
        assert_eq!(query.includes.len(), 2);
        assert_eq!(query.includes[1].relationship.text, "notes");
        assert_eq!(
            query.includes[1].order.direction,
            QueryOrderDirection::Descending
        );
    }

    #[test]
    fn parses_explicit_local_reassignment() {
        let source = r#"
type Choice = Text {}
function choose(initial: Choice, replacement: Choice) -> Choice {
    var mut selected: Choice = initial
    selected = replacement
    return selected
}
"#;
        let parsed = parse(Path::new("assignment.jadpo"), source);

        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let Declaration::Callable(callable) = &parsed.file.declarations[1] else {
            panic!("expected callable declaration");
        };
        let Statement::Assignment(assignment) = &callable.body.statements[1] else {
            panic!("expected assignment statement");
        };
        assert_eq!(assignment.target.text, "selected");
        assert!(matches!(assignment.value, crate::Expression::Name(_)));
    }

    #[test]
    fn parses_passing_fixtures_and_preserves_selected_parser_diagnostics() {
        let root = repository_root().join("tests/compile");
        let mut paths = Vec::new();
        collect_sources(&root, &mut paths);
        paths.sort();

        for path in paths {
            let source = fs::read_to_string(&path).expect("fixture should be readable");
            let parsed = parse(&path, &source);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");

            if file_name == "10_arbitrary_exception.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "SYN_UNSUPPORTED_THROW");
                let Declaration::Callable(callable) = &parsed.file.declarations[1] else {
                    panic!("expected function declaration");
                };
                assert!(matches!(
                    callable.body.statements[0],
                    Statement::Unsupported(_)
                ));
            } else if file_name == "59_route_path_binding_mismatch.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 2, "{:#?}", parsed.diagnostics);
                assert_eq!(
                    parsed
                        .diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.code)
                        .collect::<Vec<_>>(),
                    vec!["ROUTE_PATH_BINDING_MISSING", "ROUTE_PATH_BINDING_EXTRA"]
                );
            } else if file_name == "62_route_behaviour_conflict.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "ROUTE_BEHAVIOUR_CONFLICT");
            } else if file_name == "67_route_path_modifier_invalid.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(
                    parsed.diagnostics[0].code,
                    "ROUTE_PATH_FIELD_MODIFIER_INVALID"
                );
            } else if file_name == "69_duplicate_route_item.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "ROUTE_ITEM_DUPLICATE");
            } else if file_name == "70_route_block_colon_required.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 2, "{:#?}", parsed.diagnostics);
                assert!(parsed
                    .diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.code == "ROUTE_ITEM_COLON_REQUIRED"));
            } else if file_name == "71_invalid_route_auth_value.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "ROUTE_AUTH_VALUE_INVALID");
            } else if file_name == "72_top_level_route_item.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "SYN_EXPECTED_DECLARATION");
                assert_eq!(
                    parsed.diagnostics[0].message,
                    "`path` cannot start a top-level declaration"
                );
            } else if file_name == "73_route_scalar_colon_required.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 4, "{:#?}", parsed.diagnostics);
                assert!(parsed
                    .diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.code == "ROUTE_ITEM_COLON_REQUIRED"));
            } else if file_name == "74_record_field_colon_required.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "SYN_UNEXPECTED_TOKEN");
                assert_eq!(
                    parsed.diagnostics[0].message,
                    "Expected `:` after field name"
                );
            } else if file_name == "75_invalid_string_escape.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "SYN_INVALID_ESCAPE");
            } else if file_name == "76_unexpected_character.jadpo" {
                assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
                assert_eq!(parsed.diagnostics[0].code, "SYN_UNEXPECTED_CHARACTER");
            } else if path
                .components()
                .any(|component| component.as_os_str() == "pass")
            {
                assert!(
                    parsed.diagnostics.is_empty(),
                    "{}: {:#?}",
                    path.display(),
                    parsed.diagnostics
                );
            }
        }
    }

    #[test]
    fn recovers_and_reports_multiple_syntax_errors() {
        let source = r#"
type Email = Text {
    max_length
}

value Result {
    ok Bool
}
"#;
        let parsed = parse(Path::new("malformed.jadpo"), source);

        assert!(parsed.diagnostics.len() >= 2, "{:#?}", parsed.diagnostics);
        assert_eq!(parsed.file.declarations.len(), 2);
    }

    #[test]
    fn reports_each_specific_parser_expectation() {
        let cases = [
            ("input Bad { constraint named: unique(value) }", "SYN_CONSTRAINT_NON_ENTITY"),
            ("entity Bad { id: Uuid identity identity }", "SYN_DUPLICATE_FIELD_MODIFIER"),
            ("module one\nimport two { }\ntype Name = Text {}", "SYN_EMPTY_IMPORT"),
            ("enum Empty {}", "SYN_ENUM_EMPTY"),
            ("type Name = Text { unknown }", "SYN_EXPECTED_CONSTRAINT"),
            ("type Name = Text { max_length }", "SYN_EXPECTED_CONSTRAINT_VALUE"),
            ("entity User { id: Uuid identity } entity Bad { owner: User.id references User.id on_delete unknown }", "SYN_EXPECTED_DELETE_ACTION"),
            ("function bad() -> Bool { return }", "SYN_EXPECTED_EXPRESSION"),
            ("failure Bad { unknown }", "SYN_EXPECTED_FAILURE_ITEM"),
            ("output Result { ok: Bool } function bad() -> Result { return Result { 123 } }", "SYN_EXPECTED_FIELD_INITIALISER"),
            ("route UNKNOWN /bad { auth: none action: { return true } }", "SYN_EXPECTED_HTTP_METHOD"),
            ("entity User { id: Uuid identity inverse items: wrong Item via Item.user_id } entity Item { id: Uuid identity user_id: User.id references User.id on_delete cascade }", "SYN_EXPECTED_INVERSE_CARDINALITY"),
            ("route GET /bad { auth: none run: true }", "SYN_EXPECTED_INVOCATION"),
            ("test Bad { }", "SYN_EXPECTED_LITERAL"),
            ("type Name = Other.123 {}", "SYN_EXPECTED_NAME"),
            ("entity Item { id: Uuid identity } action list() -> List<Item> { return query many Item { where: id == id order_by: id sideways } }", "SYN_EXPECTED_ORDER_DIRECTION"),
            ("route GET /bad { auth: none unknown: true }", "SYN_EXPECTED_ROUTE_ITEM"),
            ("function bad() -> Bool { type }", "SYN_EXPECTED_STATEMENT"),
            ("failure MissingKind { code \"missing_kind\" }", "SYN_FAILURE_KIND_REQUIRED"),
            ("failure MissingCode { kind NotFound }", "SYN_FAILURE_CODE_REQUIRED"),
            ("import shared.names { Name }\ntype Local = Text {}", "SYN_IMPORT_REQUIRES_MODULE"),
            ("input Bad { inverse items: many Item via Item.bad }", "SYN_INVERSE_NON_ENTITY"),
            ("enum Choice { Yes No } function bad(value: Choice) -> Bool { match value { if => {} } return true }", "SYN_MATCH_PATTERN"),
            ("public route GET /bad { auth: none action: { return true } }", "SYN_ROUTE_EXPORT_INVALID"),
            ("entity Item { id: Uuid identity } input PatchItem { id: Item.id optional } failure Empty { kind InvalidValue code \"empty\" } failure Missing { kind NotFound code \"missing\" } action bad(id: Item.id, input: PatchItem) -> Item fails Empty, Missing { return update required Item { where: id == id patch: empty: Empty missing: Missing } }", "SYN_EXPECTED_PATCH_INPUT"),
            ("entity Item { id: Uuid identity } input PatchItem { id: Item.id optional } failure Empty { kind InvalidValue code \"empty\" } failure Missing { kind NotFound code \"missing\" } action bad(id: Item.id, input: PatchItem) -> Item fails Empty, Missing { return update required Item { where: id == id patch: input empty: Empty set: { id: id when return } missing: Missing } }", "SYN_EXPECTED_SUPPLIED_FIELD"),
            ("entity Item { id: Uuid identity } input PatchItem { id: Item.id optional } failure Empty { kind InvalidValue code \"empty\" } failure Missing { kind NotFound code \"missing\" } action bad(id: Item.id, input: PatchItem) -> Item fails Empty, Missing { return update required Item { where: id == id patch: input empty: Empty set: { id: id when input.id wrong } missing: Missing } }", "SYN_EXPECTED_SUPPLIED"),
            ("entity Item { id: Uuid identity } failure Missing { kind NotFound code \"missing\" } action bad(id: Item.id) -> Item fails Missing { return update required Item { where: id == id missing: Missing } }", "SYN_EXPECTED_UPDATE_BODY"),
            ("entity Item { id: Uuid identity } failure Missing { kind NotFound code \"missing\" } action bad(id: Item.id) -> Item fails Missing { return delete required Item { where: id == id missing: Missing } }", "SYN_MUTATION_CONFLICT_REQUIRED"),
        ];

        for (source, expected) in cases {
            let parsed = parse(Path::new("specific-parser-error.jadpo"), source);
            let codes = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>();
            assert!(
                codes.contains(&expected),
                "{expected}: {codes:#?}\n{source}"
            );
        }
    }

    fn collect_sources(path: &Path, output: &mut Vec<PathBuf>) {
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

    fn declaration_outline(declaration: &Declaration) -> String {
        match declaration {
            Declaration::Type(declaration) => format!("type {}", declaration.name.text),
            Declaration::Enum(declaration) => format!("enum {}", declaration.name.text),
            Declaration::Record(declaration) => {
                let keyword = match declaration.kind {
                    RecordKind::Entity => "entity",
                    RecordKind::Value => "value",
                    RecordKind::Input => "input",
                    RecordKind::Output => "output",
                };
                format!("{keyword} {}", declaration.name.text)
            }
            Declaration::Failure(declaration) => {
                format!("failure {}", declaration.name.text)
            }
            Declaration::Callable(declaration) => {
                let keyword = match declaration.kind {
                    CallableKind::Function => "function",
                    CallableKind::Action => "action",
                };
                format!("{keyword} {}", declaration.name.text)
            }
            Declaration::Test(declaration) => format!("test {}", declaration.name.text),
            Declaration::Route(declaration) => {
                let method = match declaration.method {
                    HttpMethod::Get => "GET",
                    HttpMethod::Post => "POST",
                    HttpMethod::Put => "PUT",
                    HttpMethod::Patch => "PATCH",
                    HttpMethod::Delete => "DELETE",
                };
                format!("route {method} {}", declaration.path)
            }
        }
    }
}
