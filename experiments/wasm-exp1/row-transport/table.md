# Row transport HTTP results

Medians of paired runs. CPU is server process CPU per request. See results.json for every pair and gate.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms | CPU µs/request |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | read_small | bun | 9530 | 0.1533 | 0.2442 | 53.5 |
| 1 | read_small | previous | 9901 | 0.1463 | 0.2100 | 51.6 |
| 1 | read_small | candidate | 9822 | 0.1481 | 0.2106 | 52.4 |
| 1 | read_small | noop | 14836 | 0.0983 | 0.1331 | 20.0 |
| 1 | read_large | bun | 6922 | 0.2253 | 0.4681 | 72.1 |
| 1 | read_large | previous | 5718 | 0.2692 | 0.5574 | 102.5 |
| 1 | read_large | candidate | 5918 | 0.2604 | 0.5686 | 96.2 |
| 1 | read_large | noop | 9222 | 0.1592 | 0.3010 | 37.2 |
| 16 | read_small | bun | 24340 | 1.2137 | 1.5291 | 41.2 |
| 16 | read_small | previous | 25371 | 1.1315 | 1.3690 | 39.2 |
| 16 | read_small | candidate | 25182 | 1.1326 | 1.4115 | 39.3 |
| 16 | read_small | noop | 31107 | 0.8889 | 1.0513 | 17.0 |
| 16 | read_large | bun | 16470 | 1.7647 | 2.4392 | 58.3 |
| 16 | read_large | previous | 10914 | 2.5442 | 3.1740 | 90.2 |
| 16 | read_large | candidate | 11676 | 2.4108 | 3.0480 | 84.0 |
| 16 | read_large | noop | 19023 | 1.4846 | 1.8011 | 33.7 |
| 1 | write_single | bun | 1773 | 0.8684 | 1.3656 | 430.2 |
| 1 | write_single | previous | 1946 | 0.7205 | 1.0545 | 369.5 |
| 1 | write_single | candidate | 2037 | 0.7195 | 1.0609 | 367.6 |
| 1 | write_pair | bun | 1541 | 0.9335 | 1.5581 | 493.3 |
| 1 | write_pair | previous | 1777 | 0.7947 | 1.1632 | 433.8 |
| 1 | write_pair | candidate | 1809 | 0.8006 | 1.1729 | 421.6 |
| 16 | write_single | bun | 1203 | 11.7153 | 17.8049 | 388.7 |
| 16 | write_single | previous | 881 | 51.6447 | 132.9225 | 369.5 |
| 16 | write_single | candidate | 486 | 45.3255 | 1259.1106 | 355.2 |
| 16 | write_pair | bun | 1171 | 13.2845 | 16.5441 | 468.3 |
| 16 | write_pair | previous | 1386 | 11.9712 | 53.0883 | 411.7 |
| 16 | write_pair | candidate | 1987 | 12.6287 | 30.4870 | 413.6 |
