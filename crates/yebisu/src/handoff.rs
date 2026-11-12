//! Local selection and a bounded asynchronous paste barrier.
use crate::{
    terminal_input::{Destination, GeneratedInput},
    terminal_painter::{Highlight, Tint},
};
use sheprd_client::{
    clipboard::ClipboardImageCancellation,
    protocol::{ClientPaneInputEvent, FrameData},
};
use std::{collections::VecDeque, sync::Arc};

#[path = "handoff_cells.rs"]
mod cells;

pub(crate) const TEXT_LIMIT: usize = 2 * 1024 * 1024;
pub(crate) const IMAGE_LIMIT: usize = 128 * 1024 * 1024;
pub(crate) const PIXEL_LIMIT: u64 = 64 * 1024 * 1024;
pub(crate) const EVENT_LIMIT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub selection: u64,
    pub destination: Destination,
}
#[derive(Default)]
pub(crate) struct Handoff {
    #[cfg(test)]
    pub clipboard_reads: usize,
    #[cfg(test)]
    pub clipboard_writes: usize,
    pub stamp: Option<Stamp>,
    pub selection: Option<Selection>,
    pub select_mode: bool,
    pub passive_highlight: bool,
    pub keep_highlight_on_key: bool,
    pub drag_select: bool,
    pub copy_on_select: bool,
    pub serial: u64,
    pub preparing: bool,
    // Count workers even after cancellation, so repeated switching stays bounded.
    pub workers: usize,
    pub cancellation: Option<ClipboardImageCancellation>,
    pub held: VecDeque<GeneratedInput>,
    pub refused_tail: bool,
}
impl Handoff {
    pub fn sync(&mut self, stamp: Option<Stamp>) {
        if self.stamp != stamp {
            self.cancel();
            self.selection = None;
            self.select_mode = false; // A selection belongs to one destination.
            self.passive_highlight = false;
            self.stamp = stamp;
        }
    }
    pub fn blocks_input(&self) -> bool {
        self.select_mode || (self.selection.is_some() && !self.passive_highlight)
    }
    /// Only a fresh terminal-owned key exits selection; paste FIFO is untouched.
    pub fn resume_typing(&mut self) {
        self.select_mode = false;
        self.passive_highlight = self.keep_highlight_on_key;
        if self.keep_highlight_on_key {
            if let Some(selection) = &mut self.selection {
                selection.dragging = false;
            }
        } else {
            self.selection = None;
        }
    }
    pub fn cancel(&mut self) {
        if let Some(c) = self.cancellation.take() {
            c.cancel();
        }
        self.serial = self.serial.wrapping_add(1);
        self.preparing = false;
        self.refused_tail = false;
        self.held.clear();
    }
    pub fn hold(&mut self, event: GeneratedInput) -> bool {
        if self.refused_tail {
            return false;
        }
        if self.held.len() >= EVENT_LIMIT {
            self.cancel();
            return false;
        }
        self.held.push_back(event);
        true
    }
}

pub(crate) struct Selection {
    pub frame: Arc<FrameData>,
    pub anchor: (u16, u16),
    pub end: (u16, u16),
    pub dragging: bool,
}
impl Selection {
    fn row(&self, row: u16) -> &[sheprd_client::protocol::CellData] {
        let start = usize::from(row) * usize::from(self.frame.width);
        self.frame
            .cells
            .get(start..start + usize::from(self.frame.width))
            .unwrap_or(&[])
    }

    pub fn spans(&self) -> Vec<(u16, std::ops::Range<u16>)> {
        let (mut a, mut b) = if (self.anchor.1, self.anchor.0) <= (self.end.1, self.end.0) {
            (self.anchor, self.end)
        } else {
            (self.end, self.anchor)
        };
        let start = cells::continuations(self.row(a.1));
        let end = cells::continuations(self.row(b.1));
        while a.0 > 0 && start.get(usize::from(a.0)).copied().unwrap_or(false) {
            a.0 -= 1;
        }
        while b.0.saturating_add(1) < self.frame.width
            && end.get(usize::from(b.0 + 1)).copied().unwrap_or(false)
        {
            b.0 += 1;
        }
        (a.1..=b.1)
            .map(|row| {
                (
                    row,
                    (if row == a.1 { a.0 } else { 0 })..(if row == b.1 {
                        b.0.saturating_add(1)
                    } else {
                        self.frame.width
                    }),
                )
            })
            .collect()
    }
    pub fn text(&self) -> Option<String> {
        let mut text = String::new();
        for (row, cols) in self.spans() {
            if !text.is_empty() {
                text.push('\n');
            }
            for (col, _, cell) in cells::graphemes(self.row(row)) {
                if !cols.contains(&(col as u16)) {
                    continue;
                }
                if text.len().saturating_add(cell.symbol.len()) > TEXT_LIMIT {
                    return None;
                }
                text.push_str(&cell.symbol);
            }
        }
        Some(text)
    }
    pub fn highlights(&self) -> Vec<Highlight> {
        self.spans()
            .into_iter()
            .map(|(row, columns)| Highlight {
                row,
                columns,
                tint: Tint::Selection,
            })
            .collect()
    }
}

pub(crate) enum Payload {
    Text(String),
    Image(Vec<u8>),
}
pub(crate) enum Prepared {
    Text(String),
    Image(Vec<u8>),
}
pub(crate) fn prepare(payload: Payload) -> crate::Result<Prepared> {
    let fail = |s: &str| crate::Error::Ssh(format!("Paste refused: {s}"));
    match payload {
        Payload::Text(text) if text.len() <= TEXT_LIMIT => Ok(Prepared::Text(text)),
        Payload::Text(_) => Err(fail("text exceeds 2 MiB")),
        Payload::Image(bytes) => {
            if bytes.len() > IMAGE_LIMIT {
                return Err(fail("image exceeds 128 MiB"));
            }
            let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()
                .map_err(|_| fail("image format"))?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(limits);
            let decoder = reader.into_decoder().map_err(|_| fail("invalid image"))?;
            use image::ImageDecoder;
            let (w, h) = decoder.dimensions();
            if u64::from(w) * u64::from(h) > PIXEL_LIMIT {
                return Err(fail("image exceeds 64 Mi pixels"));
            }
            let mut image =
                image::DynamicImage::from_decoder(decoder).map_err(|_| fail("image decode"))?;
            loop {
                let mut out = std::io::Cursor::new(Vec::new());
                image
                    .write_to(&mut out, image::ImageFormat::Png)
                    .map_err(|_| fail("image encode"))?;
                let bytes = out.into_inner();
                if bytes.len() <= sheprd_client::protocol::MAX_CLIPBOARD_IMAGE_PAYLOAD {
                    return Ok(Prepared::Image(bytes));
                }
                let w = (image.width() / 2).max(1);
                let h = (image.height() / 2).max(1);
                if w == 1 && h == 1 {
                    return Err(fail("image cannot fit wire limit"));
                }
                image = image.resize(w, h, image::imageops::FilterType::Triangle);
            }
        }
    }
}
impl Prepared {
    pub fn publish(
        self,
        upload: sheprd_client::clipboard::ClipboardImageUpload,
    ) -> sheprd_client::Result<()> {
        match self {
            Self::Text(text) => upload.complete_input(ClientPaneInputEvent::Paste(text)),
            Self::Image(bytes) => upload.complete("png", bytes),
        }
    }
}
#[cfg(test)]
#[path = "handoff_tests.rs"]
mod tests;
