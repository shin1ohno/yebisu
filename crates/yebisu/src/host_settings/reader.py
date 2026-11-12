import errno
import os
import stat
import sys


def run():
    # Fixed P15 FILE observation; no discovery, imports from hosts, or writes.
    if (os.open not in os.supports_dir_fd or
            any(not hasattr(os, key) for key in
                ('O_DIRECTORY', 'O_NOFOLLOW', 'O_NONBLOCK'))):
        return 43
    if len(sys.argv) != 2:
        return 41
    encoded = sys.argv[1]
    if (not encoded or len(encoded) > 8192 or len(encoded) % 2 or
            any(c not in '0123456789abcdef' for c in encoded)):
        return 41
    path = bytes.fromhex(encoded)
    if (len(path) > 4096 or not path.startswith(b'/') or
            any(c not in b'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/_-. '
                for c in path)):
        return 41
    parts = path.split(b'/')[1:]
    if any(part in (b'', b'.', b'..') for part in parts):
        return 41
    directory = None
    leaf = None
    try:
        flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
        directory = os.open(b'/', flags)
        for part in parts[:-1]:
            next_directory = os.open(part, flags, dir_fd=directory)
            os.close(directory)
            directory = next_directory
        leaf = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK,
                       dir_fd=directory)
        before = os.fstat(leaf)
        if not stat.S_ISREG(before.st_mode):
            return 41
        body = bytearray()
        while len(body) <= 65536:
            block = os.read(leaf, min(4096, 65537 - len(body)))
            if not block:
                break
            body.extend(block)
        if len(body) > 65536:
            return 45
        after = os.fstat(leaf)
        fields = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
        if any(getattr(before, field) != getattr(after, field) for field in fields):
            return 44
        sys.stdout.buffer.write(body)
        return 0
    except OSError as error:
        if error.errno == errno.ENOENT:
            return 40
        if error.errno in (errno.ELOOP, errno.ENOTDIR):
            return 41
        if error.errno in (errno.EACCES, errno.EPERM):
            return 42
        return 46
    finally:
        if leaf is not None:
            os.close(leaf)
        if directory is not None:
            os.close(directory)


try:
    status = run()
except Exception:
    status = 46
sys.exit(status)
