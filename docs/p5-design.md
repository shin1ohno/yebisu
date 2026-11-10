# P5 connection and persistence contract

Each host target has a monotonically increasing in-memory attempt generation.
The selected Link also has a monotonically increasing token across host switches.
Connection completions must match both; readers own their original inbox, and
paint callbacks must match the Link token. A different host or a return from
background never inherits an old inbox, held input, key state, or composition.

The container-local `yebisu-hosts.json` is version 1 with `targets` (up to 32
validated target strings) and `selected`. First load imports the legacy
`yebisu-target` if the new file is absent. Writes validate, write and fsync a
pending file, rename it, and fsync its parent. Invalid/future versions are errors,
not an empty replacement. The existing known_hosts file alone supplies trust;
saving an address never trusts it. All file, SSH and Keychain calls run on a
background worker. The UI serializes store jobs and displays errors.

Inactive/Background/Disconnected invalidates the current Link immediately.
Active after suspension makes one new attempt by default, through the existing
P2 scan/authentication gate and independently pin-checking SSH connection.
Version mismatch disables automatic attempts. Network failure is displayed;
there is no continuous retry loop. Tap Reconnect or return from background to
try again. Duplicate lifecycle events do not produce duplicate attempts.
The runtime policy button switches between automatic return and manual tap.

Persistence is incremental, not dependent on a final background save callback.
The runtime reconnect policy and generation counters are intentionally not saved.
Simulator QA uses a compile-time Simulator-only private peer branch; it proves
lifecycle, state and rendering, not SSH host-key acceptance or physical IME.
