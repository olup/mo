#!/usr/bin/env python3
"""Reproducible load benchmark for the Mo Pokemon demo server.

The script intentionally uses only the Python standard library so it can be run
from a fresh checkout:

    python3 scripts/load_pokemon_server.py --total 512 --concurrency 32 \
        --json-output build/pokemon_bench.json \
        --csv-output build/pokemon_bench.csv

By default it builds examples/demo/pokemon_server.mo to build/pokemon_server_bench,
writes the demo server request-capacity config, starts it, parses the ephemeral
port from stdout, samples RSS with ps(1), and sends a weighted mix of GET
/health, GET /pokemon, and POST /pokemon requests.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import queue
import re
import socket
import statistics
import subprocess
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

PORT_RE = re.compile(r"http://127\.0\.0\.1:(\d+)")
DEFAULT_ROUTE_MIX = "GET /health:1,GET /pokemon:4,POST /pokemon:1"
DEFAULT_SERVER_CONFIG = "mo_pokemon_server_requests.txt"


@dataclass(frozen=True)
class Route:
    method: str
    path: str
    weight: int

    @property
    def key(self) -> str:
        return f"{self.method} {self.path}"


@dataclass(frozen=True)
class RequestResult:
    route: str
    ok: bool
    status: int | None
    latency_ms: float
    bytes_read: int
    error: str | None


@dataclass(frozen=True)
class RssSample:
    t: float
    rss_kib: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Load-test the Mo Pokemon demo server and emit reproducible JSON/CSV metrics."
    )
    parser.add_argument("--total", type=int, default=512, help="total HTTP requests to send")
    parser.add_argument("--concurrency", type=int, default=32, help="maximum in-flight requests")
    parser.add_argument(
        "--route-mix",
        default=DEFAULT_ROUTE_MIX,
        help=(
            "weighted route mix, e.g. "
            "'GET /health:1,GET /pokemon:4,POST /pokemon:1'"
        ),
    )
    parser.add_argument(
        "--rss-interval",
        type=float,
        default=0.05,
        help="RSS sampling interval in seconds",
    )
    parser.add_argument("--timeout", type=float, default=5.0, help="per-request socket timeout in seconds")
    parser.add_argument("--warmup", type=int, default=0, help="warmup requests sent before measurement")
    parser.add_argument(
        "--json-output",
        type=Path,
        help="write full benchmark result JSON to this path; stdout is used when omitted",
    )
    parser.add_argument(
        "--csv-output",
        type=Path,
        help="append one summary row to a CSV file, creating a header when needed",
    )
    parser.add_argument(
        "--samples-csv-output",
        type=Path,
        help="write RSS samples to a CSV file with elapsed_seconds,rss_kib columns",
    )
    parser.add_argument(
        "--source",
        type=Path,
        default=Path("examples/demo/pokemon_server.mo"),
        help="Mo source file for the server",
    )
    parser.add_argument(
        "--server-binary",
        type=Path,
        default=Path("build/pokemon_server_bench"),
        help="server executable path",
    )
    parser.add_argument(
        "--compiler",
        type=Path,
        default=Path("target/debug/mo"),
        help="Mo compiler binary path used with --build",
    )
    parser.add_argument(
        "--build",
        dest="build",
        action="store_true",
        default=True,
        help="build the server before running it (default)",
    )
    parser.add_argument(
        "--no-build",
        dest="build",
        action="store_false",
        help="reuse --server-binary without rebuilding",
    )
    parser.add_argument(
        "--build-compiler",
        action="store_true",
        help="run 'cargo build' first if --compiler is missing",
    )
    parser.add_argument(
        "--server-requests",
        type=int,
        help=(
            "request capacity written for the demo server; defaults to total + warmup, "
            "rounded up to the 4-worker server capacity"
        ),
    )
    parser.add_argument(
        "--server-config",
        type=Path,
        default=Path(DEFAULT_SERVER_CONFIG),
        help="request-capacity config file read by examples/demo/pokemon_server.mo",
    )
    parser.add_argument("--host", default="127.0.0.1", help="server host")
    parser.add_argument("--startup-timeout", type=float, default=10.0, help="seconds to wait for server port")
    parser.add_argument("--label", default="", help="free-form label stored in JSON/CSV output")
    return parser.parse_args()


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def parse_route_mix(value: str) -> list[Route]:
    routes: list[Route] = []
    for raw_part in value.split(","):
        part = raw_part.strip()
        if not part:
            continue
        if ":" in part:
            route_part, weight_part = part.rsplit(":", 1)
            try:
                weight = int(weight_part)
            except ValueError as exc:
                raise SystemExit(f"invalid route weight in {part!r}") from exc
        else:
            route_part = part
            weight = 1
        bits = route_part.strip().split()
        if len(bits) != 2:
            raise SystemExit(f"invalid route entry {part!r}; expected 'METHOD /path[:weight]'")
        method, path = bits[0].upper(), bits[1]
        if method not in {"GET", "POST"}:
            raise SystemExit(f"unsupported method {method!r}; supported: GET, POST")
        if not path.startswith("/"):
            raise SystemExit(f"route path must start with '/': {path!r}")
        if weight <= 0:
            raise SystemExit(f"route weight must be > 0 in {part!r}")
        routes.append(Route(method=method, path=path, weight=weight))
    if not routes:
        raise SystemExit("--route-mix must contain at least one route")
    return routes


def expand_routes(routes: list[Route], total: int) -> list[Route]:
    weighted: list[Route] = []
    for route in routes:
        weighted.extend([route] * route.weight)
    return [weighted[i % len(weighted)] for i in range(total)]


def rounded_server_capacity(requests: int, workers: int = 4) -> int:
    if requests <= 0:
        return workers
    return ((requests + workers - 1) // workers) * workers


def write_server_config(args: argparse.Namespace, root: Path, capacity: int) -> Path:
    path = root / args.server_config
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(str(capacity))
    return path


def remove_server_config(path: Path) -> None:
    try:
        path.unlink()
    except FileNotFoundError:
        pass


def run_checked(command: list[str], cwd: Path) -> None:
    proc = subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if proc.returncode != 0:
        raise SystemExit(f"command failed ({proc.returncode}): {' '.join(command)}\n{proc.stdout}")


def build_server(args: argparse.Namespace, root: Path) -> None:
    compiler = root / args.compiler
    if args.build_compiler and not compiler.exists():
        run_checked(["cargo", "build"], cwd=root)
    if not compiler.exists():
        raise SystemExit(
            f"compiler not found: {compiler}. Run 'cargo build' or pass --build-compiler/--compiler."
        )
    output = root / args.server_binary
    output.parent.mkdir(parents=True, exist_ok=True)
    run_checked([str(compiler), "build", str(args.source), "-o", str(args.server_binary)], cwd=root)


def enqueue_output(pipe, out_queue: "queue.Queue[str]") -> None:
    try:
        for line in iter(pipe.readline, ""):
            out_queue.put(line)
    finally:
        pipe.close()


def start_server(args: argparse.Namespace, root: Path) -> tuple[subprocess.Popen[str], int, list[str]]:
    binary = root / args.server_binary
    if not binary.exists():
        raise SystemExit(f"server binary not found: {binary}; remove --no-build or pass --server-binary")
    proc = subprocess.Popen(
        [str(binary)],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=1,
    )
    assert proc.stdout is not None
    lines: list[str] = []
    out_queue: "queue.Queue[str]" = queue.Queue()
    thread = threading.Thread(target=enqueue_output, args=(proc.stdout, out_queue), daemon=True)
    thread.start()

    deadline = time.monotonic() + args.startup_timeout
    port: int | None = None
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            drain_queue(out_queue, lines)
            raise SystemExit(f"server exited before startup with code {proc.returncode}:\n{''.join(lines)}")
        try:
            line = out_queue.get(timeout=0.05)
        except queue.Empty:
            continue
        lines.append(line)
        match = PORT_RE.search(line)
        if match:
            port = int(match.group(1))
        if port is not None and line.startswith("Async executor probe:"):
            return proc, port, lines
    stop_process(proc)
    drain_queue(out_queue, lines)
    raise SystemExit(f"timed out waiting for server readiness. Output so far:\n{''.join(lines)}")


def drain_queue(out_queue: "queue.Queue[str]", lines: list[str]) -> None:
    while True:
        try:
            lines.append(out_queue.get_nowait())
        except queue.Empty:
            return


def stop_process(proc: subprocess.Popen[str]) -> None:
    if proc.poll() is not None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=2.0)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2.0)


def read_rss_kib(pid: int) -> int | None:
    proc = subprocess.run(
        ["ps", "-o", "rss=", "-p", str(pid)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if proc.returncode != 0:
        return None
    value = proc.stdout.strip()
    if not value:
        return None
    try:
        rss = int(value.splitlines()[-1].strip())
    except ValueError:
        return None
    if rss <= 0:
        return None
    return rss


def sample_rss(pid: int, interval: float, stop: threading.Event, out: list[RssSample], t0: float) -> None:
    while not stop.is_set():
        rss = read_rss_kib(pid)
        if rss is not None:
            out.append(RssSample(t=time.perf_counter() - t0, rss_kib=rss))
        stop.wait(interval)
    rss = read_rss_kib(pid)
    if rss is not None:
        out.append(RssSample(t=time.perf_counter() - t0, rss_kib=rss))


def make_request(host: str, port: int, route: Route, timeout: float) -> RequestResult:
    request = (
        f"{route.method} {route.path} HTTP/1.1\r\n"
        f"Host: {host}\r\n"
        "Connection: close\r\n"
        "Content-Length: 0\r\n"
        "\r\n"
    ).encode("ascii")
    start = time.perf_counter()
    status: int | None = None
    response = bytearray()
    try:
        with socket.create_connection((host, port), timeout=timeout) as sock:
            sock.settimeout(timeout)
            sock.sendall(request)
            while True:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                response.extend(chunk)
                parsed = parse_response_progress(response)
                if parsed is not None:
                    status, done = parsed
                    if done:
                        break
        if status is None:
            status = parse_status(response)
        latency_ms = (time.perf_counter() - start) * 1000.0
        return RequestResult(
            route=route.key,
            ok=status is not None and 200 <= status < 300,
            status=status,
            latency_ms=latency_ms,
            bytes_read=len(response),
            error=None,
        )
    except Exception as exc:  # noqa: BLE001 - benchmark should record all client-side failures.
        if status is None:
            status = parse_status(response)
        latency_ms = (time.perf_counter() - start) * 1000.0
        return RequestResult(
            route=route.key,
            ok=False,
            status=status,
            latency_ms=latency_ms,
            bytes_read=len(response),
            error=type(exc).__name__ + ": " + str(exc),
        )


def parse_status(response: bytes | bytearray) -> int | None:
    if not response:
        return None
    first_line = bytes(response).split(b"\r\n", 1)[0].decode("ascii", errors="replace")
    parts = first_line.split()
    if len(parts) >= 2 and parts[1].isdigit():
        return int(parts[1])
    return None


def parse_response_progress(response: bytes | bytearray) -> tuple[int | None, bool] | None:
    header_end = bytes(response).find(b"\r\n\r\n")
    if header_end < 0:
        return None
    header = bytes(response[:header_end]).decode("ascii", errors="replace")
    status = parse_status(response)
    content_length: int | None = None
    for line in header.split("\r\n")[1:]:
        name, separator, value = line.partition(":")
        if separator and name.lower() == "content-length":
            try:
                content_length = int(value.strip())
            except ValueError:
                content_length = None
            break
    if content_length is None:
        return status, False
    body_read = len(response) - (header_end + 4)
    return status, body_read >= content_length


def run_requests(
    host: str,
    port: int,
    routes: Iterable[Route],
    concurrency: int,
    timeout: float,
) -> list[RequestResult]:
    route_list = list(routes)
    if not route_list:
        return []
    results: list[RequestResult] = []
    with ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [executor.submit(make_request, host, port, route, timeout) for route in route_list]
        for future in as_completed(futures):
            results.append(future.result())
    return results


def percentile(sorted_values: list[float], pct: float) -> float | None:
    if not sorted_values:
        return None
    if len(sorted_values) == 1:
        return sorted_values[0]
    rank = (len(sorted_values) - 1) * pct / 100.0
    lower = math.floor(rank)
    upper = math.ceil(rank)
    if lower == upper:
        return sorted_values[lower]
    weight = rank - lower
    return sorted_values[lower] * (1.0 - weight) + sorted_values[upper] * weight


def summarize_results(results: list[RequestResult], duration_s: float, rss_samples: list[RssSample]) -> dict:
    latencies = sorted(result.latency_ms for result in results)
    ok_count = sum(1 for result in results if result.ok)
    errors = [result.error for result in results if result.error]
    statuses: dict[str, int] = {}
    routes: dict[str, dict[str, int]] = {}
    for result in results:
        status_key = str(result.status) if result.status is not None else "none"
        statuses[status_key] = statuses.get(status_key, 0) + 1
        route_summary = routes.setdefault(result.route, {"count": 0, "ok": 0})
        route_summary["count"] += 1
        if result.ok:
            route_summary["ok"] += 1
    rss_values = [sample.rss_kib for sample in rss_samples]
    return {
        "requests": len(results),
        "ok": ok_count,
        "failed": len(results) - ok_count,
        "status_counts": statuses,
        "route_counts": routes,
        "duration_seconds": duration_s,
        "throughput_rps": (len(results) / duration_s) if duration_s > 0 else None,
        "bytes_read": sum(result.bytes_read for result in results),
        "latency_ms": {
            "min": min(latencies) if latencies else None,
            "mean": statistics.fmean(latencies) if latencies else None,
            "p50": percentile(latencies, 50),
            "p90": percentile(latencies, 90),
            "p95": percentile(latencies, 95),
            "p99": percentile(latencies, 99),
            "max": max(latencies) if latencies else None,
        },
        "rss_kib": {
            "samples": len(rss_values),
            "min": min(rss_values) if rss_values else None,
            "max": max(rss_values) if rss_values else None,
            "start": rss_values[0] if rss_values else None,
            "end": rss_values[-1] if rss_values else None,
            "delta": (rss_values[-1] - rss_values[0]) if len(rss_values) >= 2 else None,
        },
        "client_errors": summarize_errors(errors),
    }


def summarize_errors(errors: list[str | None]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for error in errors:
        if error is None:
            continue
        counts[error] = counts.get(error, 0) + 1
    return counts


def write_json(path: Path | None, payload: dict) -> None:
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if path is None:
        print(text, end="")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)


def write_summary_csv(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    summary = payload["summary"]
    latency = summary["latency_ms"]
    rss = summary["rss_kib"]
    row = {
        "timestamp_unix": payload["timestamp_unix"],
        "label": payload["label"],
        "total": payload["config"]["total"],
        "concurrency": payload["config"]["concurrency"],
        "route_mix": payload["config"]["route_mix"],
        "requests": summary["requests"],
        "ok": summary["ok"],
        "failed": summary["failed"],
        "duration_seconds": summary["duration_seconds"],
        "throughput_rps": summary["throughput_rps"],
        "latency_mean_ms": latency["mean"],
        "latency_p50_ms": latency["p50"],
        "latency_p90_ms": latency["p90"],
        "latency_p95_ms": latency["p95"],
        "latency_p99_ms": latency["p99"],
        "rss_start_kib": rss["start"],
        "rss_end_kib": rss["end"],
        "rss_max_kib": rss["max"],
        "rss_delta_kib": rss["delta"],
    }
    exists = path.exists() and path.stat().st_size > 0
    with path.open("a", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(row.keys()))
        if not exists:
            writer.writeheader()
        writer.writerow(row)


def write_samples_csv(path: Path, samples: list[RssSample]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["elapsed_seconds", "rss_kib"])
        writer.writeheader()
        for sample in samples:
            writer.writerow({"elapsed_seconds": sample.t, "rss_kib": sample.rss_kib})


def main() -> int:
    args = parse_args()
    if args.total <= 0:
        raise SystemExit("--total must be > 0")
    if args.concurrency <= 0:
        raise SystemExit("--concurrency must be > 0")
    if args.rss_interval <= 0:
        raise SystemExit("--rss-interval must be > 0")
    measured_plus_warmup = args.total + args.warmup
    if args.warmup < 0:
        raise SystemExit("--warmup must be >= 0")
    if args.server_requests is not None and args.server_requests < measured_plus_warmup:
        raise SystemExit("--server-requests must be >= total + warmup")

    root = repo_root()
    routes = parse_route_mix(args.route_mix)
    if args.build:
        build_server(args, root)

    capacity = rounded_server_capacity(args.server_requests or measured_plus_warmup)
    config_path = write_server_config(args, root, capacity)

    server: subprocess.Popen[str] | None = None
    stop_sampling = threading.Event()
    rss_samples: list[RssSample] = []
    try:
        server, port, server_output = start_server(args, root)
        remove_server_config(config_path)
        if args.warmup:
            warmup_routes = expand_routes(routes, args.warmup)
            warmup_results = run_requests(args.host, port, warmup_routes, args.concurrency, args.timeout)
            warmup_failures = sum(1 for result in warmup_results if not result.ok)
            if warmup_failures:
                raise SystemExit(f"warmup failed: {warmup_failures}/{len(warmup_results)} requests failed")

        request_routes = expand_routes(routes, args.total)
        t0 = time.perf_counter()
        sampler = threading.Thread(
            target=sample_rss,
            args=(server.pid, args.rss_interval, stop_sampling, rss_samples, t0),
            daemon=True,
        )
        sampler.start()
        results = run_requests(args.host, port, request_routes, args.concurrency, args.timeout)
        duration_s = time.perf_counter() - t0
        stop_sampling.set()
        sampler.join(timeout=max(1.0, args.rss_interval * 4.0))

        try:
            server_exit = server.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            server_exit = None

        payload = {
            "timestamp_unix": time.time(),
            "label": args.label,
            "config": {
                "total": args.total,
                "concurrency": args.concurrency,
                "route_mix": args.route_mix,
                "rss_interval": args.rss_interval,
                "timeout": args.timeout,
                "warmup": args.warmup,
                "source": str(args.source),
                "server_binary": str(args.server_binary),
                "server_requests": capacity,
                "server_config": str(args.server_config),
            },
            "server": {
                "pid": server.pid,
                "host": args.host,
                "port": port,
                "exit_code_after_run": server_exit,
                "startup_output": server_output,
            },
            "summary": summarize_results(results, duration_s, rss_samples),
            "rss_samples": [
                {"elapsed_seconds": sample.t, "rss_kib": sample.rss_kib} for sample in rss_samples
            ],
        }
        write_json(args.json_output, payload)
        if args.csv_output:
            write_summary_csv(args.csv_output, payload)
        if args.samples_csv_output:
            write_samples_csv(args.samples_csv_output, rss_samples)
        return 0 if payload["summary"]["failed"] == 0 else 1
    finally:
        stop_sampling.set()
        remove_server_config(config_path)
        if server is not None:
            stop_process(server)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
