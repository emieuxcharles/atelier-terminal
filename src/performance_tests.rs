//! Opt-in benchmarks: isolated shells, no user configuration or workspace changes.
use super::*;
use egui_term::BackendCommand;
use std::time::{Duration, Instant};

fn title(rx: &mpsc::Receiver<(u64, PtyEvent)>, expected: &str) {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let (_, event) = rx.recv_timeout(end.saturating_duration_since(Instant::now())).expect("PTY response timeout");
        if matches!(event, PtyEvent::Title(ref value) if value == expected) { return; }
    }
}
fn alive(pid: u32) -> bool { unsafe { libc::kill(pid as i32, 0) == 0 } }
fn rss() -> u64 {
    String::from_utf8(std::process::Command::new("ps").args(["-p", &std::process::id().to_string(), "-o", "rss="]).output().unwrap().stdout).unwrap().trim().parse().unwrap()
}
fn heap_used() -> usize {
    let mut stats = std::mem::MaybeUninit::<libc::malloc_statistics_t>::zeroed();
    // A null zone requests aggregate statistics for all malloc zones on macOS.
    unsafe { libc::malloc_zone_statistics(std::ptr::null_mut(), stats.as_mut_ptr()); stats.assume_init().size_in_use }
}
#[test]
#[ignore = "manual performance audit: real PTYs and timing"]
fn audit_terminal_load_and_cleanup() {
    for count in [1, 4, 8] {
        let baseline = rss();
        let (tx, rx) = mpsc::channel();
        let context = egui::Context::default();
        let start = Instant::now();
        let mut backends = Vec::new();
        for id in 0..count {
            backends.push(TerminalBackend::new(id, context.clone(), tx.clone(), BackendSettings {
                shell: "/usr/bin/python3".into(),
                args: vec!["-u".into(), "-c".into(), format!("import sys\nfor i in range(100000): sys.stdout.write('log %06d '%i + 'x'*60 + '\\n')\nsys.stdout.write('\\x1b]2;READY{id}\\x07');sys.stdout.flush()\nfor line in sys.stdin: sys.stdout.write('\\x1b]2;ACK:'+line.strip()+'\\x07');sys.stdout.flush()", id=id)],
                working_directory: Some(std::env::temp_dir()), env: Default::default(),
            }).unwrap());
        }
        // Titles can arrive in any order; count each pane's ready marker once.
        let mut ready = std::collections::HashSet::new();
        while ready.len() < count as usize {
            let (id, event) = rx.recv_timeout(Duration::from_secs(30)).unwrap();
            if matches!(event, PtyEvent::Title(ref t) if t.starts_with("READY")) { ready.insert(id); }
        }
        let output_ms = start.elapsed().as_secs_f64()*1000.0;
        for backend in &mut backends { backend.sync(); }
        let loaded = rss();
        let mut frames = Vec::new();
        for _ in 0..120 {
            let start = Instant::now();
            for backend in &mut backends { std::hint::black_box(backend.sync()); }
            frames.push(start.elapsed().as_secs_f64()*1000.0);
        }
        frames.sort_by(f64::total_cmp);
        let mut echoes = Vec::new();
        for i in 0..100 {
            let start = Instant::now();
            backends[0].process_command(BackendCommand::Write(format!("{i}\n").into_bytes()));
            title(&rx, &format!("ACK:{i}"));
            backends[0].sync();
            echoes.push(start.elapsed().as_secs_f64()*1000.0);
        }
        echoes.sort_by(f64::total_cmp);
        let pids: Vec<_> = backends.iter().map(|b| b.shell_pid).collect();
        let close = Instant::now();
        drop(backends);
        let end = Instant::now()+Duration::from_secs(3);
        while pids.iter().any(|p| alive(*p)) && Instant::now()<end { std::thread::sleep(Duration::from_millis(10)); }
        let survivors: Vec<_> = pids.iter().filter(|p| alive(**p)).collect();
        println!("AUDIT {}", serde_json::json!({"panes":count,"lines_per_pane":100000,"output_ms":output_ms,"sync_p50_ms":frames[60],"sync_p95_ms":frames[114],"pty_roundtrip_p50_ms":echoes[50],"pty_roundtrip_p95_ms":echoes[95],"rss_before_kib":baseline,"rss_loaded_kib":loaded,"rss_after_close_kib":rss(),"close_ms":close.elapsed().as_secs_f64()*1000.0,"survivors":survivors}));
        assert!(survivors.is_empty(), "shells must terminate after pane close");
    }
}

