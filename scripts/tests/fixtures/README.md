# Provenance Git fixture

`provenance-history.json` contains the actual UTF-8 Git blobs from
`https://github.com/shin1ohno/herdr-gpui`: `core_terminals/panes.rs` at
`8c1719924^`, its renamed destination at `9c32a9270`, and `terminal.rs`
at `3a1b61753^` and `3a1b61753`. Full source IDs and subjects are embedded.
These blobs retain their upstream Apache-2.0 license and NOTICE attribution.

`provenance_fixture.history` projects only these files into three commits:
8c1719924's exact R100 move, 9c32a9270's actual post-move modification, then
3a1b61753's actual terminal modification, merged into main with a two-parent
merge so first-parent-only traversal cannot pass the D1 positive control. The synthetic commit IDs differ;
subjects preserve the source IDs. It uses real Git objects and production
rev-list/diff-tree/NUL parsing, requires neither network nor a developer's
fork checkout, and does not alter that checkout. Unrelated upstream files
are deliberately omitted. The earlier TSV retains the raw status reference.

The rename test must fail with any one of these production mutations:
remove `-M`, append `-- <old path>` to diff-tree, remove `--reverse`.
