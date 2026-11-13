use super::*;

#[test]
fn six_specification_device_comments() -> Result<()> {
    // Source: ssh/ipad/tests.rs:107-112 at pinned 398c132a0, not screen output.
    for (device, expected) in [
        ("Shins-iPad", "yebisu-Shins-iPad"),
        ("Shins-iPad.local", "yebisu-Shins-iPad"),
        ("Shin's iPad Pro", "yebisu-Shin-s-iPad-Pro"),
        ("  iPad  ", "yebisu-iPad"),
        ("イパッド", "yebisu"),
        ("", "yebisu"),
    ] {
        let line = format!("restrict ecdsa-sha2-nistp256 AAAA herdr-ipad-{device}");
        assert_eq!(
            authorized_line(&line)?,
            format!("restrict ecdsa-sha2-nistp256 AAAA {expected}")
        );
    }
    assert_eq!(
        authorized_line("restrict ssh-ed25519 AAAA herdr-ipad")?,
        "restrict ssh-ed25519 AAAA yebisu"
    );
    Ok(())
}

#[test]
fn bad_comment_is_refused_and_never_displayed() {
    for comment in [
        "other",
        "yebisu",
        "herdr-ipadEvil",
        "",
        "herdr-ipad\ncommand",
    ] {
        assert!(authorized_line(&format!("restrict ssh-ed25519 AAAA {comment}")).is_err());
    }
    for line in [
        "ssh-ed25519 AAAA herdr-ipad",
        "restrict ssh-ed25519  herdr-ipad",
        "restrict invalid AAAA herdr-ipad",
    ] {
        assert!(authorized_line(line).is_err());
    }
}

#[test]
fn name_is_bounded_and_safe() -> Result<()> {
    let line = authorized_line(&format!(
        "restrict ssh-ed25519 AAAA herdr-ipad-{}",
        "x".repeat(200)
    ))?;
    assert_eq!(
        line,
        format!("restrict ssh-ed25519 AAAA yebisu-{}", "x".repeat(48))
    );
    Ok(())
}

#[test]
fn f23_delimiter_valid_bidi_is_sanitized_and_control_bytes_are_rejected() -> Result<()> {
    let formatted = authorized_line("restrict ssh-ed25519 RkFLRQ== herdr-ipad-safe\u{202e}name")?;
    assert_eq!(formatted, "restrict ssh-ed25519 RkFLRQ== yebisu-safe-name");
    for control in ['\0', '\r', '\n', '\t', '\u{7f}', '\u{85}'] {
        assert!(
            authorized_line(&format!(
                "restrict ssh-ed25519 RkFLRQ== herdr-ipad-{control}"
            ))
            .is_err()
        );
    }
    Ok(())
}
