//! Validate the host receipt before it can become shell input.
pub(super) fn escape(path: &str) -> Result<String, String> {
    if !path.starts_with('/') || path.len() > 4096 || path.chars().any(char::is_control) {
        return Err("Paste refused: unsafe path from host".into());
    }
    let mut output = String::new();
    for c in path.chars() {
        if c.is_ascii() && !c.is_ascii_alphanumeric() && !"/._-+,:@%".contains(c) {
            output.push('\\');
        }
        output.push(c);
    }
    Ok(output)
}
