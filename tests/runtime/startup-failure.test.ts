import { describe, expect, test } from "bun:test";
import { join } from "node:path";

const generatedApplication = join(
  import.meta.dir,
  "../../examples/jadpo-seed/build/target/app.ts",
);

describe("generated runtime startup failures", () => {
  test("reports an unreachable database as one secret-safe structured event", async () => {
    const canary = "startup-password-canary";
    const child = Bun.spawn(
      [process.execPath, "--no-install", generatedApplication],
      {
        env: {
          ...Bun.env,
          DATABASE_URL: `postgres://jadpo:${canary}@127.0.0.1:1/jadpo`,
          JADPO_DEBUG_TARGET_STACKS: "0",
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    const [exitCode, stdout, stderr] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ]);

    expect(exitCode).toBe(1);
    expect(stdout).toBe("");
    expect(stderr).not.toContain(canary);
    expect(stderr).not.toContain("PersistenceFault");
    expect(stderr).not.toContain("    at ");
    const lines = stderr.trim().split("\n");
    expect(lines).toHaveLength(1);
    expect(JSON.parse(lines[0])).toMatchObject({
      schemaVersion: 1,
      kind: "operational_log_event",
      eventName: "operation.failed",
      classification: "RUNTIME_STARTUP_FAILED",
      requestId: "startup",
      traceId: null,
      semanticOperationId: "runtime:start",
      attributes: {},
    });
  });
});
