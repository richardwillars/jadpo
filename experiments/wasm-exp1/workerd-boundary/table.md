# Workerd boundary HTTP results

Five paired repetitions; medians. `candidate` is the frozen previous driver, `views` is the selected new driver.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | small | js | 2328 | 0.5291 | 0.6880 |
| 1 | small | candidate | 2146 | 0.5655 | 0.6925 |
| 1 | small | views | 2248 | 0.5486 | 0.6697 |
| 1 | small | noop | 2690 | 0.4590 | 0.5659 |
| 1 | large | js | 1974 | 0.6222 | 0.8045 |
| 1 | large | candidate | 1939 | 0.6629 | 0.8877 |
| 1 | large | views | 1966 | 0.6620 | 0.8610 |
| 1 | large | noop | 2304 | 0.5590 | 0.7059 |
| 1 | escaped | js | 2026 | 0.6307 | 0.7708 |
| 1 | escaped | candidate | 1591 | 0.7647 | 0.9252 |
| 1 | escaped | views | 1659 | 0.7343 | 0.9040 |
| 1 | escaped | noop | 2325 | 0.5306 | 0.6459 |
| 16 | small | js | 3110 | 8.8276 | 43.5639 |
| 16 | small | candidate | 3222 | 8.9383 | 40.3535 |
| 16 | small | views | 3267 | 8.6329 | 40.8335 |
| 16 | small | noop | 3785 | 7.2268 | 40.1190 |
| 16 | large | js | 2925 | 10.0681 | 44.7362 |
| 16 | large | candidate | 2623 | 10.7060 | 17.9531 |
| 16 | large | views | 2690 | 10.5002 | 44.5332 |
| 16 | large | noop | 3313 | 9.2702 | 16.9116 |
| 16 | escaped | js | 2985 | 9.7909 | 18.1116 |
| 16 | escaped | candidate | 2220 | 12.8656 | 45.8754 |
| 16 | escaped | views | 2196 | 12.6030 | 48.8964 |
| 16 | escaped | noop | 3291 | 8.0287 | 21.8205 |
