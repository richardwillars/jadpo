//! Closed reminder descriptor syntax; no expression evaluation or authority.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum Shape {
    Name,
    Reference,
    Open,
    Order,
    Limit,
    Version,
    Term(&'static str, JobDeliveryTerm),
    Block(Section),
}
#[derive(Clone, Copy)]
enum Section {
    Delivery,
    Selection,
    Hooks,
    Service,
    Payload,
    Authority,
    Completion,
}

enum ParsedValue {
    Name(Name),
    Reference(NameExpression),
    Open(NameExpression, NameExpression),
    Order([JobDeliveryOrder; 2]),
    Limit(JobDeliveryLimit),
    Version(JobDeliveryPayloadVersion),
    Term(JobDeliveryTerm),
    Block(ParsedBlock),
}
struct ParsedBlock {
    entries: BTreeMap<&'static str, ParsedValue>,
    range: TextRange,
}

macro_rules! getter {
    ($method:ident, $variant:ident, $ty:ty) => {
        fn $method(&mut self, key: &str) -> Option<$ty> {
            match self.entries.remove(key)? {
                ParsedValue::$variant(value) => Some(value),
                _ => None,
            }
        }
    };
}
impl ParsedBlock {
    getter!(name, Name, Name);
    getter!(reference, Reference, NameExpression);
    getter!(order, Order, [JobDeliveryOrder; 2]);
    getter!(limit, Limit, JobDeliveryLimit);
    getter!(version, Version, JobDeliveryPayloadVersion);
    getter!(term, Term, JobDeliveryTerm);
    getter!(block, Block, ParsedBlock);
    fn open(&mut self) -> Option<(NameExpression, NameExpression)> {
        match self.entries.remove("open")? {
            ParsedValue::Open(field, variant) => Some((field, variant)),
            _ => None,
        }
    }
}

impl Section {
    fn keys(self) -> &'static [(&'static str, Shape)] {
        use Section::*;
        use Shape::*;
        match self {
            Delivery => &[
                ("selection", Block(Selection)),
                ("hooks", Block(Hooks)),
                ("service", Block(Service)),
                ("authority", Block(Authority)),
                ("completion", Block(Completion)),
            ],
            Selection => &[
                ("name", Name),
                ("entity", Name),
                ("identity", Reference),
                ("due", Reference),
                (
                    "before",
                    Term("operation_time", JobDeliveryTerm::StrictBeforeOperationTime),
                ),
                ("open", Open),
                ("visible", Name),
                ("unsent", Reference),
                ("required_owner", Reference),
                ("owner_visible", Name),
                ("order_by", Order),
                ("limit", Limit),
                (
                    "continuation",
                    Term("due_identity", JobDeliveryTerm::DueIdentityContinuation),
                ),
            ],
            Hooks => &[
                ("create", Reference),
                ("patch", Reference),
                ("supplied", Reference),
            ],
            Service => &[
                ("operation", Reference),
                ("intent", Name),
                ("input", Name),
                ("output", Name),
                ("payload_version", Version),
                ("payload", Block(Payload)),
            ],
            Payload => &[
                (
                    "idempotency_key",
                    Term("generated_intent", JobDeliveryTerm::GeneratedIntent),
                ),
                ("from", Reference),
                ("to", Reference),
                ("todo_title", Reference),
                ("due_at", Reference),
            ],
            Authority => &[
                ("validator", Reference),
                ("membership", Name),
                ("role", Reference),
                (
                    "permit",
                    Term("reminder_only", JobDeliveryTerm::ReminderOnly),
                ),
            ],
            Completion => &[
                ("name", Name),
                ("field", Reference),
                (
                    "time",
                    Term(
                        "compiler_receipt_observation",
                        JobDeliveryTerm::CompilerReceiptObservation,
                    ),
                ),
            ],
        }
    }
}

