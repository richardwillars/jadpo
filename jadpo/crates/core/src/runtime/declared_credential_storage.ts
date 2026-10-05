// Included in generated persistence: all identifiers come from checked declarations.
type DeclaredCredentialBinding = Readonly<{
  binding: string; table: string; identity: string; principal: string; verifier: string;
  expires: string; revoked: string; activeField: string; activeValue: string | boolean;
  created: readonly string[]; authorityTable: string; authoritySubject: string;
  authorityId: string; authorityActiveField: string; authorityActiveValue: string | boolean;
}>;
type AuthenticationQuery = (sql: string, values: any[]) => Promise<any[]>;
const authIdentifier = (name: string): string => `"${name.replaceAll('"', '""')}"`;
const authInstant = (milliseconds: number): string | number => postgres === null ? milliseconds : new Date(milliseconds).toISOString();
function authMilliseconds(value: unknown): number {
  const milliseconds = value instanceof Date ? value.getTime() : typeof value === "number" ? value : typeof value === "string" ? Date.parse(value) : NaN;
  if (!Number.isSafeInteger(milliseconds) || !Number.isFinite(new Date(milliseconds).getTime())) throw new Error("Invalid credential timestamp");
  return milliseconds;
}
function authActive(value: unknown, expected: string | boolean): boolean {
  return (typeof expected === "boolean" && postgres === null ? value === 0 ? false : value === 1 ? true : value : value) === expected;
}
async function credentialVerifierDigest(verifier: unknown): Promise<string> {
  if (typeof verifier !== "string" || !/^[A-Za-z0-9_-]{43}$/u.test(verifier)) throw new Error("Invalid credential verifier");
  const bytes = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier)));
  return Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("");
}
async function authenticationTransaction<T>(body: (query: AuthenticationQuery) => Promise<T>): Promise<T> {
  if (postgres !== null) return persistenceAsync("authentication.transaction", () => postgres!.begin(async tx => body(async (sql, values) => tx.unsafe(sql, values))));
  return serializeSQLiteTransaction(async () => {
    persistenceSync("authentication.begin", () => sqlite!.exec("BEGIN IMMEDIATE"));
    try {
      const result = await body(async (sql, values) => persistenceSync("authentication.transaction", () => sqlite!.prepare(sql.replace(/\$[0-9]+/gu, "?")).all(...values)));
      persistenceSync("authentication.commit", () => sqlite!.exec("COMMIT"));
      return result;
    } catch (error) {
      try { sqlite!.exec("ROLLBACK"); } catch { /* Preserve the original fault, including an unknown commit outcome. */ }
      throw error;
    }
  });
}
function storedCredentialBinding(record: any, id: string): DeclaredCredentialBinding | undefined {
  if (record === null || typeof record !== "object" || Array.isArray(record) || record.id !== id || typeof record.strategy !== "string") throw new Error("Invalid credential metadata");
  const binding = declaredCredentialBindings[record.strategy];
  if (binding !== undefined) {
    if (Object.keys(record).sort().join(",") !== "binding,id,keyId,serviceId,strategy,subject,verifierDigest" || record.binding !== binding.binding
      || typeof record.verifierDigest !== "string" || !/^[0-9a-f]{64}$/u.test(record.verifierDigest)) throw new Error("Invalid declared credential metadata");
  } else if (Object.hasOwn(record, "binding")) {
    throw new Error("Unknown declared credential binding");
  }
  return binding;
}
async function getStoredServiceCredential(id: string): Promise<any> {
  // Use a single transaction executor: never re-enter the SQLite serialization queue.
  return authenticationTransaction(async query => {
    const rows = await query('SELECT "data", "revoked" FROM "__jadpo_auth_service_credentials" WHERE "id" = $1', [id]);
    if (rows.length === 0) return null;
    if (rows.length !== 1) throw new Error("Invalid credential cardinality");
    const record = JSON.parse(rows[0].data);
    const binding = storedCredentialBinding(record, id);
    if (binding === undefined) return { ...record, revoked: rows[0].revoked !== 0 };
    const declared = await query(`SELECT * FROM ${authIdentifier(binding.table)} WHERE ${authIdentifier(binding.identity)} = $1`, [id]);
    if (declared.length === 0) return null;
    if (declared.length !== 1 || declared[0][binding.principal] !== record.serviceId) throw new Error("Invalid declared credential authority");
    const row = declared[0];
    // Integrity only: this fingerprint cannot replace the declared verifier or
    // authenticate a key. Rotation creates a new credential record.
    if (await credentialVerifierDigest(row[binding.verifier]) !== record.verifierDigest) return null;
    const revokedAt = row[binding.revoked];
    if (revokedAt !== null) authMilliseconds(revokedAt);
    return { id: record.id, strategy: record.strategy, subject: record.subject, serviceId: record.serviceId,
      keyId: record.keyId, verifier: row[binding.verifier], expires: authMilliseconds(row[binding.expires]),
      revoked: !authActive(row[binding.activeField], binding.activeValue) || revokedAt !== null };
  });
}
async function putStoredServiceCredential(credential: any, now: number): Promise<void> {
  const binding = declaredCredentialBindings[credential.strategy];
  await authenticationTransaction(async query => {
    let metadata = credential;
    if (binding !== undefined) {
      // Lock the stable principal through commit so concurrent disablement/rename
      // cannot slip between the authority check and the two credential writes.
      const services = await query(`SELECT * FROM ${authIdentifier(binding.authorityTable)} WHERE ${authIdentifier(binding.authoritySubject)} = $1 AND ${authIdentifier(binding.authorityId)} = $2${postgres === null ? "" : " FOR SHARE"}`, [credential.subject, credential.serviceId]);
      if (services.length !== 1 || !authActive(services[0][binding.authorityActiveField], binding.authorityActiveValue)) throw new Error("Service authority changed during issuance");
      const columns = [binding.identity, binding.principal, binding.verifier, binding.expires, binding.revoked, binding.activeField, ...binding.created];
      const active = typeof binding.activeValue === "boolean" && postgres === null ? Number(binding.activeValue) : binding.activeValue;
      const values = [credential.id, credential.serviceId, credential.verifier, authInstant(credential.expires), null, active, ...binding.created.map(() => authInstant(now))];
      await query(`INSERT INTO ${authIdentifier(binding.table)} (${columns.map(authIdentifier).join(", ")}) VALUES (${values.map((_, index) => `$${index + 1}`).join(", ")})`, values);
      metadata = { binding: binding.binding, id: credential.id, strategy: credential.strategy, subject: credential.subject, serviceId: credential.serviceId, keyId: credential.keyId, verifierDigest: await credentialVerifierDigest(credential.verifier) };
    }
    await query('INSERT INTO "__jadpo_auth_service_credentials" ("id", "data") VALUES ($1, $2)', [credential.id, JSON.stringify(metadata)]);
  });
}
async function revokeStoredServiceCredential(id: string, now: number): Promise<void> {
  await authenticationTransaction(async query => {
    const rows = await query('SELECT "data" FROM "__jadpo_auth_service_credentials" WHERE "id" = $1', [id]);
    if (rows.length !== 1) throw new Error("Missing credential authority");
    const record = JSON.parse(rows[0].data);
    const binding = storedCredentialBinding(record, id);
    if (binding === undefined) {
      await query('UPDATE "__jadpo_auth_service_credentials" SET "revoked" = 1 WHERE "id" = $1', [id]);
    } else {
      const changed = await query(`UPDATE ${authIdentifier(binding.table)} SET ${authIdentifier(binding.revoked)} = $1 WHERE ${authIdentifier(binding.identity)} = $2 AND ${authIdentifier(binding.principal)} = $3 RETURNING ${authIdentifier(binding.identity)}`, [authInstant(now), id, record.serviceId]);
      if (changed.length !== 1) throw new Error("Missing declared credential authority");
    }
  });
}
