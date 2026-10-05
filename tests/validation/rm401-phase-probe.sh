#!/usr/bin/env bash
set -euo pipefail

for executable in initdb pg_ctl createdb postgres bun; do
  command -v "$executable" >/dev/null || { echo "Required executable: $executable" >&2; exit 1; }
done

repository=$(cd "$(dirname "$0")/../.." && pwd)
cluster=$(mktemp -d /tmp/jadpo-rm401-postgres.XXXXXX)
cleanup() {
  pg_ctl -D "$cluster/data" -m immediate -w stop >/dev/null 2>&1 || true
  rm -rf "$cluster"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

initdb -D "$cluster/data" --username=jadpo_rm401_probe --auth=trust --no-locale --encoding=UTF8 >"$cluster/init.log"
port=$(bun --no-install --env-file=/dev/null -e 'const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response() }); console.log(server.port); server.stop(true);')
if ! pg_ctl -D "$cluster/data" -l "$cluster/server.log" -o "-h 127.0.0.1 -p $port -k $cluster" -w start >/dev/null; then
  cat "$cluster/server.log" >&2
  exit 1
fi
createdb -h 127.0.0.1 -p "$port" -U jadpo_rm401_probe jadpo_rm401_probe
cd "$repository"
RM401_PROBE_PG_URL="postgres://jadpo_rm401_probe@127.0.0.1:$port/jadpo_rm401_probe" \
  bun --no-install --env-file=/dev/null tests/validation/rm401-phase-probe.ts
