use super::*;

#[test]
fn developer_buttons_only_in_simulator() {
    assert!(developer_buttons_allowed(true, true));
    assert!(!developer_buttons_allowed(true, false), "device build");
    assert!(!developer_buttons_allowed(false, true));
    assert!(!developer_buttons_allowed(false, false));
    assert!(developer_items(false).is_empty());
    let simulator = developer_items(true);
    assert!(simulator.contains(&"Developer: simulate save failure"));
    assert!(simulator.contains(&"Developer: P0 rendering check"));
    // The build constant follows the build target: only a Simulator build.
    assert_eq!(
        DEVELOPER_BUTTONS,
        cfg!(all(target_os = "ios", target_abi = "sim"))
    );
}
