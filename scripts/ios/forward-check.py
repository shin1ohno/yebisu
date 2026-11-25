#!/usr/bin/env python3
"""Issue 140 on the Simulator: a port forward from the app to a private sshd on this Mac.

  just ios-forward               # python3 scripts/guarded.py python3 scripts/ios/forward-check.py
  forward-check.py [--port 8000] [--keep]

1. A private sshd on 127.0.0.1 with a fresh ed25519 host key, configured as the fork's
   crates/test-support/sshd.rs configures its own (public keys only, ecdsa-sha2-nistp256, no PAM),
   with local forwarding allowed; and `python3 -m http.server` on 127.0.0.1:PORT serving a page
   whose text is a fresh marker.
2. The app, bundled and installed on the Simulator $HERDR_IOS_SIMULATOR names, gets that host as
   its one saved, selected host, and the host's key pinned in its own known_hosts.
3. XCUITest SmokeQA/testForwardKeyLine prints the app's public key line; it goes, without its
   `restrict` (which refuses forwarding), into the sshd's authorized_keys.
4. XCUITest SmokeQA/testForwardPort forwards PORT, reads the page in Safari, and checks the pause
   and the resume (scripts/ios/smoke-qa/Tests/SmokeQA+Forward.swift).
5. The app is terminated before any of its files is read; the run's logs stay in
   target/ios-forward/RUN.

The Simulator shares this Mac's loopback [推測, issue 140 Open questions; this check is the
probe]: the app cannot listen on 127.0.0.1:PORT while http.server holds it, so the forward moves to
a port the system chooses, which exercises that path too.
"""
import argparse
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import sys
import time

BUNDLE = "com.shin1ohno.yebisu"
KEY_LINE = re.compile(r"^FORWARD-KEY-LINE (.+)$", re.M)
# What the app shows: `restrict ecdsa-sha2-nistp256 AAAA… herdr-ipad-<device>`.
APP_KEY = re.compile(r"^(?:restrict )?(ecdsa-sha2-nistp256 [A-Za-z0-9+/]+=* [A-Za-z0-9._-]+)$")
# Where the app keeps its state (sheprd_client::local_path::state_dir on iPadOS).
STATE = "Library/Application Support/herdr/gpui"


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--port", type=int, default=8000, help="the host port to forward (1 to 65535)")
    parser.add_argument("--keep", action="store_true", help="leave sshd and http.server running")
    args = parser.parse_args(argv)
    if not 1 <= args.port <= 65535:
        parser.error("--port must be 1 to 65535")
    return args


def sshd_config(port, host_key, pid, authorized):
    """The private sshd's configuration: loopback only, public keys only, local forwarding."""
    quoted = lambda path: '"%s"' % str(path).replace("\\", "\\\\").replace('"', '\\"')
    return "\n".join([
        "Port %d" % port,
        "ListenAddress 127.0.0.1",
        "HostKey %s" % quoted(host_key),
        "PidFile %s" % quoted(pid),
        "AuthorizedKeysFile %s" % quoted(authorized),
        "PasswordAuthentication no",
        "KbdInteractiveAuthentication no",
        "UsePAM no",
        "AuthenticationMethods publickey",
        "PubkeyAcceptedAlgorithms ecdsa-sha2-nistp256",
        "PermitRootLogin no",
        "StrictModes no",
        "UseDNS no",
        "LogLevel VERBOSE",
        "AllowTcpForwarding local",
        "AllowAgentForwarding no",
        "X11Forwarding no",
        "PermitTunnel no",
        "",
    ])


def authorized_line(app_line):
    """The app's key line as authorized_keys takes it for forwarding: without `restrict`."""
    match = APP_KEY.match(app_line.strip())
    if not match:
        raise ValueError("not the app's public key line")
    return match.group(1)


def key_line_from_log(text):
    found = KEY_LINE.findall(text)
    if not found:
        raise RuntimeError("testForwardKeyLine printed no key line")
    return found[-1].strip()


def known_hosts_line(port, host_public):
    """The pin for 127.0.0.1:PORT as the app's known_hosts writes it (`[host]:port` off port 22)."""
    kind, key = host_public.split()[:2]
    return "[127.0.0.1]:%d %s %s\n" % (port, kind, key)


def hosts_json(target, name):
    return json.dumps({"version": 1, "targets": [target], "selected": target, "names": {target: name}})


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def run(*command, **kwargs):
    print("FORWARD COMMAND", list(command), flush=True)
    return subprocess.run(command, check=True, text=True, **kwargs)


def wait_listening(port, process, what, timeout=10):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if process.poll() is not None:
            raise RuntimeError("%s exited (%s)" % (what, process.returncode))
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.1)
    raise RuntimeError("%s did not listen on 127.0.0.1:%d" % (what, port))


def terminate(device):
    result = subprocess.run(["xcrun", "simctl", "terminate", device, BUNDLE], text=True, capture_output=True)
    print("FORWARD terminate before reading the app's files: exit", result.returncode, flush=True)
    if result.returncode not in (0, 3):
        raise RuntimeError("could not terminate the app (simctl exit %d)" % result.returncode)