#[test]
#[ignore = "manual cleanup audit: real process groups and repeated allocation"]
fn audit_children_and_repeated_close() {
    let mut samples = Vec::new();
    let heap_initial = heap_used();
    for cycle in 0..8 {
        let (tx, rx) = mpsc::channel();
        let context = egui::Context::default();
        let mut backends = Vec::new();
        let mut pids = Vec::new();
        for id in 0..8 {
            let backend = TerminalBackend::new(id, context.clone(), tx.clone(), BackendSettings {
                shell: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 60 & child=$!; printf '\\033]2;CHILD:%s\\007' \"$child\"; i=0; while [ $i -lt 12000 ]; do printf 'log 012345678901234567890123456789012345678901234567890123456789\n'; i=$((i+1)); done; printf '\\033]2;READY\\007'; wait".into()],
                working_directory: Some(std::env::temp_dir()), env: Default::default(),
            }).unwrap();
            pids.push(backend.shell_pid);
            backends.push(backend);
        }
        let mut ready=0;
        while ready < 8 {
            if let (_,PtyEvent::Title(value)) = rx.recv_timeout(Duration::from_secs(30)).unwrap() {
                if let Some(pid)=value.strip_prefix("CHILD:") {pids.push(pid.parse().unwrap());}
                if value=="READY" {ready+=1;}
            }
        }
        for backend in &mut backends {backend.sync();}
        let loaded=rss();
        let heap_loaded=heap_used();
        drop(backends);
        let end=Instant::now()+Duration::from_secs(3);
        while pids.iter().any(|p| alive(*p)) && Instant::now()<end {std::thread::sleep(Duration::from_millis(10));}
        let survivors: Vec<_>=pids.iter().filter(|p| alive(**p)).copied().collect();
        samples.push(serde_json::json!({"cycle":cycle,"heap_initial_bytes":heap_initial,"heap_loaded_bytes":heap_loaded,"heap_after_close_bytes":heap_used(),"rss_loaded_kib":loaded,"rss_after_close_kib":rss(),"tracked_processes":pids.len(),"survivors":survivors}));
        assert!(survivors.is_empty(), "processes remain after closing: {survivors:?}");
    }
    println!("CLEANUP {}",serde_json::to_string(&samples).unwrap());
}

#[test]
#[ignore = "manual input under concurrent PTY load"]
fn audit_input_during_output() {
    let (tx,rx)=mpsc::channel();
    let ctx=egui::Context::default();
    let mut panes=Vec::new();
    for id in 0..8 {
        let program=if id==0 {
            "import sys,tty\ntty.setraw(0)\nsys.stdout.write('\\x1b]2;INPUT_READY\\x07');sys.stdout.flush()\nwhile True:\n c=sys.stdin.read(1)\n if not c: break\n sys.stdout.write('\\x1b]2;KEY:'+c+'\\x07');sys.stdout.flush()"
        } else {
            "import sys,time\nend=time.monotonic()+5\nblock=('load '+ 'x'*60+'\\r\\n')*100\nwhile time.monotonic()<end: sys.stdout.write(block);sys.stdout.flush()\ntime.sleep(10)"
        };
        panes.push(TerminalBackend::new(id,ctx.clone(),tx.clone(),BackendSettings {
            shell:"/usr/bin/python3".into(),args:vec!["-u".into(),"-c".into(),program.into()],working_directory:Some(std::env::temp_dir()),env:Default::default(),
        }).unwrap());
    }
    title(&rx,"INPUT_READY");
    let mut times=Vec::new();
    for _ in 0..100 {
        let start=Instant::now();
        panes[0].process_command(BackendCommand::Write(vec![b'x']));
        title(&rx,"KEY:x");
        for pane in &mut panes {std::hint::black_box(pane.sync());}
        times.push(start.elapsed().as_secs_f64()*1000.0);
    }
    times.sort_by(f64::total_cmp);
    let pids:Vec<_>=panes.iter().map(|b|b.shell_pid).collect();
    drop(panes);
    let end=Instant::now()+Duration::from_secs(3);
    while pids.iter().any(|p|alive(*p)) && Instant::now()<end {std::thread::sleep(Duration::from_millis(10));}
    assert!(pids.iter().all(|p|!alive(*p)));
    println!("INPUT_LOAD {}",serde_json::json!({"panes":8,"log_writers":7,"keystrokes":100,"p50_ms":times[50],"p95_ms":times[95],"max_ms":times[99]}));
}
