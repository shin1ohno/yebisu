use super::*;

fn open_terminal() -> Facts {
    Facts {
        terminal_owns: true,
        focused: true,
        destination: true,
        sheet_ready: true,
        ..Facts::default()
    }
}

#[test]
fn an_open_focused_terminal_with_a_destination_accepts_text() {
    assert_eq!(Gate::decide(open_terminal()), Gate::Accepts);
}

#[test]
fn each_reason_is_the_first_condition_that_fails() {
    let cases = [
        (
            Facts {
                settings_modal: true,
                find: true,
                ..open_terminal()
            },
            Reason::Settings,
        ),
        (
            Facts {
                find: true,
                focused: false,
                ..open_terminal()
            },
            Reason::Find,
        ),
        (
            Facts {
                focused: false,
                destination: false,
                ..open_terminal()
            },
            Reason::Focus,
        ),
        (
            Facts {
                destination: false,
                navigating: true,
                ..open_terminal()
            },
            Reason::Destination,
        ),
        (
            Facts {
                navigating: true,
                palette: true,
                ..open_terminal()
            },
            Reason::Navigating,
        ),
        (
            Facts {
                palette: true,
                shortcuts: true,
                ..open_terminal()
            },
            Reason::Palette,
        ),
        (
            Facts {
                shortcuts: true,
                ..open_terminal()
            },
            Reason::Shortcuts,
        ),
        (
            Facts {
                terminal_owns: false,
                sheet_ready: false,
                ..open_terminal()
            },
            Reason::Sheet,
        ),
    ];
    for (facts, reason) in cases {
        assert_eq!(Gate::decide(facts), Gate::Off(reason), "{facts:?}");
    }
}

#[test]
fn a_focused_ready_sheet_accepts_text() {
    let sheet = Facts {
        terminal_owns: false,
        destination: false,
        ..open_terminal()
    };
    assert_eq!(Gate::decide(sheet), Gate::Accepts);
}

#[test]
fn every_code_is_listed_once() {
    let reasons = [
        Reason::Settings,
        Reason::Find,
        Reason::Focus,
        Reason::Destination,
        Reason::Navigating,
        Reason::Palette,
        Reason::Shortcuts,
        Reason::Sheet,
    ];
    let produced: Vec<&str> = std::iter::once(Gate::Accepts)
        .chain(reasons.map(Gate::Off))
        .map(Gate::code)
        .collect();
    assert_eq!(produced, CODES);
}

#[test]
fn a_code_is_recorded_only_when_the_answer_changes() {
    let recorder = Recorder::default();
    let focus = Gate::Off(Reason::Focus);
    assert_eq!(recorder.changed(focus), Some("text_off_focus"));
    assert_eq!(recorder.changed(focus), None);
    assert_eq!(recorder.changed(Gate::Accepts), Some("text_on"));
    assert_eq!(recorder.changed(Gate::Accepts), None);
    assert_eq!(recorder.changed(focus), Some("text_off_focus"));
}
