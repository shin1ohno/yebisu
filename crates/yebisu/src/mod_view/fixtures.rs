//! Fixture publications for the spike. Each one is encoded and adopted by the
//! real `CopyReceiver`, so only what the renderer's receiving rules accept is
//! ever drawn. `YEBISU_MOD_FIXTURE` picks one in a running app (QA).
use super::{Error, Result, Scene, tables};
use yebisu_mod_renderer::{ClientHandshake, CopyReceiver, SharedScopes};
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{Argument, Component, Counter, Op};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fixture {
    /// A count and two buttons (+1, Reset).
    Counter,
    /// Three fixed-width status cells, no callbacks.
    Status,
    /// A label, a key-accepting field and a Send button.
    Form,
    /// A button inside a tappable padded box inside a padded box.
    Nested,
}

impl Fixture {
    pub(crate) const ALL: [Fixture; 4] = [
        Fixture::Counter,
        Fixture::Status,
        Fixture::Form,
        Fixture::Nested,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Fixture::Counter => "counter",
            Fixture::Status => "status",
            Fixture::Form => "form",
            Fixture::Nested => "nested",
        }
    }

    pub(crate) fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|f| f.name() == name)
            .ok_or_else(|| Error::UnknownFixture(name.into()))
    }

    /// The root view's nodes, root first.
    pub(crate) fn nodes(self) -> Result<Vec<wire::Node>> {
        Ok(match self {
            Fixture::Counter => vec![
                with(
                    Component::VFlex,
                    None,
                    &[1, 2],
                    vec![style("size_full")?, dim("p", 12.), dim("gap", 8.)],
                ),
                with(Component::Text, Some("Count 3"), &[], vec![dim("h", 24.)]),
                with(
                    Component::HFlex,
                    None,
                    &[3, 4],
                    vec![dim("gap", 8.), dim("h", 44.)],
                ),
                button("+1", 96., 44., 7),
                button("Reset", 96., 44., 8),
            ],
            Fixture::Status => vec![
                with(
                    Component::HFlex,
                    None,
                    &[1, 2, 3],
                    vec![
                        style("size_full")?,
                        style("items_center")?,
                        dim("p", 4.),
                        dim("gap", 4.),
                    ],
                ),
                cell("CPU 3%"),
                cell("MEM 41%"),
                cell("NET 1.2M"),
            ],
            Fixture::Form => vec![
                with(
                    Component::VFlex,
                    None,
                    &[1, 2, 4],
                    vec![style("size_full")?, dim("p", 8.), dim("gap", 6.)],
                ),
                with(Component::Text, Some("Name"), &[], vec![dim("h", 20.)]),
                with(
                    Component::Div,
                    None,
                    &[3],
                    vec![
                        style("w_full")?,
                        dim("h", 36.),
                        rgb("bg", 0x22_2633),
                        callback(tables::ON_CLICK, 11),
                        callback(tables::ON_KEY_DOWN, 12),
                    ],
                ),
                with(Component::Text, Some("type here"), &[], vec![dim("h", 20.)]),
                button("Send", 80., 36., 13),
            ],
            Fixture::Nested => vec![
                with(
                    Component::Div,
                    None,
                    &[1],
                    vec![style("size_full")?, dim("p", 16.)],
                ),
                with(
                    Component::Div,
                    None,
                    &[2],
                    vec![
                        dim("p", 8.),
                        rgb("bg", 0x1B_1F2A),
                        callback(tables::ON_CLICK, 20),
                    ],
                ),
                button("Deep", 64., 32., 21),
            ],
        })
    }

    /// The fixture as publication `pub_rev` of view generation 1.
    pub(crate) fn publish(self, pub_rev: u64) -> Result<wire::Publish> {
        publication(pub_rev, 1, self.nodes()?)
    }

    /// Encodes, adopts and converts the fixture in a fresh session.
    pub(crate) fn adopt(self) -> Result<Scene> {
        Session::new()?.adopt(self.publish(1)?)
    }
}

/// One renderer session for one copy: the handshake values, the receiver and
/// its host and application ledgers.
pub(crate) struct Session {
    receiver: CopyReceiver,
    context: wire::ReceiveContext,
    tables: wire::ValidationTables,
    host: wire::ResourceLedger,
    app: wire::ResourceLedger,
}

