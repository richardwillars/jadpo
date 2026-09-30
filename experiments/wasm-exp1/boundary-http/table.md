# Local HTTP results

Medians of five runs; times are end-to-end milliseconds. CPU is total server user+system microseconds per completed request. See raw runs for variation.

| Concurrency | Workload | Target | Requests/s | p50 ms | p95 ms | p99 ms | Server CPU µs/request |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | read_small | bun | 8,993 | 0.0931 | 0.1739 | 0.2630 | 58.9 |
| 1 | read_small | previous | 8,425 | 0.1000 | 0.1738 | 0.2888 | 69.4 |
| 1 | read_small | candidate | 8,904 | 0.0935 | 0.1668 | 0.3003 | 63.0 |
| 1 | read_small | noop | 14,556 | 0.0590 | 0.1018 | 0.1418 | 20.6 |
| 1 | read_large | bun | 6,761 | 0.1261 | 0.2330 | 0.6038 | 73.7 |
| 1 | read_large | previous | 4,727 | 0.1870 | 0.3178 | 0.6330 | 138.6 |
| 1 | read_large | candidate | 5,231 | 0.1667 | 0.2980 | 0.5941 | 114.1 |
| 1 | read_large | noop | 9,053 | 0.0954 | 0.1680 | 0.3184 | 37.8 |
| 1 | write_single | bun | 1,625 | 0.5189 | 1.0387 | 1.7305 | 440.2 |
| 1 | write_single | previous | 1,943 | 0.4574 | 0.8362 | 1.1987 | 373.3 |
| 1 | write_single | candidate | 1,785 | 0.4827 | 0.8610 | 1.2763 | 390.0 |
| 1 | write_pair | bun | 1,554 | 0.5526 | 1.0126 | 1.5225 | 505.4 |
| 1 | write_pair | previous | 1,665 | 0.5404 | 0.9611 | 1.4397 | 475.7 |
| 1 | write_pair | candidate | 1,570 | 0.5657 | 0.9885 | 1.7811 | 474.3 |
| 16 | read_small | bun | 23,151 | 0.5597 | 1.2857 | 2.0221 | 41.8 |
| 16 | read_small | previous | 18,746 | 0.7242 | 1.4944 | 1.9986 | 52.5 |
| 16 | read_small | candidate | 21,455 | 0.6086 | 1.3385 | 1.9035 | 45.4 |
| 16 | read_small | noop | 30,534 | 0.4536 | 0.9052 | 1.0966 | 17.4 |
| 16 | read_large | bun | 15,297 | 0.8503 | 1.8971 | 2.7396 | 59.4 |
| 16 | read_large | previous | 7,811 | 1.8519 | 3.5242 | 4.3041 | 131.9 |
| 16 | read_large | candidate | 9,520 | 1.4543 | 2.8654 | 4.0498 | 103.8 |
| 16 | read_large | noop | 18,367 | 0.7455 | 1.5245 | 2.0683 | 34.4 |
| 16 | write_single | bun | 1,938 | 6.8041 | 13.2407 | 26.3985 | 401.9 |
| 16 | write_single | previous | 1,568 | 6.1299 | 11.2343 | 14.1372 | 349.2 |
| 16 | write_single | candidate | 1,631 | 5.9801 | 10.7257 | 14.8995 | 339.8 |
| 16 | write_pair | bun | 1,845 | 7.5796 | 14.2232 | 22.3390 | 454.6 |
| 16 | write_pair | previous | 1,913 | 7.3258 | 14.2722 | 31.1970 | 462.0 |
| 16 | write_pair | candidate | 1,834 | 7.0187 | 13.5157 | 18.1710 | 441.8 |
