# Workerd native-values HTTP results

Five paired repetitions; medians. `views` is the frozen previous JSON driver; `hybrid` is the sampled candidate.

| Concurrency | Workload | Target | Requests/s | p95 ms | p99 ms |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | small | js | 2377 | 0.5509 | 0.6816 |
| 1 | small | views | 2325 | 0.5851 | 0.7165 |
| 1 | small | hybrid | 2267 | 0.5761 | 0.6958 |
| 1 | small | noop | 2666 | 0.4735 | 0.5855 |
| 1 | large | js | 2029 | 0.6663 | 0.9494 |
| 1 | large | views | 1880 | 0.7259 | 1.0945 |
| 1 | large | hybrid | 1781 | 0.7187 | 0.9871 |
| 1 | large | noop | 2169 | 0.6090 | 0.8449 |
| 1 | escaped | js | 1925 | 0.6820 | 0.9024 |
| 1 | escaped | views | 1588 | 0.8339 | 1.0629 |
| 1 | escaped | hybrid | 1795 | 0.7415 | 0.9045 |
| 1 | escaped | noop | 2260 | 0.5712 | 0.7650 |
| 1 | unicode | js | 1555 | 0.8650 | 1.1798 |
| 1 | unicode | views | 1368 | 0.9780 | 1.1630 |
| 1 | unicode | hybrid | 1336 | 0.9620 | 1.4019 |
| 1 | unicode | noop | 1916 | 0.6610 | 0.8143 |
| 1 | late | js | 1718 | 0.7484 | 1.2362 |
| 1 | late | views | 1279 | 1.0073 | 1.5821 |
| 1 | late | hybrid | 1316 | 0.9475 | 1.6492 |
| 1 | late | noop | 2008 | 0.6217 | 0.7785 |
| 16 | small | js | 3312 | 8.8328 | 38.9185 |
| 16 | small | views | 3247 | 8.9340 | 42.5251 |
| 16 | small | hybrid | 3289 | 9.2305 | 42.1737 |
| 16 | small | noop | 3729 | 7.6758 | 43.5498 |
| 16 | large | js | 2982 | 10.3793 | 16.1419 |
| 16 | large | views | 2715 | 10.8518 | 47.9774 |
| 16 | large | hybrid | 2685 | 10.6796 | 17.1727 |
| 16 | large | noop | 3449 | 8.8406 | 15.6242 |
| 16 | escaped | js | 2706 | 11.0271 | 48.5675 |
| 16 | escaped | views | 2240 | 12.4312 | 19.1855 |
| 16 | escaped | hybrid | 2650 | 10.6625 | 20.7185 |
| 16 | escaped | noop | 3137 | 7.9974 | 23.3294 |
| 16 | unicode | js | 2224 | 12.2636 | 39.7786 |
| 16 | unicode | views | 2015 | 13.9657 | 18.4698 |
| 16 | unicode | hybrid | 1975 | 14.0981 | 20.7422 |
| 16 | unicode | noop | 2896 | 10.1117 | 38.4809 |
| 16 | late | js | 2567 | 10.5997 | 17.3559 |
| 16 | late | views | 1995 | 13.1085 | 18.3333 |
| 16 | late | hybrid | 2080 | 13.5464 | 18.2728 |
| 16 | late | noop | 3066 | 9.3800 | 14.9105 |
