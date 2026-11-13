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

The legacy D4 implementation resumes text on explicit Paste, Select-off and
ordinary terminal taps. It cannot identify a late prior IME commit at those
boundaries. This is a known gap against SH1-156, not permission to deliver
discarded text. Do not count the legacy resumption test as SH1-156 acceptance.

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


## SH1-156 partial app repair

A destination/stamp change closes Select mode and removes the old highlight.
A fresh terminal-owned semantic key (for example Ctrl-A or Left) ends selection
and is sent once. Copy/native edit/app shortcuts are resolved first; a repeat
from an earlier press does not end selection. The key does not acknowledge native
composition cancellation, so later injected discarded commits remain fenced.
Pending paste is not cancelled by the key. P7 navigation and P9 destination
checks remain in place; this repair adds no buffering, replay or text comparison.

The handoff strip offers **Typing: Clear highlight** (default) and **Typing: Keep
highlight**. The latter retains a copyable, passive highlight until the frame or
destination changes; it does not block keys or weaken the IME fence. New selection
begins an active input block again. This preference is session-local.

Status distinguishes disconnected/Detached, command barrier, Commands palette,
selection, waiting frame and paused text input. Paused text is not evidence that
all hardware keys are refused. A successful semantic key does not hide a still
active text-input refusal.

Full SH1-156 remains blocked: hardware printable keys can go to UIKit before GPUI
KeyDown, and new software compositions are still rejected while cancelled. At the
pinned SDK, insertText/direct text/IME commit share the replacement callback.
Notification and responder APIs have not established native session retirement.
Injected old callbacks test the consumer fence; they do not show that physical
UIKit delivers those callbacks after cancellation. No blanket impossibility or
mandatory SDK change follows from injection alone. The ignored new-composition
regression remains; original Z1-Z7 acceptance is unchanged.

Run the new real-adapter suite with `cargo test --locked --workspace sh1_156_ --
--nocapture --test-threads=1`. On an exclusively owned Mac checkout, use `just
guarded python3 scripts/check-discard-mutations.py` for mutation negatives.
Compile failures do not count as successful negatives. The script restores each
source and reruns its test. These checks do not establish native UIKit delivery.

### D1 clean-state correction

A clean connect/detach/barrier no longer creates a fictitious composition discard.
Ordinary direct text stays eligible when no actual discarded or rejected mark
requires a fence. Actual discards survive repeated detach, writer failure and
host changes, including marked setup text entering the terminal. Rejected marks
while selected, blurred or behind a barrier also preserve the fence. This does
not clear a real fence on a key and does not implement U1 native retirement.
See [the consumer integration requirements](input-retirement-integration.md).
