"""Stop one POSIX herdr-core server if it is still the one that was seen and runs nothing.

Arguments: session, core id, pid ('-' for any), least uptime in ms ('-' for any).
Never starts a server: it talks to the session's socket directly (no `hc`, whose
`server status` starts a core on Unix). Exit status: 0 stopped, or no server
(printed `absent`); 3 another server than the one seen (id, pid, or younger than
the connection that saw it); 4 a terminal is not finished, left running; 5 the
server could not be asked (unknown); 6 no usable Python."""
import errno
import json
import os
import socket
import sys
import time

session, want_id, want_pid, least_uptime = sys.argv[1:5]
env = os.environ
home = env["HOME"]
state = os.path.join(env.get("XDG_STATE_HOME") or os.path.join(home, ".local/state"), "herdr-core")
config = os.path.join(env.get("XDG_CONFIG_HOME") or os.path.join(home, ".config"), "herdr-core/config.toml")
runtime = env.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
runtime = os.path.join(runtime, "herdr-core") if os.path.isdir(runtime) else None
endpoint = env.get("HC_SOCKET")
if not endpoint and os.path.isfile(config):
    try:
        import tomllib

        with open(config, "rb") as stream:
            endpoint = tomllib.load(stream).get("socket")
    except (ImportError, ValueError, OSError):
        pass
if not endpoint:
    endpoint = os.path.join(runtime, session + ".sock") if runtime else os.path.join(state, session, session + ".sock")

connection = socket.socket(socket.AF_UNIX)
connection.settimeout(2.0)
try:
    connection.connect(endpoint)
except (FileNotFoundError, ConnectionRefusedError):
    print("absent")
    sys.exit(0)
except OSError:
    print("unknown")
    sys.exit(5)
replies = connection.makefile("rb")
counter = [0]


def rpc(method):
    counter[0] += 1
    request = dict(jsonrpc="2.0", id=counter[0], method=method, params={})
    connection.sendall(json.dumps(request).encode("ascii") + b"\n")
    response = json.loads(replies.readline(262144))
    if response.get("id") != request["id"] or "result" not in response:
        raise ValueError("bad reply")
    return response["result"]


try:
    seen_id = rpc("hello")["core"]["id"]
    status = rpc("server.status")
    pid, live, uptime = status["pid"], status["live"], status["uptime_ms"]
except (OSError, ValueError, KeyError, TypeError):
    print("unknown")
    sys.exit(5)
if seen_id != want_id or (want_pid != "-" and str(pid) != want_pid):
    print("another server")
    sys.exit(3)
if least_uptime != "-" and uptime < int(least_uptime):
    print("restarted since it was seen")
    sys.exit(3)
if live != 0:
    print("a terminal is running")
    sys.exit(4)
try:
    rpc("server.stop")
except (OSError, ValueError, KeyError, TypeError):
    pass
deadline = time.time() + 5
connection.settimeout(0.5)
while time.time() < deadline:
    try:
        if connection.recv(4096) == b"":
            break
    except socket.timeout:
        continue
    except OSError:
        break
print("stopped")
