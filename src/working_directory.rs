use std::{os::unix::ffi::OsStringExt, path::PathBuf};

/// Query the local shell only on explicit actions, never on a timer.
pub fn of_process(pid: u32) -> Option<PathBuf> {
    let mut info = std::mem::MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>();
    // macOS writes at most `size` bytes into this correctly sized, aligned buffer.
    let written = unsafe {
        libc::proc_pidinfo(pid.try_into().ok()?, libc::PROC_PIDVNODEPATHINFO, 0,
            info.as_mut_ptr().cast(), size as i32)
    };
    if written != size as i32 { return None; }
    let info = unsafe { info.assume_init() };
    let bytes: Vec<u8> = info.pvi_cdir.vip_path.iter().flatten()
        .map(|byte| *byte as u8).take_while(|byte| *byte != 0).collect();
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{BufRead, BufReader, Write}, process::{Command, Stdio}};
    #[test]
    fn terminal_starts_zsh_with_stale_inherited_pwd() {
        use egui_term::{BackendSettings, PtyEvent, TerminalBackend};
        use std::{sync::mpsc, time::{Duration, Instant}};
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().canonicalize().unwrap();
        let (sender, receiver) = mpsc::channel();
        let backend = TerminalBackend::new(9001, eframe::egui::Context::default(), sender, BackendSettings {
            shell: "/bin/zsh".into(),
            args: vec!["-f".into(), "-c".into(), "printf '\\033]2;READY:%s\\007' \"$PWD\"".into()],
            working_directory: Some(directory.clone()),
            env: [("PWD".into(), "/".into())].into(),
        }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut title = None;
        while let Ok((_, event)) = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            if let PtyEvent::Title(value) = event { title = Some(value); break; }
        }
        // Always release the PTY, including when startup is stuck.
        drop(backend);
        assert_eq!(title, Some(format!("READY:{}", directory.display())), "zsh must start in the project even when the app inherited PWD=/");
    }
    #[test]
    fn observes_shell_cd_on_demand() {
        let root = std::env::temp_dir().join(format!("atelier-cwd-{}", std::process::id()));
        let child_dir = root.join("dossier avec espaces é");
        std::fs::create_dir_all(&child_dir).unwrap();
        let mut child = Command::new("/bin/zsh").args(["-f", "-s"])
            .current_dir(&root).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        writeln!(input, "cd 'dossier avec espaces é'; print READY").unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let actual = of_process(child.id());
        writeln!(input, "exit").unwrap();
        child.wait().unwrap();
        assert_eq!(line.trim(), "READY");
        assert_eq!(actual, Some(child_dir.canonicalize().unwrap()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
