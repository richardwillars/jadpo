use jadpo_core::analyze_sources;
use jadpo_syntax::SourceFile;

const SELF_IDENTITY: &str = r#"
enum UserRole {
    self
}

failure UserNotFound {
    kind: NotFound
    code: "user_not_found"
}

output UserIdentity {
    id: User.id
}

entity User {
    id: Uuid {
        role: UserRole.self
        immutable: true
    }
    email: Email {
        policy {}
    }
    identity: id
    persistence {
        store: primary
        role: authority
    }
    policy {
        UserRole.self: [read]
    }

    query own_identity(id: User.id)
        freshness: authoritative
        fails UserNotFound
        -> UserIdentity
    {
        var user = attempt query required User {
            where: id == id
            missing: UserNotFound
        }
        return UserIdentity { id: user.id }
    }
}
"#;

const PRINCIPAL_OWNED_CREATE: &str = r#"
enum TodoRole {
    owner
}

enum TodoStatus {
    open
    completed
}

principal Principal {
    user { user_id: Uuid }
    service { service_id: Uuid }
}

entity User {
    id: Uuid
    identity: id
    persistence {
        store: primary
        role: authority
    }
}

input CreateTodo {
    requested_owner: User.id
}

entity Todo {
    id: Uuid { generated: identity }
    status: TodoStatus
    owner_id: User.id {
        role: TodoRole.owner
        immutable: true
    }

    identity: id
    persistence {
        store: primary
        role: authority
        references owner_id: User.id as: owner on_delete: restrict
    }
    policy {
        TodoRole.owner: [create, read]
    }

    action create_todo(input: CreateTodo, principal: Principal.user) -> Todo {
        return attempt create Todo {
            status: Todo.status(TodoStatus.open)
            owner_id: Todo.owner_id(principal.user_id)
        }
    }
}
"#;

fn analyze(source: &str) -> jadpo_core::AnalyzedProject {
    analyze_sources(vec![SourceFile::new(
        "identity-policy.jadpo".into(),
        source.into(),
    )])
    .unwrap()
}

fn diagnostics(project: &jadpo_core::AnalyzedProject) -> Vec<String> {
    project
        .syntax
        .diagnostics()
        .chain(project.semantics.diagnostics.iter())
        .chain(project.typing.diagnostics.iter())
        .chain(project.failures.diagnostics.iter())
        .map(|diagnostic| diagnostic.code.to_owned())
        .collect()
}

#[test]
fn identity_bound_self_role_scopes_reads_and_preserves_field_narrowing() {
    let project = analyze(SELF_IDENTITY);
    assert!(
        diagnostics(&project).is_empty(),
        "{:#?}",
        diagnostics(&project)
    );

    let binding = project
        .policy
        .bindings
        .iter()
        .find(|binding| binding.role == "UserRole.self")
        .expect("identity-bound UserRole.self binding");
    assert_eq!(binding.entity, "User");
    assert_eq!(binding.field, "id");
    assert_eq!(binding.scope, "User");
    assert_eq!(binding.principal, "User");

    let user = project
        .policy
        .entities
        .iter()
        .find(|policy| policy.entity == "User")
        .expect("User policy");
    assert!(user.rules.iter().any(|rule| {
        rule.subject == "UserRole.self" && rule.effects.iter().any(|effect| effect == "read")
    }));
    assert!(user
        .fields
        .iter()
        .any(|field| { field.field == "email" && field.rules.is_empty() }));
}

#[test]
fn same_entity_role_binding_is_rejected_on_a_non_identity_field() {
    let source = SELF_IDENTITY.replace(
        "id: Uuid {\n        role: UserRole.self\n        immutable: true\n    }\n    email:",
        "id: Uuid\n    principal_id: Uuid {\n        role: UserRole.self\n        immutable: true\n    }\n    email:",
    );
    let project = analyze(&source);
    assert!(
        diagnostics(&project)
            .iter()
            .any(|code| code == "POLICY_BINDING_INVALID"),
        "unexpected diagnostics: {:#?}",
        diagnostics(&project)
    );
}

#[test]
fn self_read_cannot_project_a_provisioning_only_field() {
    let source = SELF_IDENTITY
        .replace(
            "output UserIdentity {\n    id: User.id\n}",
            "output UserIdentity {\n    email: User.email\n}",
        )
        .replace(
            "return UserIdentity { id: user.id }",
            "return UserIdentity { email: user.email }",
        );
    let project = analyze(&source);
    assert!(
        diagnostics(&project)
            .iter()
            .any(|code| code == "POLICY_OUTPUT_FIELD_UNPROVED"),
        "unexpected diagnostics: {:#?}",
        diagnostics(&project)
    );
}

#[test]
fn role_bound_owner_accepts_its_explicit_constructor_from_the_principal() {
    let project = analyze(PRINCIPAL_OWNED_CREATE);
    assert!(
        diagnostics(&project).is_empty(),
        "unexpected diagnostics: {:#?}",
        diagnostics(&project)
    );
}

#[test]
fn role_bound_owner_rejects_the_same_constructor_from_request_input() {
    let source = PRINCIPAL_OWNED_CREATE.replace(
        "Todo.owner_id(principal.user_id)",
        "Todo.owner_id(input.requested_owner)",
    );
    let project = analyze(&source);
    assert!(
        diagnostics(&project)
            .iter()
            .any(|code| code == "POLICY_ROLE_FIELD_CREATE_FORBIDDEN"),
        "unexpected diagnostics: {:#?}",
        diagnostics(&project)
    );
}
