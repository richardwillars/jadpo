//! Independent contract cases: docs/type-system.md §§2–4,6; grammar §§5–7.
//! Expected semantic outcomes were recorded in tests/validation/types-findings.md
//! before compiler implementation inspection. These are compiler proofs only.
use jadpo_core::analyze_sources;
use jadpo_diagnostics::Diagnostic;
use jadpo_syntax::SourceFile;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let project = analyze_sources(vec![SourceFile::new(
        "types-contract.jadpo".into(),
        source.into(),
    )])
    .expect("nonempty contract source should produce an analysis");
    project
        .syntax
        .diagnostics()
        .cloned()
        .chain(project.semantics.diagnostics.iter().cloned())
        .chain(project.typing.diagnostics.iter().cloned())
        .chain(project.failures.diagnostics.iter().cloned())
        .collect()
}
fn accepts(source: &str) {
    let found = diagnostics(source);
    assert!(
        found.is_empty(),
        "expected accepted source:\n{source}\n{found:#?}"
    );
}
fn rejects(source: &str, code: &str, offending: &str) {
    let found = diagnostics(source);
    let diagnostic = found
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected {code} at {offending:?}:\n{source}\n{found:#?}"));
    let span = diagnostic
        .primary
        .as_ref()
        .expect("type rejection must locate source");
    assert_eq!(
        &source[span.start..span.end],
        offending,
        "wrong primary span for {code}: {diagnostic:#?}"
    );
    assert!(
        !diagnostic.reason.is_empty(),
        "rejection needs an explanation"
    );
    assert!(
        !diagnostic.recommended_next_step.title.is_empty(),
        "rejection needs a repair"
    );
    assert_eq!(
        found.len(),
        1,
        "a single local mismatch must not create unrelated cascades: {found:#?}"
    );
    assert!(
        !found.iter().any(|d| d.code.starts_with("SYN_")),
        "test must reach typechecking: {found:#?}"
    );
}

const DOMAIN: &str = r#"
type Label = Text { min_length: 2 max_length: 8 }
type Alias = Label {}
type OtherLabel = Text { min_length: 2 max_length: 8 }
type Customer = Object { label: Label }
type Supplier = Object { label: Label }
type Envelope = Object { value: Customer.label }
"#;

#[test]
fn explicit_field_widening_is_transitive() {
    accepts(&format!(
        "{DOMAIN}\nfunction read_label(input: Envelope) -> Label {{ return input.value }}"
    ));
}
#[test]
fn field_can_widen_to_exact_declared_field_parent() {
    accepts(&format!(
        "{DOMAIN}\nfunction read_label(input: Envelope) -> Customer.label {{ return input.value }}"
    ));
}
#[test]
fn sibling_fields_do_not_substitute() {
    rejects(
        &format!(
            "{DOMAIN}\nfunction mix(input: Supplier) -> Customer.label {{ return input.label }}"
        ),
        "TYPE_SIBLING_MISMATCH",
        "input.label",
    );
}
#[test]
fn base_values_cannot_implicitly_narrow_to_fields() {
    rejects(
        &format!("{DOMAIN}\nfunction mix(input: Label) -> Customer.label {{ return input }}"),
        "TYPE_MISMATCH",
        "input",
    );
}
#[test]
fn identical_scalar_constraints_do_not_establish_compatibility() {
    rejects(
        &format!("{DOMAIN}\nfunction mix(input: OtherLabel) -> Label {{ return input }}"),
        "TYPE_MISMATCH",
        "input",
    );
}
#[test]
fn named_type_equals_does_not_create_transparent_alias() {
    rejects(
        &format!("{DOMAIN}\nfunction mix(input: Label) -> Alias {{ return input }}"),
        "TYPE_MISMATCH",
        "input",
    );
}
#[test]
fn primitive_variable_is_not_a_semantic_value() {
    let source = format!("{DOMAIN}\ntype Raw = Object {{ text: Text }}\nfunction mix(input: Raw) -> Label {{ return input.text }}");
    rejects(&source, "TYPE_MISMATCH", "input.text");
}
#[test]
fn semantic_values_do_not_implicitly_unwrap_to_primitive_fields() {
    let source = format!("{DOMAIN}\ntype Raw = Object {{ text: Text }}\nfunction mix(input: Label) -> Raw {{ return Raw {{ text: input }} }}");
    rejects(&source, "TYPE_MISMATCH", "input");
}
#[test]
fn identical_record_fields_do_not_create_structural_compatibility() {
    rejects("type Left = Object { value: Email }\ntype Right = Object { value: Email }\nfunction mix(input: Left) -> Right { return input }", "TYPE_MISMATCH", "input");
}
#[test]
fn nested_selection_retains_declared_record_field_identity() {
    accepts("type Address = Object { postal: Text { min_length: 2 } }\ntype Customer = Object { billing: Address shipping: Address }\nfunction billing(input: Customer) -> Address.postal { return input.billing.postal }\nfunction shipping(input: Customer) -> Address.postal { return input.shipping.postal }");
}
#[test]
fn nested_sibling_record_shapes_do_not_erase_field_identity() {
    rejects("type Billing = Object { postal: Email }\ntype Shipping = Object { postal: Email }\ntype Customer = Object { shipping: Shipping }\nfunction mix(input: Customer) -> Billing.postal { return input.shipping.postal }", "TYPE_SIBLING_MISMATCH", "input.shipping.postal");
}
#[test]
fn none_requires_nullable_return() {
    accepts("function make_empty() -> Email? { return none }");
    rejects(
        "function make_empty() -> Email { return none }",
        "TYPE_MISMATCH",
        "none",
    );
}
#[test]
fn present_value_lifts_to_nullable_without_narrowing() {
    accepts(&format!(
        "{DOMAIN}\nfunction lift(input: Customer) -> Label? {{ return input.label }}"
    ));
    rejects(
        &format!(
            "{DOMAIN}\nfunction mix(input: Supplier) -> Customer.label? {{ return input.label }}"
        ),
        "TYPE_SIBLING_MISMATCH",
        "input.label",
    );
}
#[test]
fn nullable_value_does_not_implicitly_become_present() {
    rejects(
        "function mix(input: Email?) -> Email { return input }",
        "TYPE_MISMATCH",
        "input",
    );
}
#[test]
fn nullable_record_requires_handling_before_selection() {
    rejects("type Address = Object { email: Email }\ntype Customer = Object { address: Address? }\nfunction mix(input: Customer) -> Email { return input.address.email }", "TYPE_NULLABLE_SELECTION", "email");
}
#[test]
fn exhaustive_none_handling_makes_present_value_available() {
    accepts("function choose(input: Email?) -> Email { match input { none => { return Email(\"fallback@example.com\") } some(email) => { return email } } }");
}
#[test]
fn nullable_record_field_must_still_be_supplied() {
    accepts("type Reply = Object { email: Email? }\nfunction make_empty() -> Reply { return Reply { email: none } }");
    rejects(
        "type Reply = Object { email: Email? }\nfunction make_empty() -> Reply { return Reply {} }",
        "TYPE_MISSING_FIELD",
        "Reply {}",
    );
}
#[test]
fn optional_shape_does_not_propagate_through_field_reference() {
    accepts("type Patch = Object { email: Email optional }\nfunction make_empty() -> Patch { return Patch {} }");
    rejects("type Patch = Object { email: Email optional }\ntype Required = Object { email: Patch.email }\nfunction make_empty() -> Required { return Required {} }", "TYPE_MISSING_FIELD", "Required {}");
}
#[test]
fn optional_does_not_imply_nullable() {
    rejects("type Patch = Object { email: Email optional }\nfunction make_empty() -> Patch { return Patch { email: none } }", "TYPE_MISMATCH", "none");
}
#[test]
fn integer_constructor_boundaries_are_inclusive() {
    for value in [1, 2, 9, 10] {
        accepts(&format!("type Quantity = Int {{ min: 1 max: 10 }}\nfunction quantity() -> Quantity {{ return Quantity({value}) }}"));
    }
    for value in [0, 11] {
        rejects(&format!("type Quantity = Int {{ min: 1 max: 10 }}\nfunction quantity() -> Quantity {{ return Quantity({value}) }}"), "TYPE_INVALID_LITERAL", &format!("Quantity({value})"));
    }
}
#[test]
fn text_constructor_length_boundaries_are_inclusive() {
    for value in ["ab", "abc", "abcd"] {
        accepts(&format!("type Code = Text {{ min_length: 2 max_length: 4 }}\nfunction make_code() -> Code {{ return Code(\"{value}\") }}"));
    }
    for value in ["", "a", "abcde"] {
        rejects(&format!("type Code = Text {{ min_length: 2 max_length: 4 }}\nfunction make_code() -> Code {{ return Code(\"{value}\") }}"), "TYPE_INVALID_LITERAL", &format!("Code(\"{value}\")"));
    }
}
#[test]
fn field_constructor_checks_parent_and_local_constraints() {
    let declarations = "type Code = Text { min_length: 2 max_length: 6 }\ntype Customer = Object { code: Code { min_length: 3 max_length: 4 } }";
    for value in ["abc", "abcd"] {
        accepts(&format!("{declarations}\nfunction make_code() -> Customer.code {{ return Customer.code(\"{value}\") }}"));
    }
    for value in ["", "ab", "abcde", "abcdefg"] {
        rejects(&format!("{declarations}\nfunction make_code() -> Customer.code {{ return Customer.code(\"{value}\") }}"), "TYPE_INVALID_LITERAL", &format!("Customer.code(\"{value}\")"));
    }
}
#[test]
fn field_reference_inherits_all_intrinsic_constraints() {
    let declarations = "type Code = Text { min_length: 2 max_length: 6 }\ntype Customer = Object { code: Code { max_length: 4 } }\ntype Request = Object { code: Customer.code }";
    accepts(&format!(
        "{declarations}\nfunction make_code() -> Request.code {{ return Request.code(\"abcd\") }}"
    ));
    for value in ["a", "abcde"] {
        rejects(&format!("{declarations}\nfunction make_code() -> Request.code {{ return Request.code(\"{value}\") }}"), "TYPE_INVALID_LITERAL", &format!("Request.code(\"{value}\")"));
    }
}

#[test]
fn explicit_construction_narrows_to_field_contract() {
    accepts(&format!(
        "{DOMAIN}\nfunction make_label() -> Customer.label {{ return Customer.label(\"valid\") }}"
    ));
}
#[test]
fn redundant_nullable_field_reference_is_rejected() {
    let source =
        "type Contact = Object { email: Email? }\ntype Request = Object { email: Contact.email? }";
    rejects(source, "TYPE_REDUNDANT_NULLABILITY", "Contact.email?");
    let found = diagnostics(source);
    assert_eq!(found[0].message, "`Contact.email` already permits `none`");
    assert!(found[0]
        .recommended_next_step
        .title
        .contains("Remove the redundant `?`"));
    assert!(found[0].reason.contains("inherits nullability"));
    // Applying the repair preserves absence; it does not make the field required-present.
    accepts("type Contact = Object { email: Email? }\ntype Request = Object { email: Contact.email }\nfunction make_absent() -> Request { return Request { email: none } }");
}

#[test]
fn record_construction_rejects_undeclared_fields() {
    let source = "type Reply = Object { email: Email }\nfunction reply() -> Reply { return Reply { email: Email(\"a@example.com\") other: Email(\"b@example.com\") } }";
    rejects(source, "TYPE_UNKNOWN_FIELD", "other");
}
#[test]
fn refinement_cannot_relax_parent_constraints() {
    let declarations = "type Code = Text { min_length: 3 max_length: 4 }\ntype Customer = Object { label: Code { min_length: 1 max_length: 8 } }";
    for value in ["ab", "abcde"] {
        rejects(&format!("{declarations}\nfunction make_label() -> Customer.label {{ return Customer.label(\"{value}\") }}"), "TYPE_INVALID_LITERAL", &format!("Customer.label(\"{value}\")"));
    }
}
#[test]
fn entity_snapshot_can_project_to_its_own_reference() {
    accepts("entity Customer { id: Uuid identity: id }\nfunction reference(input: Customer) -> Customer.Ref { return input }");
}
#[test]
fn reference_cannot_implicitly_load_an_entity() {
    rejects("entity Customer { id: Uuid identity: id }\nfunction hydrate(input: Customer.Ref) -> Customer { return input }", "TYPE_MISMATCH", "input");
}
#[test]
fn entity_references_do_not_substitute_across_entities() {
    rejects("entity Customer { id: Uuid identity: id }\nentity Supplier { id: Uuid identity: id }\nfunction mix(input: Supplier.Ref) -> Customer.Ref { return input }", "TYPE_MISMATCH", "input");
}

#[test]
fn container_arguments_preserve_nominal_identity() {
    for container in ["List", "Set"] {
        let source = format!("{DOMAIN}\nfunction mix(input: {container}<Customer.label>) -> {container}<Supplier.label> {{ return input }}");
        rejects(&source, "TYPE_MISMATCH", "input");
        accepts(&format!("{DOMAIN}\nfunction preserve(input: {container}<Customer.label>) -> {container}<Customer.label> {{ return input }}"));
    }
    for target in [
        "Map<Supplier.label, Customer.label>",
        "Map<Customer.label, Supplier.label>",
    ] {
        rejects(&format!("{DOMAIN}\nfunction mix(input: Map<Customer.label, Customer.label>) -> {target} {{ return input }}"), "TYPE_MISMATCH", "input");
    }
}
#[test]
fn nullable_widening_keeps_sibling_types_distinct() {
    accepts(&format!(
        "{DOMAIN}\nfunction widen(input: Customer.label?) -> Label? {{ return input }}"
    ));
    rejects(
        &format!(
            "{DOMAIN}\nfunction mix(input: Supplier.label?) -> Customer.label? {{ return input }}"
        ),
        "TYPE_SIBLING_MISMATCH",
        "input",
    );
}
#[test]
fn integer_singleton_constraint_accepts_only_the_exact_value() {
    accepts("type ExactlyFive = Int { min: 5 max: 5 }\nfunction make_five() -> ExactlyFive { return ExactlyFive(5) }");
    for value in [4, 6] {
        rejects(&format!("type ExactlyFive = Int {{ min: 5 max: 5 }}\nfunction make_five() -> ExactlyFive {{ return ExactlyFive({value}) }}"), "TYPE_INVALID_LITERAL", &format!("ExactlyFive({value})"));
    }
}
#[test]
fn semantic_numeric_literals_require_correct_representation() {
    rejects(
        r#"type Quantity = Int { min: 1 }
function make_quantity() -> Quantity { return Quantity("1") }"#,
        "TYPE_CONSTRUCTOR_INPUT",
        r#""1""#,
    );
}

#[test]
fn closed_enum_cannot_be_constructed_from_wire_string() {
    accepts("type Status = Enum { active disabled }\nfunction make_status() -> Status { return Status.active }");
    let source = "type Status = Enum { active disabled }\nfunction make_status() -> Status { return Status(\"active\") }";
    let found = diagnostics(source);
    assert!(
        !found.is_empty(),
        "enum wire-string backdoor must be rejected"
    );
    assert!(
        found.iter().all(|d| !d.code.starts_with("SYN_")),
        "constructor rejection should reach type checking: {found:#?}"
    );
}

#[test]
fn sibling_diagnostic_explains_the_semantic_decision_without_applying_a_cast() {
    let source = format!(
        "{DOMAIN}\nfunction mix(input: Supplier) -> Customer.label {{ return input.label }}"
    );
    let found = diagnostics(&source);
    let diagnostic = found
        .iter()
        .find(|d| d.code == "TYPE_SIBLING_MISMATCH")
        .expect("sibling rejection");
    for (key, value) in [
        ("expected", "Customer.label"),
        ("received", "Supplier.label"),
    ] {
        assert!(
            diagnostic
                .context
                .iter()
                .any(|(k, v)| k == key && v == value),
            "missing type relationship: {diagnostic:#?}"
        );
        assert!(
            diagnostic.message.contains(value),
            "human diagnostic must name {value}: {diagnostic:#?}"
        );
    }
    assert!(
        diagnostic.recommended_next_step.edits.is_empty(),
        "choosing which nominal value is intended cannot be an automatic cast"
    );
}

#[test]
fn inherited_nullable_fields_propagate_through_references_in_any_declaration_order() {
    let declarations = "type Envelope = Object { email: Request.email }\ntype Request = Object { email: Contact.email }\ntype Contact = Object { email: Email? }";
    accepts(&format!(
        "{declarations}\nfunction make_absent() -> Envelope {{ return Envelope {{ email: none }} }}"
    ));
    accepts(&format!(
        "{declarations}\nfunction preserve(input: Envelope.email) -> Email? {{ return input }}"
    ));
    rejects(&format!("{declarations}\nfunction lose_absence(input: Envelope) -> Email {{ return input.email }}"), "TYPE_MISMATCH", "input.email");
    rejects(
        &format!("{declarations}\ntype Redundant = Object {{ email: Envelope.email? }}"),
        "TYPE_REDUNDANT_NULLABILITY",
        "Envelope.email?",
    );
}
#[test]
fn inherited_nullable_parameter_requires_handling() {
    let declarations = "type Contact = Object { email: Email? }";
    rejects(&format!("{declarations}\nfunction lose_absence(input: Contact.email) -> Email {{ return input }}"), "TYPE_MISMATCH", "input");
    accepts(&format!("{declarations}\nfunction choose(input: Contact.email) -> Email {{ match input {{ none => {{ return Email(\"fallback@example.com\") }} some(email) => {{ return email }} }} }}"));
}
#[test]
fn inherited_nullable_record_selection_is_rejected_before_narrowing() {
    rejects("type Address = Object { email: Email }\ntype Contact = Object { address: Address? }\ntype Envelope = Object { address: Contact.address }\nfunction lose_absence(input: Envelope) -> Email { return input.address.email }", "TYPE_NULLABLE_SELECTION", "email");
}
#[test]
fn redundant_nullable_checks_nested_collection_arguments_and_signatures() {
    for declaration in [
        "type Request = Object { emails: List<Contact.email?> }",
        "function preserve(input: Contact.email?) -> Email? { return input }",
        "function make_absent() -> Contact.email? { return none }",
    ] {
        rejects(
            &format!("type Contact = Object {{ email: Email? }}\n{declaration}"),
            "TYPE_REDUNDANT_NULLABILITY",
            "Contact.email?",
        );
    }
}
#[test]
fn inherited_nullable_local_annotations_preserve_absence() {
    accepts("type Contact = Object { email: Email? }\nfunction make_absent() -> Email? { var email: Contact.email = none return email }");
    rejects("type Contact = Object { email: Email? }\nfunction make_absent() -> Email? { var email: Contact.email? = none return email }", "TYPE_REDUNDANT_NULLABILITY", "Contact.email?");
}