impl<'source> Parser<'source> {
    pub(super) fn parse_job_delivery(&mut self, start: usize) -> Option<JobReminderDelivery> {
        let diagnostic_start = self.diagnostics.len();
        self.delivery_expect(TokenKind::Colon)?;
        let mode = self.expect_contextual_name("expected closed delivery mode")?;
        if mode.text != "reminder_v1" {
            self.error_at("SYN_JOB_DELIVERY_MODE_INVALID", mode.range);
        }
        let mut block = self.delivery_block(Section::Delivery)?;
        let range = TextRange::new(start, block.range.end);
        let mut selection = block.block("selection")?;
        let (open_field, open_variant) = selection.open()?;
        let selection = JobDeliverySelection {
            name: selection.name("name")?,
            entity: selection.name("entity")?,
            identity: selection.reference("identity")?,
            due: selection.reference("due")?,
            before: selection.term("before")?,
            open_field,
            open_variant,
            visible: selection.name("visible")?,
            unsent: selection.reference("unsent")?,
            required_owner: selection.reference("required_owner")?,
            owner_visible: selection.name("owner_visible")?,
            order_by: selection.order("order_by")?,
            limit: selection.limit("limit")?,
            continuation: selection.term("continuation")?,
            range: selection.range,
        };
        let mut hooks = block.block("hooks")?;
        let hooks = JobDeliveryHooks {
            create: hooks.reference("create")?,
            patch: hooks.reference("patch")?,
            supplied: hooks.reference("supplied")?,
            range: hooks.range,
        };
        let mut service = block.block("service")?;
        let mut payload = service.block("payload")?;
        let payload = JobDeliveryPayload {
            idempotency_key: payload.term("idempotency_key")?,
            from: payload.reference("from")?,
            to: payload.reference("to")?,
            todo_title: payload.reference("todo_title")?,
            due_at: payload.reference("due_at")?,
            range: payload.range,
        };
        let service = JobDeliveryService {
            operation: service.reference("operation")?,
            intent: service.name("intent")?,
            input: service.name("input")?,
            output: service.name("output")?,
            payload_version: service.version("payload_version")?,
            payload,
            range: service.range,
        };
        let mut authority = block.block("authority")?;
        let authority = JobDeliveryAuthority {
            validator: authority.reference("validator")?,
            membership: authority.name("membership")?,
            role: authority.reference("role")?,
            permit: authority.term("permit")?,
            range: authority.range,
        };
        let mut completion = block.block("completion")?;
        let completion = JobDeliveryCompletion {
            name: completion.name("name")?,
            field: completion.reference("field")?,
            time: completion.term("time")?,
            range: completion.range,
        };
        // Malformed/duplicate syntax cannot become a source descriptor either.
        (diagnostic_start == self.diagnostics.len()).then_some(JobReminderDelivery {
            selection,
            hooks,
            service,
            authority,
            completion,
            range,
        })
    }

