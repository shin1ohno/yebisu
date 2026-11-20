use crate::shell::style;
use gpui::{
    AnyElement, Bounds, Hsla, PathBuilder, Pixels, Window, canvas, div, point, prelude::*, px, rgb,
};

#[derive(Clone, Copy)]
pub(super) enum RailIcon {
    NewSpace,
    Hosts,
    Settings,
    Expand,
}

// Rail.dc.html:26, :54, :57, :59.
const ICON_SIZE: f32 = 20.0;
const STROKE: f32 = 1.5;
const EXPAND_SIZE: f32 = 16.0;

pub(super) fn icon(kind: RailIcon, enabled: bool) -> AnyElement {
    let color: Hsla = rgb(if enabled {
        style::color::MUTED
    } else {
        style::color::DISABLED
    })
    .into();
    match kind {
        RailIcon::Expand => div()
            .debug_selector(|| "rail-expand-icon".to_owned())
            .text_size(px(EXPAND_SIZE))
            .text_color(color)
            .child("»")
            .into_any_element(),
        _ => div()
            .debug_selector(move || {
                match kind {
                    RailIcon::NewSpace => "rail-new-space-icon",
                    RailIcon::Hosts => "rail-hosts-icon",
                    RailIcon::Settings => "rail-settings-icon",
                    RailIcon::Expand => "rail-expand-icon",
                }
                .to_owned()
            })
            .size(px(ICON_SIZE))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| draw(kind, bounds, color, window),
                )
                .size_full(),
            )
            .into_any_element(),
    }
}

fn draw(kind: RailIcon, bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let at = |x, y| bounds.origin + point(px(x), px(y));
    let mut path = PathBuilder::stroke(px(STROKE));
    match kind {
        RailIcon::NewSpace => {
            for ((x1, y1), (x2, y2)) in [((10., 4.), (10., 16.)), ((4., 10.), (16., 10.))] {
                path.move_to(at(x1, y1));
                path.line_to(at(x2, y2));
            }
        }
        RailIcon::Hosts => {
            for y in [4., 11.] {
                path.move_to(at(4.5, y));
                path.line_to(at(15.5, y));
                path.curve_to(at(17., y + 1.5), at(17., y));
                path.line_to(at(17., y + 3.5));
                path.curve_to(at(15.5, y + 5.), at(17., y + 5.));
                path.line_to(at(4.5, y + 5.));
                path.curve_to(at(3., y + 3.5), at(3., y + 5.));
                path.line_to(at(3., y + 1.5));
                path.curve_to(at(4.5, y), at(3., y));
                path.close();
            }
        }
        RailIcon::Settings => {
            let radius = point(px(2.5), px(2.5));
            path.move_to(at(7.5, 10.));
            path.arc_to(radius, px(0.), false, true, at(12.5, 10.));
            path.arc_to(radius, px(0.), false, true, at(7.5, 10.));
            path.close();
            for ((x1, y1), (x2, y2)) in [
                ((10., 2.5), (10., 4.5)),
                ((10., 15.5), (10., 17.5)),
                ((2.5, 10.), (4.5, 10.)),
                ((15.5, 10.), (17.5, 10.)),
                ((4.7, 4.7), (6.1, 6.1)),
                ((13.9, 13.9), (15.3, 15.3)),
                ((4.7, 15.3), (6.1, 13.9)),
                ((13.9, 6.1), (15.3, 4.7)),
            ] {
                path.move_to(at(x1, y1));
                path.line_to(at(x2, y2));
            }
        }
        RailIcon::Expand => return,
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
