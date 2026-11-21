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
        // CoreSimulator can report a keyboard after the device's keyboard
        // forwarding is disabled. Capture jobs opt in only for that launch.
        #[cfg(target_abi = "sim")]
        if std::env::var("YEBISU_QA_KEY_BAND_SOFTWARE_ONLY").as_deref() == Ok("1") {
            return false;
        }
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
