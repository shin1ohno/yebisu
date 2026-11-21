use sheprd_client::protocol::ClientShellSnapshot;
use std::borrow::Cow;

pub(super) fn name<'a>(snapshot: &'a ClientShellSnapshot, id: &str, index: usize) -> Cow<'a, str> {
    let label = snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == id)
        .and_then(|pane| pane.label.as_deref())
        .filter(|label| has_text(label));
    let agent = snapshot
        .agents
        .iter()
        .find(|agent| agent.pane_id == id)
        .and_then(|agent| {
            [
                agent.display_agent.as_deref(),
                agent.name.as_deref(),
                agent.terminal_title_stripped.as_deref(),
            ]
            .into_iter()
            .flatten()
            .find(|label| has_text(label))
        });
    label
        .or(agent)
        .map_or_else(|| Cow::Owned(format!("Pane {}", index + 1)), Cow::Borrowed)
}

fn has_text(label: &str) -> bool {
    label
        .chars()
        .any(|ch| !ch.is_control() && !ch.is_whitespace())
}
