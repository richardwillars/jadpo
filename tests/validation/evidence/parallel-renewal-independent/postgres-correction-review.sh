#!/usr/bin/env bash
set -euo pipefail
export PATH="/Applications/Postgres.app/Contents/Versions/16/bin:/Users/richardwillars/.bun/bin:$PATH"
cluster=$(mktemp -d /private/tmp/jadpo-renewal-review-pg.XXXXXX)
printf '%s\n' "$cluster" > /private/tmp/jadpo-renewal-review-cluster.txt
cleanup(){ pg_ctl -D "$cluster/data" -m immediate -w stop >/dev/null 2>&1 || true; }
trap cleanup EXIT
initdb -D "$cluster/data" --username=jadpo_auth_test --auth=trust --no-locale --encoding=UTF8 >"$cluster/init.log"
port=$(bun --no-install --env-file=/dev/null -e 'const s=Bun.serve({hostname:"127.0.0.1",port:0,fetch:()=>new Response()});console.log(s.port);s.stop(true)')
pg_ctl -D "$cluster/data" -l "$cluster/server.log" -o "-h 127.0.0.1 -p $port -k $cluster" -w start >/dev/null
createdb -h 127.0.0.1 -p "$port" -U jadpo_auth_test jadpo_auth_test_review
unset DATABASE_URL SQLITE_PATH JADPO_DELIVERY_HOOK_DATABASE_URL JADPO_DELIVERY_HOOK_COMPONENT_VARIANT
export CARGO_TARGET_DIR=/private/tmp/jadpo-independent-renewal-cargo CARGO_INCREMENTAL=0
cd /private/tmp/jadpo-review-final
JADPO_DELIVERY_HOOK_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_review" bun --no-install --env-file=/dev/null test tests/runtime/delivery-hook-components.test.ts --test-name-pattern 'closed scheduler assembly components|native durable singleton activation storage'
REVIEW_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_review" bun --no-install --env-file=/dev/null /private/tmp/jadpo-parallel-integration/tests/validation/evidence/parallel-renewal-independent/native-correction-probe.ts
