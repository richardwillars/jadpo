// Read-only test port for the unchanged generated application functions.
// No Bun or Node APIs. Each call receives an explicit authority adapter.
export class PersistenceFault extends Error {constructor(readonly kind:string){super(kind);}}
export class PolicyFault extends Error {}
export const persistence = undefined;
