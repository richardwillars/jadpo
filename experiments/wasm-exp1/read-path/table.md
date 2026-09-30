# Large-read HTTP results

Five paired repetitions; medians. Every pair and range is retained in results.json.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms | CPU µs/request |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | read_small | bun | 10492 | 0.1268 | 0.2029 | 50.8 |
| 1 | read_small | previous | 10705 | 0.1231 | 0.1926 | 49.1 |
| 1 | read_small | candidate | 10722 | 0.1232 | 0.1910 | 49.5 |
| 1 | read_small | noop | 15918 | 0.0815 | 0.1193 | 19.0 |
| 1 | read_large | bun | 7681 | 0.1808 | 0.3295 | 65.2 |
| 1 | read_large | previous | 6614 | 0.2100 | 0.5442 | 86.7 |
| 1 | read_large | candidate | 6898 | 0.2028 | 0.5422 | 81.1 |
| 1 | read_large | noop | 9940 | 0.1372 | 0.2667 | 35.1 |
| 16 | read_small | bun | 24419 | 1.1796 | 1.4594 | 40.8 |
| 16 | read_small | previous | 25190 | 1.1265 | 1.3750 | 39.3 |
| 16 | read_small | candidate | 25892 | 1.1150 | 1.3456 | 38.5 |
| 16 | read_small | noop | 31547 | 0.8885 | 1.0040 | 16.4 |
| 16 | read_large | bun | 17017 | 1.7032 | 2.3775 | 56.5 |
| 16 | read_large | previous | 12372 | 2.2563 | 2.8365 | 79.4 |
| 16 | read_large | candidate | 13783 | 2.0606 | 2.6750 | 70.6 |
| 16 | read_large | noop | 19766 | 1.4193 | 1.7427 | 32.6 |
