#!/usr/bin/env bash
set -euo pipefail

mode=${1:-}
case "$mode" in
  authentication|service-authentication|jwt-authentication|persistence|validation-persistence|validation-auth-policy) ;;
  *) echo 'Usage: bash tests/runtime/postgres.sh authentication|service-authentication|jwt-authentication|persistence|validation-persistence|validation-auth-policy' >&2; exit 2 ;;
esac
for executable in initdb pg_ctl createdb postgres bun; do
  command -v "$executable" >/dev/null || { echo "Required executable: $executable" >&2; exit 1; }
done
repository=$(cd "$(dirname "$0")/../.." && pwd)
cluster=$(mktemp -d /tmp/jadpo-validation-postgres.XXXXXX)
cleanup() {
  pg_ctl -D "$cluster/data" -m immediate -w stop >/dev/null 2>&1 || true
  rm -rf "$cluster"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
postgres --version
bun --version
initdb -D "$cluster/data" --username=jadpo_auth_test --auth=trust --no-locale --encoding=UTF8 >"$cluster/init.log"
port=$(bun --no-install --env-file=/dev/null -e 'const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => new Response() }); console.log(server.port); server.stop(true);')
if ! pg_ctl -D "$cluster/data" -l "$cluster/server.log" -o "-h 127.0.0.1 -p $port -k $cluster" -w start >/dev/null; then
  cat "$cluster/server.log" >&2
  exit 1
fi
createdb -h 127.0.0.1 -p "$port" -U jadpo_auth_test jadpo_auth_test_runtime
cd "$repository"
unset DATABASE_URL SQLITE_PATH JADPO_AUTH_TEST_DATABASE_URL JADPO_SERVICE_AUTH_DATABASE_URL JADPO_JWT_AUTH_DATABASE_URL JADPO_VALIDATION_PERSISTENCE_DATABASE_URL JADPO_VALIDATION_AUTH_POLICY_DATABASE_URL JADPO_DEBUG_TARGET_STACKS
if [[ "$mode" == authentication ]]; then
  JADPO_AUTH_TEST_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_runtime" \
    bun --no-install --env-file=/dev/null test tests/runtime/first-party-authentication.test.ts
elif [[ "$mode" == service-authentication ]]; then
  JADPO_SERVICE_AUTH_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_runtime" \
    bun --no-install --env-file=/dev/null test tests/runtime/service-authentication.test.ts
elif [[ "$mode" == jwt-authentication ]]; then
  JADPO_JWT_AUTH_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_runtime" \
    bun --no-install --env-file=/dev/null test tests/runtime/jwt-integration.test.ts
elif [[ "$mode" == validation-persistence ]]; then
  JADPO_VALIDATION_PERSISTENCE_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_runtime" \
    bun --no-install --env-file=/dev/null test tests/runtime/validation-persistence.test.ts
elif [[ "$mode" == validation-auth-policy ]]; then
  JADPO_VALIDATION_AUTH_POLICY_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_runtime" \
    bun --no-install --env-file=/dev/null test tests/runtime/validation-auth-policy.test.ts
else
  DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_runtime" \
    bun --no-install --env-file=/dev/null test tests/runtime/persistence-postgres.test.ts
fi
