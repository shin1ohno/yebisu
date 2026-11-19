/// QA accessibility identifiers the S3 tests press (P9, SH1-154, P3, the self-test). Each one is
/// a constant in crates/yebisu/src/qa_ax_ids.rs, marked on the control it names;
/// scripts/ios/qa-ax-ids-check.py fails if a string here is not defined there.
enum QAIds {
    static let tabsMore = "qa.tabs.more"
    static let menuPaste = "qa.menu.paste"
    static let menuPasteImage = "qa.menu.paste-image"
    static let menuSplit = "qa.menu.split"
    static let menuSplitDirection = "qa.menu.split-direction"
    static let menuPane1 = "qa.menu.pane-1"
    static let menuPane2 = "qa.menu.pane-2"
    static let menuDetach = "qa.menu.detach"
    static let bandMark = "qa.band.mark"
    static let bandDelete = "qa.band.delete"
    static let bandCommit = "qa.band.commit"
    static let bandImage = "qa.band.image"
    static let bandImageOnly = "qa.band.image-only"
    static let statusReconnect = "qa.status.reconnect"
    static let footerSettings = "qa.footer.settings"
    static let railSettings = "qa.rail.settings"
    static let settingsClose = "qa.settings.close"
    static let settingsPageDisplay = "qa.settings.page.display"
    static let densityCompact = "qa.settings.density.compact"
    static let densityRegular = "qa.settings.density.regular"
    static let terminalInput = "qa.terminal.input"
}
