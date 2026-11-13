//! Simulator-only probes call real UIKit outside GPUI entity updates.
#![allow(
    unsafe_code,
    reason = "main-thread QA uses public UIKit responder and menu methods"
)]
use objc2::{
    MainThreadMarker, Message, class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::{UICommand, UIEditMenuConfiguration, UIMenu, UIMenuElement};

pub(crate) struct Capture {
    pub window: Retained<AnyObject>,
    pub interaction: Retained<AnyObject>,
    pub configuration: Retained<UIEditMenuConfiguration>,
    pub commands: Retained<NSArray<UIMenuElement>>,
}

unsafe fn descendants(view: &AnyObject, output: &mut Vec<Retained<AnyObject>>) {
    output.push(view.retain());
    let children: Retained<NSArray<AnyObject>> = unsafe { msg_send![view, subviews] };
    for child in children {
        unsafe {
            descendants(&child, output);
        }
    }
}

pub(crate) fn responder(window: &AnyObject) -> Option<String> {
    let mut views = Vec::new();
    unsafe {
        descendants(window, &mut views);
    }
    views
        .iter()
        .find(|view| unsafe { msg_send![&***view, isFirstResponder] })
        .map(|view| view.class().name().to_string_lossy().into_owned())
}

pub(crate) fn capture(token: herdr_uikit::NativeEditMenuToken) -> Option<Capture> {
    let mtm = MainThreadMarker::new()?;
    unsafe {
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        let windows: Retained<NSArray<AnyObject>> = msg_send![app, windows];
        let id = NSString::from_str(&format!(
            "{}{}",
            herdr_uikit::EDIT_MENU_IDENTIFIER_PREFIX,
            token.value()
        ));
        let configuration = UIEditMenuConfiguration::configurationWithIdentifier_sourcePoint(
            Some(ProtocolObject::from_ref(&*id)),
            Default::default(),
            mtm,
        );
        for window in windows {
            let mut views = Vec::new();
            descendants(&window, &mut views);
            for view in views {
                let interactions: Retained<NSArray<AnyObject>> = msg_send![&*view, interactions];
                for interaction in interactions {
                    let is_menu: bool =
                        msg_send![&*interaction, isKindOfClass:class!(UIEditMenuInteraction)];
                    if !is_menu {
                        continue;
                    }
                    let delegate: Option<Retained<AnyObject>> = msg_send![&*interaction, delegate];
                    let Some(delegate) = delegate else {
                        continue;
                    };
                    if delegate.class().name().to_bytes() != b"HerdrUIKitEditMenuDelegate" {
                        continue;
                    }
                    let menu: Retained<UIMenu> = msg_send![&*delegate, editMenuInteraction:&*interaction, menuForConfiguration:&*configuration, suggestedActions:&*NSArray::<UIMenuElement>::new()];
                    if menu.children().len() == 2 {
                        return Some(Capture {
                            window,
                            interaction,
                            configuration,
                            commands: menu.children(),
                        });
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn replay(commands: &NSArray<UIMenuElement>) -> Vec<bool> {
    unsafe {
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        commands.iter().filter_map(|item| {
            let command = item.downcast_ref::<UICommand>()?;
            Some(msg_send![app, sendAction:command.action(), to:std::ptr::null::<AnyObject>(), from:command, forEvent:std::ptr::null::<AnyObject>()])
        }).collect()
    }
}

pub(crate) fn delegate(capture: &Capture, owned: bool) -> Option<Retained<NSArray<UIMenuElement>>> {
    let mtm = MainThreadMarker::new()?;
    unsafe {
        let copy = UICommand::commandWithTitle_image_action_propertyList(
            &NSString::from_str("Copy"),
            None,
            sel!(copy:),
            None,
            mtm,
        );
        let paste = UICommand::commandWithTitle_image_action_propertyList(
            &NSString::from_str("Paste"),
            None,
            sel!(paste:),
            None,
            mtm,
        );
        let suggestions =
            NSArray::from_slice(&[&*copy as &UIMenuElement, &*paste as &UIMenuElement]);
        let foreign = UIEditMenuConfiguration::configurationWithIdentifier_sourcePoint(
            Some(ProtocolObject::from_ref(&*NSString::from_str(
                "ordinary-system-menu",
            ))),
            Default::default(),
            mtm,
        );
        let configuration = if owned {
            &*capture.configuration
        } else {
            &*foreign
        };
        let delegate: Retained<AnyObject> = msg_send![&*capture.interaction, delegate];
        let menu: Retained<UIMenu> = msg_send![&*delegate, editMenuInteraction:&*capture.interaction, menuForConfiguration:configuration, suggestedActions:&*suggestions];
        Some(menu.children())
    }
}

pub(crate) fn external_round_trip(capture: &Capture) -> serde_json::Value {
    unsafe {
        let before = responder(&capture.window);
        let field: Retained<AnyObject> = msg_send![class!(UITextField), new];
        let _: () = msg_send![&*capture.window, addSubview:&*field];
        let acquired: bool = msg_send![&*field, becomeFirstResponder];
        let during = responder(&capture.window);
        let released: bool = msg_send![&*field, resignFirstResponder];
        let _: () = msg_send![&*field, removeFromSuperview];
        serde_json::json!({"before":before,"during":during,"acquired":acquired,"released":released})
    }
}
