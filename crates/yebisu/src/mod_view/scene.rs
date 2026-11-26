//! An adopted publication's root view as the closed set of boxes this
//! prototype draws. GPUI-free, so it is built on a background task.
use super::{
    Error, Result,
    tables::{ON_CLICK, ON_KEY_DOWN},
};
use yebisu_mod_wire as wire;

/// The components the prototype draws; every other one is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Div,
    /// `HFlex`: children left to right.
    Row,
    /// `VFlex`: children top to bottom.
    Column,
    Text,
    Button,
}

/// The style subset, in points and 0xRRGGBB.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Style {
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
    pub(crate) padding: f32,
    pub(crate) gap: f32,
    pub(crate) grow: bool,
    pub(crate) fill: bool,
    pub(crate) full_width: bool,
    pub(crate) center: bool,
    pub(crate) background: Option<u32>,
    pub(crate) color: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SceneNode {
    pub(crate) kind: Kind,
    pub(crate) text: Option<String>,
    pub(crate) style: Style,
    /// Callback IDs; the event's target is the node index (`Action.target`).
    pub(crate) click: Option<u64>,
    pub(crate) key: Option<u64>,
    pub(crate) children: Vec<usize>,
}

/// One drawable root view. Node indices are the wire's, so an event target
/// names the same node the runner published.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scene {
    pub(crate) generation: u64,
    pub(crate) pub_rev: u64,
    pub(crate) root: usize,
    pub(crate) nodes: Vec<SceneNode>,
}

impl Scene {
    /// The first view of `publish` (its root view: the prototype draws no
    /// ChildView). `publish` must already be adopted by `CopyReceiver`, which
    /// checked indices, cycles and limits.
    pub(crate) fn from_publish(publish: &wire::Publish) -> Result<Self> {
        let view = publish.views_full.first().ok_or(Error::NoView)?;
        let nodes = view.nodes.iter().map(node).collect::<Result<Vec<_>>>()?;
        Ok(Self {
            generation: view.generation.0,
            pub_rev: publish.pub_rev.0,
            root: view.root,
            nodes,
        })
    }

    pub(crate) fn node(&self, index: usize) -> Option<&SceneNode> {
        self.nodes.get(index)
    }
}

fn kind(component: wire::Component) -> Result<Kind> {
    Ok(match component {
        wire::Component::Div => Kind::Div,
        wire::Component::HFlex => Kind::Row,
        wire::Component::VFlex => Kind::Column,
        wire::Component::Text => Kind::Text,
        wire::Component::Button => Kind::Button,
        other => return Err(Error::UnsupportedComponent(other)),
    })
}

fn node(wire: &wire::Node) -> Result<SceneNode> {
    let mut out = SceneNode {
        kind: kind(wire.component)?,
        text: wire.text.clone(),
        style: Style::default(),
        click: None,
        key: None,
        children: wire.children.clone(),
    };
    for op in &wire.ops {
        apply(&mut out, op)?;
    }
    Ok(out)
}

fn apply(node: &mut SceneNode, op: &wire::Op) -> Result<()> {
    let style = &mut node.style;
    match op {
        wire::Op::NullaryStyle { name, .. } => match name.as_str() {
            "flex_1" => style.grow = true,
            "size_full" => style.fill = true,
            "items_center" => style.center = true,
            "w_full" => style.full_width = true,
            other => return Err(Error::UnsupportedOp(other.into())),
        },
        wire::Op::ParamStyle { name, args } => match name.as_str() {
            "w" => style.width = Some(dimension(name, args)?),
            "h" => style.height = Some(dimension(name, args)?),
            "p" => style.padding = dimension(name, args)?,
            "gap" => style.gap = dimension(name, args)?,
            "bg" => style.background = Some(color(name, args)?),
            "text_color" => style.color = Some(color(name, args)?),
            other => return Err(Error::UnsupportedOp(other.into())),
        },
        wire::Op::Callback { name, id } => match name.as_str() {
            ON_CLICK => node.click = Some(id.0),
            ON_KEY_DOWN => node.key = Some(id.0),
            other => return Err(Error::UnsupportedOp(other.into())),
        },
        wire::Op::Method { name, .. }
        | wire::Op::ActionCallback { name, .. }
        | wire::Op::Slot { name, .. } => return Err(Error::UnsupportedOp(name.clone())),
        wire::Op::StateStyle { name, .. } => {
            return Err(Error::UnsupportedOp(format!("{name:?}")));
        }
    }
    Ok(())
}

fn dimension(name: &str, args: &[wire::Argument]) -> Result<f32> {
    match args {
        // The wire checked it is finite and positive.
        [wire::Argument::Dimension(v)] => Ok(*v as f32),
        _ => Err(Error::StyleArgument(name.into())),
    }
}

fn color(name: &str, args: &[wire::Argument]) -> Result<u32> {
    match args {
        [wire::Argument::Number(v)] if (0.0..=f64::from(0xFF_FFFF_u32)).contains(v) => {
            Ok(*v as u32)
        }
        _ => Err(Error::StyleArgument(name.into())),
    }
}
