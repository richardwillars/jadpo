//! Resolved necessary facts, deliberately not the core checked binding.
use super::{joined_name, CheckedJobBinding, NodeKind, RecordField, TypeChecker};
use jadpo_syntax::{JobReminderDelivery, RecordKind};

/// Constructor is crate-private and every accessor is explicitly candidate data.
/// Core must still validate all errors, origins, hooks, phases and completeness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryTypeCandidate {
    schedule: CheckedJobBinding,
    source: String,
    descriptor: JobReminderDelivery,
    service_operation: String,
}

impl DeliveryTypeCandidate {
    pub(super) fn new(
        schedule: CheckedJobBinding,
        source: &str,
        descriptor: &JobReminderDelivery,
        service_operation: String,
    ) -> Self {
        Self {
            schedule,
            source: source.to_owned(),
            descriptor: descriptor.clone(),
            service_operation,
        }
    }
    pub fn schedule_candidate(&self) -> &CheckedJobBinding {
        &self.schedule
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn descriptor_candidate(&self) -> &JobReminderDelivery {
        &self.descriptor
    }
    pub fn service_operation_candidate(&self) -> &str {
        &self.service_operation
    }
}

impl TypeChecker<'_> {
    pub(super) fn check_delivery_service_types(
        &mut self,
        delivery: &JobReminderDelivery,
        source: &str,
    ) -> Option<String> {
        if self.delivery_service_types_valid(delivery) {
            Some(joined_name(&delivery.service.operation.path))
        } else {
            self.push_diagnostic(
                "TYPE_JOB_DELIVERY_SERVICE_INVALID",
                source,
                delivery.service.range,
            );
            None
        }
    }

    fn delivery_service_types_valid(&self, delivery: &JobReminderDelivery) -> bool {
        let service = &delivery.service;
        let intent = &service.intent.text;
        let input = &service.input.text;
        let output = &service.output.text;
        if [intent, input, output]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 3
            || self.graph.node(intent).map(|node| node.kind) != Some(NodeKind::Type)
            || self.ancestors(intent) != [intent.clone(), "Uuid".to_owned()]
            || self.graph.nullable_types.contains(intent)
            || self
                .catalogue
                .constraints
                .get(intent)
                .is_some_and(|constraints| !constraints.is_empty())
            || self.catalogue.record_kinds.get(input) != Some(&RecordKind::Value)
            || self.catalogue.record_kinds.get(output) != Some(&RecordKind::Value)
        {
            return false;
        }
        let operation = joined_name(&service.operation.path);
        if service.operation.path.len() != 2
            || self.graph.node(&operation).map(|node| node.kind) != Some(NodeKind::ServiceOperation)
        {
            return false;
        }
        let effects = self
            .graph
            .external_effects
            .iter()
            .filter(|effect| format!("{}.{}", effect.service, effect.operation) == operation)
            .collect::<Vec<_>>();
        let [effect] = effects.as_slice() else {
            return false;
        };
        if effect.input != *input || effect.output != *output || effect.idempotency_type != *intent
        {
            return false;
        }
        let Some(signature) = self.catalogue.callables.get(&operation) else {
            return false;
        };
        if signature.parameters.len() != 1
            || signature.parameters[0].name != *input
            || signature.parameters[0].nullable
            || !signature.parameters[0].arguments.is_empty()
            || signature.result.name != *output
            || signature.result.nullable
            || !signature.result.arguments.is_empty()
        {
            return false;
        }
        let (Some(request), Some(receipt)) = (
            self.catalogue.records.get(input),
            self.catalogue.records.get(output),
        ) else {
            return false;
        };
        if request.keys().map(String::as_str).collect::<Vec<_>>()
            != ["due_at", "from", "idempotency_key", "to", "todo_title"]
            || receipt.keys().map(String::as_str).collect::<Vec<_>>() != ["accepted_at"]
        {
            return false;
        }
        let exact = |field: &RecordField, name: &str, nullable: bool| {
            field.declared_type.name == name
                && field.declared_type.nullable == nullable
                && !field.optional
                && field.declared_type.arguments.is_empty()
                && !field.declared_type.secret
                && field.generated.is_none()
        };
        let entity = &delivery.selection.entity.text;
        let owner = &delivery.selection.owner_visible.text;
        let payload = &service.payload;
        let title = joined_name(&payload.todo_title.path);
        let due = joined_name(&payload.due_at.path);
        if !exact(&request["idempotency_key"], intent, false)
            || !exact(&request["from"], "Email", false)
            || !exact(&request["to"], &format!("{owner}.email"), false)
            || !exact(&request["todo_title"], &title, false)
            || !exact(&request["due_at"], &due, true)
            || !exact(&receipt["accepted_at"], "Instant", false)
            || due != joined_name(&delivery.selection.due.path)
            || payload.todo_title.path.len() != 2
            || payload.todo_title.path[0].text != *entity
            || payload.from.path.len() != 2
            || payload.from.path[0].text != "config"
            || payload.to.path.len() != 3
            || !payload.to.path[..2]
                .iter()
                .map(|part| &part.text)
                .eq(delivery
                    .selection
                    .required_owner
                    .path
                    .iter()
                    .map(|part| &part.text))
            || payload.to.path[2].text != "email"
        {
            return false;
        }
        let Some(sender) = self.catalogue.configuration.get(&payload.from.path[1].text) else {
            return false;
        };
        let Some(title_field) = self
            .catalogue
            .records
            .get(entity)
            .and_then(|fields| fields.get(&payload.todo_title.path[1].text))
        else {
            return false;
        };
        let Some(recipient) = self
            .catalogue
            .records
            .get(owner)
            .and_then(|fields| fields.get("email"))
        else {
            return false;
        };
        // Actual slot/field type evidence, not the display spelling or raw map.
        exact(sender, "Email", false)
            && !title_field.optional
            && !title_field.declared_type.nullable
            && !title_field.declared_type.secret
            && title_field.declared_type.arguments.is_empty()
            && self
                .ancestors(&title_field.declared_type.name)
                .last()
                .is_some_and(|name| name == "Text")
            && !recipient.optional
            && !recipient.declared_type.nullable
            && !recipient.declared_type.secret
            && recipient.declared_type.arguments.is_empty()
            && self
                .ancestors(&recipient.declared_type.name)
                .contains(&"Email".to_owned())
    }
}
