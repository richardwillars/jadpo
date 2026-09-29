use super::*;

impl TargetGenerator<'_> {
    pub(super) fn first_party_supported(&self) -> bool {
        if !self.has_authentication() || self.configuration.is_none() {
            return false;
        }
        let user = self
            .principal
            .unwrap()
            .variants
            .iter()
            .find(|v| v.kind.as_str() == "user")
            .unwrap();
        if user.fields.iter().any(|f| {
            !matches!(
                f.name.text.as_str(),
                "subject" | "user_id" | "authentication_strength"
            ) || f.optional
        }) {
            return false;
        }
        if !user.fields.iter().any(|f| f.name.text == "user_id") {
            return false;
        }
        if user.fields.iter().any(|f| {
            matches!(f.name.text.as_str(), "subject" | "authentication_strength")
                && self.representation_root_for(&f.field_type) != "Text"
        }) {
            return false;
        }
        let bounded = self.application.unwrap().authentication.revocation.mode
            == jadpo_syntax::RevocationMode::Bounded;
        self.authentication_strategies.iter().all(|strategy| {
            if strategy.validators.len() != 1 || strategy.resolutions.len() != 1 {
                return false;
            }
            let validator = &strategy.validators[0];
            if validator.principal.text != "user"
                || !matches!(validator.mode.text.as_str(), "signed" | "opaque")
                || (!bounded && validator.mode.text == "signed")
            {
                return false;
            }
            let names = validator
                .settings
                .iter()
                .map(|s| s.name.text.as_str())
                .collect::<BTreeSet<_>>();
            if !names.contains("secret")
                || !names.contains("audience")
                || (matches!(
                    strategy.transport.location,
                    jadpo_syntax::CredentialLocation::Cookie(_)
                ) && !names.contains("origin"))
            {
                return false;
            }
            let resolution = &strategy.resolutions[0];
            if resolution.principal.text != "user" || resolution.authority.path.len() != 2 {
                return false;
            }
            let Some(record) = self.records.get(&resolution.authority.path[0].text) else {
                return false;
            };
            if !record.is_persistent_entity() {
                return false;
            }
            let Some(id) = resolution
                .mappings
                .iter()
                .find(|m| m.target.path.last().is_some_and(|n| n.text == "user_id"))
            else {
                return false;
            };
            record.fields.iter().any(|f| {
                f.name.text == id.source.text
                    && f.persistence.contains(&PersistenceModifier::Identity)
                    && self.representation_root_for(&f.field_type) == "Uuid"
            }) && resolution.mappings.iter().any(|m| {
                m.source.text == resolution.authority.path[1].text
                    && m.target.path.last().is_some_and(|n| n.text == "subject")
            })
        })
    }

    pub(super) fn first_party_application(&self, output: &mut String) {
        if !self.first_party_supported() {
            return;
        }
        line(output, "let firstPartyAuthentication: Awaited<ReturnType<typeof createFirstPartyAuthentication>> | undefined;");
        line(output, "let authenticationInitialization = 0;");
        line(output, "async function initializeAuthentication() {");
        line(
            output,
            "  const generation = ++authenticationInitialization;",
        );
        line(output, "  firstPartyAuthentication = undefined;\n  const initialized = await createFirstPartyAuthentication([");
        let delay = self.application.unwrap().authentication.revocation.maximum_delay.as_ref().map(|v| format!("durationMilliseconds(normalizeConfigurationDuration({}, \"authentication.maximum_delay\"), \"authentication.maximum_delay\")", ts_string(&v.text))).unwrap_or("0".to_owned());
        for strategy in &self.authentication_strategies {
            let validator = &strategy.validators[0];
            let setting = |name: &str| {
                validator
                    .settings
                    .iter()
                    .find(|s| s.name.text == name)
                    .map(|s| self.expression(&s.value))
                    .unwrap_or("undefined".to_owned())
            };
            let cookie = match &strategy.transport.location {
                jadpo_syntax::CredentialLocation::Cookie(c) => ts_string(unquote(&c.text)),
                _ => "null".to_owned(),
            };
            line(output, &format!("    {{ name: {}, mode: {}, cookie: {}, secret: {}, previousSecret: {}, audience: {}, origin: {} ?? null, maximumDelayMs: {} }},", ts_string(&strategy.name.text), ts_string(&validator.mode.text), cookie, setting("secret"), setting("previous_secret"), setting("audience"), setting("origin"), delay));
        }
        line(
            output,
            "  ], authenticationStorage, async (strategy, subject, strength) => {",
        );
        line(
            output,
            "    const rows = await resolveAuthenticationAuthority(strategy, subject);",
        );
        line(
            output,
            "    if (rows.length === 0) return { kind: \"missing\" };",
        );
        line(
            output,
            "    if (rows.length !== 1) return { kind: \"duplicate\" };",
        );
        line(output, "    switch (strategy) {");
        for strategy in &self.authentication_strategies {
            let resolution = &strategy.resolutions[0];
            let entity = &resolution.authority.path[0].text;
            let record = self.records.get(entity).unwrap();
            let fields = record
                .fields
                .iter()
                .map(|f| f.name.text.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let id = resolution
                .mappings
                .iter()
                .find(|m| m.target.path.last().is_some_and(|n| n.text == "user_id"))
                .unwrap();
            line(
                output,
                &format!("      case {}: {{", ts_string(&strategy.name.text)),
            );
            line(
                output,
                "        const authorityRow = expectObject(rows[0], \"authentication.authority\");",
            );
            line(
                output,
                &format!("        const {{ {fields} }} = validate_{entity}({{"),
            );
            for field in &record.fields {
                let raw = format!("authorityRow[{}]", ts_string(&field.name.text));
                let decoded = self.database_decode_expression(
                    &field.field_type,
                    &raw,
                    "\"authentication.authority\"",
                );
                line(
                    output,
                    &format!("          {}: {},", field.name.text, decoded),
                );
            }
            line(output, "        }, \"authentication.authority\");");
            line(
                output,
                &format!(
                    "        if (!({})) return {{ kind: \"inactive\", failureName: {} }};",
                    self.expression(&resolution.active),
                    ts_string(&resolution.inactive.text)
                ),
            );
            line(output, &format!("        return {{ kind: \"active\", principal: {{ kind: \"user\", subject: {}, authenticationStrength: strength, values: {{ user_id: {} }} }} }};", resolution.authority.path[1].text, id.source.text));
            line(output, "      }");
        }
        line(
            output,
            "      default: throw new AuthenticationFault(\"authentication_misconfigured\", 503);",
        );
        line(output, "    }");
        line(output, "  });");
        line(output, "  if (generation !== authenticationInitialization) throw new AuthenticationFault(\"authentication_misconfigured\", 503);");
        line(output, "  firstPartyAuthentication = initialized;");
        line(output, "}");
        // The host integration surface is not an authored callable or a route.
        line(output, "export async function initializeApplication(environment: Record<string, string | undefined>) { ++authenticationInitialization; firstPartyAuthentication = undefined; configuration = loadConfiguration(environment); await initializeAuthentication(); }");
        line(output, "export function authenticationHost() { if (!firstPartyAuthentication) throw new AuthenticationFault(\"authentication_misconfigured\", 503); return firstPartyAuthentication; }");
    }

    pub(super) fn first_party_storage(&self, output: &mut String) {
        if !self.first_party_supported() {
            return;
        }
        output.push_str(r#"
// Internal auth data cannot be reached through authored persistence operations.
async function initializeAuthenticationStorage() {
const authenticationSchema = 'CREATE TABLE IF NOT EXISTS "__jadpo_auth_sessions" ("id" TEXT PRIMARY KEY, "data" TEXT NOT NULL, "revoked" INTEGER NOT NULL DEFAULT 0)';
if (postgres !== null) await persistenceAsync("authentication.schema", () => postgres!.unsafe(authenticationSchema));
else persistenceSync("authentication.schema", () => sqlite!.exec(authenticationSchema));
// Reject an incompatible pre-existing internal table before opening a listener.
const probe = 'SELECT sessions.id, sessions.data, sessions.revoked FROM "__jadpo_auth_sessions" AS sessions LIMIT 0';
if (postgres !== null) await persistenceAsync("authentication.schema.validate", () => postgres!.unsafe(probe));
else persistenceSync("authentication.schema.validate", () => sqlite!.prepare(probe).all());
}
async function authenticationRows(sql: string, values: string[]): Promise<any[]> {
  if (postgres !== null) return persistenceAsync("authentication.authority", () => postgres!.unsafe(sql, values));
  return serializeSQLiteTransaction(async () => persistenceSync("authentication.authority", () => sqlite!.prepare(sql.replace(/\$[0-9]+/gu, "?")).all(...values)));
}
export const authenticationStorage = {
  async get(id: string) {
    const rows = await authenticationRows('SELECT "data", "revoked" FROM "__jadpo_auth_sessions" WHERE "id" = $1', [id]);
    if (rows.length === 0) return null;
    if (rows.length !== 1) throw new Error("Invalid authentication authority cardinality");
    const record = JSON.parse(rows[0].data);
    return { ...record, revoked: rows[0].revoked !== 0 };
  },
  async put(session: any) {
    const values = [session.id, JSON.stringify(session)];
    if (postgres !== null) await persistenceAsync("authentication.issue", () => postgres!.unsafe('INSERT INTO "__jadpo_auth_sessions" ("id", "data") VALUES ($1, $2)', values));
    else await serializeSQLiteTransaction(async () => persistenceSync("authentication.issue", () => sqlite!.prepare('INSERT INTO "__jadpo_auth_sessions" ("id", "data") VALUES (?, ?)').run(...values)));
  },
  async revoke(id: string) {
    if (postgres !== null) await persistenceAsync("authentication.revoke", () => postgres!.unsafe('UPDATE "__jadpo_auth_sessions" SET "revoked" = 1 WHERE "id" = $1', [id]));
    else await serializeSQLiteTransaction(async () => persistenceSync("authentication.revoke", () => sqlite!.prepare('UPDATE "__jadpo_auth_sessions" SET "revoked" = 1 WHERE "id" = ?').run(id)));
  },
};
export async function resolveAuthenticationAuthority(strategy: string, subject: string): Promise<unknown[]> {
  switch (strategy) {
"#);
        for strategy in &self.authentication_strategies {
            let resolution = &strategy.resolutions[0];
            let entity = &resolution.authority.path[0].text;
            let query = format!(
                "SELECT * FROM {} WHERE {} = $1 LIMIT 2",
                sql_identifier(&snake_case(entity)),
                sql_identifier(&resolution.authority.path[1].text)
            );
            let record = self.records.get(entity).unwrap();
            // Reuse the persistence result decoder, including Boolean/Temporal values.
            line(
                output,
                &format!(
                    "    case {}: return (await authenticationRows({}, [subject])).map(row => ({{",
                    ts_string(&strategy.name.text),
                    ts_string(&query)
                ),
            );
            for field in &record.fields {
                let raw = format!("row[{}]", ts_string(&snake_case(&field.name.text)));
                let value = if self.representation_root_for(&field.field_type) == "Bool" {
                    format!("({raw} === 0 ? false : {raw} === 1 ? true : {raw})")
                } else {
                    raw
                };
                line(output, &format!("      {}: {},", field.name.text, value));
            }
            line(output, "    }));");
        }
        line(
            output,
            "    default: throw new Error(\"Unsupported authentication authority\");",
        );
        line(output, "  }");
        line(output, "}");
    }
}
