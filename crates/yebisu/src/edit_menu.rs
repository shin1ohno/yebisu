//! Per-window ownership of an explicit native Copy/Paste presentation.
use crate::{
    app::Smoke,
    handoff::{Selection, Stamp},
};
use gpui::{AnyWindowHandle, Context, Pixels, Point, Window};
use sheprd_client::protocol::FrameData;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuToken {
    #[cfg(target_os = "ios")]
    Native(herdr_uikit::NativeEditMenuToken),
    #[cfg(test)]
    Recorded(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Availability {
    pub copy: bool,
    pub cut: bool,
    pub paste: bool,
    pub select_all: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SelectionKey {
    frame: Arc<FrameData>,
    anchor: (u16, u16),
    end: (u16, u16),
}
impl PartialEq for SelectionKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frame, &other.frame)
            && self.anchor == other.anchor
            && self.end == other.end
    }
}
impl From<&Selection> for SelectionKey {
    fn from(selection: &Selection) -> Self {
        Self {
            frame: selection.frame.clone(),
            anchor: selection.anchor,
            end: selection.end,
        }
    }
}

pub(crate) struct EditMenuTicket {
    pub token: MenuToken,
    pub stamp: Option<Stamp>,
    pub selection: Option<SelectionKey>,
}

pub(crate) trait MenuPresenter {
    fn publish(&mut self, window: AnyWindowHandle, availability: Option<Availability>);
    fn present(&mut self, window: AnyWindowHandle, at: Point<Pixels>) -> Option<MenuToken>;
    fn dismiss(&mut self, window: AnyWindowHandle, token: MenuToken);
    /// None means Current; any other result is an irreversible revocation or unknown token.
    fn revoked(&self, window: AnyWindowHandle, token: MenuToken) -> Option<String>;
}

#[derive(Default)]
pub(crate) struct NativePresenter;
impl MenuPresenter for NativePresenter {
    fn publish(&mut self, _window: AnyWindowHandle, _availability: Option<Availability>) {
        #[cfg(target_os = "ios")]
        herdr_uikit::set_native_edit_availability(
            _window,
            _availability.map(|a| herdr_uikit::NativeEditAvailability {
                copy: a.copy,
                cut: a.cut,
                paste: a.paste,
                select_all: a.select_all,
            }),
        );
    }
    fn present(&mut self, _window: AnyWindowHandle, _at: Point<Pixels>) -> Option<MenuToken> {
        #[cfg(target_os = "ios")]
        return herdr_uikit::present_native_edit_menu(_window, _at).map(MenuToken::Native);
        #[cfg(not(target_os = "ios"))]
        None
    }
    fn dismiss(&mut self, _window: AnyWindowHandle, _token: MenuToken) {
        #[cfg(target_os = "ios")]
        match _token {
            MenuToken::Native(token) => {
                herdr_uikit::dismiss_native_edit_menu(_window, token);
            }
            #[cfg(test)]
            MenuToken::Recorded(_) => {}
        }
    }
    fn revoked(&self, _window: AnyWindowHandle, _token: MenuToken) -> Option<String> {
        #[cfg(target_os = "ios")]
        match _token {
            MenuToken::Native(token) => {
                let status = herdr_uikit::native_edit_menu_status(_window, token);
                (status != herdr_uikit::NativeEditMenuStatus::Current)
                    .then(|| format!("{status:?}"))
            }
            #[cfg(test)]
            MenuToken::Recorded(_) => Some("Unknown".into()),
        }
        #[cfg(not(target_os = "ios"))]
        Some("Unknown".into())
    }
}

pub(crate) struct EditMenu {
    pub ticket: Option<EditMenuTicket>,
    pub gesture: Option<MenuToken>,
    pub presenter: Box<dyn MenuPresenter>,
    pub activation: Option<gpui::Subscription>,
}
impl Default for EditMenu {
    fn default() -> Self {
        Self {
            ticket: None,
            gesture: None,
            presenter: Box::new(NativePresenter),
            activation: None,
        }
    }
}

