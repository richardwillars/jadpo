import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { handleRequest } from "../../examples/outcome-sequencing/build/target/app.ts";

let server: ReturnType<typeof Bun.serve>;

beforeAll(() => {
  let lastError: unknown;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const port = 30_000 + Math.floor(Math.random() * 20_000);
    try {
      server = Bun.serve({ port, fetch: handleRequest });
      return;
    } catch (error) {
      lastError = error;
      // A parallel process won this port; try another bounded candidate.
    }
  }
  throw new Error(`could not allocate an outcome-test port: ${String(lastError)}`);
});

afterAll(() => {
  server?.stop(true);
});

async function get(path: string): Promise<Response> {
  return fetch(new URL(path, server.url));
}

describe("generated exhaustive outcome sequencing", () => {
  test("continues with the successful value", async () => {
    const response = await get("/outcomes/success");
    expect(response.status).toBe(200);
    expect(await response.json()).toBe("found");
  });

  test("recovers locally with a compatible successful value", async () => {
    const response = await get("/outcomes/recover");
    expect(response.status).toBe(200);
    expect(await response.json()).toBe("recovered");
  });

  test("maps a matched failure to the declared replacement", async () => {
    const response = await get("/outcomes/map");
    expect(response.status).toBe(422);
    expect(await response.json()).toMatchObject({
      error: { code: "lookup_rejected" },
    });
  });

  test("propagates the exact matched failure", async () => {
    const response = await get("/outcomes/propagate");
    expect(response.status).toBe(404);
    expect(await response.json()).toMatchObject({
      error: { code: "lookup_missing" },
    });
  });
});
