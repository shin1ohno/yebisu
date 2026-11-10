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
