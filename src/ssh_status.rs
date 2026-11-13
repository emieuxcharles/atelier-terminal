use std::{collections::HashMap, path::PathBuf};

pub struct Integration { directory: PathBuf, real_ssh: PathBuf }

impl Integration {
    pub fn install() -> std::io::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        let directory = std::env::temp_dir().join(format!("atelier-ssh-{}-{}", std::process::id(), token()));
        std::fs::create_dir(&directory)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let real_ssh = std::env::var_os("ATELIER_REAL_SSH").map(PathBuf::from).filter(|p| p.is_file()).or_else(|| std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|path| path.join("ssh"))
            .find(|path| path.is_file() && path.parent() != Some(directory.as_path())))
            .unwrap_or_else(|| PathBuf::from("/usr/bin/ssh"));
        let wrapper = directory.join("ssh");
        std::fs::write(&wrapper, include_str!("../resources/ssh-wrapper.zsh"))?;
        std::fs::set_permissions(wrapper, std::fs::Permissions::from_mode(0o700))?;
        let docker = directory.join("docker");
        std::fs::write(&docker, include_str!("../resources/docker-wrapper.zsh"))?;
        std::fs::set_permissions(docker, std::fs::Permissions::from_mode(0o700))?;
        std::fs::write(directory.join("directory-tracking.zsh"), include_str!("../resources/directory-tracking.zsh"))?;
        // macOS path_helper can reorder PATH during login. Proxy zsh startup files
        // without editing them, then prepend the relay after user startup finishes.
        std::fs::write(directory.join(".zshenv"), r#"
ZDOTDIR=${ATELIER_USER_ZDOTDIR:-$HOME}
[[ -r "$ZDOTDIR/.zshenv" ]] && source "$ZDOTDIR/.zshenv"
export ATELIER_USER_ZDOTDIR=${ZDOTDIR:-$HOME}
export ZDOTDIR=$ATELIER_SSH_BIN
"#)?;
        for file in [".zprofile", ".zshrc"] {
            let source = format!(r#"
ZDOTDIR=$ATELIER_USER_ZDOTDIR
[[ -r "$ZDOTDIR/{file}" ]] && source "$ZDOTDIR/{file}"
export ATELIER_USER_ZDOTDIR=${{ZDOTDIR:-$HOME}}
export ZDOTDIR=$ATELIER_SSH_BIN
export PATH="$ATELIER_SSH_BIN:$PATH"
"#);
            let source = if file == ".zshrc" {
                format!("{source}\nsource \"$ATELIER_SSH_BIN/directory-tracking.zsh\"\n")
            } else { source };
            std::fs::write(directory.join(file), source)?;
        }
        std::fs::write(directory.join(".zlogin"), r#"
export ZDOTDIR=$ATELIER_USER_ZDOTDIR
[[ -r "$ZDOTDIR/.zlogin" ]] && source "$ZDOTDIR/.zlogin"
export PATH="$ATELIER_SSH_BIN:$PATH"
source "$ATELIER_SSH_BIN/directory-tracking.zsh"
"#)?;
        Ok(Self { directory, real_ssh })
    }

    pub fn environment(&self, token: &str, zsh: bool) -> HashMap<String, String> {
        let mut env = HashMap::new();
        if zsh {
            env.insert("ATELIER_CWD_OWNER".into(), String::new());
            env.insert("ATELIER_USER_ZDOTDIR".into(), std::env::var("ZDOTDIR").unwrap_or_else(|_| std::env::var("HOME").unwrap_or_default()));
            env.insert("ZDOTDIR".into(), self.directory.to_string_lossy().into_owned());
            env.insert("ATELIER_SSH_BIN".into(), self.directory.to_string_lossy().into_owned());
        }
        env.insert("PATH".into(), format!("{}:{}", self.directory.display(), std::env::var("PATH").unwrap_or_default()));
        env.insert("ATELIER_REAL_SSH".into(), self.real_ssh.to_string_lossy().into_owned());
        env.insert("ATELIER_SSH_TOKEN".into(), token.into());
        env
    }
}

impl Drop for Integration {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

pub fn token() -> String {
    let mut bytes = [0u8; 16];
    unsafe { libc::arc4random_buf(bytes.as_mut_ptr().cast(), bytes.len()); }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Only the local wrapper's per-pane token can change the badge, not ordinary titles.
pub fn parse(title: &str, token: &str) -> Option<Option<String>> {
    let value = title.strip_prefix("ATELIER_SSH:")?.strip_prefix(token)?.strip_prefix(':')?;
    if value == "local" { return Some(None); }
    let host = value.strip_prefix("ssh:")?;
    let host: String = host.chars().filter(|c| !c.is_control()).take(100).collect();
    if host.is_empty() { return None; }
    Some(Some(host))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installs_docker_relay_in_shell_path() {
        let integration = Integration::install().unwrap();
        assert!(integration.directory.join("docker").is_file());
        let output = std::process::Command::new("/bin/zsh")
            .args(["-f", "-c", "whence -p docker"])
            .envs(integration.environment("test", false)).output().unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), integration.directory.join("docker").to_str().unwrap());
    }
    #[test]
    fn login_shell_preserves_user_config_and_finds_relay() {
        let integration = Integration::install().unwrap();
        let config = integration.directory.join("user-config");
        std::fs::create_dir(&config).unwrap();
        std::fs::write(config.join(".zshrc"), "export PATH=/usr/bin:/bin\nexport STARTUP_SEEN=yes\n").unwrap();
        let output = std::process::Command::new("/bin/zsh")
            .args(["-l", "-i", "-c", "print -r -- $STARTUP_SEEN; whence -p ssh; print -r -- $ZDOTDIR"])
            .envs(integration.environment("test", true))
            .env("ATELIER_USER_ZDOTDIR", &config)
            .output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("yes\n{}/ssh\n{}\n", integration.directory.display(), config.display()));
        std::fs::remove_dir_all(&integration.directory).unwrap();
    }
    #[test]
    fn bundled_login_shell_tracks_directories_without_plugins() {
        let integration = Integration::install().unwrap();
        let config = integration.directory.join("empty-config");
        std::fs::create_dir(&config).unwrap();
        let output = std::process::Command::new("python3")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/directory_tracking.py"))
            .envs(integration.environment("test", true))
            .env("ATELIER_USER_ZDOTDIR", config)
            .env("ATELIER_TEST_LOGIN", "1")
            .output().unwrap();
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
    #[test]
    fn accepts_only_local_tokens_and_clears_on_return() {
        assert_eq!(parse("ATELIER_SSH:secret:ssh:user@host", "secret"), Some(Some("user@host".into())));
        assert_eq!(parse("ATELIER_SSH:secret:local", "secret"), Some(None));
        assert_eq!(parse("ATELIER_SSH:other:local", "secret"), None);
        assert_eq!(parse("user@remote: ~", "secret"), None);
        assert_eq!(parse("ATELIER_SSH:secret:ssh:", "secret"), None);
    }
}
