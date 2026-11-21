//! The active physical keyboard comes from Game Controller; the UIKit guide
//! only reports occupied geometry and can still contain a software keyboard.
#![allow(
    unsafe_code,
    reason = "calls the public GCKeyboard class property through Objective-C"
)]

#[cfg(all(target_os = "ios", not(test)))]
#[link(name = "GameController", kind = "framework")]
unsafe extern "C" {}

#[cfg(not(test))]
pub(crate) fn hardware_keyboard_attached() -> bool {
    #[cfg(target_os = "ios")]
    {
        use objc2::{class, msg_send, runtime::AnyObject};
        // SAFETY: GCKeyboard.coalescedKeyboard is a class property returning
        // a nullable object. We only inspect the pointer and never retain it.
        let keyboard: *mut AnyObject = unsafe { msg_send![class!(GCKeyboard), coalescedKeyboard] };
        !keyboard.is_null()
    }
    #[cfg(not(target_os = "ios"))]
    {
        false
    }
}
