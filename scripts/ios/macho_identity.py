"""Narrow signature-aware identity for linker-signed ARM64 Simulator executables.

Only a terminal embedded signature, its LC datasize and __LINKEDIT filesize
are excluded. All other bytes (including UUID, VM layout and commands) are bound.
Unsupported layouts are rejected, never approximated by UUID or __text hashes.
"""
import hashlib
import struct


def code_identity(blob):
    if len(blob) < 32:
        raise ValueError('truncated Mach-O')
    magic, cpu, subtype, kind, count, size, flags, reserved = struct.unpack_from('<8I', blob)
    if (magic, cpu, kind) != (0xfeedfacf, 0x100000c, 2):
        raise ValueError('requires thin little-endian ARM64 MH_EXECUTE')
    end = 32 + size
    if end > len(blob) or count > size // 8:
        raise ValueError('invalid load command bounds')
    offset, signatures, linkedit, segments, sections, uuid = 32, [], [], [], [], []
    for _ in range(count):
        if offset + 8 > end:
            raise ValueError('truncated command')
        cmd, length = struct.unpack_from('<II', blob, offset)
        if length < 8 or length % 8 or offset + length > end:
            raise ValueError('invalid command size')
        if cmd == 0x1d:
            if length != 16:
                raise ValueError('invalid signature command')
            signatures.append((offset, *struct.unpack_from('<II', blob, offset + 8)))
        if cmd == 0x1b:
            if length != 24:
                raise ValueError('invalid UUID command')
            uuid.append(blob[offset + 8:offset + 24].hex())
        if cmd == 0x19:
            if length < 72:
                raise ValueError('truncated segment')
            name = blob[offset + 8:offset + 24].rstrip(b'\0')
            vmaddr, vmsize, fileoff, filesize = struct.unpack_from('<4Q', blob, offset + 24)
            nsects = struct.unpack_from('<I', blob, offset + 64)[0]
            if length != 72 + nsects * 80 or fileoff + filesize > len(blob):
                raise ValueError('invalid segment bounds')
            segments.append((name, fileoff, fileoff + filesize))
            for i in range(nsects):
                start = offset + 72 + i * 80
                sectsize = struct.unpack_from('<Q', blob, start + 40)[0]
                sectoff = struct.unpack_from('<I', blob, start + 48)[0]
                secttype = struct.unpack_from('<I', blob, start + 64)[0] & 0xff
                if secttype not in (1, 0xc, 0x12) and sectsize:
                    sections.append((sectoff, sectoff + sectsize))
            if name == b'__LINKEDIT':
                if nsects or filesize > vmsize:
                    raise ValueError('unsupported linkedit layout')
                linkedit.append((offset, fileoff, filesize))
        offset += length
    if offset != end or len(signatures) != 1 or len(linkedit) != 1 or len(uuid) != 1:
        raise ValueError('requires unique signature/linkedit/UUID and exact commands')
    command, start, length = signatures[0]
    segment, fileoff, filesize = linkedit[0]
    if start < end or start < fileoff or start % 16 or start + length != len(blob):
        raise ValueError('signature must be aligned terminal range')
    if fileoff + filesize != len(blob) or length < 12:
        raise ValueError('signature must end linkedit')
    if any(name != b'__LINKEDIT' and hi > start for name, lo, hi in segments):
        raise ValueError('signature overlaps another segment')
    if any(lo < end or hi > start for lo, hi in sections):
        raise ValueError('section overlaps header/signature')
    sigmagic, siglength, slots = struct.unpack_from('>III', blob, start)
    if sigmagic != 0xfade0cc0 or not 12 <= siglength <= length or slots > (siglength - 12) // 8:
        raise ValueError('invalid embedded signature SuperBlob')
    if any(blob[start + siglength:]):
        raise ValueError('nonzero signature trailing padding')
    ranges, kinds = [], set()
    for i in range(slots):
        slot, relative = struct.unpack_from('>II', blob, start + 12 + 8 * i)
        if slot in kinds or relative < 12 + 8 * slots or relative + 8 > siglength:
            raise ValueError('invalid signature slot')
        kinds.add(slot)
        submagic, sublength = struct.unpack_from('>II', blob, start + relative)
        if sublength < 8 or relative + sublength > siglength:
            raise ValueError('invalid signature blob bounds')
        if any(relative < hi and lo < relative + sublength for lo, hi in ranges):
            raise ValueError('overlapping signature blobs')
        ranges.append((relative, relative + sublength))
    if 0 not in kinds:
        raise ValueError('missing CodeDirectory slot')
    canonical = bytearray(blob[:start])
    struct.pack_into('<I', canonical, command + 12, 0)
    struct.pack_into('<Q', canonical, segment + 48, start - fileoff)
    return dict(schema='arm64-terminal-signature-v1',
                sha256=hashlib.sha256(canonical).hexdigest(), uuid=uuid[0],
                signature_offset=start, signature_size=length,
                non_signature_size=len(canonical))


def same_code(left, right):
    for field in ('schema', 'sha256', 'uuid', 'signature_offset', 'non_signature_size'):
        if left[field] != right[field]:
            raise ValueError('non-signature code identity mismatch: ' + field)
