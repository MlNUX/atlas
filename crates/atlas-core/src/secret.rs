// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! A remembered secret (the Nextcloud app password) without a keyring
//! daemon, in `$XDG_STATE_HOME/atlas/secrets/` (never in `config.toml` or a
//! project):
//!
//! - `<name>.cred`: encrypted with `systemd-creds --user` (systemd ≥ 256),
//!   bound to this machine (host key or TPM), useless elsewhere;
//! - `<name>.secret`: only if that is unavailable, plain text readable only
//!   by the user (mode 600).

use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

/// How a secret is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Encrypted with `systemd-creds`.
    Encrypted,
    /// Plain text, mode 600.
    File,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Encrypted => "encrypted",
            Kind::File => "file",
        }
    }
}

const TIMEOUT: Duration = Duration::from_secs(15);

/// Temporary file names unique within the process (several threads may
/// store at once).
fn unique() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!("{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

pub struct SecretStore {
    dir: PathBuf,
    name: String,
    /// The `systemd-creds` program (replaceable for tests).
    creds: String,
}

impl SecretStore {
    /// `name`: letters, digits, `-` and `_` (also the credential name).
    pub fn new(dir: impl Into<PathBuf>, name: &str) -> Self {
        let name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
        Self { dir: dir.into(), name, creds: "systemd-creds".into() }
    }

    /// The default place: `<state>/secrets/`. `ATLAS_SYSTEMD_CREDS` replaces
    /// the `systemd-creds` program (tests).
    pub fn default_for(name: &str) -> Self {
        let store = Self::new(crate::paths::state_root().join("secrets"), name);
        match std::env::var("ATLAS_SYSTEMD_CREDS") {
            Ok(program) if !program.is_empty() => store.with_creds_program(&program),
            _ => store,
        }
    }

    pub fn with_creds_program(mut self, program: &str) -> Self {
        self.creds = program.into();
        self
    }

    fn encrypted_path(&self) -> PathBuf {
        self.dir.join(format!("{}.cred", self.name))
    }

    fn file_path(&self) -> PathBuf {
        self.dir.join(format!("{}.secret", self.name))
    }

    /// How the secret is kept now (`None`: not remembered).
    pub fn kind(&self) -> Option<Kind> {
        if self.encrypted_path().exists() {
            Some(Kind::Encrypted)
        } else if self.file_path().exists() {
            Some(Kind::File)
        } else {
            None
        }
    }

    /// Remembers `secret`: encrypted if `systemd-creds` works, otherwise as
    /// a private file. The other form is removed.
    pub fn store(&self, secret: &str) -> Result<Kind> {
        if secret.is_empty() || secret.contains('\n') {
            bail!("ungültiges Passwort");
        }
        create_private_dir(&self.dir)?;
        let kind = match self.encrypt(secret) {
            Ok(()) => Kind::Encrypted,
            Err(err) => {
                log::info!("systemd-creds unavailable, keeping the secret as a private file: {err:#}");
                write_private(&self.file_path(), secret.as_bytes())?;
                Kind::File
            }
        };
        let other = if kind == Kind::Encrypted { self.file_path() } else { self.encrypted_path() };
        remove_if_exists(&other)?;
        Ok(kind)
    }

    /// Remembers `secret` only encrypted (`systemd-creds`); without it,
    /// nothing is stored (for passwords too valuable for a plain file, like
    /// the university login).
    pub fn store_encrypted(&self, secret: &str) -> Result<()> {
        if secret.is_empty() || secret.contains('\n') {
            bail!("ungültiges Passwort");
        }
        create_private_dir(&self.dir)?;
        self.encrypt(secret).context("systemd-creds kann hier nicht verschlüsseln – nichts gespeichert")?;
        remove_if_exists(&self.file_path())
    }

    fn encrypt(&self, secret: &str) -> Result<()> {
        let target = self.encrypted_path();
        let tmp = self.dir.join(format!(".{}.cred.tmp-{}", self.name, unique()));
        let name = format!("--name={}", self.name);
        let tmp_arg = tmp.to_string_lossy().into_owned();
        let result =
            run(&self.creds, &["--user", "encrypt", &name, "-", &tmp_arg], Some(secret.as_bytes())).and_then(|_| {
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
                std::fs::rename(&tmp, &target).with_context(|| format!("cannot replace {}", target.display()))
            });
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        // Only a round trip proves the credential can be read back.
        result?;
        match self.decrypt() {
            Ok(back) if back == secret => Ok(()),
            Ok(_) => {
                let _ = std::fs::remove_file(&target);
                bail!("systemd-creds liefert ein anderes Passwort zurück")
            }
            Err(err) => {
                let _ = std::fs::remove_file(&target);
                Err(err)
            }
        }
    }

    fn decrypt(&self) -> Result<String> {
        let name = format!("--name={}", self.name);
        let path = self.encrypted_path().to_string_lossy().into_owned();
        let out = run(&self.creds, &["--user", "decrypt", &name, &path, "-"], None)?;
        let text = String::from_utf8(out).context("entschlüsseltes Passwort ist kein UTF-8")?;
        Ok(text.trim_end_matches(['\n', '\r']).to_owned())
    }

    /// The remembered secret, if any.
    pub fn load(&self) -> Result<Option<(String, Kind)>> {
        match self.kind() {
            Some(Kind::Encrypted) => Ok(Some((self.decrypt()?, Kind::Encrypted))),
            Some(Kind::File) => {
                let path = self.file_path();
                let mode = std::fs::metadata(&path)?.permissions().mode();
                if mode & 0o077 != 0 {
                    bail!("{} ist für andere lesbar (Rechte {:o}) – bitte neu eingeben", path.display(), mode & 0o777);
                }
                let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
                let secret = text.trim_end_matches(['\n', '\r']).to_owned();
                Ok((!secret.is_empty()).then_some((secret, Kind::File)))
            }
            None => Ok(None),
        }
    }

    /// Forgets the secret (both forms).
    pub fn forget(&self) -> Result<()> {
        remove_if_exists(&self.encrypted_path())?;
        remove_if_exists(&self.file_path())
    }
}

fn create_private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("cannot protect {}", dir.display()))
}

