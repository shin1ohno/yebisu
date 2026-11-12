# P12 observation boundary (attempt 3)

This source stage is QA-only. It is not native acceptance. The production catalog, menu routing, `NativeMenus::take`, `run_menu_shortcut`, SDK pin, SH1-153 selectors and terminal priority/IME policy are unchanged.

## Identity blocker — stop before semantic repair

Pinned SDK `b2fa7811fa9fad9592f47b06d851fc0c33facb16` primary source:
- `src/platform/menus.rs:96-134`: `Performed` retains a private MenuTag; `shortcut()` returns only Shortcut, while dispatch_action retains the tag.
- `src/menu/shortcut.rs:91-95`: Shortcut contains flags, input and Keystroke, not the captured action.
- `src/window/view.rs:173-184`: handled text-input shortcuts suppress dispatch_action.
- `src/window/text_input/keys.rs:301-337`: the Keyboard branch emits key_down(keystroke, held).
- `src/lib.rs:46-55`: the exported native-edit API is separate; it does not carry this menu identity.

The consumer's `run_menu_shortcut` constructs a NativeCommand with the current revision. A callback reached via the SDK's Keyboard route does not carry the original captured NativeCommand revision. Therefore observation writes origin=shortcut_key_down and captured_revision=null, separately from current_revision. Correlation with an OS pick is timing evidence, not proof of a captured revision or of a physical-key origin. The strict oracle rejects this with an explicit SDK identity blocker. It never substitutes the current revision or permits a missing receipt. No product bug is established by the previous native run.

Proposal (not implemented/authorized here): a scoped SDK callback context that carries the captured action or an opaque resolvable token, the menu generation, and a truthful event source through synchronous key delivery. Preserve ordinary physical-key routing, IME/prefix/priority precedence, native edit selectors, and exactly-once suppression. Specify unknown event source and asynchronous reentrancy/lifetime behavior. A separately authorized SDK review must cover stale/replaced menus, rejection without terminal fallback, consumed/replayed commands, held keys and physical input. Do not change a consumer guard or SDK pin to rescue this QA.

## Observation and setup

Both consumer entry routes log a bounded command name, origin, captured/current revisions, scene/focus/pending/context and before/after local state. The existing central acceptance guard is retained. Early key focus/scene and held-key exits are explicit. Raw key strings, typed text and RPC parameters are not added to P12 logs.

The fixture panel uses GPUI occlude and mouse/click propagation controls already used by settings_spike/sheet.rs. A fixture click prepares state only. Each actual control and OS pick has a timestamp interval; app setup begin/end and command ingress must correlate. Each case records its actual context; a negative must change exactly its intended field. The restoration changes only the injected host/pane/capability field; it never assigns Live, connected, inbox, handle or generation.

The peer records ingress before parsing/gating/responding, plus transport start, one-shot gate release and exit reasons. An unexpected second request remains visible rather than waiting on a dropped sender. No reconnect or replay is introduced. Exactly one pane.split request and zero terminal input are required, followed by the actual final receiving FIFO marker on the same transport after all negatives. Geometry is separately identified; setup zero-wire assertions fail on geometry too (the final fence is the only deliberate control wire).

The runner keeps Cargo/Xcode/tmp outputs below the leased checkout's .git, saves the actual compiler transcript, and checks source/manifest/target plus compiler/bundle/installed UUID and bundle/installed hash bindings. Shared caches may only be read or copied into the owned namespace; never build into a prior target. Existing artifacts are not overwritten.

## Unrun gates

No Rust compilation, native/Simulator run, SSH, Command-hold, physical IME/keyboard, P11/P13 integration or independent review has been performed for this head. Python fixtures and source mutations are not those gates. All M1–M4 remain unverified. Controller must decide the SDK proposal and issue a new exclusive lease before any further native observation; the observation may still fail and cannot grant acceptance with missing identity.
