use super::*;
use crate::{input_geometry::PanePlacement, shell::style};
use sheprd_client::protocol::PaneSurfaceFrame;

// Main.dc.html:162,173 and Herdr2.dc.html:96,104.
const NAME_HANG: f32 = 10.;
const NAME_PADDING: f32 = 6.;
const NAME_HEIGHT: f32 = 20.;

impl TerminalPainter {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_framed_pane(
        &mut self,
        surface: &PaneSurfaceFrame,
        placement: PanePlacement<'_>,
        label: &str,
        cell_width: f32,
        font: &Font,
        highlights: &[Highlight],
        assets: &sheprd_client::SurfaceImages,
        slide: Option<&crate::upstream::smooth_scroll::Slide>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let pane = placement.pane;
        let outline = placement.outline;
        let edge = if pane.focused {
            style::color::ACCENT
        } else {
            style::color::SLATE
        };
        window.paint_quad(quad(
            outline,
            px(0.),
            rgb(self.theme.background),
            px(style::metric::LINE),
            rgb(edge),
            BorderStyle::default(),
        ));

        let rect = pane.inner_rect;
        let mut area = (rect.y..rect.y.saturating_add(rect.height))
            .map(|row| Span {
                row,
                columns: rect.x..rect.x.saturating_add(rect.width),
            })
            .collect::<Vec<_>>();
        if let Some(bar) = pane.scrollbar_rect {
            let columns = bar.x..bar.x.saturating_add(bar.width);
            area.extend((bar.y..bar.y.saturating_add(bar.height)).map(|row| Span {
                row,
                columns: columns.clone(),
            }));
            area.sort_unstable_by_key(|span| (span.row, span.columns.start));
        }
        window.with_content_mask(
            Some(ContentMask {
                bounds: placement.body,
            }),
            |window| {
                // Issue #137 PR 4: the pane's content drawn between rows.
                if let Some(slide) = slide {
                    self.paint_sliding(
                        SlidingFrame {
                            frame: &surface.frame,
                            origin: placement.frame_origin,
                            available: None,
                            area: &area,
                            highlights,
                            panes: std::slice::from_ref(pane),
                        },
                        &Sliding::new(slide, &area),
                        cell_width,
                        font,
                        window,
                        cx,
                    );
                    return;
                }
                self.paint_frame_area(
                    &surface.frame,
                    placement.frame_origin,
                    None,
                    Ground::Opaque,
                    cell_width,
                    font,
                    highlights,
                    std::slice::from_ref(pane),
                    Some(&area),
                    None,
                    Some(PlacedImages {
                        placements: &surface.graphics.placements,
                        images: assets,
                        target: ImageTarget::Pane(&pane.pane_id),
                    }),
                    window,
                    cx,
                );
            },
        );

        paint_name(label, outline, pane.focused, window, cx);
    }
}

fn paint_name(
    label: &str,
    outline: Bounds<Pixels>,
    focused: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let color = if focused {
        style::color::ACCENT
    } else {
        style::color::MUTED
    };
    let label: SharedString = label
        .chars()
        .filter(|ch| !ch.is_control())
        .take(80)
        .collect::<String>()
        .into();
    let mut font = window.text_style().font();
    font.weight = style::weight::BOLD;
    let line = window.text_system().shape_line(
        label.clone(),
        px(style::text::NAME),
        &[TextRun {
            len: label.len(),
            font,
            color: rgb(color).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    let origin = outline.origin + point(px(style::metric::PANE_NAME_INSET), px(-NAME_HANG));
    let width = (line.width + px(2. * NAME_PADDING)).min(outline.right() - origin.x);
    let background = Bounds::new(origin, size(width, px(NAME_HEIGHT)));
    window.paint_quad(fill(background, rgb(style::color::BACKGROUND)));
    window.with_content_mask(Some(ContentMask { bounds: background }), |window| {
        let _ = line.paint(
            origin + point(px(NAME_PADDING), px(0.)),
            px(NAME_HEIGHT),
            TextAlign::Left,
            None,
            window,
            cx,
        );
    });
}
