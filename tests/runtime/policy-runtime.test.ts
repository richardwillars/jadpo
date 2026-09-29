import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Database } from "bun:sqlite";

const root = mkdtempSync(join(tmpdir(), "jadpo-policy-runtime-"));
const databasePath = join(root, "policy.sqlite");
Bun.env.SQLITE_PATH = databasePath;

const { persistence, PolicyFault } = await import(
  "../../examples/policy-runtime/build/target/persistence.ts"
);
const { handleRequest } = await import("../../examples/policy-runtime/build/target/app.ts");

const ids = {
  owner: "00000000-0000-0000-0000-000000000001",
  editor: "00000000-0000-0000-0000-000000000002",
  viewer: "00000000-0000-0000-0000-000000000003",
  outsider: "00000000-0000-0000-0000-000000000004",
  service: "00000000-0000-4000-8000-000000000005",
  company: "00000000-0000-0000-0000-000000000010",
  otherCompany: "00000000-0000-0000-0000-000000000011",
  article: "00000000-0000-0000-0000-000000000020",
  visibleSharedArticle: "00000000-0000-0000-0000-000000000030",
};

function principal(userId: string) {
  return {
    kind: "user" as const,
    subject: `subject:${userId}`,
    values: { user_id: userId },
  };
}

function servicePrincipal(serviceId: string) {
  return {
    kind: "service" as const,
    subject: `service:${serviceId}`,
    values: { service_id: serviceId },
  };
}

beforeAll(() => {
  const database = new Database(databasePath, { strict: true });
  try {
    const insertUser = database.prepare(
      'INSERT INTO "user" ("id", "display_name", "private_email") VALUES (?1, ?2, ?3)',
    );
    insertUser.run(ids.owner, "Owner", "owner@example.test");
    insertUser.run(ids.editor, "Editor", "editor@example.test");
    insertUser.run(ids.viewer, "Viewer", "viewer@example.test");
    insertUser.run(ids.outsider, "Outsider", "outsider@example.test");
    database.prepare('INSERT INTO "service" ("id") VALUES (?1)').run(ids.service);
    database
      .prepare('INSERT INTO "company" ("id", "owner_id", "name") VALUES (?1, ?2, ?3)')
      .run(ids.company, ids.owner, "Policy Co");
    database
      .prepare('INSERT INTO "company" ("id", "owner_id", "name") VALUES (?1, ?2, ?3)')
      .run(ids.otherCompany, ids.outsider, "Other Co");
    const insertMembership = database.prepare(
      'INSERT INTO "company_membership" ("id", "company_id", "user_id", "role") VALUES (?1, ?2, ?3, ?4)',
    );
    insertMembership.run(
      "00000000-0000-0000-0000-000000000101",
      ids.company,
      ids.editor,
      "editor",
    );
    insertMembership.run(
      "00000000-0000-0000-0000-000000000102",
      ids.company,
      ids.viewer,
      "viewer",
    );
    database
      .prepare(
        'INSERT INTO "application_membership" ("id", "user_id", "role") VALUES (?1, ?2, ?3)',
      )
      .run("00000000-0000-0000-0000-000000000103", ids.editor, "operator");
    database
      .prepare(
        'INSERT INTO "application_membership" ("id", "user_id", "role") VALUES (?1, ?2, ?3)',
      )
      .run("00000000-0000-0000-0000-000000000105", ids.owner, "administrator");
    database
      .prepare(
        'INSERT INTO "application_service_membership" ("id", "service_id", "role") VALUES (?1, ?2, ?3)',
      )
      .run("00000000-0000-0000-0000-000000000104", ids.service, "operator");
    database
      .prepare(
        'INSERT INTO "article" ("id", "company_id", "title", "billing_reference") VALUES (?1, ?2, ?3, ?4)',
      )
      .run(ids.article, ids.company, "Initial", "invoice-secret");
    const insertArticle = database.prepare(
      'INSERT INTO "article" ("id", "company_id", "title", "billing_reference") VALUES (?1, ?2, ?3, ?4)',
    );
    insertArticle.run(
      "00000000-0000-0000-0000-000000000021",
      ids.otherCompany,
      "Shared",
      null,
    );
    insertArticle.run(
      "00000000-0000-0000-0000-000000000022",
      ids.otherCompany,
      "Shared",
      null,
    );
    insertArticle.run(ids.visibleSharedArticle, ids.company, "Shared", null);
  } finally {
    database.close();
  }
});

afterAll(() => {
  delete Bun.env.SQLITE_PATH;
  rmSync(root, { recursive: true, force: true });
});