/// Writes `contents` atomically, readable only by the user from the start.
fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("file without parent directory")?;
    let tmp = parent.join(format!(".{}.tmp-{}", path.file_name().unwrap_or_default().to_string_lossy(), unique()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        file.write_all(contents)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("cannot remove {}", path.display())),
    }
}

/// Runs `program` with `input` on stdin; its stdout on success. Killed
/// after `TIMEOUT`; errors carry the last line of stderr.
fn run(program: &str, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("„{program}“ kann nicht gestartet werden"))?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input)?;
    }
    let mut stdout = child.stdout.take().context("no stdout")?;
    let mut stderr = child.stderr.take().context("no stderr")?;
    let out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            bail!("„{program}“ hat nach {} s nicht geantwortet", TIMEOUT.as_secs());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if !status.success() {
        let detail = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_owned();
        bail!(
            "„{program}“ ist fehlgeschlagen ({status}){}",
            if detail.is_empty() { String::new() } else { format!(": {detail}") }
        );
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_creds(dir: &Path) -> String {
        // Stands in for systemd-creds: "encrypts" with ROT13 (`tr` is in
        // every build environment, unlike `rev`).
        let script = dir.join("fake-creds");
        std::fs::write(
            &script,
            "#!/bin/sh\n\
             [ \"$1\" = --user ] || exit 9\n\
             case \"$2\" in\n\
               encrypt) tr a-z n-za-m > \"$5\" ;;\n\
               decrypt) tr a-z n-za-m < \"$4\" ;;\n\
               *) exit 8 ;;\n\
             esac\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script.to_string_lossy().into_owned()
    }

    #[test]
    fn encrypted_with_systemd_creds() {
        let tmp = tempfile::tempdir().unwrap();
        let store =
            SecretStore::new(tmp.path().join("secrets"), "nextcloud").with_creds_program(&fake_creds(tmp.path()));
        assert_eq!(store.kind(), None);
        assert_eq!(store.store("geheim 123").unwrap(), Kind::Encrypted);
        let raw = std::fs::read_to_string(tmp.path().join("secrets/nextcloud.cred")).unwrap();
        assert!(!raw.contains("geheim"), "{raw}");
        assert_eq!(store.load().unwrap(), Some(("geheim 123".to_owned(), Kind::Encrypted)));
        let mode = |p: &str| std::fs::metadata(tmp.path().join(p)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("secrets"), 0o700);
        assert_eq!(mode("secrets/nextcloud.cred"), 0o600);
        store.forget().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn private_file_without_systemd_creds() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SecretStore::new(tmp.path().join("secrets"), "nextcloud").with_creds_program("/nonexistent/creds");
        assert_eq!(store.store("geheim").unwrap(), Kind::File);
        let path = tmp.path().join("secrets/nextcloud.secret");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(store.load().unwrap(), Some(("geheim".to_owned(), Kind::File)));
        // Readable by others: refused instead of used.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.load().is_err());
        store.forget().unwrap();
        assert_eq!(store.kind(), None);
    }

    #[test]
    fn switching_forms_leaves_one_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("secrets");
        SecretStore::new(&dir, "nc").with_creds_program("/nonexistent").store("a").unwrap();
        let store = SecretStore::new(&dir, "nc").with_creds_program(&fake_creds(tmp.path()));
        assert_eq!(store.store("b").unwrap(), Kind::Encrypted);
        assert!(!dir.join("nc.secret").exists());
        assert_eq!(store.load().unwrap().unwrap().0, "b");
        assert!(store.store("").is_err());
        assert!(store.store("a\nb").is_err());
    }

    #[test]
    fn concurrent_stores_do_not_collide() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("s");
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    SecretStore::new(&dir, "nc").with_creds_program("/nonexistent").store(&format!("pw{i}"))
                })
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap().unwrap(), Kind::File);
        }
        let stored = SecretStore::new(&dir, "nc").load().unwrap().unwrap().0;
        assert!(stored.starts_with("pw"), "{stored}");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn encrypted_only_stores_nothing_without_systemd_creds() {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("kaputt");
        let store = SecretStore::new(dir.path().join("secrets"), "ilias").with_creds_program(&broken.to_string_lossy());
        assert!(store.store_encrypted("geheim").is_err());
        assert_eq!(store.kind(), None, "no plain file as a fallback");

        let creds = fake_creds(dir.path());
        let store = SecretStore::new(dir.path().join("secrets"), "ilias").with_creds_program(&creds);
        store.store_encrypted("geheim").unwrap();
        assert_eq!(store.kind(), Some(Kind::Encrypted));
        assert_eq!(store.load().unwrap(), Some(("geheim".to_owned(), Kind::Encrypted)));
    }

    #[test]
    fn broken_systemd_creds_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        // Encrypts, but cannot decrypt: the round trip fails.
        let script = tmp.path().join("half-creds");
        std::fs::write(&script, "#!/bin/sh\n[ \"$2\" = encrypt ] && cat > \"$5\" && exit 0\nexit 1\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let store = SecretStore::new(tmp.path().join("s"), "nc").with_creds_program(&script.to_string_lossy());
        assert_eq!(store.store("x").unwrap(), Kind::File);
        assert!(!tmp.path().join("s/nc.cred").exists());
    }
}