def xcodebuild_test(spec, device, root, out, name, environment):
    log = out / ("%s.log" % name)
    command = ["xcodebuild", "test", "-project", str(spec / "SmokeOracle.xcodeproj"), "-scheme", "SmokeOracle",
               "-destination", "id=" + device, "-derivedDataPath", str(root / "target/ios-forward/build"),
               "-resultBundlePath", str(out / ("%s.xcresult" % name)), "CODE_SIGNING_ALLOWED=NO",
               "-only-testing:SmokeOracle/SmokeQA/%s" % name]
    print("FORWARD COMMAND", command, flush=True)
    with log.open("w") as stream:
        result = subprocess.run(command, text=True, stdout=stream, stderr=subprocess.STDOUT, env=environment)
    text = log.read_text(errors="replace")
    print("FORWARD %s: exit %d (%s)" % (name, result.returncode, log), flush=True)
    if result.returncode != 0:
        tail = "\n".join(text.splitlines()[-40:])
        raise RuntimeError("%s failed:\n%s" % (name, tail))
    return text


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    out = root / "target/ios-forward" / (time.strftime("%Y%m%d-%H%M%S") + "-" + secrets.token_hex(3))
    host_dir = out / "host"
    host_dir.mkdir(parents=True)
    marker = "yebisu-forward-qa-" + secrets.token_hex(6)
    (host_dir / "www").mkdir()
    (host_dir / "www/index.html").write_text("<!doctype html><title>%s</title><p>%s</p>\n" % (marker, marker))
    run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(host_dir / "host_key"))
    sshd_port = free_port()
    authorized = host_dir / "authorized_keys"
    authorized.write_text("")
    (host_dir / "sshd_config").write_text(
        sshd_config(sshd_port, host_dir / "host_key", host_dir / "sshd.pid", authorized))
    children = []
    try:
        sshd = subprocess.Popen(["/usr/sbin/sshd", "-D", "-e", "-f", str(host_dir / "sshd_config")],
                                stdout=(host_dir / "sshd.log").open("w"), stderr=subprocess.STDOUT)
        children.append(sshd)
        wait_listening(sshd_port, sshd, "the private sshd")
        server = subprocess.Popen([sys.executable, "-I", "-m", "http.server", str(args.port), "--bind", "127.0.0.1",
                                   "--directory", str(host_dir / "www")],
                                  stdout=(host_dir / "http.log").open("w"), stderr=subprocess.STDOUT)
        children.append(server)
        wait_listening(args.port, server, "http.server")
        user = os.environ.get("USER") or os.getlogin()
        target = "%s@127.0.0.1:%d" % (user, sshd_port)
        app = run("bash", "scripts/ios/bundle.sh", "--package", "yebisu", "--bin", "yebisu",
                  capture_output=True).stdout.strip().splitlines()[-1]
        run("bash", "scripts/ios/sim.sh", "--wait", "3", "--terminate", "--out", str(out / "install"), app)
        device = subprocess.check_output(
            ["bash", "-c", 'source scripts/ios/lib.sh; ios_sim_resolve "$HERDR_IOS_SIMULATOR"'], text=True).split()[0]
        container = Path(subprocess.check_output(
            ["xcrun", "simctl", "get_app_container", device, BUNDLE, "data"], text=True).strip())
        state = container / STATE
        (state / "ssh").mkdir(parents=True, exist_ok=True)
        (state / "yebisu-hosts.json").write_text(hosts_json(target, "forward-qa"))
        (state / "ssh/known_hosts").write_text(
            known_hosts_line(sshd_port, (host_dir / "host_key.pub").read_text()))
        spec = root / "scripts/ios/smoke-qa"
        run("xcodegen", "generate", "--spec", str(spec / "project.yml"), "--project", str(spec))
        environment = dict(os.environ, TEST_RUNNER_FORWARD_QA_TARGET=target,
                           TEST_RUNNER_FORWARD_QA_MARKER=marker, TEST_RUNNER_FORWARD_QA_PORT=str(args.port))
        try:
            keys = xcodebuild_test(spec, device, root, out, "testForwardKeyLine", environment)
            authorized.write_text(authorized_line(key_line_from_log(keys)) + "\n")
            xcodebuild_test(spec, device, root, out, "testForwardPort", environment)
        finally:
            terminate(device)
        summary = {"target_port": sshd_port, "port": args.port, "marker": marker, "device": device,
                   "result": "pass", "out": str(out)}
        (out / "summary.json").write_text(json.dumps(summary, indent=2))
        print("FORWARD PASS: the app forwarded 127.0.0.1:%d through the private sshd; Safari read the page; "
              "the port closed off screen and came back on the same local port." % args.port, flush=True)
    finally:
        if not args.keep:
            for child in children:
                child.terminate()
            for child in children:
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
