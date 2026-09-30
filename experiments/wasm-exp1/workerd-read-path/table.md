# Local workerd read results

Five rotated repetitions; medians. Generated JS and WASM share workerd and the SQLite authority.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | small | js | 2732 | 0.4226 | 0.5330 |
| 1 | small | previous | 2614 | 0.4510 | 0.5591 |
| 1 | small | candidate | 2718 | 0.4511 | 0.5619 |
| 1 | small | typed | 2641 | 0.4385 | 0.5426 |
| 1 | small | noop | 3037 | 0.3840 | 0.4704 |
| 1 | large | js | 2465 | 0.4922 | 0.6364 |
| 1 | large | previous | 2018 | 0.5988 | 0.8734 |
| 1 | large | candidate | 2039 | 0.5520 | 0.7430 |
| 1 | large | typed | 2182 | 0.5548 | 0.7462 |
| 1 | large | noop | 2576 | 0.4558 | 0.5691 |
| 16 | small | js | 3673 | 8.5951 | 35.0316 |
| 16 | small | previous | 3450 | 9.0505 | 35.9882 |
| 16 | small | candidate | 3284 | 8.7946 | 42.3491 |
| 16 | small | typed | 3538 | 8.7465 | 34.4815 |
| 16 | small | noop | 4131 | 7.2348 | 38.5740 |
| 16 | large | js | 3309 | 9.1714 | 14.7337 |
| 16 | large | previous | 2843 | 10.4356 | 17.3618 |
| 16 | large | candidate | 3019 | 9.9943 | 37.4658 |
| 16 | large | typed | 2731 | 10.1738 | 42.8971 |
| 16 | large | noop | 3611 | 8.6195 | 14.7937 |
