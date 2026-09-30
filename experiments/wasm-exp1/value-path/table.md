# Local HTTP results

Medians of five runs; times are end-to-end milliseconds. CPU is total server user+system microseconds per completed request. See raw runs for variation.

| Concurrency | Workload | Target | Requests/s | p50 ms | p95 ms | p99 ms | Server CPU µs/request |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | read_small | bun | 9,586 | 0.0895 | 0.1560 | 0.2190 | 54.6 |
| 1 | read_small | previous | 9,388 | 0.0924 | 0.1529 | 0.2298 | 59.9 |
| 1 | read_small | candidate | 9,555 | 0.0895 | 0.1583 | 0.2533 | 56.9 |
| 1 | read_small | noop | 14,860 | 0.0584 | 0.0975 | 0.1324 | 20.0 |
| 1 | read_large | bun | 6,894 | 0.1254 | 0.2285 | 0.4478 | 72.2 |
| 1 | read_large | previous | 5,456 | 0.1627 | 0.2723 | 0.5740 | 111.6 |
| 1 | read_large | candidate | 5,491 | 0.1620 | 0.2773 | 0.5995 | 107.8 |
| 1 | read_large | noop | 8,682 | 0.0958 | 0.1716 | 0.4127 | 39.2 |
| 1 | write_single | bun | 1,809 | 0.4935 | 0.8225 | 1.2430 | 407.3 |
| 1 | write_single | previous | 2,010 | 0.4525 | 0.7606 | 1.0989 | 361.0 |
| 1 | write_single | candidate | 1,992 | 0.4439 | 0.7859 | 1.2398 | 352.7 |
| 1 | write_pair | bun | 1,679 | 0.5298 | 0.8812 | 1.4079 | 472.8 |
| 1 | write_pair | previous | 1,687 | 0.4997 | 0.8593 | 1.2590 | 440.8 |
| 1 | write_pair | candidate | 1,688 | 0.4961 | 0.8427 | 1.2778 | 417.1 |
| 16 | read_small | bun | 23,071 | 0.5745 | 1.2942 | 1.7133 | 43.3 |
| 16 | read_small | previous | 22,648 | 0.6008 | 1.2638 | 1.5237 | 43.8 |
| 16 | read_small | candidate | 25,121 | 0.5348 | 1.1492 | 1.4103 | 39.7 |
| 16 | read_small | noop | 30,354 | 0.4601 | 0.9118 | 1.0822 | 17.8 |
| 16 | read_large | bun | 16,143 | 0.8354 | 1.8143 | 2.4988 | 59.3 |
| 16 | read_large | previous | 10,025 | 1.4334 | 2.7516 | 3.3380 | 99.1 |
| 16 | read_large | candidate | 10,561 | 1.3497 | 2.6208 | 3.2290 | 94.5 |
| 16 | read_large | noop | 18,825 | 0.7478 | 1.4930 | 1.8201 | 33.8 |
| 16 | write_single | bun | 2,233 | 6.4843 | 12.3557 | 19.0044 | 386.7 |
| 16 | write_single | previous | 2,190 | 5.8792 | 12.1075 | 16.8594 | 347.2 |
| 16 | write_single | candidate | 1,186 | 5.8235 | 10.7571 | 79.5137 | 335.7 |
| 16 | write_pair | bun | 2,122 | 7.0744 | 12.0056 | 15.1828 | 445.3 |
| 16 | write_pair | previous | 1,549 | 6.7882 | 20.7737 | 33.5449 | 418.9 |
| 16 | write_pair | candidate | 1,292 | 6.6324 | 13.3588 | 31.1063 | 403.3 |
