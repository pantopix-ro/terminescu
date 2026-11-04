# Spike 0.4: throughput

Run on 2026-11-04 with the spike app (Tauri 2.12, WKWebView, xterm.js 6.0, addon-webgl 0.19).
Path: `cat` in a pane → spike daemon → Unix socket → Rust forwarder → `tauri::ipc::Channel` as
raw bytes, batched every 16 ms → `term.write` → ack per write callback, 1 MB unacknowledged limit
(D33). The input was a 50 MB generated log with ANSI colors (434,604 lines).

The machine was under heavy load during every run: load average 165-200, a VM at about 390%
CPU and a CI runner job. Throughput varied by 10x between identical runs, so the table gives
the range. Frame times held steady across all runs.

## Results

| Run | MB/s (runs) | Frame p50 / p95 / max | WebGL contexts |
|---|---|---|---|
| Daemon only, Node client, no app | 46, 97, 87 | n/a | n/a |
| 1 pane, WebGL, 50 MB | 38.8, 3.9, 38, 9.5, 18.1 | 14-17 / 19-23 / 72-99 ms | 1 live |
| 1 pane, DOM renderer, 50 MB | 15.7, 9.8, 48.3, 52.1 | 14-17 / 21-34 / 71-112 ms | 0 |
| 8 panes over 2 windows, WebGL, 10 MB each at once | 2.3-2.4 each, about 19 combined | 17 / 26 / 96-111 ms per window | 8 live, 0 lost |
| 17 idle panes, 1 window, WebGL | n/a | n/a | 17th load drops the oldest: 16 live, 1 lost |
| 10 + 10 idle panes over 2 windows, WebGL | n/a | n/a | 20 live, 0 lost |
| `precmd` stand-in for `terminescu var env` (spawn, connect, handshake, one request) | p50 4.0 ms, p95 7.7 ms; 9.7 ms through zsh `eval "$(…)"` | n/a | n/a |

## Findings

- Raw bytes over `ipc::Channel` work: `InvokeResponseBody::Raw` arrives as an `ArrayBuffer`.
  The best app runs (38-52 MB/s) reach about half the daemon-only rate, and the slow runs track
  machine load, not the renderer. The pipeline is not the bottleneck at these rates.
- Backpressure engaged briefly: 81 ms paused over 43 MB in one run.
- The UI stays responsive while a pane floods: p50 frame time stays at 14-17 ms and p95 under
  35 ms. The worst frames (up to 112 ms) came with batches above 1 MB. D33 now caps a batch at
  256 KB so one `write` stays short.
- WebKit's limit of 16 WebGL contexts applies per window, since each window has its own web
  content process. D33's rule (WebGL for visible panes, DOM past 12 in a window, DOM on
  `onContextLoss`) holds; `onContextLoss` fires as expected.
- WebGL and DOM renderers performed alike in this setup; xterm.js parses a large batch once and
  renders once per frame either way. WebGL stays the default for visible panes because it costs
  less CPU while scrolling, which this spike did not measure.
- `precmd` cost is acceptable without caching; the generation check stays as an optimisation.
