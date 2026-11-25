//! The SDK's inline field behind the mirror types (iOS only).
use super::{
    FieldControl, FieldDismiss, FieldError, FieldEvent, FieldEvents, FieldRequest, FieldRestored,
    Presented, normalize::MAX_CHARS,
};
use crate::qa_ax::ids as qa_ax_ids;
use futures::Stream;
use gpui::{AnyWindowHandle, Bounds, Pixels};
use herdr_uikit::{
    NativeInlineDismissReason as Dismiss, NativeInlineFieldError, NativeInlineFieldEvent,
    NativeInlineFieldEvents, NativeInlineFieldHandle, NativeInlineFieldIdentifiers,
    NativeInlineFieldRequest, NativeInlineFieldStyle, RestoredResponder,
};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub(super) fn present(
    window: AnyWindowHandle,
    request: FieldRequest,
) -> Result<Presented, FieldError> {
    let request = NativeInlineFieldRequest {
        frame: request.frame,
        clip: request.clip,
        text: request.text,
        select_all: true,
        placeholder: request.placeholder,
        accessibility_label: request.accessibility_label,
        allow_empty: request.allow_empty,
        max_chars: MAX_CHARS,
        empty_hint: super::EMPTY_HINT.into(),
        length_hint: super::LENGTH_HINT.into(),
        style: NativeInlineFieldStyle {
            font_size: request.font_size,
            font_weight: request.font_weight,
            // The rest is yebisu's look (shell/style.rs): the SDK default.
            ..NativeInlineFieldStyle::default()
        },
        identifiers: NativeInlineFieldIdentifiers {
            field: qa_ax_ids::RENAME_FIELD.into(),
            overlay: qa_ax_ids::RENAME_OVERLAY.into(),
            hint: qa_ax_ids::RENAME_HINT.into(),
        },
    };
    let (handle, events) =
        herdr_uikit::present_native_inline_field(window, request).map_err(error)?;
    Ok((Box::new(Control(handle)), Box::new(Events(events))))
}

struct Control(NativeInlineFieldHandle);

impl FieldControl for Control {
    fn set_frame(
        &self,
        frame: Bounds<Pixels>,
        clip: Option<Bounds<Pixels>>,
    ) -> Result<(), FieldError> {
        self.0.set_frame(frame, clip).map_err(error)
    }
    fn dismiss(&self) -> Result<(), FieldError> {
        self.0.dismiss().map_err(error)
    }
}

struct Events(NativeInlineFieldEvents);

impl FieldEvents for Events {
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<FieldEvent>> {
        Pin::new(&mut self.0)
            .poll_next(cx)
            .map(|event| event.map(convert))
    }
}

fn error(error: NativeInlineFieldError) -> FieldError {
    match error {
        NativeInlineFieldError::NotMainThread => FieldError::NotMainThread,
        NativeInlineFieldError::WindowUnavailable => FieldError::WindowUnavailable,
        NativeInlineFieldError::AlreadyPresented => FieldError::AlreadyPresented,
        NativeInlineFieldError::RetirementInProgress => FieldError::RetirementInProgress,
        NativeInlineFieldError::Ended => FieldError::Ended,
    }
}

fn convert(event: NativeInlineFieldEvent) -> FieldEvent {
    match event {
        NativeInlineFieldEvent::Opened => FieldEvent::Opened,
        NativeInlineFieldEvent::Committed(text) => FieldEvent::Committed(text),
        NativeInlineFieldEvent::Dismissed(reason) => FieldEvent::Dismissed(match reason {
            Dismiss::Committed => FieldDismiss::Committed,
            Dismiss::Escape => FieldDismiss::Escape,
            Dismiss::OutsideTap => FieldDismiss::OutsideTap,
            Dismiss::PresentationFailed(_) => FieldDismiss::Failed,
            Dismiss::Requested => FieldDismiss::Requested,
            Dismiss::Unchanged => FieldDismiss::Unchanged,
            Dismiss::WindowClosed => FieldDismiss::WindowClosed,
        }),
        NativeInlineFieldEvent::InputRestored(restored) => {
            FieldEvent::InputRestored(match restored {
                RestoredResponder::Text => FieldRestored::Text,
                RestoredResponder::Drawing => FieldRestored::Drawing,
                RestoredResponder::WindowClosed => FieldRestored::WindowClosed,
                RestoredResponder::None => FieldRestored::None,
            })
        }
    }
}
