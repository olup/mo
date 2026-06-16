# Pokemon Server Benchmark

Use `scripts/load_pokemon_server.py` to run a reproducible load benchmark against `examples/demo/pokemon_server.mo`.

The benchmark script uses only the Python standard library. It can:

- build the Mo demo server,
- write the demo server request-capacity config,
- start it, detect the ephemeral port from stdout, and wait for the demo
  readiness marker before sending traffic,
- send a configurable number of HTTP requests with configurable concurrency,
- use a weighted route mix,
- sample RSS at a configurable interval,
- emit full JSON and append summary CSV rows for before/after comparisons.

## Quick smoke test

```sh
python3 scripts/load_pokemon_server.py \
  --total 24 \
  --concurrency 4 \
  --json-output build/pokemon_bench_smoke.json \
  --csv-output build/pokemon_bench_smoke.csv \
  --samples-csv-output build/pokemon_bench_smoke_rss.csv \
  --label smoke
```

## Before/after comparison

Run the same command before and after an optimization, changing only `--label`:

```sh
python3 scripts/load_pokemon_server.py \
  --total 1024 \
  --concurrency 32 \
  --route-mix 'GET /health:1,GET /pokemon:4,POST /pokemon:1' \
  --rss-interval 0.05 \
  --json-output build/pokemon_bench_before.json \
  --csv-output build/pokemon_bench_compare.csv \
  --samples-csv-output build/pokemon_bench_before_rss.csv \
  --label before

python3 scripts/load_pokemon_server.py \
  --total 1024 \
  --concurrency 32 \
  --route-mix 'GET /health:1,GET /pokemon:4,POST /pokemon:1' \
  --rss-interval 0.05 \
  --json-output build/pokemon_bench_after.json \
  --csv-output build/pokemon_bench_compare.csv \
  --samples-csv-output build/pokemon_bench_after_rss.csv \
  --label after
```

`build/pokemon_bench_compare.csv` is append-only and contains one summary row per run, including throughput, latency percentiles, and RSS start/end/max/delta.

The script waits until the demo prints `Async executor probe:` before it starts
client requests. The port line alone is not a readiness signal because the demo
prints it before route registration and async startup probes finish.

## Route mix

The route mix format is comma-separated `METHOD /path:weight` entries:

```sh
--route-mix 'GET /health:1,GET /pokemon:4,POST /pokemon:1'
```

The default mix is the same as above. `POST /pokemon` mutates `mo_pokemon_server.json`, and the demo server resets that file on startup.

## Server request capacity

`examples/demo/pokemon_server.mo` reads `mo_pokemon_server_requests.txt` from the repo root at startup. The value is the desired total request capacity. Because the demo uses a fixed 4-worker serving model, the server rounds that value up internally to `ceil(total / 4) * 4`.

The benchmark writes this config file automatically before startup and removes it after the server has started. By default it writes `total + warmup`, rounded up to the 4-worker capacity. To keep the server alive for extra manual probing after the benchmark requests finish, pass a larger capacity:

```sh
python3 scripts/load_pokemon_server.py \
  --total 5000 \
  --concurrency 64 \
  --server-requests 10000 \
  --label plateau
```

If the config file is absent, the demo falls back to `1024` requests, preserving the previous behavior for direct manual runs.

## Build notes

By default the script uses `target/debug/mo` and builds `build/pokemon_server_bench`. If the compiler binary is missing, use:

```sh
python3 scripts/load_pokemon_server.py --build-compiler --total 512 --concurrency 32
```

To reuse an already-built server binary:

```sh
python3 scripts/load_pokemon_server.py --no-build --server-binary build/pokemon_server_bench
```