pub(crate) fn menu_action_allowed(
    ticket: Option<&EditMenuTicket>,
    token: MenuToken,
    stamp: Option<&Stamp>,
    selection: Option<&SelectionKey>,
    action: EditAction,
    active: bool,
) -> bool {
    active
        && stamp.is_some()
        && ticket.is_some_and(|ticket| {
            ticket.token == token
                && ticket.stamp.as_ref() == stamp
                && (action == EditAction::Paste
                    || (action == EditAction::Copy && ticket.selection.as_ref() == selection))
        })
}

impl Smoke {
    pub(crate) fn dismiss_edit_menu(&mut self, window: &Window) {
        if let Some(ticket) = self.edit_menu.ticket.take() {
            self.edit_menu
                .presenter
                .dismiss(window.window_handle(), ticket.token);
        }
    }

    pub(crate) fn edit_menu_check(&mut self, window: &Window) {
        let Some(ticket) = &self.edit_menu.ticket else {
            return;
        };
        let selection = self.handoff.selection.as_ref().map(SelectionKey::from);
        let revoked = self
            .edit_menu
            .presenter
            .revoked(window.window_handle(), ticket.token);
        if !window.is_window_active()
            || !self.terminal_focus.is_focused(window)
            || self.handoff.stamp.is_none()
            || ticket.stamp != self.handoff.stamp
            || ticket.selection != selection
            || revoked.is_some()
        {
            #[cfg(all(target_os = "ios", target_abi = "sim"))]
            self.observation.record(serde_json::json!({"kind":"edit-menu-revoked", "reason":revoked.unwrap_or_else(|| "ConsumerStateChanged".into())}));
            self.dismiss_edit_menu(window);
        }
    }

    pub(crate) fn present_edit_menu(
        &mut self,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<MenuToken> {
        self.terminal_focus.focus(window, cx);
        self.publish_native_edits(window);
        self.dismiss_edit_menu(window);
        let token = self
            .edit_menu
            .presenter
            .present(window.window_handle(), at)?;
        self.edit_menu.ticket = Some(EditMenuTicket {
            token,
            stamp: self.handoff.stamp.clone(),
            selection: self.handoff.selection.as_ref().map(SelectionKey::from),
        });
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        self.sh1_152_capture(token, window.window_handle(), cx);
        Some(token)
    }

    pub(crate) fn native_menu_action(
        &mut self,
        action: EditAction,
        token: Option<MenuToken>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let availability = self.native_edit_availability(window);
        self.edit_menu_check(window);
        let selection = self.handoff.selection.as_ref().map(SelectionKey::from);
        let allowed = token.is_none_or(|token| {
            menu_action_allowed(
                self.edit_menu.ticket.as_ref(),
                token,
                self.handoff.stamp.as_ref(),
                selection.as_ref(),
                action,
                window.is_window_active(),
            )
        }) && availability.is_some_and(|a| match action {
            EditAction::Copy => a.copy,
            EditAction::Cut => a.cut,
            EditAction::Paste => a.paste,
            EditAction::SelectAll => a.select_all,
        });
        #[cfg(all(target_os = "ios", target_abi = "sim"))]
        if let Some(token) = token {
            self.observation.record(serde_json::json!({"kind":"edit-menu-exec", "action":format!("{action:?}"), "token":format!("{token:?}"), "result":if allowed {"executed"} else {"rejected"}}));
        }
        if allowed {
            match action {
                EditAction::Paste => self.start_paste(false, cx),
                EditAction::Copy | EditAction::Cut => self.copy_selection(cx),
                EditAction::SelectAll => {}
            }
        } else if token.is_some() {
            self.dismiss_edit_menu(window);
        }
        cx.stop_propagation();
        window.prevent_default();
    }
}
