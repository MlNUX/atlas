// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unix permissions, owner and group.
//!
//! Atlas changes them only with the rights the user already has: the mode
//! of own files, the group to one of the user's groups, the owner only when
//! running as root. There is no privilege escalation.

use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Permission bits including setuid/setgid/sticky (`0o7777`).
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub owner: String,
    pub group: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub link_target: Option<PathBuf>,
    pub size: u64,
    /// Seconds since the epoch.
    pub modified: i64,
    pub accessed: i64,
    pub changed: i64,
    pub inode: u64,
    pub links: u64,
}

fn buffer_size(name: libc::c_int) -> usize {
    // SAFETY: sysconf has no preconditions.
    let n = unsafe { libc::sysconf(name) };
    if n > 0 { n as usize } else { 16 * 1024 }
}

/// Name of a user (`uid` as text if unknown).
pub fn user_name(uid: u32) -> String {
    let mut buf = vec![0 as libc::c_char; buffer_size(libc::_SC_GETPW_R_SIZE_MAX)];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    // SAFETY: buffers are live and sized as passed.
    let rc = unsafe { libc::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result) };
    if rc == 0 && !result.is_null() && !pwd.pw_name.is_null() {
        // SAFETY: getpwuid_r filled a NUL-terminated name.
        return unsafe { CStr::from_ptr(pwd.pw_name) }.to_string_lossy().into_owned();
    }
    uid.to_string()
}

/// Name of a group (`gid` as text if unknown).
pub fn group_name(gid: u32) -> String {
    let mut buf = vec![0 as libc::c_char; buffer_size(libc::_SC_GETGR_R_SIZE_MAX)];
    let mut grp: libc::group = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    // SAFETY: buffers are live and sized as passed.
    let rc = unsafe { libc::getgrgid_r(gid, &mut grp, buf.as_mut_ptr(), buf.len(), &mut result) };
    if rc == 0 && !result.is_null() && !grp.gr_name.is_null() {
        // SAFETY: getgrgid_r filled a NUL-terminated name.
        return unsafe { CStr::from_ptr(grp.gr_name) }.to_string_lossy().into_owned();
    }
    gid.to_string()
}

pub fn stat(path: &Path) -> Result<Info> {
    let lmeta = std::fs::symlink_metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    let is_symlink = lmeta.file_type().is_symlink();
    // Permissions of a symlink are meaningless on Linux: show the target's.
    let meta = if is_symlink { std::fs::metadata(path).unwrap_or(lmeta.clone()) } else { lmeta.clone() };
    Ok(Info {
        mode: meta.mode() & 0o7777,
        uid: meta.uid(),
        gid: meta.gid(),
        owner: user_name(meta.uid()),
        group: group_name(meta.gid()),
        is_dir: meta.is_dir(),
        is_symlink,
        link_target: is_symlink.then(|| std::fs::read_link(path).ok()).flatten(),
        size: meta.len(),
        modified: meta.mtime(),
        accessed: meta.atime(),
        changed: meta.ctime(),
        inode: meta.ino(),
        links: meta.nlink(),
    })
}

/// `rwxr-x---` style text (with s/S, t/T for the special bits).
pub fn format_mode(mode: u32) -> String {
    let mut out = String::with_capacity(9);
    let special = [(0o4000, 0o100), (0o2000, 0o010), (0o1000, 0o001)];
    for (i, shift) in [6, 3, 0].into_iter().enumerate() {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 4 != 0 { 'r' } else { '-' });
        out.push(if bits & 2 != 0 { 'w' } else { '-' });
        let (flag, xbit) = special[i];
        let x = mode & xbit != 0;
        out.push(match (mode & flag != 0, x, i) {
            (true, true, 2) => 't',
            (true, false, 2) => 'T',
            (true, true, _) => 's',
            (true, false, _) => 'S',
            (false, true, _) => 'x',
            (false, false, _) => '-',
        });
    }
    out
}

/// Sets the permission bits (`0o7777`). Symbolic links are followed.
pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o7777))
        .map_err(|err| describe(err, path, "Rechte ändern"))
}

/// Changes owner and/or group (`None` keeps it). Symbolic links themselves
/// are changed, not their targets.
pub fn set_owner(path: &Path, uid: Option<u32>, gid: Option<u32>) -> Result<()> {
    std::os::unix::fs::lchown(path, uid, gid).map_err(|err| describe(err, path, "Besitzer oder Gruppe ändern"))
}

fn describe(err: std::io::Error, path: &Path, what: &str) -> anyhow::Error {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match err.raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => {
            anyhow::anyhow!("„{name}“: {what} ist mit Ihren Rechten nicht erlaubt.")
        }
        Some(libc::EROFS) => anyhow::anyhow!("„{name}“ liegt auf einem schreibgeschützten Dateisystem."),
        Some(libc::EINVAL) => {
            anyhow::anyhow!("„{name}“: {what} ist hier nicht möglich (Benutzer oder Gruppe unbekannt).")
        }
        _ => anyhow::anyhow!("„{name}“: {what} fehlgeschlagen: {err}"),
    }
}

