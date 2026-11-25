# The `herdr-app` document

The official Herdr app keeps its spaces, tabs, names and pane trees in one
document, `herdr-app`, in the herdr-core of the Mac's `default` session. yebisu
reads and writes the same document so the iPad and the Mac show the same spaces
and tabs (issue #144). The model is `crates/yebisu/src/core/herdr_doc.rs`; its
tests are `core/herdr_doc_tests.rs`; the fixtures are
`crates/yebisu/fixtures/herdr-app/`.

The document's shape comes from reading the official app 0.1.15 and herdr-core
0.1.20. yebisu's developers cannot run either, so the promise below is held by
fixtures and tests, and checked against a real document whenever one is added.

## Compatibility promise

yebisu keeps these, and its fixtures and checks test them:

1. **H-1.** The document is `herdr-app`, in the `default` session's core.
2. **H-2.** The shape is `{spaces: [{id, name, folder, tabs: [{id, name,
   root}]}]}`. Every field may be omitted, and an omitted field reads as empty.
   A `null` document is empty.
3. **H-3.** An empty name is the automatic name. Names are trimmed.
4. **H-4.** A tree node's `type` is `block`, `row`, `column` or `stack`. A
   block's `content` is `{terminal}`, `{mod, props}`, `{tab}` or `{agent}`. A
   `size` is `{px}` or `{weight}`. An unknown `type` or `content` is kept and not
   drawn.
5. **H-5.** Ids belong to one machine. A tab's root has the tab's id.
6. **H-6.** Writes name the version they read (`doc.set` refuses a stale one with
   `kind: "conflict"`). A value is at most 1 MiB. The event is `{"type": "doc",
   "data": {"name", "version"}, "seq"}`.
7. **H-7.** The last document herdr-core accepted wins.
8. **H-8.** A live terminal labelled `herdr.space` (and `herdr.tab`) is taken in by
   the official app.

yebisu never builds the document again from what it draws. Each change is an
intent applied to the latest value, which leaves every other key (including
fields a newer app adds) byte for byte as read. Before each write, four gates
must pass: the size, the shape, the changed path, and the ids. If the
document's outer shape changes in a way this build cannot read, yebisu writes
nothing (Design D-5).

## Fixtures

All are synthetic until a real document is attached to issue #144 (owner
decision 12):

| Fixture | What it holds |
|---|---|
| `unknown-fields.json` | Unknown keys at the top level, in a space, a tab, a node and a floating block |
| `omitted.json`, `empty-object.json`, `empty-spaces.json` | Omitted fields, `{}` and `{"spaces": []}` |
| `core-roundtrip-v0.json`, `core-roundtrip-written.json` | Stand-ins for the private core's captures: a document never written (`null`, version 0), and one written and read back |
| `tree-three-panes.json`, `tree-stack.json`, `tree-mod.json`, `tree-float.json`, `tree-tab-agent.json`, `tree-px.json` | One tree shape each |
| `new-node-fields.json` | A node field from a newer official app |
| `two-spaces.json` | Two spaces, one named and one automatic, for the Simulator QA (`YEBISU_CORE_DOC_QA=two-spaces`) |
| `unreadable-*.json` | Shapes the official app cannot read: `spaces` not an array, a root without `type`, a child that is not an object, a top level that is not an object |

A Simulator QA run names its starting document with `YEBISU_CORE_DOC_QA=<fixture>`.
`YEBISU_CORE_DOC_QA_CONFLICTS=<n>` makes the mock core refuse the first `n` writes as stale,
as if another app wrote first.

The tests list the directory, so a new fixture is checked without a code
change. Each fixture must read and write back byte for byte. Each readable
fixture must accept every intent, and the result must pass the pre-write
gates. Each fixture name starting with `unreadable-` must be refused.

## Adding the document of an official-app release

For each official-app release:

1. The owner confirms which Mac runs the release. On that Mac, the owner reads
   `doc.get {"name": "herdr-app"}` once from the `default` session's core.
2. The owner masks paths and other personal values, and keeps every key and its
   order. The owner does not pass the value through a tool that rewrites
   numbers, such as `1.0` to `1`.
3. Add the result as `crates/yebisu/fixtures/herdr-app/real-<version>.json`.
4. Run `just test`. The golden tests pick up the file and check the round trip
   and every intent against it. If one fails, the release changed the
   document's shape: change `parse_view` and the gates in `herdr_doc.rs` before
   yebisu writes to that release.

## Moving the iPad's saved tabs (D5)

An earlier yebisu kept the `default` session's tabs only in the iPad file
`herdr2/core-<host hex>-default.json`. The first readable document of a
`default` connection takes them once, in one version-checked write, as a new
space named `iPad` (owner decision 4). Two panes become a `row`, or a `column`
when they were stacked.

- Before the file is rewritten, it is copied once to
  `core-<host hex>-default.v2.json`. An existing copy is never overwritten, and
  the session list ignores the copy. To undo the move, put the copy back as the
  `default` file and remove its `migrated` key.
- The file records `"migrated": <document version>` and keeps only the iPad's
  own view (`doc_view`), never document tabs. No connection migrates again.
- A write that landed without the record is recognised by its terminals already
  being in the document: the record is written and no second space is made. A
  migration that fails is tried again at the next connection.
- The moved terminals keep no `herdr.space` or `herdr.tab` labels: the pinned
  client has no `terminal.label`. Reading the official app's source says it
  shows a terminal its tree names without the labels, but nobody has seen it.
  This is owner check O5 in the D5 pull request; if the tabs do not appear, the
  fix is a herdr-gpui issue to add `terminal.label`.

The opt-in test `core::herdr_doc_sync::live_tests` checks the document
semantics against a real herdr-core: version 0 for a document never written, a
stale version refused as `conflict` with nothing changed, the `doc` event,
values over 1 MiB refused, and value, version and unknown fields kept across a
restart. It starts its own core with the private fleet of `core/space_fleet.rs`,
in the session `test-doc` (checked to start with `test-`), reaches it through
`hc connect` as the iPad does, and stops only that core. Like the other fleet
tests it runs one at a time:

    YEBISU_HERDR_CORE_TEST_BINARY=/path/to/herdr-core cargo test -p yebisu herdr_doc_live -- --ignored --test-threads=1

## Still to check against a real document

- The split rule: halving a `weight`, a block without `size`, and wrapping a
  block in a new container when the direction differs.
- The placement of a new space ("the end of this machine's spaces"). yebisu
  appends it to `spaces`.
- Whether the official app shows a terminal without labels that the tree names.
  This matters for the migration of the iPad's saved tabs.
