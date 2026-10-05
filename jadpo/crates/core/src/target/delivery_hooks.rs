//! Persistence-only lowering from finished opaque bindings. This does not enable
//! jobs, emit worker grants, or establish an execution profile. The full target
//! keeps its unsupported-job gate until the worker boundary is implemented.
use super::TargetGenerator;
use jadpo_syntax::CallableDeclaration;

impl TargetGenerator<'_> {
    // The complete census proves every create of this bound entity is its one
    // compatible captured create site. Never infer a hook from a job spelling,
    // untrusted metadata JSON, or an ordinary checked-job row.
    pub(super) fn delivery_create_hook(&self, entity: &str) -> Option<&str> {
        self.project
            .delivery_model()
            .bindings()
            .iter()
            .find_map(|binding| {
                let selection = &binding.descriptor().selection;
                (selection.entity.text == entity).then(|| {
                    selection
                        .identity
                        .path
                        .last()
                        .expect("checked identity")
                        .text
                        .as_str()
                })
            })
    }

    pub(super) fn delivery_patch_hook<'a>(
        &'a self,
        callable: &CallableDeclaration,
        entity: &str,
    ) -> Option<(&'a str, &'a str)> {
        // Parser-owned callable names already contain the owner qualification.
        let qualified = &callable.name.text;
        self.project
            .delivery_model()
            .bindings()
            .iter()
            .find_map(|binding| {
                let descriptor = binding.descriptor();
                let patch = descriptor
                    .hooks
                    .patch
                    .path
                    .iter()
                    .map(|name| name.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                if descriptor.selection.entity.text != entity || qualified != &patch {
                    return None;
                }
                // The census has proved this exact callable has a single direct
                // compatible patch with this parameter's supplied-field origin.
                Some((
                    descriptor
                        .selection
                        .identity
                        .path
                        .last()
                        .expect("checked identity")
                        .text
                        .as_str(),
                    descriptor
                        .hooks
                        .supplied
                        .path
                        .last()
                        .expect("checked supplied field")
                        .text
                        .as_str(),
                ))
            })
    }
}
