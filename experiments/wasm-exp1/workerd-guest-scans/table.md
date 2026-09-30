# Workerd guest scans HTTP results

Five paired repetitions; medians. `hybrid` is frozen prior sampled WASM; `bulk` is the selected typed SIMD/bulk-memory guest.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | small | js | 2351 | 0.5202 | 0.6474 |
| 1 | small | hybrid | 2309 | 0.5350 | 0.6609 |
| 1 | small | bulk | 2199 | 0.5632 | 0.7683 |
| 1 | small | noop | 2771 | 0.4565 | 0.5534 |
| 1 | large | js | 1966 | 0.6537 | 0.9003 |
| 1 | large | hybrid | 1873 | 0.6936 | 0.9488 |
| 1 | large | bulk | 1997 | 0.6679 | 0.8621 |
| 1 | large | noop | 2267 | 0.5719 | 0.7665 |
| 1 | escaped | js | 1978 | 0.6470 | 0.8458 |
| 1 | escaped | hybrid | 1862 | 0.7006 | 0.9046 |
| 1 | escaped | bulk | 1796 | 0.6750 | 0.8301 |
| 1 | escaped | noop | 2220 | 0.5438 | 0.6824 |
| 1 | unicode | js | 1587 | 0.8050 | 1.0310 |
| 1 | unicode | hybrid | 1315 | 0.9065 | 1.2572 |
| 1 | unicode | bulk | 1481 | 0.8535 | 1.0499 |
| 1 | unicode | noop | 2055 | 0.6409 | 0.8106 |
| 1 | late | js | 1635 | 0.7296 | 1.0537 |
| 1 | late | hybrid | 1428 | 0.8897 | 1.3357 |
| 1 | late | bulk | 1655 | 0.7588 | 1.0695 |
| 1 | late | noop | 1971 | 0.6217 | 0.8781 |
| 16 | small | js | 3277 | 9.2112 | 45.1150 |
| 16 | small | hybrid | 3347 | 9.2239 | 39.2884 |
| 16 | small | bulk | 3318 | 8.6886 | 40.2969 |
| 16 | small | noop | 3831 | 7.7675 | 38.9193 |
| 16 | large | js | 3085 | 10.0779 | 18.7046 |
| 16 | large | hybrid | 2735 | 10.7374 | 42.2395 |
| 16 | large | bulk | 2828 | 10.7358 | 43.8193 |
| 16 | large | noop | 3304 | 9.0939 | 18.2350 |
| 16 | escaped | js | 2851 | 9.9680 | 43.0516 |
| 16 | escaped | hybrid | 2533 | 11.2246 | 22.1662 |
| 16 | escaped | bulk | 2679 | 11.0282 | 20.9270 |
| 16 | escaped | noop | 3069 | 8.3841 | 43.2233 |
| 16 | unicode | js | 2260 | 12.2864 | 44.9485 |
| 16 | unicode | hybrid | 2082 | 13.6171 | 19.3432 |
| 16 | unicode | bulk | 2023 | 13.8662 | 46.4465 |
| 16 | unicode | noop | 3062 | 9.6895 | 15.5298 |
| 16 | late | js | 2631 | 10.7438 | 15.5453 |
| 16 | late | hybrid | 1930 | 13.4683 | 50.4179 |
| 16 | late | bulk | 2372 | 11.6239 | 48.8618 |
| 16 | late | noop | 3040 | 9.4759 | 44.1679 |
