// A distinct compiler-checked source variant exercises role-based generated
// timestamps and durable derived-representation completion. It never rewrites
// generated outputs or activates the public authored job target.
Bun.env.JADPO_DELIVERY_HOOK_COMPONENT_VARIANT = "completion-representation";
await import("./delivery-hook-components.test.ts");
