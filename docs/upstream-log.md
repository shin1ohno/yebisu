# Upstream tracking

P0 pins herdr-uikit to its unmerged GPUI 0.3.12 port. Do not merge the platform PR automatically.

`PROVENANCE.tsv` keeps the original `source_repository`, `source_commit`
and `source_path` immutable. Separate `upstream_repository`, `upstream_ref`
and `upstream_path` columns track `https://github.com/penso/herdr-gpui` on
`main`, including later moves. The initial upstream cursor is
`9855311562a23e1f32f6284ba4d69e72648a0ea4`, the merge base of the copied
fork revision `398c132a06723a0cc1494842a34666d173ec0200` and upstream main.
This is an explicit migration from a fork cursor to an upstream cursor;
the copy revision remains unchanged. Changes after this baseline remain due
for review, including `3a1b61753` in the registered `terminal.rs`.

`just triage-upstream` clones/fetches that upstream/ref into a URL/ref-specific
cache under target, and compares every row from its upstream cursor. It refuses
backwards or divergent positions before writing the ledger or review log.
`HERDR_GPUI_REPO` may select an existing read-only checkout with the requested
`refs/remotes/origin/<upstream_ref>`; it does not change the watched repository.
It reads each commit's full diff without a path filter, oldest first (including merged branch commits), so renames
are followed. It updates `upstream_path` and `last_triaged` only after writing
the commit and affected-path list below. Review every listed change and record
`apply` or `skip` with a reason before closing the entry. Inspect all diffs,
security scopes and the watched repository's GitHub Security Advisories;
security changes must be applied regardless of the commit subject.

`just bump-fork REV` updates every declared yebisu dependency on the Herdr
fork to the same full revision. It updates only lockfile packages from that
fork, one package at a time, then verifies locked Cargo metadata. It does not
run a blanket Cargo update. Run `just check`, `just lint-ios`, `just test`, and
the device contract checks before accepting a fork bump. Inspect the lock diff
by git source IDs, registry resolution after patch removal, and GPUI changes.
Run the depended-on fork crates' own `cargo test --locked` in a separately
authorized fork checkout; consumer tests do not execute dependency-crate tests.
Then run consumer contract regressions (including P3 negatives when present)
and physical connection/IME checks. An on-device regression that cannot be
fixed within one working day must be reverted.

The bump parses TOML declarations (inline, table, target and workspace forms),
checks every fork rev plus resolved fork lock SHA, and restores every manifest
and the lockfile byte-for-byte if Cargo or the revision checks fail. Python
3.11+ uses `tomllib`; the supported Xcode stock Python 3.9 uses pip's bundled
`tomli` parser. No dependency is installed by the recipe.
