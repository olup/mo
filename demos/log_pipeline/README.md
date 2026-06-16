# Concurrent Log Pipeline Demo

This demo models a small backend analytics workload:

- four independent log shards,
- worker-queue concurrency through `std/task`,
- deterministic severity and latency aggregation,
- bounded checksums for verification,
- explicit cleanup for worker job cells.

Run it from the repository root:

```sh
./target/debug/mo build demos/log_pipeline/main.mo -o /tmp/mo_log_pipeline
/tmp/mo_log_pipeline
```

Expected exit code: `42`.