    fn delivery_expect(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            self.error_current("SYN_JOB_DELIVERY_SYNTAX_INVALID");
            None
        }
    }

    fn delivery_block(&mut self, section: Section) -> Option<ParsedBlock> {
        let start = self.delivery_expect(TokenKind::LeftBrace)?.range.start;
        let mut entries = BTreeMap::new();
        let mut seen = BTreeSet::new();
        while !self.at(TokenKind::RightBrace)
            && !self.at(TokenKind::Eof)
            && !self.delivery_declaration_ahead()
        {
            let item = self.bump();
            let key = item.text(self.source);
            let Some(&(key, shape)) = section.keys().iter().find(|(name, _)| *name == key) else {
                self.error_at("SYN_JOB_DELIVERY_KEY_UNKNOWN", item.range);
                self.recover_delivery_value();
                continue;
            };
            if !seen.insert(key) {
                self.error_at("SYN_JOB_DELIVERY_KEY_DUPLICATE", item.range);
            }
            if self.delivery_expect(TokenKind::Colon).is_none() {
                self.recover_delivery_value();
                continue;
            }
            if let Some(value) = self.delivery_value(shape) {
                if !self.at(TokenKind::RightBrace)
                    && !self.at(TokenKind::Eof)
                    && !self.delivery_key_ahead()
                {
                    self.error_current(if matches!(shape, Shape::Reference) {
                        "SYN_JOB_DELIVERY_REFERENCE_INVALID"
                    } else {
                        "SYN_JOB_DELIVERY_SYNTAX_INVALID"
                    });
                    self.recover_delivery_value();
                } else {
                    entries.entry(key).or_insert(value);
                }
            } else {
                self.recover_delivery_value();
            }
        }
        let end = self.delivery_expect(TokenKind::RightBrace)?.range.end;
        for (key, _) in section.keys() {
            if !entries.contains_key(key) {
                self.error_at("SYN_JOB_DELIVERY_KEY_REQUIRED", TextRange::new(start, end));
            }
        }
        Some(ParsedBlock {
            entries,
            range: TextRange::new(start, end),
        })
    }

    fn delivery_value(&mut self, shape: Shape) -> Option<ParsedValue> {
        use Shape::*;
        match shape {
            Name => Some(ParsedValue::Name(
                self.expect_contextual_name("expected declaration name")?,
            )),
            Reference => Some(ParsedValue::Reference(self.delivery_reference()?)),
            Open => {
                let field = self.delivery_reference()?;
                self.delivery_expect(TokenKind::LeftParen)?;
                let variant = self.delivery_reference()?;
                self.delivery_expect(TokenKind::RightParen)?;
                Some(ParsedValue::Open(field, variant))
            }
            Order => {
                let first = self.delivery_order()?;
                self.delivery_expect(TokenKind::Comma)?;
                let second = self.delivery_order()?;
                Some(ParsedValue::Order([first, second]))
            }
            Limit => {
                if self.at(TokenKind::IntegerLiteral) && self.current().text(self.source) == "500" {
                    self.bump();
                    Some(ParsedValue::Limit(JobDeliveryLimit::FiveHundred))
                } else {
                    self.error_current("SYN_JOB_DELIVERY_LIMIT_INVALID");
                    None
                }
            }
            Version => {
                if self.at(TokenKind::StringLiteral)
                    && self.current().text(self.source) == "\"reminder.v1\""
                {
                    self.bump();
                    Some(ParsedValue::Version(JobDeliveryPayloadVersion::ReminderV1))
                } else {
                    self.error_current("SYN_JOB_DELIVERY_VERSION_INVALID");
                    None
                }
            }
            Term(expected, term) => {
                if self.current().text(self.source) == expected {
                    self.bump();
                    Some(ParsedValue::Term(term))
                } else {
                    self.error_current("SYN_JOB_DELIVERY_TERM_INVALID");
                    None
                }
            }
            Block(section) => Some(ParsedValue::Block(self.delivery_block(section)?)),
        }
    }

    fn delivery_reference(&mut self) -> Option<NameExpression> {
        let start = self.current().range.start;
        let path = self.parse_qualified_name()?;
        let range = TextRange::new(start, self.previous_significant_end());
        if path.len() < 2 {
            self.error_at("SYN_JOB_DELIVERY_REFERENCE_INVALID", range);
            return None;
        }
        Some(NameExpression { path, range })
    }

    fn delivery_order(&mut self) -> Option<JobDeliveryOrder> {
        let field = self.delivery_reference()?;
        if !self.at(TokenKind::Asc) {
            self.error_current("SYN_JOB_DELIVERY_DIRECTION_INVALID");
            return None;
        }
        self.bump();
        Some(JobDeliveryOrder {
            field,
            direction: JobDeliveryDirection::Ascending,
        })
    }

    fn delivery_key_ahead(&self) -> bool {
        let mut index = self.cursor + 1;
        while self
            .tokens
            .get(index)
            .is_some_and(|token| token.kind.is_trivia())
        {
            index += 1;
        }
        self.tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Colon)
    }

    fn recover_delivery_value(&mut self) {
        let mut depth = 0usize;
        while !self.at(TokenKind::Eof) {
            if self.delivery_declaration_ahead()
                || (depth == 0 && (self.at(TokenKind::RightBrace) || self.delivery_key_ahead()))
            {
                break;
            }
            match self.bump().kind {
                TokenKind::LeftBrace | TokenKind::LeftParen | TokenKind::LeftBracket => depth += 1,
                TokenKind::RightBrace | TokenKind::RightParen | TokenKind::RightBracket => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
        }
    }

    pub(super) fn delivery_declaration_ahead(&self) -> bool {
        let next = self
            .tokens
            .iter()
            .skip(self.cursor + 1)
            .find(|token| !token.kind.is_trivia());
        if next.is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::Dot | TokenKind::Colon | TokenKind::LeftParen
            )
        }) {
            return false;
        }
        !self.delivery_key_ahead()
            && (self.at_contextual("job")
                || self.at_contextual("service")
                || self.at_contextual("authentication")
                || self.at_contextual("principal")
                || self.at_contextual("application")
                || matches!(
                    self.current_kind(),
                    TokenKind::Type
                        | TokenKind::Enum
                        | TokenKind::Entity
                        | TokenKind::Value
                        | TokenKind::Input
                        | TokenKind::Output
                        | TokenKind::Failure
                        | TokenKind::Function
                        | TokenKind::Action
                        | TokenKind::Query
                        | TokenKind::Route
                        | TokenKind::Config
                        | TokenKind::Public
                ))
    }
}
