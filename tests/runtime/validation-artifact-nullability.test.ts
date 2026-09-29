import { afterAll, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Type-system §3.6: field references inherit nullability; absence and
// optional shape are independent. Generated public contracts must agree.
const root = mkdtempSync(join(tmpdir(), "jadpo-nullable-artifacts-"));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const compiler = Bun.env.JADPO_BIN ?? new URL("../../jadpo/target/debug/jadpo", import.meta.url).pathname;
writeFileSync(join(root, "app.jadpo"), `
type Contact = Object { email: Email? }
type Link = Object { email: Contact.email }
type NullableEmail = Link.email { max_length: 96 }
type Reply = Object {
    direct: Email?
    inherited: Link.email
    named: NullableEmail
    present: Email
    optional_email: Link.email optional
    emails: List<Link.email>
}
`);
const build = Bun.spawnSync([compiler, "build", root], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) throw new Error(`Fixture failed to build: ${build.stdout}\n${build.stderr}`);
const read = (path: string) => JSON.parse(readFileSync(join(root, "build", path), "utf8"));
const schemas = read("openapi/openapi.json").components.schemas;
const plan = read("validators/plan.json");

function allowsNull(schema: any, seen = new Set<string>()): boolean {
  if (schema.$ref && !seen.has(schema.$ref)) {
    seen.add(schema.$ref);
    return allowsNull(schemas[schema.$ref.split("/").at(-1)], seen);
  }
  if (schema.anyOf) return schema.anyOf.some((branch: any) => allowsNull(branch, new Set(seen)));
  if (schema.allOf) return schema.allOf.every((branch: any) => allowsNull(branch, new Set(seen)));
  // String/number constraints do not reject null unless a type assertion does.
  return schema.type === undefined || schema.type === "null";
}

test("OpenAPI preserves direct, chained and named nullable contracts", () => {
  expect(allowsNull(schemas.NullableEmail)).toBe(true);
  for (const field of ["direct", "inherited", "named", "optional_email"]) {
    expect(allowsNull(schemas.Reply.properties[field])).toBe(true);
  }
  expect(allowsNull(schemas.Reply.properties.present)).toBe(false);
});
test("nullable elements do not make the whole collection nullable", () => {
  const schema = schemas.Reply.properties.emails;
  expect(schema.type).toBe("array");
  expect(allowsNull(schema.items)).toBe(true);
  expect(allowsNull(schema)).toBe(false);
});
test("required shape stays independent of inherited absence", () => {
  expect(schemas.Reply.required.sort()).toEqual(["direct", "emails", "inherited", "named", "present"]);
});
test("validator plans agree with resolved field nullability", () => {
  const reply = plan.records.find((record: any) => record.name === "Reply");
  for (const field of ["direct", "inherited", "named", "optional_email"]) {
    expect(reply.fields.find((item: any) => item.name === field).nullable).toBe(true);
  }
  expect(reply.fields.find((item: any) => item.name === "present").nullable).toBe(false);
  expect(reply.fields.find((item: any) => item.name === "emails").nullable).toBe(false);
  expect(reply.fields.find((item: any) => item.name === "inherited").optional).toBe(false);
});
