"""Read terminal.list on existing POSIX herdr-core sockets."""
import json
import os
import socket
import sys

env = os.environ
home = env["HOME"]
state = os.path.join(env.get("XDG_STATE_HOME") or os.path.join(home, ".local/state"), "herdr-core")
config = os.path.join(env.get("XDG_CONFIG_HOME") or os.path.join(home, ".config"), "herdr-core/config.toml")
runtime = env.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
runtime = os.path.join(runtime, "herdr-core") if os.path.isdir(runtime) else None
selected = (env.get("HC_SESSION") or "default").strip() or "default"
override = env.get("HC_SOCKET")
if not override and os.path.isfile(config):
    try:
        import tomllib

        with open(config, "rb") as stream:
            override = tomllib.load(stream).get("socket")
    except (ImportError, ValueError, OSError):
        pass

def rpc(connection, replies, method, params):
    request = dict(jsonrpc="2.0", id=1, method=method, params=params)
    connection.sendall(json.dumps(request).encode("ascii") + b"\n")
    response = json.loads(replies.readline(262144))
    if response["id"] != request["id"]:
        raise ValueError("mismatched reply")
    return response["result"]


def list_terminals(endpoint):
    try:
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(0.6)
            connection.connect(endpoint)
            with connection.makefile("rb") as replies:
                core_id = rpc(connection, replies, "hello", {})["core"]["id"]
                terminals = rpc(connection, replies, "terminal.list", {"all": False})["terminals"]
        rows = []
        for terminal in terminals:
            row = {k: terminal[k] for k in ("address", "state", "name", "title", "status") if k in terminal}
            row["argv"] = terminal.get("argv", [])[:1]
            rows.append(row)
        return [core_id, rows]
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        return None


def sessions():
    names = set()
    if os.path.isdir(state):
        with os.scandir(state) as entries:
            names.update(entry.name for entry in entries if entry.is_dir(follow_symlinks=False))
    if runtime and os.path.isdir(runtime):
        with os.scandir(runtime) as entries:
            names.update(
                entry.name[:-5]
                for entry in entries
                if entry.name.endswith(".sock") and not entry.name.endswith(".holder.sock")
                and ".holder." not in entry.name
            )
    if override:
        names.add(selected)
    for name in sorted(names):
        if name == selected and override:
            endpoint = override
        elif runtime:
            endpoint = os.path.join(runtime, name + ".sock")
        else:
            endpoint = os.path.join(state, name, name + ".sock")
        yield name, list_terminals(endpoint)


remaining = 48000
for name, rows in sessions():
    line = json.dumps([name, rows], ensure_ascii=True)
    if len(line) + 1 > remaining:
        line = json.dumps([name, None], ensure_ascii=True)
    if len(line) + 1 > remaining:
        break
    sys.stdout.write(line + "\n")
    remaining -= len(line) + 1
