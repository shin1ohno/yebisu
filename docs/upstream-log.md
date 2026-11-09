# Upstream tracking

P0 pins herdr-uikit to its unmerged GPUI 0.3.12 port. Do not merge the platform PR automatically.

`just triage-upstream` compares every provenance row with the recorded source
main. It reads each commit's full diff without a path filter so renames can be
followed. It updates `source_path` and `last_triaged` only after writing the
commit and affected-path list below. Review every listed change and record
`apply` or `skip` with a reason before closing the entry. Security changes must
be applied regardless of the commit subject. The original `source_commit`
continues to identify the revision from which the local file was copied or
modeled.

`just bump-fork REV` updates every declared yebisu dependency on the Herdr
fork to the same full revision. It updates only lockfile packages from that
fork, one package at a time, then verifies locked Cargo metadata. It does not
run a blanket Cargo update. Run `just check`, `just lint-ios`, `just test`, and
the device contract checks before accepting a fork bump.
