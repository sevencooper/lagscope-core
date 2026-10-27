# Test protocol and server API (version 1)

How a client (Pulse's website today; the LagScope app later) runs a test against LagScope
Pulse's measurement server. The server's code is private; this is its public contract.
`GET /v1/status` reports `"api": 1`; a client that sees a higher number than it knows should
tell the person to update.

Server: `https://us-east.lagscope.sevencooper.com` (US East). Browsers may call it only from
`https://lagscope.sevencooper.com` (CORS). JSON errors look like `{"error": "text for people"}`.

## Endpoints

| Method and path | What |
|---|---|
| `GET /v1/status` | `{"api": 1, "lag": "on" \| "busy", "turnstile": true}`: whether a test can start |
| `POST /v1/offer` | start a test over WebRTC: body = test parameters + `offer` (SDP); answer = SDP |
| `GET /v1/ws` | start a test over a WebSocket (fallback when UDP is blocked); see below |
| `POST /v1/pair`, `GET /v1/pair/{code}`, `POST`/`GET /v1/pair/{code}/answer` | pass connection descriptions between a PC and a phone (home network test); the server never sees that test's traffic |
| `POST /v1/event` | `{"name": "..."}`: a usage count from a fixed list of names; nothing else is stored |

Test parameters (JSON):

```json
{ "down": 64, "up": 64, "size": 120, "seconds": 180, "token": "<Turnstile token>",
  "preset": "generic-64", "load": false, "home": false }
```

`down` / `up`: packets per second from the server / from the client, 1-200. `size`: bytes per
packet, 21-1200. `seconds`: 10-600. `token`: a Cloudflare Turnstile token (single use, 5
minutes). `preset`, `load`, `home`: for the usage counts only.

## Limits (refusals)

| Answer | Why |
|---|---|
| 400 | parameters out of range, or an unreadable offer |
| 403 | the Turnstile token was missing, used or wrong |
| 429 | two tests from this address (IPv6: /64) already running, or 10 started in the last hour |
| 503 `busy` | 50 tests running, or 3000 started today |

Clients other than the website (the LagScope app) will get a ticket after a one-time browser
check, with their own, smaller quotas (to be added in API version 2).

## The stream

WebRTC: the client opens two data channels, `stream` (unordered, no retransmits) and `ctl`
(reliable). WebSocket: binary messages are `stream`, text messages are `ctl`; the first text
message from the client is the test parameters, the server answers `{"type": "ready"}` or an
error.

All numbers little-endian. Times: the client uses its own clock in milliseconds (f64); the
server uses microseconds since the session started (u64). No clock sync is needed.

| Type (byte 0) | From | Bytes 1-4 | Bytes 5-12 | Bytes 13-20 | Rest |
|---|---|---|---|---|---|
| 1 client packet | client | seq (u32) | planned send time (f64 ms) | actual send time (f64 ms) | padding to `size` |
| 2 server packet | server | seq (u32) | planned send time (u64 µs) | actual send time (u64 µs) | padding to `size` |
| 3 echo | server | the client packet's seq | the client packet's actual send time (f64 ms) | when the server got it (u64 µs) | |

`ctl`, client to server: `"stop"` (send the last report, end in a few seconds); a phone acting
as the far end in the home network test also understands `{"type": "start", "rate", "size"}`.
Server to client, once a second:

```json
{ "type": "report", "last": false, "arrivals": [[seq, arrival_us], ...] }
```

The last report after `"stop"` has `"last": true`. A session ends 30 s after its planned length
at the latest.

## The recording

What a client keeps and gives to `stream::analyze` (`src/stream.rs`, `Recording`): its own
sent packets, the server packets with their arrival times, the echoes with round trips, the
server's arrival reports, visibility changes, slow frames and load phases.
