//! Selection/enrollment only, from finished bindings. Public jobs remain gated.
use super::{line, snake_case, sql_identifier, ts_string, type_name, TargetGenerator};
use jadpo_syntax::NameExpression;
use std::collections::BTreeSet;

fn last(path: &NameExpression) -> &str {
    &path.path.last().expect("finished field path").text
}
fn variant(path: &str) -> &str {
    path.rsplit_once('.').expect("finished enum variant").1
}

impl TargetGenerator<'_> {
    pub(super) fn delivery_selection_bridges(&self, output: &mut String) {
        if self.project.delivery_model().bindings().is_empty() {
            return;
        }
        // Pure native validation does not issue an intent or a policy grant.
        line(output, "export function validateDeliveryPayload(binding: string, payload: unknown): Readonly<Record<string, unknown>> {");
        line(output, "  switch (binding) {");
        for binding in self.project.delivery_model().bindings() {
            line(output, &format!("    case {}: {{ const value = validate_{}(payload, \"delivery.payload\"); if (value.due_at === null) invalid(\"delivery.payload.due_at\", \"a non-null Instant\"); return Object.freeze(value); }}", ts_string(binding.binding_identity()), binding.descriptor().service.input.text));
        }
        line(
            output,
            "    default: throw new AuthenticationFault(\"authority_invariant\", 500);",
        );
        line(output, "  }");
        line(output, "}");
        line(output, "export function readDeliverySelectionSender(binding: string, proof: unknown): string {");
        line(output, "  readCurrentDeliveryCredentialProof(proof);");
        line(output, "  switch (binding) {");
        for binding in self.project.delivery_model().bindings() {
            line(
                output,
                &format!(
                    "    case {}: return configuration.{};",
                    ts_string(binding.binding_identity()),
                    last(&binding.descriptor().service.payload.from)
                ),
            );
        }
        line(
            output,
            "    default: throw new AuthenticationFault(\"authority_invariant\", 500);",
        );
        line(output, "  }");
        line(output, "}");
        line(
            output,
            "export function validateDeliverySelectionCursor(binding: string, cursor: unknown) {",
        );
        line(output, "  const value = expectObject(cursor, \"delivery.cursor\"); rejectUnknownFields(value, [\"binding\", \"operationTime\", \"dueAt\", \"id\"], \"delivery.cursor\");");
        line(output, "  if (value.binding !== binding) invalid(\"delivery.cursor.binding\", \"the selected binding\");");
        line(output, "  const operationTime = validateInstant(value.operationTime, \"delivery.cursor.operationTime\");");
        line(output, "  switch (binding) {");
        for binding in self.project.delivery_model().bindings() {
            let d = &binding.descriptor().selection;
            let field = |path: &NameExpression| {
                path.path
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            };
            let identity = self
                .field_validator_name(&field(&d.identity))
                .expect("finished identity validator");
            let due = self
                .field_validator_name(&field(&d.due))
                .expect("finished due validator");
            line(output, &format!("    case {}: {{ const dueAt = {due}(value.dueAt, \"delivery.cursor.dueAt\"); if (dueAt === null) invalid(\"delivery.cursor.dueAt\", \"a non-null Instant\"); const id = {identity}(value.id, \"delivery.cursor.id\"); return Object.freeze({{ binding, operationTime, dueAt, id }}); }}", ts_string(binding.binding_identity())));
        }
        line(
            output,
            "    default: throw new AuthenticationFault(\"authority_invariant\", 500);",
        );
        line(output, "  }");
        line(output, "}");
    }

    // Inserted inside the outbox's closed allocation boundary, not emitted as
    // a host callback or an ordinary authored callable/query.
    pub(super) fn delivery_selection_methods(&self) -> String {
        let mut output = String::new();
        if self.project.delivery_model().bindings().is_empty() {
            return output;
        }
        line(&mut output, "async select_delivery_intents(binding: string, proof: unknown, after: unknown = null) {");
        line(&mut output, "  requireTransaction();");
        line(
            &mut output,
            "  if (operationTime === null) return fault(\"delivery.operation_time\");",
        );
        line(
            &mut output,
            "  const at = persistenceInstant(operationTime, postgres !== null);",
        );
        line(&mut output, "  const cursor = after === null ? null : validateDeliverySelectionCursor(binding, after);");
        line(&mut output, "  if (cursor !== null && (Date.parse(cursor.operationTime) !== Date.parse(operationTime) || Date.parse(cursor.dueAt) >= Date.parse(operationTime))) return fault(\"delivery.cursor_snapshot\");");
        line(&mut output, "  return client.transaction(async () => {");
        line(&mut output, "    if (!await client.check_delivery_authority(binding, proof)) return Object.freeze({ intents: Object.freeze([]), after: null });");
        line(
            &mut output,
            "    const sender = readDeliverySelectionSender(binding, proof);",
        );
        line(&mut output, "    switch (binding) {");
        for binding in self.project.delivery_model().bindings() {
            let d = binding.descriptor();
            let s = &d.selection;
            let phase = binding.selection_for_lowering();
            let todo = self
                .records
                .get(&s.entity.text)
                .expect("finished selected entity");
            let relationship = last(&s.required_owner);
            let owner_reference = todo
                .fields
                .iter()
                .find_map(|field| {
                    let reference = field.reference.as_ref()?;
                    (reference.relationship.as_ref()?.text == relationship)
                        .then_some((field.name.text.as_str(), reference))
                })
                .expect("finished required owner reference");
            let owner_target = type_name(&owner_reference.1.target);
            let (owner_entity, owner_identity) = owner_target
                .split_once('.')
                .expect("finished owner identity");
            assert_eq!(owner_entity, s.owner_visible.text);
            let column = |alias: &str, field: &str| {
                format!("{alias}.{}", sql_identifier(&snake_case(field)))
            };
            let identity = last(&s.identity);
            let due = last(&s.due);
            let sent = last(&s.unsent);
            let status = last(&s.open_field);
            let title = last(&d.service.payload.todo_title);
            let email = last(&d.service.payload.to);
            let owner_active = phase
                .owner_active
                .0
                .rsplit_once('.')
                .expect("finished owner active")
                .1;
            let todo_columns = [identity, due, sent, status, title, owner_reference.0]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|f| sql_identifier(&snake_case(f)))
                .collect::<Vec<_>>()
                .join(", ");
            let owner_columns = [owner_identity, email, owner_active]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|f| sql_identifier(&snake_case(f)))
                .collect::<Vec<_>>()
                .join(", ");
            line(
                &mut output,
                &format!("      case {}: {{", ts_string(binding.binding_identity())),
            );
            let (Some(todo_visible), Some(owner_visible)) = (
                self.lifecycle_visibility_sql(&s.visible.text),
                self.lifecycle_visibility_sql(&s.owner_visible.text),
            ) else {
                line(
                    &mut output,
                    "        return fault(\"delivery.visibility_unsupported\");",
                );
                line(&mut output, "      }");
                continue;
            };
            // Project only permitted fields; reuse checked visibility lowering
            // in each entity's own scope, before join/order/LIMIT. No User.*.
            let query = format!("SELECT {} AS source_id, {} AS source_due, {} AS source_title, {} AS recipient FROM (SELECT {todo_columns} FROM {} WHERE {todo_visible}) t JOIN (SELECT {owner_columns} FROM {} WHERE {owner_visible}) u ON {} = {} WHERE {} IS NOT NULL AND {} < $1 AND {} = $2 AND {} IS NULL AND {} = $3", column("t", identity), column("t", due), column("t", title), column("u", email), sql_identifier(&snake_case(&s.entity.text)), sql_identifier(&snake_case(owner_entity)), column("t", owner_reference.0), column("u", owner_identity), column("t", due), column("t", due), column("t", status), column("t", sent), column("u", owner_active));
            let continuation = format!(
                " AND ({} > $4 OR ({} = $4 AND {} > $5))",
                column("t", due),
                column("t", due),
                column("t", identity)
            );
            let order = format!(
                " ORDER BY {} ASC, {} ASC LIMIT 500",
                column("t", due),
                column("t", identity)
            );
            line(
                &mut output,
                &format!(
                    "        const values: unknown[] = [at, {}, {}];",
                    ts_string(last(&s.open_variant)),
                    ts_string(variant(phase.owner_active.1))
                ),
            );
            line(
                &mut output,
                &format!("        let statement = {};", ts_string(&query)),
            );
            line(&mut output, &format!("        if (cursor !== null) {{ statement += {}; values.push(persistenceInstant(cursor.dueAt, postgres !== null), cursor.id); }}", ts_string(&continuation)));
            line(
                &mut output,
                &format!("        statement += {};", ts_string(&order)),
            );
            // READ COMMITTED may replace a tuple after waiting for its lock,
            // without re-establishing ORDER/LIMIT. Keep the original bounded
            // page and refuse any changed boundary/eligibility before enrollment.
            line(&mut output, "        const captured = await execute(\"delivery.selection.capture\", statement, values);");
            line(
                &mut output,
                "        if (postgres !== null) statement += \" FOR SHARE OF t, u\";",
            );
            line(&mut output, "        const selected = await execute(\"delivery.selection\", statement, values);");
            line(&mut output, "        if (captured.length !== selected.length || captured.some((row, index) => row.source_id !== selected[index].source_id || authMilliseconds(row.source_due) !== authMilliseconds(selected[index].source_due) || row.recipient !== selected[index].recipient)) return fault(\"delivery.selection_changed\");");
            line(
                &mut output,
                "        if (selected.length > 500) return fault(\"delivery.selection_bound\");",
            );
            line(
                &mut output,
                "        const intents = []; let lastCursor = null;",
            );
            line(&mut output, "        for (const row of selected) {");
            // Read revision after acquiring source row locks: a joined revision
            // in an earlier READ COMMITTED snapshot could be stale after a wait.
            line(&mut output, &format!("          const revision = await client.read_delivery_schedule_revision({}, row.source_id); if (revision === null) return fault(\"delivery.schedule_missing\");", ts_string(&s.entity.text)));
            line(
                &mut output,
                "          const dueAt = new Date(authMilliseconds(row.source_due)).toISOString();",
            );
            line(&mut output, &format!("          const intent = await enqueueIntent({{ job: {}, sourceOperation: {}, sourceEntity: {}, sourceEntityId: row.source_id, sourceRevision: revision, orderingKey: row.source_id, payloadVersion: \"reminder.v1\" }}, (id, original) => {{", ts_string(&binding.schedule().job), ts_string(&format!("{}.selection", binding.binding_identity())), ts_string(&s.entity.text)));
            line(&mut output, "            let payload: unknown; if (original === null) payload = { idempotency_key: id, from: sender, to: row.recipient, todo_title: row.source_title, due_at: dueAt }; else { try { payload = JSON.parse(original); } catch { return fault(\"delivery.payload\"); } }");
            line(&mut output, "            const validated = validateDeliveryPayload(binding, payload); if (validated.idempotency_key !== id) return fault(\"delivery.payload_identity\"); return canonicalPayload(validated);");
            line(&mut output, "          });");
            line(&mut output, "          intents.push(intent); lastCursor = validateDeliverySelectionCursor(binding, { binding, operationTime, dueAt, id: row.source_id });");
            line(&mut output, "        }");
            line(
                &mut output,
                "        readCurrentDeliveryCredentialProof(proof);",
            );
            line(&mut output, "        return Object.freeze({ intents: Object.freeze(intents), after: selected.length === 500 ? lastCursor : null });");
            line(&mut output, "      }");
        }
        line(
            &mut output,
            "      default: return fault(\"delivery.binding\");",
        );
        line(&mut output, "    }");
        line(&mut output, "  });");
        line(&mut output, "},");
        output
    }
}
