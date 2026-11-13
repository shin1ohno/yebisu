"""Restricted ARM64 structural identity; ordinary dynamic apps are Unsupported.

Bind every prefix byte except LC signature.datasize and LINKEDIT.filesize. This
does not validate cryptographic hashes, signature policy, or native compatibility.
Nonempty symbol/dyld/linkedit payloads are deliberately outside the admitted set.
"""
import hashlib
import struct

MAX_FILE_SIZE = 128 * 1024 * 1024
MAX_COMMAND_BYTES = 1024 * 1024
MAX_COMMANDS = 4096
MAX_SECTIONS = 16384
MAX_SIGNATURE_SIZE = 16 * 1024 * 1024
MAX_SLOTS = 64
U64_MAX = (1 << 64) - 1
EMPTY_LINKEDIT = (0x1e, 0x26, 0x29, 0x2b, 0x2e, 0x80000033, 0x80000034)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_file_size(size):
    require(size <= MAX_FILE_SIZE, 'Mach-O file size cap')


def extent(start, size, message):
    end = start + size
    require(end <= U64_MAX, message + ' overflow')
    return end


def disjoint(ranges, message):
    previous = 0
    for lo, hi in sorted((lo, hi) for lo, hi in ranges if lo != hi):
        require(lo >= previous, message)
        previous = hi


