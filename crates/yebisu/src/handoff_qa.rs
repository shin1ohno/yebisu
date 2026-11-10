//! Simulator-only adapters exercise production handoff against the private peer.
use crate::app::Smoke;
use gpui::{AnyElement, ClipboardItem, Context, div, prelude::*, rgb};
impl Smoke {
    pub(crate) fn handoff_qa_controls(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        std::env::var_os("YEBISU_HANDOFF_QA")?;
        Some(div().flex().gap_2().px_4().children([("qa-text","QA Text"),("qa-image","QA Image"),("qa-select","QA Selection")].into_iter().map(|(id,label)|div().id(id).p_2().bg(rgb(0x3b4252)).child(label).on_click(cx.listener(move |this,_,window,cx| {
            match id {
                "qa-text"=>cx.write_to_clipboard(ClipboardItem::new_string("P8-text-かな".into())),
                "qa-image"=> {
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
                                seed_dual_representation(&bytes);
                                this.terminal.live.status="QA image ready".into(); cx.notify();
                            }
                        });
                    }).detach();
                },
                "qa-select"=> {
                    let g=this.terminal.geometry;
                    let at=|x:f32|g.grid.origin+gpui::point(gpui::px((x+0.5)*g.cell_width),gpui::px(2.5*g.cell_height));
                    this.handoff.select_mode=true;
                    this.selection_down(&gpui::MouseDownEvent {position:at(1.),button:gpui::MouseButton::Left,click_count:1,modifiers:Default::default(),first_mouse:false},window,cx);
                    this.selection_move(&gpui::MouseMoveEvent {position:at(6.),pressed_button:Some(gpui::MouseButton::Left),modifiers:Default::default()},cx);
                    this.selection_up(cx); this.copy_selection(cx);
                    let text=cx.read_from_clipboard().and_then(|item|item.text());
                    this.observation.record(serde_json::json!({"kind":"p8-selection-copy","text":text}));
                }, _=>{}
            }
            cx.notify();
        })))).into_any_element())
    }
}

// GPUI deliberately collapses a mixed item to text. Seed UIKit representations
// directly so the real image-only acquisition must ignore a present text value.
#[allow(
    unsafe_code,
    reason = "Simulator QA writes the pasteboard on the UI thread"
)]
fn seed_dual_representation(bytes: &[u8]) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    let text = NSString::from_str("public.utf8-plain-text");
    let png = NSString::from_str("public.png");
    let string = NSData::with_bytes(b"P8-shadow");
    let image = NSData::with_bytes(bytes);
    let item = NSDictionary::from_slices(
        &[&*text, &*png],
        &[&*string as &AnyObject, &*image as &AnyObject],
    );
    let items = NSArray::from_slice(&[&*item]);
    // SAFETY: GPUI callback runs on UIKit main thread; values are immutable data.
    unsafe {
        objc2_ui_kit::UIPasteboard::generalPasteboard().setItems(&items);
    }
}
