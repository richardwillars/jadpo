//! Necessary selector type checks only, never a checked delivery binding.
//! The caller retains the unsupported binding guard regardless of success.
use super::{joined_name, RecordField, TypeChecker};
use jadpo_syntax::{Declaration, JobReminderDelivery, NameExpression, ParsedSyntax};

impl TypeChecker<'_> {
    pub(super) fn check_delivery_selector_types(
        &mut self,
        delivery: &JobReminderDelivery,
        files: &[ParsedSyntax],
        source: &str,
    ) {
        if !self.delivery_selector_types_valid(delivery, files) {
            self.push_diagnostic(
                "TYPE_JOB_DELIVERY_SELECTOR_INVALID",
                source,
                delivery.selection.range,
            );
        }
    }

    fn delivery_selector_types_valid(
        &self,
        delivery: &JobReminderDelivery,
        files: &[ParsedSyntax],
    ) -> bool {
        let selection = &delivery.selection;
        let entity = selection.entity.text.as_str();
        let owner = selection.owner_visible.text.as_str();
        // Require explicit authoritative persistence and declared visibility;
        // neither a record name nor a generic read policy implies these facts.
        let Some(Some(store)) = self.catalogue.authority_stores.get(entity) else {
            return false;
        };
        if self.catalogue.authority_stores.get(owner) != Some(&Some(store.clone()))
            || ![entity, owner].iter().all(|name| {
                self.catalogue.persistent_entities.contains(*name)
                    && self
                        .catalogue
                        .lifecycles
                        .get(*name)
                        .is_some_and(|lifecycle| lifecycle.visible.is_some())
            })
            || selection.visible.text != entity
        {
            return false;
        }
        let Some(identity) = self.delivery_field(&selection.identity, entity) else {
            return false;
        };
        if !self
            .catalogue
            .identity_fields
            .contains(&(entity.to_owned(), selection.identity.path[1].text.clone()))
            || !self.delivery_scalar(identity, "Uuid", false)
        {
            return false;
        }
        let (Some(due), Some(unsent)) = (
            self.delivery_field(&selection.due, entity),
            self.delivery_field(&selection.unsent, entity),
        ) else {
            return false;
        };
        if !self.delivery_scalar(due, "Instant", true)
            || !self.delivery_scalar(unsent, "Instant", true)
            || due.update_forbidden
            || unsent.update_forbidden
            || due.generated.is_some()
            || unsent.generated.is_some()
            || joined_name(&selection.due.path) == joined_name(&selection.unsent.path)
            || joined_name(&delivery.completion.field.path) != joined_name(&selection.unsent.path)
            || joined_name(&selection.order_by[0].field.path) != joined_name(&selection.due.path)
            || joined_name(&selection.order_by[1].field.path)
                != joined_name(&selection.identity.path)
        {
            return false;
        }
        let Some(status) = self.delivery_field(&selection.open_field, entity) else {
            return false;
        };
        let variant = &selection.open_variant.path;
        if variant.len() != 2
            || status.optional
            || status.declared_type.nullable
            || status.declared_type.secret
            || !status.declared_type.arguments.is_empty()
            || !self
                .ancestors(&status.declared_type.name)
                .contains(&variant[0].text)
            || !self
                .catalogue
                .enums
                .get(&variant[0].text)
                .and_then(|variants| variants.get(&variant[1].text))
                .is_some_and(|fields| fields.is_empty())
        {
            return false;
        }
        let reference = &selection.required_owner.path;
        if reference.len() != 2 || reference[0].text != entity {
            return false;
        }
        let selected_records = files
            .iter()
            .flat_map(|file| &file.file.declarations)
            .filter_map(|declaration| match declaration {
                Declaration::Record(record) if record.name.text == entity => Some(record),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [record] = selected_records.as_slice() else {
            return false;
        };
        let references = record
            .fields
            .iter()
            .filter_map(|field| {
                let target = field.reference.as_ref()?;
                (target.relationship.as_ref()?.text == reference[1].text).then_some((field, target))
            })
            .collect::<Vec<_>>();
        let [(field, target)] = references.as_slice() else {
            return false;
        };
        let target_path = &target.target.path;
        if target_path.len() != 2
            || target_path[0].text != owner
            || !self
                .catalogue
                .identity_fields
                .contains(&(owner.to_owned(), target_path[1].text.clone()))
        {
            return false;
        }
        // Use effective inherited nullability, not the reference catalogue's
        // raw source nullable flag. The alias must reference actual identity.
        let Some(owner_field) = self
            .catalogue
            .records
            .get(entity)
            .and_then(|fields| fields.get(&field.name.text))
        else {
            return false;
        };
        let Some(owner_identity) = self
            .catalogue
            .records
            .get(owner)
            .and_then(|fields| fields.get(&target_path[1].text))
        else {
            return false;
        };
        !owner_field.optional
            && !owner_field.declared_type.nullable
            && self.delivery_scalar(owner_field, "Uuid", false)
            && self.delivery_scalar(owner_identity, "Uuid", false)
            && self
                .catalogue
                .owning_references
                .get(entity)
                .and_then(|references| references.get(&reference[1].text))
                .is_some_and(|relationship| relationship.parent == owner)
    }

    fn delivery_field<'a>(
        &'a self,
        name: &NameExpression,
        entity: &str,
    ) -> Option<&'a RecordField> {
        if name.path.len() != 2 || name.path[0].text != entity {
            return None;
        }
        self.catalogue.records.get(entity)?.get(&name.path[1].text)
    }

    fn delivery_scalar(&self, field: &RecordField, base: &str, nullable: bool) -> bool {
        let value = &field.declared_type;
        !field.optional
            && value.nullable == nullable
            && !value.secret
            && value.arguments.is_empty()
            && self
                .ancestors(&value.name)
                .last()
                .is_some_and(|name| name == base)
    }
}
