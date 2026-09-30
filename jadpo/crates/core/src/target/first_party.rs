use super::*;

impl TargetGenerator<'_> {
    pub(super) fn first_party_supported(&self) -> bool {
        if !self.has_authentication() || self.configuration.is_none() {
            return false;
        }
        let bounded = self.application.unwrap().authentication.revocation.mode
            == jadpo_syntax::RevocationMode::Bounded;
        self.authentication_strategies.iter().all(|strategy| {
            let mut modes = BTreeSet::new();
            if strategy.validators.is_empty() {
                return false;
            }
            if strategy.resolutions.iter().any(|resolution| {
                !strategy
                    .validators
                    .iter()
                    .any(|validator| validator.principal.text == resolution.principal.text)
            }) {
                return false;
            }
            for validator in &strategy.validators {
                let kind = validator.principal.text.as_str();
                let mode = validator.mode.text.as_str();
                if !modes.insert((kind, mode))
                    || !matches!(
                        (kind, mode),
                        ("user", "signed" | "opaque" | "jwt") | ("service", "api_key" | "signed")
                    )
                    || (mode == "signed" && !bounded)
                {
                    return false;
                }
                let cookie = matches!(
                    strategy.transport.location,
                    jadpo_syntax::CredentialLocation::Cookie(_)
                );
                if (kind == "service" || mode == "jwt") && cookie {
                    return false;
                }
                let names = validator
                    .settings
                    .iter()
                    .map(|s| s.name.text.as_str())
                    .collect::<BTreeSet<_>>();
                if (mode != "jwt" && !names.contains("secret"))
                    || (mode == "jwt" && !names.contains("issuer"))
                    || !names.contains("audience")
                    || (cookie && !names.contains("origin"))
                {
                    return false;
                }
                let Some(resolution) = strategy
                    .resolutions
                    .iter()
                    .find(|r| r.principal.text == kind)
                else {
                    return false;
                };
                if resolution.authority.path.len() != 2 {
                    return false;
                }
                let Some(record) = self.records.get(&resolution.authority.path[0].text) else {
                    return false;
                };
                if !record.is_persistent_entity() {
                    return false;
                }
                let id_name = if kind == "user" {
                    "user_id"
                } else {
                    "service_id"
                };
                let variant = self
                    .principal
                    .unwrap()
                    .variants
                    .iter()
                    .find(|v| v.kind.as_str() == kind)
                    .unwrap();
                if !variant.fields.iter().any(|f| f.name.text == id_name)
                    || variant.fields.iter().any(|f| {
                        f.optional
                            || self.reference_is_nullable(&f.field_type)
                            || !matches!(
                                f.name.text.as_str(),
                                "subject" | "authentication_strength"
                            ) && f.name.text != id_name
                            || matches!(f.name.text.as_str(), "subject" | "authentication_strength")
                                && self.representation_root_for(&f.field_type) != "Text"
                    })
                {
                    return false;
                }
                let Some(id) = resolution
                    .mappings
                    .iter()
                    .find(|m| m.target.path.last().is_some_and(|n| n.text == id_name))
                else {
                    return false;
                };
                if !record.fields.iter().any(|f| {
                    f.name.text == id.source.text
                        && f.persistence.contains(&PersistenceModifier::Identity)
                        && self.representation_root_for(&f.field_type) == "Uuid"
                }) || !resolution.mappings.iter().any(|m| {
                    m.source.text == resolution.authority.path[1].text
                        && m.target.path.last().is_some_and(|n| n.text == "subject")
                }) {
                    return false;
                }
                if mode == "api_key" {
                    let Some(owner) = validator.settings.iter().find(|s| s.name.text == "owner")
                    else {
                        return false;
                    };
                    let Expression::Name(owner) = &owner.value else {
                        return false;
                    };
                    if owner.path.len() != 2 || owner.path[0].text != record.name.text {
                        return false;
                    }
                    let Some(field) = record
                        .fields
                        .iter()
                        .find(|f| f.name.text == owner.path[1].text)
                    else {
                        return false;
                    };
                    if field.optional || self.reference_is_nullable(&field.field_type) {
                        return false;
                    }
                    let Some(reference) = &field.reference else {
                        return false;
                    };
                    let target = type_name(&reference.target);
                    let Some((entity, field)) = target.split_once('.') else {
                        return false;
                    };
                    let Some(owner_record) = self.records.get(entity) else {
                        return false;
                    };
                    if !owner_record.is_persistent_entity()
                        || !owner_record.fields.iter().any(|f| {
                            f.name.text == field
                                && f.persistence.contains(&PersistenceModifier::Identity)
                        })
                    {
                        return false;
                    }
                }
            }
            // A bounded service token can only originate from the declared key
            // authority in this same credential slot; there is no host mint shortcut.
            !modes.contains(&("service", "signed")) || modes.contains(&("service", "api_key"))
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
        line(output, "  firstPartyAuthentication = undefined;");
        line(output, "  const resolvePrincipal = async (strategy: string, subject: string, strength: string, principalKind: \"user\" | \"service\" = \"user\"): Promise<import(\"./authentication.ts\").ResolutionResult> => {");
        line(
            output,
            "    const rows = await resolveAuthenticationAuthority(strategy, subject, principalKind);",
        );
        line(
            output,
            "    if (rows.length === 0) return { kind: \"missing\" };",
        );
        line(
            output,
            "    if (rows.length !== 1) return { kind: \"duplicate\" };",
        );
        line(output, "    switch (`${strategy}:${principalKind}`) {");
        for strategy in &self.authentication_strategies {
            for resolution in &strategy.resolutions {
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
                    .find(|m| {
                        m.target.path.last().is_some_and(|n| {
                            n.text
                                == if resolution.principal.text == "user" {
                                    "user_id"
                                } else {
                                    "service_id"
                                }
                        })
                    })
                    .unwrap();
                line(
                    output,
                    &format!(
                        "      case {}: {{",
                        ts_string(&format!(
                            "{}:{}",
                            strategy.name.text, resolution.principal.text
                        ))
                    ),
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
                line(output, &format!("        return {{ kind: \"active\", principal: {{ kind: {}, subject: {}, authenticationStrength: strength, values: {{ {}: {} }} }} }};", ts_string(&resolution.principal.text), resolution.authority.path[1].text, if resolution.principal.text == "user" { "user_id" } else { "service_id" }, id.source.text));
                line(output, "      }");
            }
        }
        line(
            output,
            "      default: throw new AuthenticationFault(\"authentication_misconfigured\", 503);",
        );
        line(output, "    }");
        line(output, "  };");
        if self.has_jwt() {
            line(output, "  const jwt = createJwtAuthentication([");
            for strategy in &self.authentication_strategies {
                for validator in &strategy.validators {
                    if validator.mode.text != "jwt" {
                        continue;
                    }
                    let setting = |name: &str| {
                        validator
                            .settings
                            .iter()
                            .find(|s| s.name.text == name)
                            .map(|s| self.expression(&s.value))
                            .unwrap_or("undefined".to_owned())
                    };
                    line(
                        output,
                        &format!(
                            "    {{ name: {}, issuer: {}, audience: {}, jwksUri: {} }},",
                            ts_string(&strategy.name.text),
                            setting("issuer"),
                            setting("audience"),
                            setting("jwks_uri")
                        ),
                    );
                }
            }
            line(output, "  ]);");
            line(output, "  if (!jwt.configured) throw new AuthenticationFault(\"authentication_misconfigured\", 503);");
            let slots = self
                .authentication_strategies
                .iter()
                .filter(|s| s.validators.iter().any(|v| v.mode.text == "jwt"))
                .map(|s| ts_string(&s.name.text))
                .collect::<Vec<_>>()
                .join(", ");
            line(output, &format!("  const jwtSlots = new Set([{slots}]);"));
            output.push_str(r#"
  const external = { async validate(name: string, credential: string, now: number): Promise<import("./authentication.ts").ValidationResult> {
    if (!jwtSlots.has(name)) return { kind: "invalid" };
    const result = await jwt.verify(name, credential, now);
    if (result.kind !== "valid") return result;
    const resolution = await resolvePrincipal(name, result.subject, result.authenticationStrength, "user");
    return { kind: "authoritative", principalKind: "user", subject: result.subject, resolution };
  } };
"#);
        }
        line(
            output,
            "  const initialized = await createFirstPartyAuthentication([",
        );
        let delay = self.application.unwrap().authentication.revocation.maximum_delay.as_ref().map(|v| format!("durationMilliseconds(normalizeConfigurationDuration({}, \"authentication.maximum_delay\"), \"authentication.maximum_delay\")", ts_string(&v.text))).unwrap_or("0".to_owned());
        for strategy in &self.authentication_strategies {
            for validator in &strategy.validators {
                if validator.mode.text == "jwt" {
                    continue;
                }
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
                line(output, &format!("    {{ name: {}, principal: {}, mode: {}, cookie: {}, secret: {}, previousSecret: {}, audience: {}, origin: {} ?? null, maximumDelayMs: {} }},", ts_string(&strategy.name.text), ts_string(&validator.principal.text), ts_string(&validator.mode.text), cookie, setting("secret"), setting("previous_secret"), setting("audience"), setting("origin"), delay));
            }
        }
        line(
            output,
            if self.has_jwt() {
                "  ], authenticationStorage, resolvePrincipal, external);"
            } else {
                "  ], authenticationStorage, resolvePrincipal);"
            },
        );
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
const serviceSchema = 'CREATE TABLE IF NOT EXISTS "__jadpo_auth_service_credentials" ("id" TEXT PRIMARY KEY, "data" TEXT NOT NULL, "revoked" INTEGER NOT NULL DEFAULT 0)';
if (postgres !== null) await persistenceAsync("authentication.schema", () => postgres!.unsafe(serviceSchema));
else persistenceSync("authentication.schema", () => sqlite!.exec(serviceSchema));
const serviceProbe = 'SELECT credentials.id, credentials.data, credentials.revoked FROM "__jadpo_auth_service_credentials" AS credentials LIMIT 0';
if (postgres !== null) await persistenceAsync("authentication.schema.validate", () => postgres!.unsafe(serviceProbe));
else persistenceSync("authentication.schema.validate", () => sqlite!.prepare(serviceProbe).all());
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
  async getServiceCredential(id: string) {
    const rows = await authenticationRows('SELECT "data", "revoked" FROM "__jadpo_auth_service_credentials" WHERE "id" = $1', [id]);
    if (rows.length === 0) return null;
    if (rows.length !== 1) throw new Error("Invalid service credential cardinality");
    const record = JSON.parse(rows[0].data);
    return { ...record, revoked: rows[0].revoked !== 0 };
  },
  async putServiceCredential(credential: any) {
    const values = [credential.id, JSON.stringify(credential)];
    if (postgres !== null) await persistenceAsync("authentication.issue", () => postgres!.unsafe('INSERT INTO "__jadpo_auth_service_credentials" ("id", "data") VALUES ($1, $2)', values));
    else await serializeSQLiteTransaction(async () => persistenceSync("authentication.issue", () => sqlite!.prepare('INSERT INTO "__jadpo_auth_service_credentials" ("id", "data") VALUES (?, ?)').run(...values)));
  },
  async revokeServiceCredential(id: string) {
    if (postgres !== null) await persistenceAsync("authentication.revoke", () => postgres!.unsafe('UPDATE "__jadpo_auth_service_credentials" SET "revoked" = 1 WHERE "id" = $1', [id]));
    else await serializeSQLiteTransaction(async () => persistenceSync("authentication.revoke", () => sqlite!.prepare('UPDATE "__jadpo_auth_service_credentials" SET "revoked" = 1 WHERE "id" = ?').run(id)));
  },
};
export async function resolveAuthenticationAuthority(strategy: string, subject: string, principalKind = "user"): Promise<unknown[]> {
  switch (`${strategy}:${principalKind}`) {
"#);
        for strategy in &self.authentication_strategies {
            for resolution in &strategy.resolutions {
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
                    ts_string(&format!("{}:{}", strategy.name.text, resolution.principal.text)),
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
        }
        line(
            output,
            "    default: throw new Error(\"Unsupported authentication authority\");",
        );
        line(output, "  }");
        line(output, "}");
    }
}