impl Session {
    pub(crate) fn new() -> Result<Self> {
        let tables = tables();
        let limits = wire::NegotiatedLimits::default();
        let nonce = id(9)?;
        let copy = wire::CopyInfo {
            id: "fixture".into(),
            incarnation: id(3)?,
            r#mod: wire::ModIdentity {
                location: wire::ModLocation::HerdrApp,
                name: "fixture".into(),
            },
        };
        let handshake = ClientHandshake::new(
            "ipad".into(),
            nonce.clone(),
            tables.digests()?,
            Vec::new(),
            limits.clone(),
        )?;
        let negotiated = handshake.accept(&wire::HelloAck {
            nonce,
            protocol: wire::PROTOCOL,
            serve: wire::ServeVersion {
                version: "0.1.0".into(),
                commit: "fixture".into(),
            },
            boot: id(1)?,
            epoch: id(2)?,
            limits: limits.clone(),
            copies: vec![copy.clone()],
        })?;
        let context =
            negotiated.context("SHA256:fixture".into(), "ipad".into(), &copy, Counter(1))?;
        Ok(Self {
            receiver: CopyReceiver::new(context.clone(), limits.clone(), tables.clone())?,
            context,
            tables,
            host: wire::ResourceLedger::negotiated(&limits)?,
            app: wire::ResourceLedger::negotiated(&limits)?,
        })
    }

    /// The bytes the line would carry for `publish`.
    pub(crate) fn encode(&self, publish: wire::Publish) -> Result<Vec<u8>> {
        self.encode_with(publish, &self.tables)
    }

    /// The bytes a runner with reflection tables `tables` would send.
    pub(crate) fn encode_with(
        &self,
        publish: wire::Publish,
        tables: &wire::ValidationTables,
    ) -> Result<Vec<u8>> {
        let seq = publish.pub_rev.0;
        let envelope = wire::Envelope::new(
            self.context.session.clone(),
            self.context.scope.clone(),
            self.context.owner_fence,
            format!("message-{seq}"),
            Counter(seq),
            publish,
        );
        Ok(wire::encode_publish(&envelope, tables)?)
    }

    /// Adopts encoded bytes and converts the adopted root view.
    pub(crate) fn adopt_bytes(&mut self, bytes: &[u8]) -> Result<Scene> {
        let mut scopes = SharedScopes {
            host: &mut self.host,
            app: &mut self.app,
        };
        let adopted = self.receiver.adopt_bytes(bytes, &mut scopes)?;
        Scene::from_publish(adopted)
    }

    pub(crate) fn adopt(&mut self, publish: wire::Publish) -> Result<Scene> {
        let bytes = self.encode(publish)?;
        self.adopt_bytes(&bytes)
    }
}

/// A publication whose single view (entity 1) has `nodes`, root 0.
pub(crate) fn publication(
    pub_rev: u64,
    generation: u64,
    nodes: Vec<wire::Node>,
) -> Result<wire::Publish> {
    let view_ref = wire::EntityRef {
        id: Counter(1),
        incarnation: id(3)?,
        kind: wire::EntityKind::View,
    };
    let publish = wire::Publish {
        pub_rev: Counter(pub_rev),
        wm: wire::OpWatermark {
            processed_op_seq: Counter(0),
            state_rev: Counter(pub_rev),
            settled_local_seq: Counter(0),
        },
        entities_full: vec![wire::EntityCreate {
            r#ref: view_ref.clone(),
            rev: Counter(1),
            config: wire::EntityConfig::View {},
            state: wire::EntityState::View {
                parent: None,
                root: 0,
                generation: Counter(generation),
                props_rev: Counter(1),
            },
            subscriptions: wire::Subscriptions {
                rev: Counter(1),
                events: Vec::new(),
            },
        }],
        views_full: vec![wire::ViewSnapshot {
            view_ref,
            generation: Counter(generation),
            root: 0,
            nodes,
            state_rev: Counter(pub_rev),
        }],
        focus_state: Vec::new(),
        overlays_full: Vec::new(),
        list_manifests: Vec::new(),
        asset_manifests: Vec::new(),
    };
    publish.validate_shape()?;
    Ok(publish)
}

fn id(n: u8) -> Result<wire::Id128> {
    Ok(wire::Id128::new(format!("{n:032x}"))?)
}

pub(crate) fn with(
    component: Component,
    text: Option<&str>,
    children: &[usize],
    ops: Vec<Op>,
) -> wire::Node {
    wire::Node {
        component,
        text: text.map(str::to_string),
        entity: None,
        asset: None,
        text_format: None,
        child_view: None,
        list_id: None,
        children: children.to_vec(),
        ops,
    }
}

fn button(label: &str, width: f64, height: f64, click: u64) -> wire::Node {
    with(
        Component::Button,
        Some(label),
        &[],
        vec![
            dim("w", width),
            dim("h", height),
            callback(tables::ON_CLICK, click),
        ],
    )
}

fn cell(label: &str) -> wire::Node {
    with(
        Component::Text,
        Some(label),
        &[],
        vec![dim("w", 80.), dim("h", 28.)],
    )
}

/// A nullary style by its table index.
pub(crate) fn style(name: &str) -> Result<Op> {
    let (index, _) = tables::STYLES
        .iter()
        .find(|(_, n)| *n == name)
        .ok_or_else(|| Error::UnsupportedOp(name.into()))?;
    Ok(Op::NullaryStyle {
        index: *index,
        name: name.into(),
    })
}

pub(crate) fn dim(name: &str, value: f64) -> Op {
    Op::ParamStyle {
        name: name.into(),
        args: vec![Argument::Dimension(value)],
    }
}

fn rgb(name: &str, value: u32) -> Op {
    Op::ParamStyle {
        name: name.into(),
        args: vec![Argument::Number(f64::from(value))],
    }
}

pub(crate) fn callback(name: &str, id: u64) -> Op {
    Op::Callback {
        name: name.into(),
        id: Counter(id),
    }
}
