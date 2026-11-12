# P8 handoff contract

Reference: shin1ohno/herdr-gpui at 398c132a06723a0cc1494842a34666d173ec0200,
`docs/ios.md:88-91` (main-thread UIKit acquisition, background preparation,
limits and stale-target cancellation) and
`crates/sheprd-gpui/src/window/pending_input.rs:1-43` (FIFO and cancellation;
line 39 bounds held events). These are model sources, not live checkout lines.

Selection is local, scoped to a displayed surface and destination. Starting
selection cancels composition and queued input; dragging never sends terminal
input. Copy emits only bounded UTF-8. Changing host, boot, pane, popup, connection
or leaving the terminal invalidates selection and asynchronous paste work.

Pastes reserve FIFO order before acquisition. Normal paste prefers text;
image-only paste ignores text. UIKit access stays on the main thread. Image
decoding/encoding and transfer execute off-thread. One outstanding paste preparation (below the reference ceiling of four) and
256 held input events are accepted. A second paste is visibly refused while busy; only input after that refusal is discarded. Keys made after an unfinished
paste wait behind it. Failed or cancelled paste discards its following held
input. No completion can borrow the newly selected host's handle. Before
publication, recheck host selection epoch and reader destination; cancellation
also revokes the original client's reservation. Disconnect and navigation never
replay old input.

Limits from the reference image_source.rs: 128 MiB encoded acquisition,
64 Mi pixels decoded, 256 MiB decoder allocation; wire payload limit is
sheprd_client::protocol::MAX_CLIPBOARD_IMAGE_PAYLOAD. Larger supported images
are resized/recompressed within the wire limit; malformed or excessive images
are refused visibly. Text acquisition/copy is capped at 2 MiB.

SurfaceImages enters pending_surface_images and is attached only when a coherent
surface is committed, following `state/projection.rs:127-129`. Never open a
GraphicsFile path. OSC 52 remains the existing bounded UTF-8-only, one-shot queue.

Runtime choices: explicit Select mode versus double-click/drag initiation;
manual Copy versus Copy on selection. Both remain local and share cancellation.

The canvas uses a content mask, not an enclosing paint layer: the painter owns
its background/text/image layers, so images must not inherit an earlier parent
draw order below the backgrounds.

Explicit paste or turning Select off ends the old composition and resumes input.
A commit UIKit delivers after that boundary is accepted, even if it originated
from the preceding composition. The SDK routes insertText and IME commits through
the same `replace_text_in_range(None, text)` without composition identity, so the
consumer cannot distinguish them. Commits while selection is active remain
rejected. This is the settled P8 input contract, not a missing stale-commit check.

Native edit actions use the SDK's scoped origin, independent of Command mapping
and shortcut priority: Paste starts normal paste; Copy and Cut copy the selection;
Select All is a no-op. Neither edge of a native action sends terminal keys.
Physical Command-C/V retain the runtime Command/priority rule. Selection keeps
native editing focus; text callbacks themselves enforce selection/input isolation.

Physical clipboard chords match exact modifier sets: Command-C copies;
Command-V pastes text (falling back to image); Command-Shift-V pastes only an
image. Command-Shift-C is not a clipboard chord. Adding Option, Control or Function
to any chord bypasses clipboard access and follows the existing terminal path.
The Command: Ctrl plus Terminal priority exclusion still applies to all physical
clipboard chords. Native edit actions retain their independent SDK-origin route.


SH1-156 investigation: the preceding D4 late-commit allowance conflicts with the
new requirement that discarded composition never be sent. A fresh GPUI KeyDown
already clears `TerminalInput.cancelled`, but `send_terminal` still refuses input
while `selection` or `select_mode` is active. `mark` refuses a new composition
while cancelled. The pinned UIKit adapter passes both ordinary input and IME
commit through `replace_text_in_range(None, text)` without origin/epoch. Changing
only the cancelled flag cannot establish the stronger lifetime guarantee. The
ignored SH1-156 adapter regression records the new-composition failure; it is not
an acceptance pass. Resolve the callback-origin contract before changing D4.
