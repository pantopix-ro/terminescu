//! Starts a pane's login shell with the Terminescu integration (D9). The scripts ship inside
//! the daemon and get written to `<run>/shell` at start, so they always match the daemon that
//! parses their reports.

use portable_pty::CommandBuilder;
use std::io;
use std::path::{Path, PathBuf};

const FILES: &[(&str, &str)] = &[
    ("zsh/.zshenv", include_str!("../shell/zshenv.zsh")),
    ("terminescu.zsh", include_str!("../shell/terminescu.zsh")),
    ("bashrc.bash", include_str!("../shell/bashrc.bash")),
    ("terminescu.bash", include_str!("../shell/terminescu.bash")),
    (
        "fish/vendor_conf.d/terminescu.fish",
        include_str!("../shell/terminescu.fish"),
    ),
];

pub const SHELL_DIR_ENV: &str = "TERMINESCU_SHELL_DIR";
pub const CLI_ENV: &str = "TERMINESCU_CLI";

/// Writes the scripts under `dir`, replacing each file in one rename so a shell starting at the
/// same moment never reads half a script.
pub fn install(dir: &Path) -> io::Result<()> {
    for (name, body) in FILES {
        let path = dir.join(name);
        if std::fs::read(&path).is_ok_and(|old| old == body.as_bytes()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, body)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Zsh,
    Bash,
    Fish,
}

pub fn kind_of(shell: &str) -> Option<Kind> {
    match Path::new(shell).file_name()?.to_str()? {
        "zsh" => Some(Kind::Zsh),
        "bash" => Some(Kind::Bash),
        "fish" => Some(Kind::Fish),
        _ => None,
    }
}

/// Turns `cmd`, a default-program command with its environment already set, into the login
/// shell with the integration. A shell we have no integration for starts as it would anyway.
pub fn inject(cmd: &mut CommandBuilder, dir: &Path, cli: Option<&PathBuf>) {
    let shell = cmd.get_shell();
    let Some(kind) = kind_of(&shell) else {
        return;
    };
    cmd.env(SHELL_DIR_ENV, dir);
    if let Some(cli) = cli {
        cmd.env(CLI_ENV, cli);
    }
    let argv: Vec<String> = match kind {
        Kind::Zsh => {
            if let Some(orig) = cmd.get_env("ZDOTDIR").map(|v| v.to_owned()) {
                cmd.env("TERMINESCU_ZDOTDIR", orig);
            }
            cmd.env("ZDOTDIR", dir.join("zsh"));
            vec![shell, "-l".into()]
        }
        // `--rcfile` only applies to a non-login shell; the file reads the login files itself.
        Kind::Bash => vec![
            shell,
            "--rcfile".into(),
            dir.join("bashrc.bash").to_string_lossy().into_owned(),
            "-i".into(),
        ],
        Kind::Fish => {
            let orig = cmd
                .get_env("XDG_DATA_DIRS")
                .map(|v| v.to_string_lossy().into_owned());
            cmd.env("TERMINESCU_XDG_DATA_DIRS", orig.clone().unwrap_or_default());
            let rest = orig.unwrap_or_else(|| "/usr/local/share:/usr/share".into());
            cmd.env("XDG_DATA_DIRS", format!("{}:{rest}", dir.display()));
            vec![shell, "-l".into()]
        }
    };
    *cmd.get_argv_mut() = argv.into_iter().map(Into::into).collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert_eq!(kind_of("/bin/zsh"), Some(Kind::Zsh));
        assert_eq!(kind_of("/opt/homebrew/bin/fish"), Some(Kind::Fish));
        assert_eq!(kind_of("bash"), Some(Kind::Bash));
        assert_eq!(kind_of("/bin/sh"), None);
    }

    #[test]
    fn zsh_keeps_the_persons_zdotdir() {
        let mut cmd = CommandBuilder::new_default_prog();
        cmd.env("SHELL", "/bin/zsh");
        cmd.env("ZDOTDIR", "/Users/x/.config/zsh");
        inject(&mut cmd, Path::new("/run/shell"), None);
        assert_eq!(cmd.get_argv(), &["/bin/zsh", "-l"]);
        assert_eq!(cmd.get_env("ZDOTDIR").unwrap(), "/run/shell/zsh");
        assert_eq!(
            cmd.get_env("TERMINESCU_ZDOTDIR").unwrap(),
            "/Users/x/.config/zsh"
        );
    }

    #[test]
    fn install_writes_every_script() {
        let dir = std::env::temp_dir().join(format!("tsh-{}", std::process::id()));
        install(&dir).unwrap();
        install(&dir).unwrap();
        for (name, body) in FILES {
            assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), *body);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
