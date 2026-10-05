#!/usr/bin/env bash
set -euo pipefail
export PATH="/Applications/Postgres.app/Contents/Versions/16/bin:/Users/richardwillars/.bun/bin:$PATH"
review_root=/private/tmp/rm306-scheduler-review-8cb7amu7
cluster=$(mktemp -d /private/tmp/rm306-scheduler-review-pg.XXXXXX)
cleanup() { pg_ctl -D "$cluster/data" -m immediate -w stop >/dev/null 2>&1 || true; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
initdb -D "$cluster/data" --username=jadpo_auth_test --auth=trust --no-locale --encoding=UTF8 >"$cluster/init.log"
port=$(bun --no-install --env-file=/dev/null -e 'const s=Bun.serve({hostname:"127.0.0.1",port:0,fetch:()=>new Response()});console.log(s.port);s.stop(true)')
pg_ctl -D "$cluster/data" -l "$cluster/server.log" -o "-h 127.0.0.1 -p $port -k $cluster" -w start >/dev/null
unset DATABASE_URL SQLITE_PATH JADPO_DELIVERY_HOOK_DATABASE_URL JADPO_DELIVERY_HOOK_COMPONENT_VARIANT
for db in jadpo_auth_test_scheduler_review_focus jadpo_auth_test_scheduler_review_probe; do createdb -h 127.0.0.1 -p "$port" -U jadpo_auth_test "$db"; done
export CARGO_TARGET_DIR="$review_root/target" CARGO_INCREMENTAL=0
JADPO_DELIVERY_HOOK_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_scheduler_review_focus" bun --no-install --env-file=/dev/null test tests/runtime/delivery-hook-components.test.ts --test-name-pattern 'closed scheduler assembly components' >"$review_root/postgres-focused.log" 2>&1
JADPO_DELIVERY_HOOK_DATABASE_URL="postgres://jadpo_auth_test@127.0.0.1:$port/jadpo_auth_test_scheduler_review_probe" bun --no-install --env-file=/dev/null "$review_root/corrected-provider-probe.ts" >"$review_root/postgres-corrected.log" 2>&1
printf 'retained_cluster=%s\n' "$cluster"
cat "$review_root/postgres-focused.log"
cat "$review_root/postgres-corrected.log"
