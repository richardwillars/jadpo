//! Stage-one syntax only: deliberately unresolved delivery declarations.
//! No syntactic success is a checked binding, grant or runtime proof.
use jadpo_core::{analyze_sources, derive_approval_subject, derive_target, format_source};
use jadpo_syntax::{
    lex, parse, Declaration, JobDeliveryDirection, JobDeliveryLimit, JobDeliveryPayloadVersion,
    JobDeliveryTerm, JobReminderDelivery, SourceFile, TokenKind,
};
use std::path::Path;

const SOURCE: &str = r#"type ReminderRunAt = Instant {}
action reminder_tick(at: ReminderRunAt) -> Unit {}
job overdue_reminders every 15m {
    concurrency: singleton
    run: reminder_tick(ReminderRunAt(clock.now))
    retry: next_schedule
    delivery: reminder_v1 {
        selection: {
            name: reminder_candidates
            entity: Todo
            identity: Todo.id
            due: Todo.due_at
            before: operation_time
            open: Todo.status(TodoStatus.open)
            visible: Todo
            unsent: Todo.reminder_sent_at
            required_owner: Todo.owner
            owner_visible: User
            order_by: Todo.due_at asc, Todo.id asc
            limit: 500
            continuation: due_identity
        }
        hooks: {
            create: Todo.create_todo
            patch: Todo.patch_todo
            supplied: PatchTodo.due_at
        }
        service: {
            operation: ReminderMail.send_overdue_reminder
            intent: ReminderIntentId
            input: ReminderMessage
            output: ReminderReceipt
            payload_version: "reminder.v1"
            payload: {
                idempotency_key: generated_intent
                from: config.mail_sender
                to: Todo.owner.email
                todo_title: Todo.title
                due_at: Todo.due_at
            }
        }
        authority: {
            validator: api_bearer.service_key
            membership: ReminderServiceMembership
            role: ReminderRole.sender
            permit: reminder_only
        }
        completion: {
            name: reminder_sent
            field: Todo.reminder_sent_at
            time: compiler_receipt_observation
        }
    }
}
"#;