def discover(blob):
    """Only bounded framing/allocation discovery, before subset diagnostics."""
    check_file_size(len(blob))
    require(len(blob) >= 32, 'truncated Mach-O')
    header = struct.unpack_from('<8I', blob)
    magic, cpu, subtype, kind, count, size, flags, reserved = header
    require((magic, cpu, kind) == (0xfeedfacf, 0x100000c, 2),
            'requires thin little-endian ARM64 MH_EXECUTE')
    end = 32 + size
    require(end <= MAX_COMMAND_BYTES, 'load command byte cap')
    require(count <= MAX_COMMANDS, 'load command count cap')
    require(end <= len(blob) and count <= size // 8, 'invalid load command bounds')
    offset, commands, segments, signatures, section_count = 32, [], [], [], 0
    for _ in range(count):
        require(offset + 8 <= end, 'truncated command')
        cmd, length = struct.unpack_from('<II', blob, offset)
        require(length >= 8 and length % 8 == 0 and offset + length <= end,
                'invalid command size')
        commands.append((cmd, offset, length))
        if cmd == 0x1d:
            require(length == 16, 'invalid signature command')
            signatures.append((offset, *struct.unpack_from('<II', blob, offset + 8)))
        if cmd == 0x19:
            require(length >= 72, 'truncated segment')
            values = struct.unpack_from('<16s4Q4I', blob, offset + 8)
            name, vmaddr, vmsize, fileoff, filesize, maxprot, initprot, nsects, segflags = values
            section_count += nsects
            require(section_count <= MAX_SECTIONS, 'section count cap')
            require(length == 72 + nsects * 80, 'invalid segment command size')
            segments.append(dict(command=offset, name=name.rstrip(b'\0'), vmaddr=vmaddr,
                                 vmsize=vmsize, fileoff=fileoff, filesize=filesize,
                                 maxprot=maxprot, initprot=initprot, nsects=nsects,
                                 flags=segflags))
        offset += length
    require(offset == end, 'load commands not exact')
    linkedit = [s for s in segments if s['name'] == b'__LINKEDIT']
    require(len(signatures) == 1 and len(linkedit) == 1, 'requires unique signature/linkedit')
    signature, link = signatures[0], linkedit[0]
    command, start, length = signature
    require(length <= MAX_SIGNATURE_SIZE, 'signature allocation cap')
    require(start >= end and start >= link['fileoff'] and start % 16 == 0
            and start + length == len(blob), 'signature must be aligned terminal range')
    require(extent(link['fileoff'], link['filesize'], 'segment file') == len(blob)
            and length >= 12, 'signature must end linkedit')
    magic, declared, slots = struct.unpack_from('>III', blob, start)
    require(slots <= MAX_SLOTS, 'signature slot cap')
    require(magic == 0xfade0cc0 and 12 <= declared <= length
            and slots <= (declared - 12) // 8, 'invalid embedded signature SuperBlob')
    # Preserve the historical real-pair diagnostic BEFORE deferred subset gates.
    require(not any(memoryview(blob)[start + declared:]), 'nonzero signature trailing padding')
    return header, end, commands, segments, signature, link, declared, slots


def layout(blob, end, segments, start):
    names, vm_ranges, file_ranges = set(), [], []
    section_vm, section_file, executable = [], [], []
    text = None
    for segment in segments:
        name = segment['name']
        require(name in (b'__TEXT', b'__LINKEDIT', b'__PAGEZERO', b'__DATA', b'__DATA_CONST'),
                'Unsupported segment name')
        require(name not in names, 'duplicate segment')
        names.add(name)
        vm, size = segment['vmaddr'], segment['vmsize']
        fileoff, filesize = segment['fileoff'], segment['filesize']
        vmend = extent(vm, size, 'segment VM')
        fileend = extent(fileoff, filesize, 'segment file')
        require(segment['flags'] == 0, 'Unsupported segment flags')
        if name == b'__PAGEZERO':
            require(vm == 0 and size > 0 and size % 4096 == 0
                    and not any(segment[k] for k in ('fileoff', 'filesize', 'maxprot', 'initprot', 'nsects')),
                    'Unsupported PAGEZERO layout')
        else:
            require(size > 0 and filesize > 0 and filesize <= size, 'invalid segment sizes')
            require(vm % 4096 == size % 4096 == fileoff % 4096 == 0, 'segment alignment')
            require(fileend <= len(blob), 'segment outside file')
            protection = {b'__TEXT': 5, b'__LINKEDIT': 1, b'__DATA': 3, b'__DATA_CONST': 3}[name]
            require(segment['maxprot'] == segment['initprot'] == protection,
                    'Unsupported segment protection')
            if name == b'__TEXT':
                require(fileoff == 0, 'TEXT must start at file zero')
                text = segment
            if name == b'__LINKEDIT':
                require(segment['nsects'] == 0 and fileend == len(blob), 'Unsupported linkedit layout')
            else:
                require(fileend <= start, 'signature overlaps another segment')
        vm_ranges.append((vm, vmend))
        file_ranges.append((fileoff, fileend))
        for i in range(segment['nsects']):
            off = segment['command'] + 72 + i * 80
            values = struct.unpack_from('<16s16sQQ8I', blob, off)
            _, owner, addr, length, file, align, reloff, nreloc, flags, r1, r2, r3 = values
            kind, attrs = flags & 0xff, flags & ~0xff
            require(owner.rstrip(b'\0') == name, 'section parent mismatch')
            require(kind in (0, 1, 2, 3, 4, 0xe), 'Unsupported section type')
            require(attrs & ~0x80000400 == 0 and (not attrs or (name == b'__TEXT' and kind == 0)),
                    'Unsupported section attributes')
            require(r1 == r2 == r3 == 0, 'Unsupported section reserved fields')
            require(reloff == nreloc == 0, 'Unsupported section relocation payload')
            require(align <= 31, 'section alignment exponent')
            alignment = 1 << align
            addrend = extent(addr, length, 'section VM')
            require(vm <= addr <= addrend <= vmend, 'section VM outside parent')
            require(addr % alignment == 0, 'section VM alignment')
            section_vm.append((addr, addrend))
            if kind == 1:
                require(file == 0 and attrs == 0, 'Unsupported zerofill mapping')
                continue
            filehi = extent(file, length, 'section file')
            if length == 0 and file == 0:
                continue  # Explicit empty-file sentinel, VM endpoint was checked above.
            require(fileoff <= file <= filehi <= fileend, 'section file outside parent')
            require(file - fileoff == addr - vm, 'section VM/file mapping')
            require(file % alignment == 0, 'section file alignment')
            require(file >= end and filehi <= start, 'section overlaps header/signature')
            section_file.append((file, filehi))
            if length and name == b'__TEXT' and kind == 0 and attrs:
                executable.append((file, filehi))
    require(text is not None, 'requires unique TEXT')
    disjoint(vm_ranges, 'overlapping segment VM ranges')
    disjoint(file_ranges, 'overlapping segment file ranges')
    disjoint(section_vm, 'overlapping section VM ranges')
    disjoint(section_file, 'overlapping section file ranges')
    require(executable, 'requires executable TEXT section')
    return text, executable


def command_modes(blob, commands, executable):
    seen, uuid, entry, version = set(), None, None, False
    present = {cmd for cmd, _, _ in commands}
    repeated = (0x19, 0xc, 0x80000018, 0x8000001c)
    for cmd, off, size in commands:
        require(cmd in repeated or cmd not in seen, 'duplicate load command')
        seen.add(cmd)
        if cmd in (0x19, 0x1d):
            continue
        if cmd == 0x1b:
            require(size == 24, 'invalid UUID command')
            uuid = bytes(memoryview(blob)[off + 8:off + 24]).hex()
        elif cmd == 0x80000028:
            require(size == 24, 'invalid MAIN command')
            entry = struct.unpack_from('<Q', blob, off + 8)[0]
        elif cmd in (2, 0xb, 0x22, 0x80000022):
            fields = {2: 4, 0xb: 18, 0x22: 10, 0x80000022: 10}[cmd]
            require(size == 8 + fields * 4, 'invalid table command size')
            require(not any(struct.unpack_from('<' + str(fields) + 'I', blob, off + 8)),
                    'Unsupported symbol/dyld payload')
            require(cmd != 0xb or 2 in present, 'DYSYMTAB requires SYMTAB')
            require(not {0x22, 0x80000022}.issubset(present), 'duplicate DYLD_INFO variants')
        elif cmd in EMPTY_LINKEDIT:
            require(size == 16, 'invalid linkedit data command size')
            require(struct.unpack_from('<II', blob, off + 8) == (0, 0),
                    'Unsupported linkedit payload')
        elif cmd in (0xe, 0xc, 0x80000018, 0x8000001c):
            fixed = 24 if cmd in (0xc, 0x80000018) else 12
            require(size >= fixed, 'invalid string command size')
            relative = struct.unpack_from('<I', blob, off + 8)[0]
            require(fixed <= relative < size and blob.find(b'\0', off + relative, off + size) != -1,
                    'invalid command string bounds')
        elif cmd == 0x2a:
            require(size == 16, 'invalid source version command')
        elif cmd in (0x32, 0x24, 0x25):
            require(not version, 'conflicting version commands')
            version = True
            if cmd == 0x32:
                require(size >= 24, 'invalid build version command')
                count = struct.unpack_from('<I', blob, off + 20)[0]
                require(count <= 128, 'build tool count cap')
                require(size == 24 + 8 * count, 'invalid build tool command size')
            else:
                require(size == 16, 'invalid minimum version command')
        else:
            raise ValueError('Unsupported load command: ' + hex(cmd))
    require(uuid is not None, 'requires unique UUID')
    require(entry is not None, 'requires unique MAIN')
    require(any(lo <= entry < hi for lo, hi in executable), 'MAIN outside executable section')
    return uuid


def code_directory(blob, off, length, start, text):
    require(length >= 88, 'truncated CodeDirectory v20400')
    version, flags, hashes, identifier, special, count, limit = struct.unpack_from('>7I', blob, off + 8)
    hashsize, hashtype, platform, page = struct.unpack_from('>4B', blob, off + 36)
    spare2, scatter, team, spare3, limit64, base, execsize, execflags = struct.unpack_from('>4I4Q', blob, off + 40)
    require(version == 0x20400, 'Unsupported CodeDirectory version')
    require(flags & 2 and flags & ~0x20002 == 0, 'Unsupported CodeDirectory flags')
    require((hashsize, hashtype, platform) == (32, 2, 0), 'Unsupported CodeDirectory hash/platform')
    require(spare2 == scatter == team == spare3 == 0, 'Unsupported CodeDirectory spare/scatter/team')
    require(limit64 == 0, 'Unsupported CodeDirectory codeLimit64')
    require(limit == start, 'CodeDirectory codeLimit mismatch')
    require(page in (0, 12, 14), 'Unsupported CodeDirectory page exponent')
    expected = (1 if start else 0) if page == 0 else (start + (1 << page) - 1) >> page
    require(count == expected, 'CodeDirectory code slot coverage')
    require(special <= 7, 'CodeDirectory special slot cap')
    extent(base, execsize, 'CodeDirectory exec segment')
    require((base, execsize, execflags) == (text['fileoff'], text['filesize'], 1),
            'CodeDirectory exec segment mismatch')
    require(hashes >= 32 * special and hashes - 32 * special >= 88
            and hashes + 32 * count == length, 'CodeDirectory hash array bounds')
    first = hashes - 32 * special
    require(88 <= identifier < first, 'CodeDirectory identifier bounds')
    terminator = blob.find(b'\0', off + identifier, off + min(first, identifier + 4096))
    require(terminator > off + identifier, 'CodeDirectory identifier termination')


def signature_modes(blob, start, declared, slots, text):
    ranges, kinds, children = [], set(), []
    for i in range(slots):
        slot, relative = struct.unpack_from('>II', blob, start + 12 + 8 * i)
        require(slot not in kinds, 'duplicate signature slot')
        kinds.add(slot)
        require(relative >= 12 + 8 * slots and relative + 8 <= declared, 'invalid signature slot index')
        magic, length = struct.unpack_from('>II', blob, start + relative)
        require(length >= 8 and relative + length <= declared, 'invalid signature blob bounds')
        ranges.append((relative, relative + length))
        children.append((slot, start + relative, magic, length))
    disjoint(ranges, 'overlapping signature blobs')
    require(0 in kinds, 'missing CodeDirectory slot')
    for slot, off, magic, length in children:
        if slot == 0:
            require(magic == 0xfade0c02, 'Unsupported CodeDirectory magic')
            code_directory(blob, off, length, start, text)
        elif slot == 2:
            require(magic == 0xfade0c01 and length == 12
                    and struct.unpack_from('>I', blob, off + 8)[0] == 0,
                    'Unsupported requirements vector')
        elif slot in (5, 7):
            require(magic == {5: 0xfade7171, 7: 0xfade7172}[slot] and length > 8,
                    'Unsupported entitlement blob')
        elif slot == 0x10000:
            require(magic == 0xfade0b01 and length == 8, 'Unsupported CMS wrapper')
        else:
            raise ValueError('Unsupported signature slot: ' + hex(slot))


def code_identity(blob):
    header, end, commands, segments, signature, link, declared, slots = discover(blob)
    require(header[2] == header[7] == 0 and header[6] & ~0x00200085 == 0,
            'Unsupported Mach-O subtype/reserved/flags')
    command, start, length = signature
    text, executable = layout(blob, end, segments, start)
    uuid = command_modes(blob, commands, executable)
    signature_modes(blob, start, declared, slots, text)
    digest, view, cursor = hashlib.sha256(), memoryview(blob), 0
    replacements = [(command + 12, struct.pack('<I', 0)),
                    (link['command'] + 48, struct.pack('<Q', start - link['fileoff']))]
    for off, value in sorted(replacements):
        digest.update(view[cursor:off])
        digest.update(value)
        cursor = off + len(value)
    digest.update(view[cursor:start])
    return dict(schema='arm64-terminal-signature-v2', sha256=digest.hexdigest(), uuid=uuid,
                signature_offset=start, signature_size=length, non_signature_size=start)


def same_code(left, right):
    for field in ('schema', 'sha256', 'uuid', 'signature_offset', 'non_signature_size'):
        require(left[field] == right[field], 'non-signature code identity mismatch: ' + field)
