# Immutable-row HTTP results

Medians of paired runs. CPU is server process CPU per request. See results.json for every pair and gate.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms | CPU µs/request |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | read_small | bun | 9600 | 0.1519 | 0.2169 | 53.9 |
| 1 | read_small | previous | 9729 | 0.1498 | 0.2160 | 52.3 |
| 1 | read_small | candidate | 9753 | 0.1495 | 0.2210 | 53.1 |
| 1 | read_small | noop | 14897 | 0.0975 | 0.1301 | 19.8 |
| 1 | read_large | bun | 6923 | 0.2260 | 0.4588 | 70.7 |
| 1 | read_large | previous | 5887 | 0.2610 | 0.5740 | 97.4 |
| 1 | read_large | candidate | 5926 | 0.2601 | 0.5804 | 94.8 |
| 1 | read_large | noop | 9188 | 0.1591 | 0.3052 | 37.3 |
| 16 | read_small | bun | 24280 | 1.2123 | 1.5367 | 41.1 |
| 16 | read_small | previous | 24997 | 1.1500 | 1.4137 | 39.6 |
| 16 | read_small | candidate | 25045 | 1.1462 | 1.4618 | 39.7 |
| 16 | read_small | noop | 31205 | 0.8858 | 1.0510 | 17.2 |
| 16 | read_large | bun | 16355 | 1.7882 | 2.4598 | 58.6 |
| 16 | read_large | previous | 11642 | 2.4132 | 3.0535 | 84.4 |
| 16 | read_large | candidate | 11984 | 2.3660 | 3.0463 | 81.8 |
| 16 | read_large | noop | 19037 | 1.4846 | 1.8219 | 33.8 |
| 1 | write_single | bun | 1718 | 0.8607 | 1.3668 | 429.1 |
| 1 | write_single | previous | 2016 | 0.7371 | 1.0292 | 370.2 |
| 1 | write_single | candidate | 1992 | 0.7241 | 1.0403 | 368.2 |
| 1 | write_pair | bun | 1622 | 0.8868 | 1.4525 | 485.3 |
| 1 | write_pair | previous | 1793 | 0.7993 | 1.1495 | 420.5 |
| 1 | write_pair | candidate | 1860 | 0.7780 | 1.1678 | 417.0 |
| 16 | write_single | bun | 1378 | 14.4885 | 86.6419 | 394.2 |
| 16 | write_single | previous | 1208 | 13.1712 | 46.9482 | 379.3 |
| 16 | write_single | candidate | 888 | 11.2375 | 50.1903 | 365.2 |
| 16 | write_pair | bun | 2038 | 13.2297 | 17.5395 | 466.2 |
| 16 | write_pair | previous | 1377 | 12.6365 | 104.9719 | 411.4 |
| 16 | write_pair | candidate | 1220 | 11.6588 | 15.1522 | 412.7 |
