//! R1 and R2: what the renderer adopted becomes the drawn scene, and what the
//! prototype cannot draw is refused whole.
use super::fixtures::{Session, callback, dim, publication, style, with};
use super::*;
use yebisu_mod_wire::{Argument, Component, Op};

fn adopt(nodes: Vec<yebisu_mod_wire::Node>) -> Result<Scene> {
    let mut session = Session::new()?;
    session.adopt(publication(1, 1, nodes)?)
}

#[test]
fn r1_scene_keeps_the_adopted_root_view_order_and_indices() {
    let scene = Fixture::Counter
        .adopt()
        .unwrap_or_else(|e| panic!("counter: {e}"));
    assert_eq!((scene.generation, scene.pub_rev, scene.root), (1, 1, 0));
    let kinds: Vec<Kind> = scene.nodes.iter().map(|n| n.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Column,
            Kind::Text,
            Kind::Row,
            Kind::Button,
            Kind::Button
        ]
    );
    assert_eq!(scene.nodes[0].children, [1, 2]);
    assert_eq!(scene.nodes[2].children, [3, 4]);
    assert_eq!(scene.nodes[1].text.as_deref(), Some("Count 3"));
    assert_eq!(
        (scene.nodes[3].click, scene.nodes[4].click),
        (Some(7), Some(8))
    );
    let root = scene.nodes[0].style;
    assert!(root.fill);
    assert_eq!((root.padding, root.gap), (12., 8.));
    assert_eq!(
        (scene.nodes[3].style.width, scene.nodes[3].style.height),
        (Some(96.), Some(44.))
    );
}

#[test]
fn r1_every_fixture_passes_the_renderers_receiving_rules() {
    for fixture in Fixture::ALL {
        let scene = fixture
            .adopt()
            .unwrap_or_else(|e| panic!("{}: {e}", fixture.name()));
        assert!(scene.node(scene.root).is_some(), "{}", fixture.name());
        assert_eq!(Fixture::from_name(fixture.name()), Ok(fixture));
    }
    assert_eq!(
        Fixture::from_name("missing"),
        Err(Error::UnknownFixture("missing".into()))
    );
}

#[test]
fn r1_a_name_outside_the_reflection_tables_is_refused_by_the_renderer() -> Result<()> {
    let unknown = Op::Callback {
        name: "on_hover".into(),
        id: yebisu_mod_wire::Counter(3),
    };
    let publish = publication(1, 1, vec![with(Component::Div, None, &[], vec![unknown])])?;
    let mut session = Session::new()?;
    // A runner whose tables differ: it encodes what this renderer never listed.
    let mut theirs = tables();
    theirs.callbacks.insert("on_hover".into());
    let bytes = session.encode_with(publish, &theirs)?;
    let result = session.adopt_bytes(&bytes);
    assert!(matches!(result, Err(Error::Renderer(_))), "{result:?}");
    Ok(())
}

#[test]
fn r2_a_component_the_prototype_cannot_draw_refuses_the_scene() {
    let result = adopt(vec![
        with(Component::VFlex, None, &[1], vec![]),
        with(Component::Progress, None, &[], vec![]),
    ]);
    assert_eq!(
        result,
        Err(Error::UnsupportedComponent(Component::Progress))
    );
}

#[test]
fn r2_a_style_argument_of_the_wrong_kind_refuses_the_scene() {
    let text = Op::ParamStyle {
        name: "w".into(),
        args: vec![Argument::Text("wide".into())],
    };
    assert_eq!(
        adopt(vec![with(Component::Div, None, &[], vec![text])]),
        Err(Error::StyleArgument("w".into()))
    );
    let color = Op::ParamStyle {
        name: "bg".into(),
        args: vec![Argument::Number(f64::from(0x0100_0000_u32))],
    };
    assert_eq!(
        adopt(vec![with(Component::Div, None, &[], vec![color])]),
        Err(Error::StyleArgument("bg".into()))
    );
}

#[test]
fn r2_a_refused_publication_clears_the_drawn_scene() {
    let mut state = ModViewState::default();
    state.install(Fixture::Counter.adopt());
    state.focused = Some(3);
    assert!(state.scene.is_some());
    let refused = adopt(vec![with(Component::Progress, None, &[], vec![])]);
    state.install(refused);
    assert!(state.scene.is_none());
    assert_eq!(state.focused, None);
    assert_eq!(
        state.error,
        Some(Error::UnsupportedComponent(Component::Progress))
    );
    assert!(!state.hit(Hit::Tap(0)));
}

#[test]
fn r1_fixture_helpers_name_registered_ops() -> Result<()> {
    let tables = tables();
    assert!(matches!(
        style("size_full")?,
        Op::NullaryStyle { index: 1, .. }
    ));
    assert!(style("flex_row").is_err());
    assert!(tables.param_styles.contains("w"));
    assert!(matches!(dim("h", 2.), Op::ParamStyle { .. }));
    assert!(tables.callbacks.contains("on_click"));
    assert!(matches!(callback("on_click", 1), Op::Callback { .. }));
    Ok(())
}