fn delivery(source: &str) -> JobReminderDelivery {
    let parsed = parse(Path::new("delivery.jadpo"), source);
    assert!(
        parsed.diagnostics.is_empty(),
        "{:?}\n{source}",
        parsed.diagnostics
    );
    let Declaration::Job(job) = &parsed.file.declarations[2] else {
        panic!("job missing");
    };
    job.delivery.clone().expect("complete source descriptor")
}
fn path(name: &jadpo_syntax::NameExpression) -> String {
    name.path
        .iter()
        .map(|part| part.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}
fn tokens(source: &str) -> Vec<(String, String)> {
    let mut tokens = lex(Path::new("delivery.jadpo"), source)
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
        .map(|token| (format!("{:?}", token.kind), token.text(source).to_owned()))
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}
fn reject(source: &str, expected: &str) {
    let parsed = parse(Path::new("delivery.jadpo"), source);
    assert!(
        parsed.diagnostics.iter().any(|d| d.code == expected),
        "expected {expected}: {:?}\n{source}",
        parsed.diagnostics
    );
    let project = analyze_sources(vec![SourceFile::new(
        "delivery.jadpo".into(),
        source.into(),
    )])
    .unwrap();
    assert!(
        derive_approval_subject(Path::new("delivery.jadpo"), &project, None, None, None).is_err()
    );
}

#[test]
fn closed_descriptor_retains_all_sections_terms_and_order_without_authority() {
    let descriptor = delivery(SOURCE);
    assert_eq!(descriptor.selection.name.text, "reminder_candidates");
    assert_eq!(descriptor.selection.entity.text, "Todo");
    assert_eq!(path(&descriptor.selection.identity), "Todo.id");
    assert_eq!(path(&descriptor.selection.due), "Todo.due_at");
    assert_eq!(
        descriptor.selection.before,
        JobDeliveryTerm::StrictBeforeOperationTime
    );
    assert_eq!(path(&descriptor.selection.open_field), "Todo.status");
    assert_eq!(path(&descriptor.selection.open_variant), "TodoStatus.open");
    assert_eq!(descriptor.selection.visible.text, "Todo");
    assert_eq!(path(&descriptor.selection.unsent), "Todo.reminder_sent_at");
    assert_eq!(path(&descriptor.selection.required_owner), "Todo.owner");
    assert_eq!(descriptor.selection.owner_visible.text, "User");
    assert_eq!(
        descriptor
            .selection
            .order_by
            .each_ref()
            .map(|order| path(&order.field)),
        ["Todo.due_at", "Todo.id"]
    );
    assert!(descriptor
        .selection
        .order_by
        .iter()
        .all(|order| order.direction == JobDeliveryDirection::Ascending));
    assert_eq!(descriptor.selection.limit, JobDeliveryLimit::FiveHundred);
    assert_eq!(
        descriptor.selection.continuation,
        JobDeliveryTerm::DueIdentityContinuation
    );
    assert_eq!(path(&descriptor.hooks.create), "Todo.create_todo");
    assert_eq!(path(&descriptor.hooks.patch), "Todo.patch_todo");
    assert_eq!(path(&descriptor.hooks.supplied), "PatchTodo.due_at");
    assert_eq!(
        path(&descriptor.service.operation),
        "ReminderMail.send_overdue_reminder"
    );
    assert_eq!(descriptor.service.intent.text, "ReminderIntentId");
    assert_eq!(descriptor.service.input.text, "ReminderMessage");
    assert_eq!(descriptor.service.output.text, "ReminderReceipt");
    assert_eq!(
        descriptor.service.payload_version,
        JobDeliveryPayloadVersion::ReminderV1
    );
    assert_eq!(
        descriptor.service.payload.idempotency_key,
        JobDeliveryTerm::GeneratedIntent
    );
    assert_eq!(path(&descriptor.service.payload.from), "config.mail_sender");
    assert_eq!(path(&descriptor.service.payload.to), "Todo.owner.email");
    assert_eq!(path(&descriptor.service.payload.todo_title), "Todo.title");
    assert_eq!(path(&descriptor.service.payload.due_at), "Todo.due_at");
    assert_eq!(
        path(&descriptor.authority.validator),
        "api_bearer.service_key"
    );
    assert_eq!(
        descriptor.authority.membership.text,
        "ReminderServiceMembership"
    );
    assert_eq!(path(&descriptor.authority.role), "ReminderRole.sender");
    assert_eq!(descriptor.authority.permit, JobDeliveryTerm::ReminderOnly);
    assert_eq!(descriptor.completion.name.text, "reminder_sent");
    assert_eq!(path(&descriptor.completion.field), "Todo.reminder_sent_at");
    assert_eq!(
        descriptor.completion.time,
        JobDeliveryTerm::CompilerReceiptObservation
    );
}

#[test]
fn parsed_descriptor_cannot_export_checked_job_or_lower_to_noop_worker() {
    let project = analyze_sources(vec![SourceFile::new(
        "delivery.jadpo".into(),
        SOURCE.into(),
    )])
    .unwrap();
    assert!(project
        .semantics
        .diagnostics
        .iter()
        .any(|d| d.code == "TYPE_JOB_DELIVERY_BINDING_NOT_IMPLEMENTED"));
    assert!(project.typing.jobs.is_empty());
    assert!(
        derive_approval_subject(Path::new("delivery.jadpo"), &project, None, None, None).is_err()
    );
    assert_eq!(
        derive_target(Path::new("delivery.jadpo"), &project)
            .unwrap_err()
            .code,
        "JADPO_TARGET_JOB_NOT_IMPLEMENTED"
    );
    let without = SOURCE[..SOURCE.find("    delivery:").unwrap()].to_owned() + "}\n";
    let project =
        analyze_sources(vec![SourceFile::new("plain.jadpo".into(), without.clone())]).unwrap();
    assert!(project.typing.diagnostics.is_empty());
    assert_eq!(project.typing.jobs.len(), 1);
    assert!(derive_approval_subject(Path::new("plain.jadpo"), &project, None, None, None).is_ok());
    let parsed = parse(Path::new("plain.jadpo"), &without);
    let Declaration::Job(job) = &parsed.file.declarations[2] else {
        panic!();
    };
    assert!(job.delivery.is_none());
}

#[test]
fn every_descriptor_leaf_is_required_unique_and_closed() {
    for line in SOURCE.lines().filter(|line| {
        line.trim().contains(": ") && !line.contains('{') && line.starts_with("        ")
    }) {
        let removed = SOURCE.replacen(line, "", 1);
        reject(&removed, "SYN_JOB_DELIVERY_KEY_REQUIRED");
        let duplicated = SOURCE.replacen(line, &format!("{line}\n{line}"), 1);
        reject(&duplicated, "SYN_JOB_DELIVERY_KEY_DUPLICATE");
        let key = line.trim().split(':').next().unwrap();
        let unknown = line.replacen(key, "unknown_key", 1);
        reject(
            &SOURCE.replacen(line, &unknown, 1),
            "SYN_JOB_DELIVERY_KEY_UNKNOWN",
        );
    }
    let descriptor = delivery(SOURCE);
    for (key, range) in [
        ("selection", descriptor.selection.range),
        ("hooks", descriptor.hooks.range),
        ("service", descriptor.service.range),
        ("authority", descriptor.authority.range),
        ("completion", descriptor.completion.range),
    ] {
        let start = SOURCE[..range.start].rfind(&format!("{key}:")).unwrap();
        let section = &SOURCE[start..range.end];
        reject(
            &SOURCE.replacen(section, "", 1),
            "SYN_JOB_DELIVERY_KEY_REQUIRED",
        );
        reject(
            &SOURCE.replacen(section, &format!("{section}\n{section}"), 1),
            "SYN_JOB_DELIVERY_KEY_DUPLICATE",
        );
        reject(
            &SOURCE.replacen(&format!("{key}:"), "unknown_section:", 1),
            "SYN_JOB_DELIVERY_KEY_UNKNOWN",
        );
    }
}

#[test]
fn terms_references_versions_limits_and_cursor_shapes_reject_forged_syntax() {
    for (before, after, code) in [
        (
            "delivery: reminder_v1",
            "delivery: other_v1",
            "SYN_JOB_DELIVERY_MODE_INVALID",
        ),
        (
            "before: operation_time",
            "before: clock.now",
            "SYN_JOB_DELIVERY_TERM_INVALID",
        ),
        (
            "before: operation_time",
            "before: scheduled_for",
            "SYN_JOB_DELIVERY_TERM_INVALID",
        ),
        (
            "continuation: due_identity",
            "continuation: offset",
            "SYN_JOB_DELIVERY_TERM_INVALID",
        ),
        (
            "idempotency_key: generated_intent",
            "idempotency_key: input.id",
            "SYN_JOB_DELIVERY_TERM_INVALID",
        ),
        (
            "permit: reminder_only",
            "permit: Access.authenticated",
            "SYN_JOB_DELIVERY_TERM_INVALID",
        ),
        (
            "time: compiler_receipt_observation",
            "time: receipt.accepted_at",
            "SYN_JOB_DELIVERY_TERM_INVALID",
        ),
        ("limit: 500", "limit: 501", "SYN_JOB_DELIVERY_LIMIT_INVALID"),
        (
            "limit: 500",
            "limit: input.count",
            "SYN_JOB_DELIVERY_LIMIT_INVALID",
        ),
        (
            "limit: 500",
            "limit: 500.0",
            "SYN_JOB_DELIVERY_LIMIT_INVALID",
        ),
        (
            "payload_version: \"reminder.v1\"",
            "payload_version: \"reminder.v2\"",
            "SYN_JOB_DELIVERY_VERSION_INVALID",
        ),
        (
            "Todo.due_at asc",
            "Todo.due_at desc",
            "SYN_JOB_DELIVERY_DIRECTION_INVALID",
        ),
        (
            "Todo.due_at asc, Todo.id asc",
            "Todo.due_at asc",
            "SYN_JOB_DELIVERY_SYNTAX_INVALID",
        ),
        (
            "Todo.due_at asc, Todo.id asc",
            "Todo.due_at asc Todo.id asc",
            "SYN_JOB_DELIVERY_SYNTAX_INVALID",
        ),
        (
            "Todo.due_at asc, Todo.id asc",
            "Todo.due_at asc, Todo.id asc, Todo.title asc",
            "SYN_JOB_DELIVERY_SYNTAX_INVALID",
        ),
        (
            "open: Todo.status(TodoStatus.open)",
            "open: Todo.status(TodoStatus.open, TodoStatus.closed)",
            "SYN_JOB_DELIVERY_SYNTAX_INVALID",
        ),
        (
            "from: config.mail_sender",
            "from: generated_intent",
            "SYN_JOB_DELIVERY_REFERENCE_INVALID",
        ),
        (
            "from: config.mail_sender",
            "from: config.mail_sender()",
            "SYN_JOB_DELIVERY_REFERENCE_INVALID",
        ),
        (
            "to: Todo.owner.email",
            "to: { value: Todo.owner.email }",
            "SYN_EXPECTED_NAME",
        ),
    ] {
        reject(&SOURCE.replace(before, after), code);
    }
    // Syntactically valid wrong semantic identities are not name heuristics.
    delivery(
        &SOURCE
            .replace("Todo.owner", "Other.account")
            .replace("ReminderIntentId", "OrdinaryUuid"),
    );
    let reversed = delivery(&SOURCE.replace(
        "Todo.due_at asc, Todo.id asc",
        "Todo.id asc, Todo.due_at asc",
    ));
    assert_eq!(path(&reversed.selection.order_by[0].field), "Todo.id");
}

#[test]
fn formatter_canonicalizes_sections_and_keys_preserving_tokens_comments_and_cursor() {
    let descriptor = delivery(SOURCE);
    let mut shuffled = SOURCE.to_owned();
    // Reverse each block's leaf lines while retaining braces and nested payload.
    for range in [
        descriptor.selection.range,
        descriptor.hooks.range,
        descriptor.service.payload.range,
        descriptor.authority.range,
        descriptor.completion.range,
    ]
    .into_iter()
    .rev()
    {
        let body = &SOURCE[range.start + 1..range.end - 1];
        let mut lines = body
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        lines.reverse();
        shuffled.replace_range(
            range.start + 1..range.end - 1,
            &format!("\n{}\n", lines.join("\n")),
        );
    }
    // Ranges changed: parse the permuted source before reversing sections.
    let changed = delivery(&shuffled);
    let ranges = [
        changed.selection.range,
        changed.hooks.range,
        changed.service.range,
        changed.authority.range,
        changed.completion.range,
    ];
    let mut chunks = Vec::new();
    for range in ranges {
        let start = shuffled[..range.start].rfind('\n').unwrap() + 1;
        chunks.push((start, range.end, shuffled[start..range.end].to_owned()));
    }
    let replacement = chunks
        .iter()
        .rev()
        .map(|(_, _, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    shuffled.replace_range(chunks[0].0..chunks[4].1, &replacement);
    let formatted = format_source(&shuffled);
    delivery(&formatted);
    assert_eq!(formatted, SOURCE);
    assert_eq!(format_source(&formatted), formatted);
    assert_eq!(tokens(&shuffled), tokens(&formatted));
    let comments = shuffled
        .replace(
            "name: reminder_candidates",
            "// selection name stays\nname: reminder_candidates",
        )
        .replace(
            "from: config.mail_sender",
            "from: config.mail_sender // sender stays",
        );
    let formatted = format_source(&comments);
    delivery(&formatted);
    assert_eq!(tokens(&comments), tokens(&formatted));
    assert_eq!(format_source(&formatted), formatted);
    let reversed = SOURCE.replace(
        "Todo.due_at asc, Todo.id asc",
        "Todo.id asc, Todo.due_at asc",
    );
    assert!(format_source(&reversed).contains("Todo.id asc, Todo.due_at asc"));
    let compact = SOURCE.replace('\n', " ");
    let formatted = format_source(&compact);
    delivery(&formatted);
    assert_eq!(tokens(&compact), tokens(&formatted));
    assert_eq!(format_source(&formatted), formatted);
    let malformed = SOURCE.replace("limit: 500", "limit: input.count");
    assert_eq!(tokens(&malformed), tokens(&format_source(&malformed)));
}

#[test]
fn nested_malformed_value_recovers_to_siblings_job_and_later_declaration() {
    for replacement in [
        "from: provider.make({ to: Todo.owner.email })",
        "from: { forged: { fence: input.fence } }",
        "unknown_key: { deep: { from: config.wrong } }",
    ] {
        let source=SOURCE.replace("from: config.mail_sender",replacement)
            +"\njob later every 1s { concurrency: singleton run: reminder_tick(ReminderRunAt(clock.now)) retry: next_schedule }\noutput Later { ok: Bool }\n";
        let parsed = parse(Path::new("recovery.jadpo"), &source);
        assert!(!parsed.diagnostics.is_empty());
        assert!(parsed.file.declarations.iter().any(
            |d| matches!(d,Declaration::Job(job) if job.name.text=="later"&&job.delivery.is_none())
        ), "{replacement}: {:?}", parsed.diagnostics);
        assert!(parsed
            .file
            .declarations
            .iter()
            .any(|d| matches!(d,Declaration::Record(record) if record.name.text=="Later")));
        let Declaration::Job(job) = &parsed.file.declarations[2] else {
            panic!();
        };
        assert!(job.delivery.is_none());
    }
    let truncated = &SOURCE[..SOURCE.find("            from:").unwrap()];
    let source = format!("{truncated}\njob later every 1s {{ concurrency: singleton run: reminder_tick(ReminderRunAt(clock.now)) retry: next_schedule }}\noutput Later {{ ok: Bool }}\n");
    let parsed = parse(Path::new("truncated.jadpo"), &source);
    assert!(!parsed.diagnostics.is_empty());
    assert!(parsed
        .file
        .declarations
        .iter()
        .any(|d| matches!(d,Declaration::Job(job) if job.name.text=="later")));
    assert!(parsed
        .file
        .declarations
        .iter()
        .any(|d| matches!(d,Declaration::Record(record) if record.name.text=="Later")));
}
