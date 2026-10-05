import { connect } from "node:net";

// Run with `bun tests/validation/rm104-raw-header-probe.ts` where localhost
// binding is permitted. This observes Bun's real HTTP parser, not Fetch's
// synthetic Headers construction.
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  fetch(request) {
    return Response.json({
      authorization: request.headers.get("authorization"),
      entries: [...request.headers.entries()].filter(([name]) => name === "authorization"),
    });
  },
});

try {
  const response = await new Promise<string>((resolve, reject) => {
    const socket = connect(server.port, "127.0.0.1");
    let received = "";
    socket.on("connect", () => socket.write(
      "POST /auth/exchange HTTP/1.1\r\n" +
      "Host: 127.0.0.1\r\n" +
      "Authorization: Bearer first\r\n" +
      "Authorization: Bearer second\r\n" +
      "Connection: close\r\n" +
      "Content-Length: 0\r\n\r\n",
    ));
    socket.on("data", chunk => { received += chunk.toString(); });
    socket.on("end", () => resolve(received.slice(received.indexOf("\r\n\r\n") + 4)));
    socket.on("error", reject);
  });
  const observed = JSON.parse(response);
  if (observed.authorization !== "Bearer first, Bearer second" || observed.entries.length !== 1) {
    throw new Error(`Unexpected Authorization header presentation: ${JSON.stringify(observed)}`);
  }
  console.log(JSON.stringify({
    schemaVersion: 1,
    runtime: `Bun ${Bun.version}`,
    request: "two raw Authorization lines over a TCP socket to Bun.serve",
    observed,
    bearerInventoryValues: observed.authorization.split(",").map((value: string) => value.trim()),
  }, null, 2));
} finally {
  server.stop(true);
}
