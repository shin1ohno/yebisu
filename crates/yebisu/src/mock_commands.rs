//! P7 sandbox API semantics. IDs and labels are fixture constants, not image-derived.
use serde_json::Value;
use sheprd_client::protocol::ClientShellSnapshot;
pub(crate) fn apply(s: &mut ClientShellSnapshot, request: &Value) -> bool {
    let method = request["method"].as_str().unwrap_or_default();
    let params = &request["params"];
    match method {
        "tab.create" => {
            let Some(mut tab) = s.tabs.first().cloned() else {
                return false;
            };
            tab.tab_id = format!("w1:t{}", s.tabs.len() + 1);
            tab.number = s.tabs.len() + 1;
            tab.label = "P7 new tab".into();
            tab.workspace_id = params["workspace_id"].as_str().unwrap_or("w1").into();
            let Some(mut pane) = s.panes.first().cloned() else {
                return false;
            };
            pane.pane_id = format!("w1:p{}", s.panes.len() + 10);
            pane.tab_id = tab.tab_id.clone();
            pane.workspace_id = tab.workspace_id.clone();
            s.focused_tab_id = Some(tab.tab_id.clone());
            s.focused_pane_id = Some(pane.pane_id.clone());
            s.tabs.push(tab);
            s.panes.push(pane);
        }
        "pane.split" => {
            let Some(mut pane) = s
                .panes
                .iter()
                .find(|p| Some(p.pane_id.as_str()) == params["target_pane_id"].as_str())
                .cloned()
            else {
                return false;
            };
            pane.pane_id = format!("w1:p{}", s.panes.len() + 10);
            pane.label = Some("P7 split".into());
            s.focused_pane_id = Some(pane.pane_id.clone());
            s.panes.push(pane);
        }
        "tab.focus" => {
            let Some(tab) = params["tab_id"].as_str() else {
                return false;
            };
            let Some(pane) = s.panes.iter().find(|p| p.tab_id == tab) else {
                return false;
            };
            s.focused_workspace_id = Some(pane.workspace_id.clone());
            s.focused_tab_id = Some(tab.into());
            s.focused_pane_id = Some(pane.pane_id.clone());
        }
        "pane.focus_direction" => {
            let Some(pane) = s.panes.iter().find(|p| {
                Some(&p.tab_id) == s.focused_tab_id.as_ref()
                    && Some(&p.pane_id) != s.focused_pane_id.as_ref()
            }) else {
                return false;
            };
            s.focused_pane_id = Some(pane.pane_id.clone());
        }
        "pane.zoom" => {
            if let Some(tab) = s
                .tabs
                .iter_mut()
                .find(|t| Some(&t.tab_id) == s.focused_tab_id.as_ref())
            {
                tab.zoomed ^= true;
            }
        }
        "pane.close" | "tab.close" => {
            return false;
        }
        _ => return false,
    }
    for t in &mut s.tabs {
        t.focused = Some(&t.tab_id) == s.focused_tab_id.as_ref();
    }
    for p in &mut s.panes {
        p.focused = Some(&p.pane_id) == s.focused_pane_id.as_ref();
    }
    s.revision += 1;
    true
}
