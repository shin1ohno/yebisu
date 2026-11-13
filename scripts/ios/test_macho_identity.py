"""Independent packed fixtures; placeholder hashes are NOT Apple/native evidence."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

assert sys.dont_write_bytecode


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


m = module('macho', 'macho_identity.py')
START = 10240
VM = 0x100000000


def code_directory(payload=b'fixture', page=12, special=0, count=None):
    """v20400: header88, identifier, special hashes, code hashes; no real hashes."""
    count = (1 if page == 0 else (START + (1 << page) - 1) >> page) if count is None else count
    identifier = payload + b'\0'
    identifier += bytes((-len(identifier)) % 8)
    hashes = 88 + len(identifier) + 32 * special
    length = hashes + 32 * count
    header = struct.pack('>9I4B4I4Q', 0xfade0c02, length, 0x20400, 2, hashes,
                         88, special, count, START, 32, 2, 0, page,
                         0, 0, 0, 0, 0, 0, 4096, 1)
    return header + identifier + b'H' * (32 * (special + count))


def section(name=b'__text', parent=b'__TEXT', addr=VM + 1024, size=512,
            offset=1024, align=2, flags=0x80000400):
    return struct.pack('<16s16sQQ8I', name, parent, addr, size, offset, align,
                       0, 0, flags, 0, 0, 0)


def segment(name, vm, vmsize, fileoff, filesize, prot, sections=()):
    return struct.pack('<II16s4Q4I', 0x19, 72 + 80 * len(sections), name,
                       vm, vmsize, fileoff, filesize, prot, prot, len(sections), 0) + b''.join(sections)


def fixture(payload=b'fixture', extra=(), children=None, tail=0, text_sections=None,
            data_sections=None, page=12, special=0, count=None):
    if children is None:
        children = [(0, code_directory(payload, page, special, count)),
                    (2, struct.pack('>III', 0xfade0c01, 12, 0)),
                    (0x10000, struct.pack('>II', 0xfade0b01, 8))]
    index_end = 12 + 8 * len(children)
    position = (index_end + 7) // 8 * 8
    indexes, bodies, child_offsets = b'', bytes(position - index_end), {}
    for slot, content in children:
        indexes += struct.pack('>II', slot, position)
        child_offsets.setdefault(slot, START + position)
        bodies += content
        position += len(content)
    signature = struct.pack('>III', 0xfade0cc0, position, len(children)) + indexes + bodies
    signature += bytes((-len(signature)) % 16 + tail)
    text_sections = [section()] if text_sections is None else text_sections
    data_sections = [section(b'__data', b'__DATA', VM + 4096, 64, 4096, 3, 0)] if data_sections is None else data_sections
    parts = [segment(b'__TEXT', VM, 4096, 0, 4096, 5, text_sections),
             segment(b'__DATA', VM + 4096, 4096, 4096, 4096, 3, data_sections),
             segment(b'__LINKEDIT', VM + 8192, 16384, 8192, START - 8192 + len(signature), 1),
             struct.pack('<II16s', 0x1b, 24, bytes(range(16))),
             struct.pack('<IIQQ', 0x80000028, 24, 1024, 0),
             struct.pack('<IIII', 0x1d, 16, START, len(signature)), *extra]
    offsets, cursor = {}, 32
    for part in parts:
        cmd = struct.unpack_from('<I', part)[0]
        key = bytes(part[8:24]).rstrip(b'\0') if cmd == 0x19 else cmd
        offsets.setdefault(key, cursor)
        cursor += len(part)
    commands = b''.join(parts)
    header = struct.pack('<8I', 0xfeedfacf, 0x100000c, 0, 2, len(parts), len(commands), 0x00200085, 0)
    body = header + commands + b'X' * (4096 - 32 - len(commands))
    body += b'D' * 4096 + b'L' * (START - 8192) + signature
    return body, dict(offsets, children=child_offsets)


def image(signature_payload=b'fixture'):
    return fixture(signature_payload)[0]


def changed(data, offset, value, fmt='<I'):
    result = bytearray(data)
    struct.pack_into(fmt, result, offset, value)
    return result


def command(cmd, fields):
    return struct.pack('<II', cmd, 8 + 4 * len(fields)) + struct.pack('<' + str(len(fields)) + 'I', *fields)


class CodeIdentity(unittest.TestCase):
    def reject(self, case, data, oracle):
        with self.subTest(case=case, oracle=oracle), self.assertRaisesRegex(ValueError, oracle):
            m.code_identity(data)

    def test_P00_P02_signature_only_identity(self):
        baseline = m.code_identity(image())
        for name, data in [('allocation growth', fixture(tail=128)[0]),
                           ('identifier change', image(b'changed identifier' * 4)),
                           ('special hashes', fixture(special=7)[0])]:
            with self.subTest(case=name):
                m.same_code(baseline, m.code_identity(data))
        for page in (0, 12, 14):
            with self.subTest(page=page):
                m.code_identity(fixture(page=page)[0])

    def test_N01_nonempty_reference_modes(self):
        cases = [('outer signature alias', 0x26, [START, 16]),
                 ('in-bounds export to signature', 0x80000033, [8192, 32]),
                 ('chained internal out-of-range', 0x80000034, [8192, 32])]
        for name, cmd, values in cases:
            data, _ = fixture(extra=[command(cmd, values)])
            data = bytearray(data)
            if cmd == 0x80000033:
                # Valid outer region; regular export terminal operand names excluded bytes.
                data[8192:8201] = bytes([0, 1, 120, 0, 5, 3, 0, 0x80, 0x51])
                data[8201] = 0  # child count after terminal
            if cmd == 0x80000034:
                struct.pack_into('<7I', data, 8192, 0, 0xfffffff0, 28, 28, 0, 1, 0)
            self.reject(name, data, 'Unsupported linkedit payload')
        symbol, _ = fixture(extra=[command(2, [8192, 1, 8208, 16])])
        symbol = bytearray(symbol)
        struct.pack_into('<IBBHQ', symbol, 8192, START + 128 - 8208, 0xf, 1, 0, VM + 1024)
        self.reject('symbol n_strx reference', symbol, 'Unsupported symbol/dyld payload')
        data, ix = fixture()
        self.reject('relocation operand', changed(changed(data, ix[b'__TEXT'] + 72 + 56, 8192),
                                                 ix[b'__TEXT'] + 72 + 60, 1),
                    'Unsupported section relocation payload')

    def test_N02_empty_modes_and_each_nonzero_field(self):
        tables = [(2, 4), (0xb, 18), (0x22, 10)]
        links = [0x1e, 0x26, 0x29, 0x2b, 0x2e, 0x80000033, 0x80000034]
        all_empty = [command(c, [0] * n) for c, n in tables] + [command(c, [0, 0]) for c in links]
        m.code_identity(fixture(extra=all_empty)[0])
        m.code_identity(fixture(extra=[command(0x80000022, [0] * 10)])[0])
        for cmd, fields in tables + [(0x80000022, 10)] + [(c, 2) for c in links]:
            for field in range(fields):
                values = [0] * fields
                values[field] = 1
                extra = ([command(2, [0] * 4)] if cmd == 0xb else []) + [command(cmd, values)]
                self.reject(f'{cmd:#x} field {field}', fixture(extra=extra)[0],
                            'Unsupported linkedit payload' if cmd in links else 'Unsupported symbol/dyld payload')
        self.reject('DYSYMTAB alone', fixture(extra=[command(0xb, [0] * 18)])[0], 'DYSYMTAB requires SYMTAB')
        self.reject('both DYLD_INFO', fixture(extra=[command(0x22, [0] * 10), command(0x80000022, [0] * 10)])[0],
                    'duplicate DYLD_INFO variants')

    def test_N03_required_and_unique(self):
        data, ix = fixture()
        for key, size in [(0x1b, 24), (0x80000028, 24), (0x1d, 16), (b'__TEXT', 152), (b'__LINKEDIT', 72)]:
            duplicate = data[ix[key]:ix[key] + size]
            self.reject(f'duplicate {key}', fixture(extra=[duplicate])[0],
                        'requires unique signature/linkedit' if key in (0x1d, b'__LINKEDIT')
                        else 'duplicate segment' if key == b'__TEXT' else 'duplicate load command')
        for key, oracle in [(0x1b, 'requires unique UUID'), (0x80000028, 'requires unique MAIN'),
                            (0x1d, 'requires unique signature/linkedit')]:
            # Replace with an accepted repeatable string command of identical size.
            removed = bytearray(data)
            struct.pack_into('<I', removed, ix[key], 0x8000001c)
            struct.pack_into('<I', removed, ix[key] + 8, 12)
            removed[ix[key] + 12] = 0
            self.reject(f'missing {key}', removed, oracle)
        # Use no sections so the missing TEXT predicate is isolated.
        no_text, nx = fixture(text_sections=[])
        no_text = bytearray(no_text); no_text[nx[b'__TEXT'] + 8:nx[b'__TEXT'] + 24] = b'__DATA_CONST'.ljust(16, b'\0')
        struct.pack_into('<II', no_text, nx[b'__TEXT'] + 56, 3, 3)
        self.reject('missing TEXT', no_text, 'requires unique TEXT')
        removed = bytearray(data); removed[ix[b'__LINKEDIT'] + 8:ix[b'__LINKEDIT'] + 24] = b'__OTHER'.ljust(16, b'\0')
        self.reject('missing LINKEDIT', removed, 'requires unique signature/linkedit')
        optional = command(0x26, [0, 0])
        self.reject('duplicate optional', fixture(extra=[optional, optional])[0], 'duplicate load command')
        self.reject('missing slot0', fixture(children=[(2, struct.pack('>III', 0xfade0c01, 12, 0))])[0],
                    'missing CodeDirectory slot')

    def test_N04_signature_modes(self):
        data, ix = fixture(); cd = ix['children'][0]
        for name, offset, value, oracle, fmt in [
            ('outer magic', START, 0xcc0cfade, 'invalid embedded signature SuperBlob', '>I'),
            ('CD magic', cd, 0x020cdefa, 'Unsupported CodeDirectory magic', '>I'),
            ('CD version', cd + 8, 0x20500, 'Unsupported CodeDirectory version', '>I'),
            ('no ADHOC', cd + 12, 0, 'Unsupported CodeDirectory flags', '>I'),
            ('other flags', cd + 12, 3, 'Unsupported CodeDirectory flags', '>I'),
            ('hash size', cd + 36, 20, 'Unsupported CodeDirectory hash/platform', '>B'),
            ('hash type', cd + 37, 1, 'Unsupported CodeDirectory hash/platform', '>B'),
            ('platform', cd + 38, 1, 'Unsupported CodeDirectory hash/platform', '>B'),
            ('limit64', cd + 56, START, 'Unsupported CodeDirectory codeLimit64', '>Q')]:
            self.reject(name, changed(data, offset, value, fmt), oracle)
        for offset in (40, 44, 48, 52):
            self.reject(f'CD reserved offset {offset}', changed(data, cd + offset, 1, '>I'),
                        'Unsupported CodeDirectory spare/scatter/team')
        m.code_identity(changed(data, cd + 12, 0x20002, '>I'))

    def test_N05_page_coverage_and_hash_bounds(self):
        for page, bad in [(0, 0), (0, 2), (12, 2), (12, 4), (14, 0), (14, 2)]:
            self.reject(f'page {page} count {bad}', fixture(page=page, count=bad)[0], 'CodeDirectory code slot coverage')
        data, ix = fixture(); cd = ix['children'][0]
        for name, offset, value, fmt, oracle in [
            ('page255', 39, 255, '>B', 'Unsupported CodeDirectory page exponent'),
            ('special8', 24, 8, '>I', 'CodeDirectory special slot cap'),
            ('limit mismatch', 32, START - 1, '>I', 'CodeDirectory codeLimit mismatch'),
            ('hash end', 16, 97, '>I', 'CodeDirectory hash array bounds'),
            ('hash header', 16, 80, '>I', 'CodeDirectory hash array bounds')]:
            self.reject(name, changed(data, cd + offset, value, fmt), oracle)
        data, ix = fixture(special=7)
        self.reject('special underflow', changed(data, ix['children'][0] + 16, 223, '>I'), 'CodeDirectory hash array bounds')

    def test_N06_identifier_and_exec(self):
        data, ix = fixture(); cd = ix['children'][0]
        for value in (87, 96):
            self.reject(f'identifier offset {value}', changed(data, cd + 20, value, '>I'), 'CodeDirectory identifier bounds')
        self.reject('empty identifier', changed(data, cd + 88, 0, '>B'), 'CodeDirectory identifier termination')
        missing = bytearray(data); missing[cd + 88:cd + 96] = b'Z' * 8
        self.reject('unterminated identifier', missing, 'CodeDirectory identifier termination')
        self.reject('long identifier', fixture(payload=b'Z' * 4096)[0], 'CodeDirectory identifier termination')
        for name, offset, value in [('base', 64, 1), ('limit', 72, 4095), ('flags', 80, 0)]:
            self.reject('exec ' + name, changed(data, cd + offset, value, '>Q'), 'CodeDirectory exec segment mismatch')
        self.reject('exec overflow', changed(data, cd + 64, (1 << 64) - 1, '>Q'), 'CodeDirectory exec segment overflow')

    def test_N07_slot_layouts(self):
        baseline = [(0, code_directory())]
        for name, slot, content, oracle in [
            ('nested requirements', 2, struct.pack('>III', 0xfade0c01, 12, 1), 'Unsupported requirements vector'),
            ('long requirements', 2, struct.pack('>IIII', 0xfade0c01, 16, 0, 0), 'Unsupported requirements vector'),
            ('short requirements', 2, struct.pack('>II', 0xfade0c01, 8), 'Unsupported requirements vector'),
            ('XML empty', 5, struct.pack('>II', 0xfade7171, 8), 'Unsupported entitlement blob'),
            ('DER empty', 7, struct.pack('>II', 0xfade7172, 8), 'Unsupported entitlement blob'),
            ('CMS nonempty', 0x10000, struct.pack('>III', 0xfade0b01, 12, 0), 'Unsupported CMS wrapper'),
            ('unknown slot', 3, struct.pack('>II', 0xfade0b01, 8), 'Unsupported signature slot'),
            ('alternate CD', 0x1000, code_directory(), 'Unsupported signature slot'),
            ('duplicate CD', 0, code_directory(), 'duplicate signature slot')]:
            self.reject(name, fixture(children=baseline + [(slot, content)])[0], oracle)
        accepted = baseline + [(5, struct.pack('>II', 0xfade7171, 9) + b'X'),
                               (7, struct.pack('>II', 0xfade7172, 9) + b'D')]
        m.code_identity(fixture(children=accepted)[0])
        data, ix = fixture()
        self.reject('child in index', changed(data, START + 16, 12, '>I'), 'invalid signature slot index')
        self.reject('child out of bounds', changed(data, START + 16, len(data), '>I'), 'invalid signature slot index')
        self.reject('subblob length', changed(data, ix['children'][0] + 4, 7, '>I'), 'invalid signature blob bounds')
        self.reject('subblob beyond container', changed(data, ix['children'][0] + 4, len(data), '>I'), 'invalid signature blob bounds')
        self.reject('overlap child', changed(data, START + 24, ix['children'][0] - START, '>I'), 'overlapping signature blobs')

    def test_N08_segment_geometry(self):
        data, ix = fixture(); text, dat = ix[b'__TEXT'], ix[b'__DATA']
        cases = [('VM overflow', text + 24, (1 << 64) - 4096, '<Q', 'segment VM overflow'),
                 ('file overflow', text + 40, (1 << 64) - 4096, '<Q', 'segment file overflow'),
                 ('VM alignment', text + 24, VM + 1, '<Q', 'segment alignment'),
                 ('file alignment', text + 40, 1, '<Q', 'segment alignment'),
                 ('flags', text + 68, 16, '<I', 'Unsupported segment flags'),
                 ('protection', text + 56, 7, '<I', 'Unsupported segment protection'),
                 ('MAIN outside', ix[0x80000028] + 8, 1000, '<Q', 'MAIN outside executable section')]
        for name, off, val, fmt, oracle in cases:
            self.reject(name, changed(data, off, val, fmt), oracle)
        plain, px = fixture(data_sections=[])
        self.reject('VM overlap', changed(plain, px[b'__DATA'] + 24, VM, '<Q'), 'overlapping segment VM ranges')
        self.reject('file overlap', changed(plain, px[b'__DATA'] + 40, 0, '<Q'), 'overlapping segment file ranges')
        pagezero = segment(b'__PAGEZERO', 0, VM, 0, 0, 0)
        m.code_identity(fixture(extra=[pagezero])[0])
        for field in (40, 48, 56, 60):
            bad = changed(pagezero, field, 1, '<Q' if field in (40, 48) else '<I')
            self.reject(f'PAGEZERO field {field}', fixture(extra=[bad])[0], 'Unsupported PAGEZERO layout')
        self.reject('PAGEZERO sections', fixture(extra=[segment(b'__PAGEZERO', 0, VM, 0, 0, 0, [section()])])[0],
                    'Unsupported PAGEZERO layout')

    def test_N08_section_geometry(self):
        data, ix = fixture(); off = ix[b'__TEXT'] + 72
        cases = [('VM ownership', 32, VM - 4, '<Q', 'section VM outside parent'),
                 ('file ownership', 48, 4096, '<I', 'section file outside parent'),
                 ('mapping', 48, 1028, '<I', 'section VM/file mapping'),
                 ('align extreme', 52, 255, '<I', 'section alignment exponent'),
                 ('alignment', 52, 11, '<I', 'section VM alignment'),
                 ('type', 64, 0x80000406, '<I', 'Unsupported section type'),
                 ('attributes', 64, 0x200, '<I', 'Unsupported section attributes'),
                 ('reserved', 68, 1, '<I', 'Unsupported section reserved fields')]
        for name, delta, val, fmt, oracle in cases:
            self.reject(name, changed(data, off + delta, val, fmt), oracle)
        bad = bytearray(data); bad[off + 16:off + 32] = b'__OTHER'.ljust(16, b'\0')
        self.reject('parent name', bad, 'section parent mismatch')
        self.reject('section overlap', fixture(text_sections=[section(), section()])[0], 'overlapping section VM ranges')
        self.reject('header overlap', fixture(text_sections=[section(addr=VM + 256, size=16, offset=256)])[0],
                    'section overlaps header/signature')
        self.reject('no executable', fixture(text_sections=[section(flags=0)])[0], 'requires executable TEXT section')
        for kind in (0, 2, 3, 4, 0xe):
            m.code_identity(fixture(data_sections=[section(b'x', b'__DATA', VM + 4096, 64, 4096, 3, kind)])[0])
        zero = section(b'z', b'__DATA', VM + 4096, 64, 0, 3, 1)
        m.code_identity(fixture(data_sections=[zero])[0])
        self.reject('zerofill file', fixture(data_sections=[changed(zero, 48, 4096)])[0], 'Unsupported zerofill mapping')
        for offset in (0, 8192):
            m.code_identity(fixture(data_sections=[section(b'e', b'__DATA', VM + 8192, 0, offset, 3, 0)])[0])
        self.reject('empty outside VM', fixture(data_sections=[section(b'e', b'__DATA', VM + 8200, 0, 0, 3, 0)])[0],
                    'section VM outside parent')
        self.reject('empty outside file', fixture(data_sections=[section(b'e', b'__DATA', VM + 8192, 0, 8200, 3, 0)])[0],
                    'section file outside parent')

    def test_N09_caps_framing_strings_versions(self):
        data, ix = fixture()
        cases = [('header cap', 20, 1048576, 'load command byte cap'),
                 ('command cap', 16, 4097, 'load command count cap'),
                 ('section cap', ix[b'__TEXT'] + 64, 16385, 'section count cap'),
                 ('signature cap', ix[0x1d] + 12, 16777217, 'signature allocation cap'),
                 ('unaligned command', 36, 151, 'invalid command size'),
                 ('truncated command', 36, len(data), 'invalid command size'),
                 ('segment framing', 36, 144, 'invalid segment command size')]
        for name, off, val, oracle in cases:
            self.reject(name, changed(data, off, val), oracle)
        self.reject('slot cap', changed(data, START + 8, 65, '>I'), 'signature slot cap')
        self.reject('truncated index', changed(data, START + 4, 12, '>I'), 'invalid embedded signature SuperBlob')
        self.reject('truncated header', data[:31], 'truncated Mach-O')
        self.reject('non MachO', b'compiled file', 'truncated Mach-O')
        self.reject('signature unaligned', changed(data, ix[0x1d] + 8, START - 1), 'signature must be aligned terminal range')
        self.reject('header kind', changed(data, 12, 6), 'requires thin little-endian ARM64 MH_EXECUTE')
        for off in (8, 28):
            self.reject(f'header subset {off}', changed(data, off, 1), 'Unsupported Mach-O subtype/reserved/flags')
        self.reject('header flags', changed(data, 24, 0x800000), 'Unsupported Mach-O subtype/reserved/flags')
        self.reject('unknown command', fixture(extra=[command(0x777, [])])[0], 'Unsupported load command')
        for cmd, fixed in [(0xe, 12), (0xc, 24), (0x80000018, 24), (0x8000001c, 12)]:
            size = (fixed + 8 + 7) // 8 * 8
            good = bytearray(size); struct.pack_into('<III', good, 0, cmd, size, fixed)
            good[fixed:fixed + 5] = b'path\0'
            m.code_identity(fixture(extra=[good])[0])
            self.reject(f'{cmd:#x} external string', fixture(extra=[changed(good, 8, size)])[0], 'invalid command string bounds')
            bad = bytearray(good); bad[fixed:] = b'Z' * (size - fixed)
            self.reject(f'{cmd:#x} nonnul string', fixture(extra=[bad])[0], 'invalid command string bounds')
        build = command(0x32, [7, 1, 1, 0])
        m.code_identity(fixture(extra=[build, command(0x2a, [0, 0])])[0])
        for cmd in (0x24, 0x25):
            m.code_identity(fixture(extra=[command(cmd, [0, 0])])[0])
            self.reject('version conflict', fixture(extra=[build, command(cmd, [0, 0])])[0], 'conflicting version commands')
        self.reject('tool cap', fixture(extra=[command(0x32, [7, 1, 1, 129])])[0], 'build tool count cap')
        self.reject('tool framing', fixture(extra=[command(0x32, [7, 1, 1, 1])])[0], 'invalid build tool command size')

    def test_N09_file_cap_before_read(self):
        helper = module('bundle_provenance_for_cap', 'bundle-provenance.py')
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'oversized'
            with path.open('wb') as stream:
                stream.truncate(128 * 1024 * 1024 + 1)
            with patch.object(Path, 'open', side_effect=AssertionError('must reject before read')), patch.object(helper, 'uuids') as uuid:
                with self.assertRaisesRegex(ValueError, 'Mach-O file size cap'):
                    helper.identity(path)
                uuid.assert_not_called()

    def test_N10_full_prefix_identity(self):
        data, ix = fixture(); baseline = m.code_identity(data)
        for name, off in [('code', 1024), ('data', 4096), ('opaque LINKEDIT', 9000),
                          ('header flags', 24), ('UUID', ix[0x1b] + 8)]:
            mutated = bytearray(data); mutated[off] ^= 1
            with self.subTest(case=name), self.assertRaisesRegex(ValueError, 'non-signature code identity mismatch'):
                m.same_code(baseline, m.code_identity(mutated))
        moved, mx = fixture(data_sections=[])
        with self.assertRaisesRegex(ValueError, 'non-signature code identity mismatch'):
            m.same_code(m.code_identity(moved), m.code_identity(changed(moved, mx[b'__DATA'] + 24, VM + 32768, '<Q')))

    def test_N11_tail_precedes_deferred_subset_gates(self):
        data, _ = fixture(tail=16, extra=[command(0x80000034, [8192, 32])])
        data = changed(data, 24, 0x800000)
        data[-1] = 1
        self.reject('tail priority over header/payload', data, 'nonzero signature trailing padding')


if __name__ == '__main__':
    unittest.main()
