# Application diagnostics

`crates/yebisu/src/log-location.txt` defines the container-relative current
log path once. Rust uses `include_str!`, and `scripts/ios/device.sh` reads that
same file. The copied host artifact remains `target/ios-device/yebisu/yebisu.jsonl`.

The fork's `sheprd-diagnostics::init(targets)` cannot accept a caller-owned
path or name: `log_file()` uses the old app's state directory and fixed
`herdr-gpui.jsonl` filename. Its panic reports also include raw panic messages.
Yebisu therefore uses its own small tracing subscriber and writer, without
patching the fork or adding dependencies. The fork's bounded queue/rotation
policy is recorded as a model in PROVENANCE.tsv; no new source file was copied.

The file schema has exactly `time` (Unix seconds), `pid`, and `event` (a
program-owned, allowlisted static code). It records startup, window result,
lifecycle, connection transitions and error categories. It does not serialize
tracing messages, span fields, host messages, addresses, terminal output,
keys, IME, clipboard data, credentials or error bodies. A panic hook enqueues
only `panic`; it never formats the panic payload into the file. The original
stderr panic hook is preserved. Panic capture is best effort if the process
aborts before the writer runs.

Only the exact diagnostics tracing target is accepted, and even that target
cannot add fields or unknown codes to the file. Each caller uses a static
code. A 256-item synchronous channel uses `try_send`, dropping excess events
instead of waiting. One worker owns all directory, file and rotation I/O;
startup and UI callbacks never open, flush or wait on a file. Writer I/O errors
are reported as an OS error category on stderr, without payloads or paths.

The current file and one rotated file each hold at most 128 KiB (256 KiB total).
Rotation happens before a write would exceed the cap. Oversized files found
at restart are discarded. Existing yebisu records are retained across normal
launches, so use PID and timestamp to distinguish this launch. This is bounded
local diagnostics, not a durable audit log: queue overflow, I/O failure,
suspension or process termination can lose records.

Device collection makes up to six attempts, each with a 5-second devicectl
command timeout and 1-second retry intervals. Missing-file CoreDevice 7000 is
summarized plainly without dumping its raw error. A later-created file is
copied; other transfer errors remain visible. Copy failures remove partial
host artifacts and preserve the independently observed app process state.
