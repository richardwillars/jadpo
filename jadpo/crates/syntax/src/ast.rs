use crate::{TextRange, Token};
use jadpo_diagnostics::Diagnostic;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedSyntax {
    pub source_name: String,
    pub source_text: String,
    pub file: SyntaxFile,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxFile {
    pub module: Option<ModuleDeclaration>,
    pub imports: Vec<ImportDeclaration>,
    pub exports: Vec<Name>,
    pub declarations: Vec<Declaration>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDeclaration {
    pub path: Vec<Name>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDeclaration {
    pub module: Vec<Name>,
    pub names: Vec<Name>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Declaration {
    Type(TypeDeclaration),
    Enum(EnumDeclaration),
    Record(RecordDeclaration),
    Failure(FailureDeclaration),
    Callable(CallableDeclaration),
    Test(TestDeclaration),
    Route(RouteDeclaration),
}

impl Declaration {
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Type(declaration) => declaration.range,
            Self::Enum(declaration) => declaration.range,
            Self::Record(declaration) => declaration.range,
            Self::Failure(declaration) => declaration.range,
            Self::Callable(declaration) => declaration.range,
            Self::Test(declaration) => declaration.range,
            Self::Route(declaration) => declaration.range,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumDeclaration {
    pub name: Name,
    pub variants: Vec<EnumVariantDeclaration>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariantDeclaration {
    pub name: Name,
    pub fields: Vec<FieldDeclaration>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Name {
    pub text: String,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDeclaration {
    pub name: Name,
    pub parent: TypeReference,
    pub constraints: Vec<Constraint>,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordKind {
    Entity,
    Value,
    Input,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordDeclaration {
    pub kind: RecordKind,
    pub name: Name,
    pub fields: Vec<FieldDeclaration>,
    pub inverses: Vec<InverseDeclaration>,
    pub persistence_constraints: Vec<PersistenceConstraintDeclaration>,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistenceConstraintKind {
    Unique,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistenceConstraintDeclaration {
    pub name: Name,
    pub kind: PersistenceConstraintKind,
    pub fields: Vec<Name>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InverseDeclaration {
    pub name: Name,
    pub cardinality: InverseCardinality,
    pub target: Name,
    pub via: TypeReference,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InverseCardinality {
    Many,
    Optional,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDeclaration {
    pub name: Name,
    pub field_type: TypeReference,
    pub constraints: Vec<Constraint>,
    pub persistence: Vec<PersistenceModifier>,
    pub reference: Option<ReferenceDeclaration>,
    pub optional: bool,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistenceModifier {
    Identity,
    Unique,
    Index,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceDeclaration {
    pub target: TypeReference,
    pub relationship: Option<Name>,
    pub on_delete: ReferenceDeleteAction,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceDeleteAction {
    Restrict,
    Cascade,
    SetNull,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeReference {
    pub path: Vec<Name>,
    pub arguments: Vec<TypeReference>,
    pub nullable: bool,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstraintKind {
    Min,
    Max,
    MinLength,
    MaxLength,
    Pattern,
    Format,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub value: Literal,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureDeclaration {
    pub name: Name,
    pub kind: Name,
    pub code: Option<Literal>,
    pub message: Option<Literal>,
    pub public_fields: Vec<FieldDeclaration>,
    pub internal_fields: Vec<FieldDeclaration>,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    Function,
    Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDeclaration {
    pub kind: CallableKind,
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeReference,
    pub return_annotation_range: TextRange,
    pub failures: Vec<Name>,
    pub body: Block,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestDeclaration {
    pub name: Literal,
    pub body: Block,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: Name,
    pub parameter_type: TypeReference,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Binding(BindingStatement),
    Assignment(AssignmentStatement),
    Return(ReturnStatement),
    Reject(RejectStatement),
    If(IfStatement),
    Match(MatchStatement),
    Assert(AssertStatement),
    Unsupported(UnsupportedStatement),
}

impl Statement {
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Binding(statement) => statement.range,
            Self::Assignment(statement) => statement.range,
            Self::Return(statement) => statement.range,
            Self::Reject(statement) => statement.range,
            Self::If(statement) => statement.range,
            Self::Match(statement) => statement.range,
            Self::Assert(statement) => statement.range,
            Self::Unsupported(statement) => statement.range,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchStatement {
    pub subject: Expression,
    pub arms: Vec<MatchArm>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssertStatement {
    pub condition: Expression,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Block,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchPattern {
    Name(NameExpression),
    Variant(VariantPattern),
    OptionalSome(OptionalSomePattern),
    Literal(Literal),
    Wildcard(Name),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantPattern {
    pub target: NameExpression,
    pub bindings: Vec<Name>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionalSomePattern {
    pub binding: Name,
    pub range: TextRange,
}

impl MatchPattern {
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Name(pattern) => pattern.range,
            Self::Variant(pattern) => pattern.range,
            Self::OptionalSome(pattern) => pattern.range,
            Self::Literal(pattern) => pattern.range,
            Self::Wildcard(pattern) => pattern.range,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignmentStatement {
    pub target: Name,
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingStatement {
    pub mutable: bool,
    pub name: Name,
    pub annotation: Option<TypeReference>,
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnStatement {
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectStatement {
    pub failure: Name,
    pub public_values: Vec<FieldInitialiser>,
    pub internal_values: Vec<FieldInitialiser>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfStatement {
    pub condition: Expression,
    pub then_block: Block,
    pub else_block: Option<Block>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedStatement {
    pub keyword: String,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Name(NameExpression),
    Invocation(InvocationExpression),
    Construction(ConstructionExpression),
    Create(CreateExpression),
    Query(QueryExpression),
    Update(UpdateExpression),
    Delete(DeleteExpression),
    Unary(UnaryExpression),
    Binary(BinaryExpression),
    Grouped(GroupedExpression),
    Missing(TextRange),
}

impl Expression {
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Literal(expression) => expression.range,
            Self::Name(expression) => expression.range,
            Self::Invocation(expression) => expression.range,
            Self::Construction(expression) => expression.range,
            Self::Create(expression) => expression.range,
            Self::Query(expression) => expression.range,
            Self::Update(expression) => expression.range,
            Self::Delete(expression) => expression.range,
            Self::Unary(expression) => expression.range,
            Self::Binary(expression) => expression.range,
            Self::Grouped(expression) => expression.range,
            Self::Missing(range) => *range,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateExpression {
    pub target: NameExpression,
    pub fields: Vec<FieldInitialiser>,
    pub conflicts: Vec<ConflictBinding>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictBinding {
    pub constraint: Option<NameExpression>,
    pub rejection: RejectStatement,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryCardinality {
    Optional,
    Required,
    Many,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryOrderDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryOrder {
    pub field: Name,
    pub direction: QueryOrderDirection,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryPagination {
    pub limit: Box<Expression>,
    pub offset: Box<Expression>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryInclude {
    pub relationship: Name,
    pub nested_relationship: Option<Name>,
    pub cardinality: QueryIncludeCardinality,
    pub result: TypeReference,
    pub order: QueryOrder,
    pub pagination: QueryPagination,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryIncludeCardinality {
    Many,
    Required,
    Optional,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryExpression {
    pub cardinality: QueryCardinality,
    pub target: NameExpression,
    pub field: Name,
    pub value: Box<Expression>,
    pub order: Option<QueryOrder>,
    pub pagination: Option<QueryPagination>,
    pub includes: Vec<QueryInclude>,
    pub missing: Option<RejectStatement>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateExpression {
    pub target: NameExpression,
    pub field: Name,
    pub value: Box<Expression>,
    pub changes: Vec<FieldInitialiser>,
    pub conditional_changes: Vec<PatchConditionalChange>,
    pub patch: Option<NameExpression>,
    pub empty: Option<RejectStatement>,
    pub missing: RejectStatement,
    pub conflicts: Vec<ConflictBinding>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchConditionalChange {
    pub change: FieldInitialiser,
    pub supplied: NameExpression,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteExpression {
    pub target: NameExpression,
    pub field: Name,
    pub value: Box<Expression>,
    pub missing: RejectStatement,
    pub conflicts: Vec<ConflictBinding>,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralKind {
    String,
    Integer,
    Decimal,
    Boolean,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Literal {
    pub kind: LiteralKind,
    pub text: String,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameExpression {
    pub path: Vec<Name>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationExpression {
    pub callee: NameExpression,
    pub arguments: Vec<Expression>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionExpression {
    pub target: NameExpression,
    pub fields: Vec<FieldInitialiser>,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOperator {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOperator {
    Not,
    Negate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub value: Box<Expression>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryExpression {
    pub left: Box<Expression>,
    pub operator: BinaryOperator,
    pub right: Box<Expression>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedExpression {
    pub value: Box<Expression>,
    pub range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldInitialiser {
    pub name: Name,
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteDeclaration {
    pub method: HttpMethod,
    pub path: String,
    pub path_range: TextRange,
    pub public: bool,
    pub input: Option<TypeReference>,
    pub output: Option<TypeReference>,
    pub run: Option<InvocationExpression>,
    pub range: TextRange,
}
