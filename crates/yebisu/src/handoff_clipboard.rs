//! Bounded native acquisition, called only on the GPUI main thread.
use crate::handoff::{IMAGE_LIMIT, Payload, TEXT_LIMIT};
#[cfg(target_os = "ios")]
pub(crate) fn read(image_only: bool, _: &mut gpui::App) -> crate::Result<Payload> {
    use objc2_foundation::{NSArray, NSString};
    use objc2_ui_kit::UIPasteboard;
    let board = UIPasteboard::generalPasteboard();
    let formats = [
        ("public.utf8-plain-text", TEXT_LIMIT),
        ("public.png", IMAGE_LIMIT),
        ("public.jpeg", IMAGE_LIMIT),
        ("com.compuserve.gif", IMAGE_LIMIT),
        ("org.webmproject.webp", IMAGE_LIMIT),
    ];
    for (index, (kind, limit)) in formats.into_iter().enumerate() {
        if index == 0 && image_only {
            continue;
        }
        let kind = NSString::from_str(kind);
        if !board.containsPasteboardTypes(&NSArray::from_slice(&[&*kind])) {
            continue;
        }
        if let Some(data) = board.dataForPasteboardType(&kind) {
            if data.len() > limit {
                return Err(crate::Error::Ssh("Paste exceeds acquisition limit".into()));
            }
            let bytes = data.to_vec();
            return if index == 0 {
                String::from_utf8(bytes)
                    .map(Payload::Text)
                    .map_err(|_| crate::Error::Ssh("Clipboard is not UTF-8".into()))
            } else {
                Ok(Payload::Image(bytes))
            };
        }
    }
    Err(crate::Error::Ssh(
        "No supported clipboard representation".into(),
    ))
}
#[cfg(not(target_os = "ios"))]
pub(crate) fn read(image_only: bool, cx: &mut gpui::App) -> crate::Result<Payload> {
    let _ = (TEXT_LIMIT, IMAGE_LIMIT);
    let item = cx
        .read_from_clipboard()
        .ok_or_else(|| crate::Error::Ssh("Clipboard empty".into()))?;
    if !image_only && let Some(text) = item.text() {
        return Ok(Payload::Text(text));
    }
    item.entries
        .into_iter()
        .find_map(|entry| match entry {
            gpui::ClipboardEntry::Image(image) => Some(Payload::Image(image.bytes.to_vec())),
            _ => None,
        })
        .ok_or_else(|| crate::Error::Ssh("Clipboard has no image".into()))
}
