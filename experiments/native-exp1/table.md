# Native experiment measurements

Medians; raw ranges and paired ratios are in `results.json`. HTTP rates are local closed-loop observations, not capacity.

## HTTP

| Workload | C | Target | req/s | p50 ms | p95 ms | p99 ms | max ms¹ | CPU µs/req | RSS MiB |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| noop_large | 1 | bun | 10197 | 0.089 | 0.137 | 0.244 | 1.345 | 34.48 | 79.75 |
| noop_large | 1 | native | 11701 | 0.078 | 0.122 | 0.197 | 1.250 | 24.70 | 4.61 |
| noop_small | 1 | bun | 15599 | 0.058 | 0.093 | 0.128 | 1.145 | 19.43 | 90.39 |
| noop_small | 1 | native | 18233 | 0.050 | 0.080 | 0.107 | 0.514 | 13.41 | 4.53 |
| noop_unicode | 1 | bun | 8280 | 0.110 | 0.166 | 0.277 | 1.177 | 43.70 | 79.80 |
| noop_unicode | 1 | native | 9689 | 0.095 | 0.144 | 0.225 | 1.006 | 27.62 | 4.78 |
| read_large | 1 | bun | 7559 | 0.116 | 0.196 | 0.310 | 1.186 | 67.47 | 113.95 |
| read_large | 1 | native | 8128 | 0.113 | 0.171 | 0.240 | 1.242 | 60.39 | 4.20 |
| read_small | 1 | bun | 10667 | 0.083 | 0.137 | 0.209 | 1.068 | 52.56 | 106.25 |
| read_small | 1 | native | 11690 | 0.079 | 0.120 | 0.153 | 0.687 | 41.42 | 3.11 |
| read_unicode | 1 | bun | 5581 | 0.161 | 0.254 | 0.389 | 1.225 | 97.38 | 121.17 |
| read_unicode | 1 | native | 6766 | 0.136 | 0.203 | 0.273 | 1.081 | 70.71 | 4.55 |
| write_pair | 1 | bun | 4153 | 0.203 | 0.339 | 1.132 | 8.555 | 202.13 | 125.08 |
| write_pair | 1 | native | 5591 | 0.161 | 0.233 | 0.302 | 5.739 | 115.24 | 4.66 |
| write_single | 1 | bun | 5283 | 0.157 | 0.258 | 0.819 | 17.074 | 131.60 | 124.05 |
| write_single | 1 | native | 7373 | 0.124 | 0.184 | 0.229 | 5.249 | 75.99 | 4.62 |
| noop_large | 16 | bun | 20242 | 0.689 | 1.413 | 1.792 | 3.321 | 31.39 | 112.78 |
| noop_large | 16 | native | 20540 | 0.683 | 1.395 | 1.793 | 3.370 | 22.48 | 5.31 |
| noop_small | 16 | bun | 31696 | 0.446 | 0.879 | 1.028 | 1.917 | 16.90 | 112.83 |
| noop_small | 16 | native | 32249 | 0.444 | 0.870 | 1.003 | 1.573 | 11.77 | 5.19 |
| noop_unicode | 16 | bun | 16094 | 0.892 | 1.757 | 2.047 | 3.715 | 40.60 | 112.75 |
| noop_unicode | 16 | native | 16215 | 0.884 | 1.747 | 2.035 | 3.905 | 25.18 | 5.42 |
| read_large | 16 | bun | 16756 | 0.801 | 1.708 | 2.455 | 5.738 | 57.32 | 118.58 |
| read_large | 16 | native | 17646 | 0.849 | 1.148 | 2.267 | 3.265 | 53.02 | 5.59 |
| read_small | 16 | bun | 26843 | 0.508 | 1.095 | 1.467 | 4.955 | 37.00 | 125.89 |
| read_small | 16 | native | 28557 | 0.544 | 0.687 | 0.802 | 1.641 | 34.48 | 4.25 |
| read_unicode | 16 | bun | 11064 | 1.276 | 2.530 | 3.139 | 7.359 | 86.63 | 115.66 |
| read_unicode | 16 | native | 14872 | 1.021 | 1.333 | 2.498 | 3.725 | 63.47 | 5.62 |
| write_pair | 16 | bun | 5649 | 2.478 | 4.837 | 6.215 | 17.540 | 175.31 | 117.38 |
| write_pair | 16 | native | 7352 | 2.034 | 2.528 | 4.140 | 11.596 | 110.17 | 5.61 |
| write_single | 16 | bun | 7936 | 1.758 | 3.603 | 4.526 | 20.040 | 112.59 | 133.34 |
| write_single | 16 | native | 10747 | 1.401 | 1.775 | 3.481 | 20.666 | 71.10 | 5.66 |

¹ Median of each cell’s maximum, not the maximum over the campaign. Full maxima remain in the raw results.

## DELETE

| Workload | C | Target | req/s | p50 ms | p95 ms | p99 ms | max ms¹ | CPU µs/req | RSS MiB |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| write_pair | 1 | bun | 1704 | 0.518 | 0.847 | 1.396 | 34.329 | 483.18 | 100.20 |
| write_pair | 1 | native | 2067 | 0.456 | 0.661 | 0.810 | 6.230 | 342.18 | 3.48 |
| write_single | 1 | bun | 1811 | 0.499 | 0.815 | 1.221 | 18.304 | 419.89 | 87.75 |
| write_single | 1 | native | 2046 | 0.431 | 0.639 | 0.856 | 88.491 | 315.66 | 3.17 |
| write_pair | 16 | bun | 1256 | 7.075 | 12.927 | 19.790 | 182.397 | 449.02 | 102.47 |
| write_pair | 16 | native | 1500 | 6.241 | 15.464 | 46.604 | 79.000 | 340.76 | 3.92 |
| write_single | 16 | bun | 2381 | 6.144 | 11.388 | 16.044 | 47.113 | 389.58 | 96.38 |
| write_single | 16 | native | 1116 | 5.661 | 7.198 | 31.419 | 1218.636 | 300.99 | 3.81 |

¹ Median of each cell’s maximum, not the maximum over the campaign. Full maxima remain in the raw results.

