pub use jadpo_shared_core::{checked_input, valid};
pub fn invoke(
    op: i32,
    input: serde_json::Value,
    host: impl FnMut(&str, &serde_json::Value) -> serde_json::Value,
) -> serde_json::Value {
    let result = jadpo_shared_core::run(op, input, host);
    if let Some(id) = result.fault_operation {
        eprintln!("{{\"event\":\"native_internal\",\"operation\":{id}}}");
    }
    result.value
}
