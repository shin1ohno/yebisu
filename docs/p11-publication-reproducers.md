# P11 F19 A/B0 test-only reproducers

Status: authored, UNRUN. SAME final attempt 3; no production remedy, helper adoption or new lease.
The original zero-terminal-traffic obligation remains unchanged. Source inspection identifies a real race; these tests have not yet demonstrated it at runtime.

`settings_sheet::publication_reproducers::f19_a_after_compare_reproducer` holds inside the successful consumer epoch/destination comparison immediately before the actual CoreTerminal input call. The existing before-comparison PauseInput is unchanged.
`settings_sheet::publication_reproducers::f19_b0_after_dequeue_reproducer` uses public Core::over at pinned core SDK 398c132a06723a0cc1494842a34666d173ec0200. It holds the selected semantic input's complete encoded frame before its first underlying write, after SDK dequeue. Its epoch comes from the producer's separately armed identity, never from current state at dequeue.
Both are `#[ignore]`: the final assertion requires only the same-connection positive Data and rejects every later terminal Data. Current production is expected to FAIL that assertion; observing a leak is not product acceptance. Setup errors, missing positive/arrival, timeouts or an incomplete transcript are invalid/unverified runs, not evidence of the product assertion failing or of zero leakage.

The real text input handler sends the positive, selected text and a queued tail. The real begin_settings_modal clears composition and advances the consumer input epoch; only then does the fixture release the exact case/run/channel/destination/epoch while the modal stays open. After the ordered consumer Finish command, Core::list(true) supplies a read-only FIFO marker. Worker return, test-owned transport Drop, peer EOF and joined full transcript are all required. Drop is fixture teardown after the marker, not a proposed disconnect-based product fence. No elapsed quiet period or list reply alone proves PTY drain. B0's extra tail may or may not already be SDK-queued; this fixture does not certify Bqueued, Bpartial, Breopen, multi-chunk paste or native F19 coverage.

The observer handles the plain JSON hello separately; channel0 Data is RPC. It validates public HEADER/Kind/MAX_PAYLOAD and the pinned writer's complete-frame initial write, then requires exactly the saved frame's remaining suffix on every short write. Short header/payload writes and Interrupted retries are injected on the disposable Unix socketpair. Zero writes, wrong channel/payload/release key, truncated/coalesced initial writes, unknown kind and oversized frames fail setup. State is bounded (one current frame, 512 journal rows, 128 received frames, 64KiB terminal fixture data); waits and I/O have timeouts. The 20s observer/peer deadline is checked between operations, with bounded per-I/O waits; it is not a certified hard 20s wall-clock cap.
Seven `core::publication_probe::transport_tests::*` tests validate this host-shaped observer, not product acceptance. They are ordinary unit tests in future normal test gates; all seven are UNRUN here. A/B0 remain excluded from normal `just test`, whose success would therefore not certify either reproducer.

## Future entry commands (explicit controller lease required; not run)

On a supported Mac checkout of the exact reviewed clean commit, retain git status, source hashes, compiler inputs, target, SDK identity, full command/stdout/stderr and build receipt. `YEBISU_PROBE_SOURCE_HEAD` is required at compile time, but is only a label: external source/build binding is still mandatory. Use one build at a time and the existing disk guard. These are host GPUI tests, not Simulator or physical acceptance:

```sh
YEBISU_PROBE_SOURCE_HEAD="$(git rev-parse HEAD)" just guarded cargo test --locked -p yebisu --bin yebisu settings_sheet::publication_reproducers::f19_a_after_compare_reproducer -- --ignored --exact --nocapture --test-threads=1
YEBISU_PROBE_SOURCE_HEAD="$(git rev-parse HEAD)" just guarded cargo test --locked -p yebisu --bin yebisu settings_sheet::publication_reproducers::f19_b0_after_dequeue_reproducer -- --ignored --exact --nocapture --test-threads=1
just guarded cargo test --locked -p yebisu --bin yebisu core::publication_probe::transport_tests:: -- --nocapture --test-threads=1
```

A receipt includes the exact source label, SDK commit, case/run and destination/generation/input epoch, writer sequence and accepted offsets, ordered local publication and modal observations, all received frames and final settlement status. The final zero-leak assertion must be evaluated after that transcript. `product_acceptance:false` prevents a diagnostic receipt from becoming an all-pass claim. No remote host, auth, key lookup/signing or clipboard access is required.

The separate task-local bounded pre-modal transport plan remains a proposal for independent review and owner decision, including the definition of partial-frame commitment and pending-entry UX. No production wrapper, in-flight policy, replay queue, Herdr or SDK modification is included in this source change.
