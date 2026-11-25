//! The listening-port chips' parser and scan thread.
use super::*;
use crate::forwards::scan::{self, MAX_PORTS, Scanner};

#[test]
fn numbers_only() {
    let output = b"L 127.0.0.1:3000\nL *:5173 node /home/me/EVIL\nL [::1]:8080\nL 0.0.0.0:5432\nEVIL 1.2.3.4:22\nL EVIL\n";
    assert_eq!(scan::parse(output), Some(vec![3000, 5432, 8080]));
}

#[test]
fn dedup_sort_cap_32() {
    let mut output = String::new();
    for port in (1000..1040).rev() {
        output.push_str(&format!("L 127.0.0.1:{port}\nL [::]:{port}\n"));
    }
    let ports = scan::parse(output.as_bytes()).unwrap_or_default();
    assert_eq!(ports.len(), MAX_PORTS);
    assert_eq!(ports, (1000..1032).collect::<Vec<_>>());
}

#[test]
fn invalid_ports_dropped() {
    let mut output =
        b"L 127.0.0.1:0\nL *:65536\nL *:-1\nL *:+80\nL *:abc\nL nocolon\nL *:\nL :\n".to_vec();
    output.extend_from_slice(b"L \xff\xfe:80\nL 127.0.0.1:443\r\n");
    assert_eq!(scan::parse(&output), Some(vec![443]));
}

#[test]
fn no_tool_is_none() {
    assert_eq!(scan::parse(b"N\n"), None);
    assert_eq!(scan::parse(b"L 127.0.0.1:3000\nN\n"), None);
    // A host with the tool but no listener shows an empty row.
    assert_eq!(scan::parse(b""), Some(Vec::new()));
}

#[test]
fn lines_beyond_the_limit_are_ignored() {
    let mut output = "L x\n".repeat(1024);
    output.push_str("L 127.0.0.1:3000\n");
    assert_eq!(scan::parse(output.as_bytes()), Some(Vec::new()));
}

#[test]
fn the_command_prints_addresses_only() {
    // No process names, paths or environment: only `L <address>` or `N`.
    assert!(scan::COMMAND.contains(r#"print "L", $4"#));
    assert!(scan::COMMAND.contains(r#"print "L", substr($0, 2)"#));
    assert!(scan::COMMAND.contains("lsof -nP -iTCP -sTCP:LISTEN -Fn "));
    assert!(!scan::COMMAND.contains("environ"));
    assert!(!scan::COMMAND.contains("exit"));
}

#[test]
fn scanner_reports_and_stops() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let run: scan::Run = Arc::new(move |host, _| {
        assert_eq!(host, HOST);
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(b"L 127.0.0.1:3000\nL *:5173 EVIL\n".to_vec())
    });
    let scanner = ok(Scanner::start(HOST.to_owned(), run).ok_or("no scan thread"));
    let deadline = Instant::now() + PATIENCE;
    let first = loop {
        if let Some(result) = scanner.take() {
            break result;
        }
        assert!(Instant::now() < deadline, "no scan result");
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(first, Some(vec![3000]));
    assert_eq!(scanner.take(), None, "a result is taken once");
    drop(scanner);
    thread::sleep(Duration::from_millis(300));
    let after = Seen::count(&calls);
    thread::sleep(scan::INTERVAL + Duration::from_millis(500));
    assert_eq!(Seen::count(&calls), after, "a closed sheet scans no more");
}

#[test]
fn a_failed_scan_keeps_the_chips() {
    let run: scan::Run = Arc::new(|_, _| Err(crate::Error::Ssh("EVIL".into())));
    let scanner = ok(Scanner::start(HOST.to_owned(), run).ok_or("no scan thread"));
    thread::sleep(Duration::from_millis(200));
    assert_eq!(scanner.take(), None);
}
