use gpui::{AnyElement, PathBuilder, canvas, point, prelude::*, px, rgb};

use crate::shell::style::color;

#[derive(Clone, Copy)]
pub(in crate::shell) enum Icon {
    NewTab,
    Split,
    More,
    Keyboard,
    Cancel,
    Mods,
    PopoverArrow,
}

impl Icon {
    // Canvas icon sizes: Main.dc.html:149–158, :185–189; Selection.dc.html:92;
    // ModsMax.dc.html:191; Shortcuts.dc.html:63.
    pub(in crate::shell) fn size(self) -> (f32, f32) {
        match self {
            Self::NewTab => (16., 16.),
            Self::Split | Self::More | Self::Keyboard | Self::Mods => (18., 18.),
            Self::Cancel => (14., 14.),
            Self::PopoverArrow => (20., 10.),
        }
    }

    pub(in crate::shell) fn element(self, ink: u32) -> AnyElement {
        let (width, height) = self.size();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / 20.;
                let at = |x, y| bounds.origin + point(px(x * scale), px(y * scale));
                let stroke = |width| PathBuilder::stroke(px(width * scale));
                let mut path = match self {
                    Self::More | Self::PopoverArrow => PathBuilder::fill(),
                    Self::Keyboard | Self::Mods => stroke(1.4),
                    Self::Split => stroke(1.5),
                    Self::NewTab => stroke(1.6),
                    Self::Cancel => stroke(1.6 * 20. / 14.),
                };
                let mut lines = |segments: &[&[(f32, f32)]]| {
                    for segment in segments {
                        if let Some((&(x, y), rest)) = segment.split_first() {
                            path.move_to(at(x, y));
                            for &(x, y) in rest {
                                path.line_to(at(x, y));
                            }
                        }
                    }
                };
                match self {
                    Self::NewTab => lines(&[&[(10., 4.), (10., 16.)], &[(4., 10.), (16., 10.)]]),
                    Self::Split => lines(&[
                        &[(3., 4.), (17., 4.), (17., 16.), (3., 16.), (3., 4.)],
                        &[(10., 4.), (10., 16.)],
                    ]),
                    Self::More => {
                        for x in [5., 10., 15.] {
                            let points = std::array::from_fn::<_, 12, _>(|index| {
                                let angle = index as f32 * std::f32::consts::TAU / 12.;
                                at(x + 1.4 * angle.cos(), 10. + 1.4 * angle.sin())
                            });
                            path.add_polygon(&points, true);
                        }
                    }
                    Self::Keyboard => lines(&[
                        &[(2., 5.), (18., 5.), (18., 15.), (2., 15.), (2., 5.)],
                        &[(5., 8.), (6., 8.)],
                        &[(8., 8.), (9., 8.)],
                        &[(11., 8.), (12., 8.)],
                        &[(14., 8.), (15., 8.)],
                        &[(6., 12.), (14., 12.)],
                    ]),
                    Self::Cancel => {
                        lines(&[&[(4.3, 4.3), (15.7, 15.7)], &[(15.7, 4.3), (4.3, 15.7)]])
                    }
                    Self::Mods => {
                        for (x, y) in [(3., 3.), (11., 3.), (3., 11.), (11., 11.)] {
                            lines(&[&[(x, y), (x + 6., y), (x + 6., y + 6.), (x, y + 6.), (x, y)]]);
                        }
                    }
                    Self::PopoverArrow => {
                        lines(&[&[(0., 0.), (10., 10.), (20., 0.)]]);
                        path.close();
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(ink));
                }
            },
        )
        .flex_none()
        .w(px(width))
        .h(px(height))
        .into_any_element()
    }

    pub(in crate::shell) fn muted(self) -> AnyElement {
        self.element(color::MUTED)
    }
}
