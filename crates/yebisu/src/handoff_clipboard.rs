//! Bounded native acquisition, called only on the GPUI main thread.
use crate::handoff::{IMAGE_LIMIT, Payload, TEXT_LIMIT};
#[cfg(target_os = "ios")]
pub(crate) fn read(image_only: bool, _: &mut gpui::App) -> crate::Result<Payload> {
    #[cfg(test)]
    if let Some(result) = INJECTED.with(|value| value.borrow_mut().take()) {
        return result;
    }
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
            admit(index, data.len(), limit)?;
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
    #[cfg(test)]
    if let Some(result) = INJECTED.with(|value| value.borrow_mut().take()) {
        return result;
    }
    let item = cx
        .read_from_clipboard()
        .ok_or_else(|| crate::Error::Ssh("No supported clipboard representation".into()))?;
    if !image_only && let Some(text) = item.text() {
        admit(0, text.len(), TEXT_LIMIT)?;
        return Ok(Payload::Text(text));
    }
    let payload = item
        .entries
        .into_iter()
        .find_map(|entry| match entry {
            gpui::ClipboardEntry::Image(image) => Some(Payload::Image(image.bytes.to_vec())),
            _ => None,
        })
        .ok_or_else(|| crate::Error::Ssh("No supported clipboard representation".into()))?;
    if let Payload::Image(bytes) = &payload {
        admit(1, bytes.len(), IMAGE_LIMIT)?;
    }
    Ok(payload)
}

pub(crate) fn admit(_index: usize, len: usize, limit: usize) -> crate::Result<()> {
    if len > limit {
        Err(crate::Error::Ssh("Paste exceeds acquisition limit".into()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
thread_local! { pub(crate) static INJECTED: std::cell::RefCell<Option<crate::Result<Payload>>> = const { std::cell::RefCell::new(None) }; }