/// Groups the current user belongs to (`gid`, name), primary group first.
pub fn own_groups() -> Vec<(u32, String)> {
    // SAFETY: getgroups with 0 returns the count; the second call fills a
    // buffer of exactly that size.
    let mut groups = unsafe {
        let n = libc::getgroups(0, std::ptr::null_mut());
        let mut buf = vec![0 as libc::gid_t; n.max(0) as usize];
        let n = libc::getgroups(buf.len() as libc::c_int, buf.as_mut_ptr());
        buf.truncate(n.max(0) as usize);
        buf
    };
    // SAFETY: getegid has no preconditions.
    let primary = unsafe { libc::getegid() };
    // In user namespaces (containers, the Nix build sandbox) unmapped groups
    // appear as the overflow GID; files cannot be given to it.
    let overflow = std::fs::read_to_string("/proc/sys/kernel/overflowgid")
        .ok()
        .and_then(|s| s.trim().parse::<libc::gid_t>().ok())
        .unwrap_or(65534);
    groups.retain(|&g| g != primary && g != overflow);
    groups.sort_unstable();
    groups.dedup();
    std::iter::once(primary).chain(groups).map(|g| (g, group_name(g))).collect()
}

/// Whether the owner can be changed (only root may give files away).
pub fn can_change_owner() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

/// Whether the current user may change mode and group of `info`'s file.
pub fn can_change_mode(info: &Info) -> bool {
    // SAFETY: geteuid has no preconditions.
    let euid = unsafe { libc::geteuid() };
    euid == 0 || euid == info.uid
}

/// User ID of a user name (for the owner field when running as root).
pub fn uid_of(name: &str) -> Option<u32> {
    if let Ok(uid) = name.parse() {
        return Some(uid);
    }
    let c = CString::new(name).ok()?;
    let mut buf = vec![0 as libc::c_char; buffer_size(libc::_SC_GETPW_R_SIZE_MAX)];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = std::ptr::null_mut();
    // SAFETY: buffers are live and sized as passed.
    let rc = unsafe { libc::getpwnam_r(c.as_ptr(), &mut pwd, buf.as_mut_ptr(), buf.len(), &mut result) };
    (rc == 0 && !result.is_null()).then_some(pwd.pw_uid)
}

/// Whether a path's file system is mounted read-only.
pub fn read_only_fs(path: &Path) -> bool {
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return false };
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: valid path and output struct.
    unsafe { libc::statvfs(c.as_ptr(), &mut st) == 0 && st.f_flag & libc::ST_RDONLY != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_modes() {
        assert_eq!(format_mode(0o755), "rwxr-xr-x");
        assert_eq!(format_mode(0o640), "rw-r-----");
        assert_eq!(format_mode(0o4755), "rwsr-xr-x");
        assert_eq!(format_mode(0o2750), "rwxr-s---");
        assert_eq!(format_mode(0o1777), "rwxrwxrwt");
        assert_eq!(format_mode(0o1666), "rw-rw-rwT");
    }

    #[test]
    fn changes_mode_and_group_with_own_rights() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.sh");
        std::fs::write(&file, "x").unwrap();
        set_mode(&file, 0o750).unwrap();
        let info = stat(&file).unwrap();
        assert_eq!(info.mode, 0o750);
        assert!(can_change_mode(&info));
        assert_eq!(info.owner, user_name(info.uid));

        // The user's own groups are listed, primary first. Setting the
        // file's current group always works; other groups may be unmapped
        // in user namespaces (e.g. the Nix build sandbox).
        assert!(!own_groups().is_empty());
        set_owner(&file, None, Some(info.gid)).unwrap();
        let after = stat(&file).unwrap();
        assert_eq!((after.gid, after.group), (info.gid, info.group.clone()));

        // Giving the file away needs root.
        if !can_change_owner() {
            let err = set_owner(&file, Some(0), None).unwrap_err();
            // EPERM normally; EINVAL where root is not mapped (Nix sandbox).
            assert!(err.to_string().contains("nicht erlaubt") || err.to_string().contains("nicht möglich"), "{err}");
        }
    }

    #[test]
    fn symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("t");
        std::fs::write(&target, "x").unwrap();
        std::os::unix::fs::symlink("t", dir.path().join("l")).unwrap();
        let info = stat(&dir.path().join("l")).unwrap();
        assert!(info.is_symlink);
        assert_eq!(info.link_target.as_deref(), Some(Path::new("t")));
        assert_eq!(uid_of(&user_name(info.uid)), Some(info.uid));
    }
}