describe("generated scoped policy enforcement", () => {
  test("uses separate stable user projections for regular and administrator audiences", async () => {
    const regular = persistence.withPolicy(principal(ids.viewer), "User.summary");
    const regularRow = await regular.query_optional_User_by_id(ids.owner);
    expect(regularRow).not.toBeNull();
    expect({
      id: regularRow!.id,
      display_name: regularRow!.display_name,
    }).toEqual({ id: ids.owner, display_name: "Owner" });

    const forbiddenAdministrativeView = persistence.withPolicy(
      principal(ids.viewer),
      "User.administrative_details",
    );
    expect(
      await forbiddenAdministrativeView.query_optional_User_by_id(ids.owner),
    ).toBeNull();

    const administrator = persistence.withPolicy(
      principal(ids.owner),
      "User.administrative_details",
    );
    const administrativeRow = await administrator.query_optional_User_by_id(ids.owner);
    expect(administrativeRow).not.toBeNull();
    expect({
      id: administrativeRow!.id,
      display_name: administrativeRow!.display_name,
      private_email: administrativeRow!.private_email,
    }).toEqual({
      id: ids.owner,
      display_name: "Owner",
      private_email: "owner@example.test",
    });
  });

  test("conceals rows outside the principal's authoritative role scope", async () => {
    const outsider = persistence.withPolicy(principal(ids.outsider), "Article.find");
    expect(await outsider.query_optional_Article_by_id(ids.article)).toBeNull();

    const viewer = persistence.withPolicy(principal(ids.viewer), "Article.find");
    expect(await viewer.query_optional_Article_by_id(ids.article)).toMatchObject({
      id: ids.article,
      title: "Initial",
    });
  });

  test("uses entity permissions and narrowing field permissions conjunctively", async () => {
    const editor = persistence.withPolicy(principal(ids.editor), "Article.edit");
    expect(
      await editor.update_required_Article_by_id_set_title(ids.article, "Edited"),
    ).toMatchObject({ title: "Edited" });
    expect(
      await editor.update_required_Article_by_id_set_billing_reference(
        ids.article,
        "editor-must-not-write",
      ),
    ).toBeNull();

    const owner = persistence.withPolicy(principal(ids.owner), "Article.edit");
    expect(
      await owner.update_required_Article_by_id_set_billing_reference(
        ids.article,
        "owner-write",
      ),
    ).toMatchObject({ billing_reference: "owner-write" });
  });

  test("injects row policy before ordering and pagination", async () => {
    const viewer = persistence.withPolicy(principal(ids.viewer), "Article.list");
    const rows = (await viewer.query_many_Article_by_title_order_by_id_asc_paginated(
      "Shared",
      1,
      0,
    )) as Array<{ id: string }>;
    expect(rows.map((row) => row.id)).toEqual([ids.visibleSharedArticle]);
  });

  test("rejects a create before insertion when the role lacks create", async () => {
    const editor = persistence.withPolicy(principal(ids.editor), "Article.create");
    await expect(
      editor.create_Article({
        id: "00000000-0000-0000-0000-000000000040",
        company_id: ids.company,
        title: "Denied",
        billing_reference: null,
      }),
    ).rejects.toBeInstanceOf(PolicyFault);
  });

  test("authorizes create from proposed scope and conceals denied deletes", async () => {
    const createdId = "00000000-0000-0000-0000-000000000041";
    const owner = persistence.withPolicy(principal(ids.owner), "Article.create");
    expect(
      await owner.create_Article({
        id: createdId,
        company_id: ids.company,
        title: "Owner-created",
        billing_reference: null,
      }),
    ).toMatchObject({ id: createdId, company_id: ids.company });

    const viewer = persistence.withPolicy(principal(ids.viewer), "Article.delete");
    expect(await viewer.delete_required_Article_by_id(createdId)).toBeNull();

    const deletingOwner = persistence.withPolicy(principal(ids.owner), "Article.delete");
    expect(await deletingOwner.delete_required_Article_by_id(createdId)).toMatchObject({
      id: createdId,
    });
  });

  test("resolves application roles for non-entity invoke policies", async () => {
    const operator = persistence.withPolicy(principal(ids.editor), "administrative_health");
    expect(await operator.allowsPolicy("administrative_health", "invoke")).toBe(true);

    const viewer = persistence.withPolicy(principal(ids.viewer), "administrative_health");
    expect(await viewer.allowsPolicy("administrative_health", "invoke")).toBe(false);

    const service = persistence.withPolicy(
      servicePrincipal(ids.service),
      "administrative_health",
    );
    expect(await service.allowsPolicy("administrative_health", "invoke")).toBe(true);
  });

  test("enforces a public callable invoke policy through the route", async () => {
    const response = await handleRequest(new Request("http://jadpo.test/public-health"));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ok: true });
  });

  test("observes membership revocation on the next authoritative operation", async () => {
    const database = new Database(databasePath, { strict: true });
    try {
      database
        .prepare('DELETE FROM "company_membership" WHERE "user_id" = ?1')
        .run(ids.editor);
    } finally {
      database.close();
    }

    const editor = persistence.withPolicy(principal(ids.editor), "Article.find_after_revocation");
    expect(await editor.query_optional_Article_by_id(ids.article)).toBeNull();
  });
});
