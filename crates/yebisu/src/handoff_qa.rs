//! Simulator-only adapters exercise production handoff against the private peer.
use crate::app::Smoke;
use gpui::{AnyElement, Context, div, prelude::*, rgb};
impl Smoke {
    pub(crate) fn handoff_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_HANDOFF_QA")?;
        Some(div().flex().gap_2().px_4().children([("qa-image","QA Image"),("qa-image-only","QA Image only")].into_iter().map(|(id,label)|div().id(id).p_2().bg(rgb(0x3b4252)).child(label).children(if id == "qa-image-only" { crate::qa_ax::mark(crate::qa_ax::ids::BAND_IMAGE_ONLY, label) } else { crate::qa_ax::mark(crate::qa_ax::ids::BAND_IMAGE, label) }).on_click(cx.listener(move |this,_,_,cx| {
            this.handoff.qa_image_ready=false;
            let work=cx.background_executor().spawn(async move {
                let mut state=1u32;
                let image=image::RgbaImage::from_fn(2200,2200,|_,_| {
                    let mut c=[0;4];
                    for b in &mut c {state^=state<<13;state^=state>>17;state^=state<<5;*b=state as u8;}
                    image::Rgba(c)
                });
                let mut bytes=std::io::Cursor::new(Vec::new());
                image::DynamicImage::ImageRgba8(image).write_to(&mut bytes,image::ImageFormat::Png).map(|()|bytes.into_inner())
            });
            cx.spawn(async move |weak,cx| {
                let result=work.await;
                let _=weak.update(cx,|this,cx| {
                    if let Ok(bytes)=result {
                        this.observation.record(serde_json::json!({"kind":"p8-image-source","bytes":bytes.len(),"width":2200,"height":2200}));
                        seed_representation(&bytes, id == "qa-image-only");
                        this.handoff.qa_image_ready=true;
                        crate::qa_ax::core::count("qa_images");
                        this.terminal.live.status="QA image ready".into(); cx.notify();
                    }
                });
            }).detach();
            cx.notify();
        })))).child(if self.handoff.qa_image_ready { "QA image ready" } else { "" }).into_any_element())
    }
}

// GPUI deliberately collapses a mixed item to text. Seed UIKit representations
// directly so the real image-only acquisition must ignore a present text value.
#[allow(
    unsafe_code,
    reason = "Simulator QA writes the pasteboard on the UI thread"
)]
fn seed_representation(bytes: &[u8], image_only: bool) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    let text = NSString::from_str("public.utf8-plain-text");
    let png = NSString::from_str("public.png");
    let string = NSData::with_bytes(b"P8-shadow");
    let image = NSData::with_bytes(bytes);
    let item = if image_only {
        NSDictionary::from_slices(&[&*png], &[&*image as &AnyObject])
    } else {
        NSDictionary::from_slices(
            &[&*text, &*png],
            &[&*string as &AnyObject, &*image as &AnyObject],
        )
    };
    let items = NSArray::from_slice(&[&*item]);
    // SAFETY: GPUI callback runs on UIKit main thread; values are immutable data.
    unsafe {
        objc2_ui_kit::UIPasteboard::generalPasteboard().setItems(&items);
    }
}
