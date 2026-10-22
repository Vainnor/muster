//! Windows: Start Menu shortcuts (`.lnk`) under `%APPDATA%` and `%ProgramData%`
//! `\Microsoft\Windows\Start Menu\Programs`.
//!
//! The `.lnk` reader implements the part of the Shell Link Binary File Format (MS-SHLLINK) a
//! launcher needs: the header, `LinkInfo`'s local base path, the string data (arguments,
//! working directory, icon location) and the environment-variable data block. It reads bytes
//! only: no COM, no shell API, so it runs and is tested on every OS. Every offset is
//! bounds-checked; a malformed file is skipped.
//!
//! Listed: shortcuts whose target is an `.exe`. Skipped: uninstallers, shortcuts to documents,
//! folders or URLs, Windows Installer "advertised" shortcuts (their target is resolved by
//! MSI at launch), and shortcuts that only carry a shell item list. Store (UWP) apps are not
//! Start Menu shortcuts and are not detected.

use std::collections::HashSet;
use std::path::PathBuf;

use super::{DetectedApp, MAX_APPS, Scan, clean_name, read_small, walk};
use crate::launch::{LaunchTarget, MAX_ARG_LEN, MAX_ARGS};

/// Start Menu folders nest a vendor folder and occasionally one more.
const MAX_DEPTH: usize = 3;

/// The user's Start Menu first, then the machine-wide one.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    ["APPDATA", "ProgramData"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|base| base.join(r"Microsoft\Windows\Start Menu\Programs"))
        .collect()
}

/// Scans `roots` in order. `env` resolves `%VARIABLE%` references in shortcut paths. Two
/// shortcuts to the same program with the same arguments are listed once.
#[must_use]
pub fn scan(roots: &[PathBuf], env: &dyn Fn(&str) -> Option<String>) -> Scan {
    let mut seen = HashSet::new();
    let mut scan = Scan::default();
    for root in roots {
        walk(root, MAX_DEPTH, &mut |v| {
            if v.is_dir {
                return true;
            }
            let Some(stem) = v.relative.strip_suffix(".lnk") else {
                return false;
            };
            if scan.apps.len() >= MAX_APPS {
                return false;
            }
            let display = stem.rsplit('/').next().unwrap_or(stem);
            match read_small(v.path).map(|bytes| shortcut_app(&bytes, display, v.relative, env)) {
                Some(Ok(Some(app))) => {
                    if seen.insert(dedupe_key(&app.target)) {
                        scan.apps.push(app);
                    }
                }
                Some(Ok(None)) => {}
                Some(Err(_)) | None => scan.skipped += 1,
            }
            false
        });
    }
    scan
}

fn dedupe_key(target: &LaunchTarget) -> String {
    match target {
        LaunchTarget::Program { path, args, .. } => {
            format!(
                "{}\0{}",
                path.to_string_lossy().to_lowercase(),
                args.join("\0")
            )
        }
        other => format!("{other:?}"),
    }
}

/// Turns one shortcut into an app. `Ok(None)` means a valid shortcut that is not listed.
fn shortcut_app(
    bytes: &[u8],
    display: &str,
    relative: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<DetectedApp>, LinkError> {
    let link = parse_lnk(bytes)?;
    if link.darwin {
        return Ok(None);
    }
    let raw_target = link
        .env_target
        .as_deref()
        .or(link.local_path.as_deref())
        .ok_or(LinkError::NoTarget)?;
    let target = expand_env(raw_target, env).ok_or(LinkError::UnknownVariable)?;
    if !is_absolute_windows(&target) {
        return Err(LinkError::NoTarget);
    }
    let file_name = target.rsplit(['\\', '/']).next().unwrap_or(&target);
    let lower = file_name.to_ascii_lowercase();
    let is_exe = lower.rsplit_once('.').is_some_and(|(_, ext)| ext == "exe");
    if !is_exe || lower.starts_with("unins") {
        return Ok(None);
    }
    if display.to_ascii_lowercase().contains("uninstall") {
        return Ok(None);
    }
    let args = match &link.arguments {
        Some(a) => split_command_line(&expand_env(a, env).ok_or(LinkError::UnknownVariable)?)?,
        None => Vec::new(),
    };
    let cwd = link
        .working_dir
        .as_deref()
        .and_then(|d| expand_env(d, env))
        .filter(|d| is_absolute_windows(d))
        .map(PathBuf::from);
    let icon = link
        .icon_location
        .as_deref()
        .and_then(|i| expand_env(i, env))
        .filter(|i| {
            is_absolute_windows(i)
                && i.rsplit_once('.')
                    .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("ico"))
        })
        .map(PathBuf::from);
    let Some(name) = clean_name(display) else {
        return Err(LinkError::Malformed);
    };
    Ok(Some(DetectedApp {
        key: relative.to_owned(),
        name,
        target: LaunchTarget::Program {
            path: PathBuf::from(target),
            args,
            cwd,
        },
        icon,
    }))
}

/// `C:\…` or `\\server\…`. Checked as text so it behaves the same on every OS.
fn is_absolute_windows(path: &str) -> bool {
    let b = path.as_bytes();
    matches!(b, [d, b':', b'\\' | b'/', ..] if d.is_ascii_alphabetic()) || path.starts_with(r"\\")
}

/// Why a `.lnk` file was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("not a shell link")]
    NotALink,
    #[error("shell link is truncated or has an invalid offset")]
    Malformed,
    #[error("shell link has no local target path")]
    NoTarget,
    #[error("shell link references an unknown environment variable")]
    UnknownVariable,
    #[error("arguments are unterminated, too many or too long")]
    BadArguments,
}

/// The fields of a shortcut Muster uses. Paths are as stored, before `%VAR%` expansion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellLink {
    pub local_path: Option<String>,
    /// From the environment-variable data block; preferred over `local_path` when present,
    /// because it is what Windows itself resolves.
    pub env_target: Option<String>,
    pub arguments: Option<String>,
    pub working_dir: Option<String>,
    pub icon_location: Option<String>,
    /// An MSI advertised shortcut (`HasDarwinID`).
    pub darwin: bool,
}

const HEADER_SIZE: usize = 0x4C;
const LINK_CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];

const HAS_LINK_TARGET_ID_LIST: u32 = 0x1;
const HAS_LINK_INFO: u32 = 0x2;
const HAS_NAME: u32 = 0x4;
const HAS_RELATIVE_PATH: u32 = 0x8;
const HAS_WORKING_DIR: u32 = 0x10;
const HAS_ARGUMENTS: u32 = 0x20;
const HAS_ICON_LOCATION: u32 = 0x40;
const IS_UNICODE: u32 = 0x80;
const FORCE_NO_LINK_INFO: u32 = 0x100;
const HAS_DARWIN_ID: u32 = 0x1000;

const VOLUME_ID_AND_LOCAL_BASE_PATH: u32 = 0x1;
const ENVIRONMENT_BLOCK: u32 = 0xA000_0001;

struct Reader<'a> {
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn slice(&self, at: usize, len: usize) -> Result<&[u8], LinkError> {
        at.checked_add(len)
            .and_then(|end| self.bytes.get(at..end))
            .ok_or(LinkError::Malformed)
    }

    fn u16(&self, at: usize) -> Result<u16, LinkError> {
        let b = self
            .slice(at, 2)?
            .try_into()
            .map_err(|_| LinkError::Malformed)?;
        Ok(u16::from_le_bytes(b))
    }

    fn u32(&self, at: usize) -> Result<u32, LinkError> {
        let b = self
            .slice(at, 4)?
            .try_into()
            .map_err(|_| LinkError::Malformed)?;
        Ok(u32::from_le_bytes(b))
    }

    fn usize32(&self, at: usize) -> Result<usize, LinkError> {
        usize::try_from(self.u32(at)?).map_err(|_| LinkError::Malformed)
    }

    /// NUL-terminated single-byte string. Only ASCII is accepted: the code page that wrote
    /// it is unknown, and a guess could produce a different path.
    fn ascii_z(&self, at: usize) -> Result<String, LinkError> {
        let rest = self.bytes.get(at..).ok_or(LinkError::Malformed)?;
        let end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(LinkError::Malformed)?;
        let s = rest.get(..end).ok_or(LinkError::Malformed)?;
        if !s.is_ascii() {
            return Err(LinkError::NoTarget);
        }
        Ok(String::from_utf8_lossy(s).into_owned())
    }

    /// NUL-terminated UTF-16LE string.
    fn utf16_z(&self, at: usize) -> Result<String, LinkError> {
        let mut units = Vec::new();
        let mut i = at;
        loop {
            let u = self.u16(i)?;
            if u == 0 {
                break;
            }
            units.push(u);
            i += 2;
        }
        String::from_utf16(&units).map_err(|_| LinkError::Malformed)
    }

    /// Fixed-size NUL-padded UTF-16LE field.
    fn utf16_fixed(&self, at: usize, bytes: usize) -> Result<String, LinkError> {
        let raw = self.slice(at, bytes)?;
        let units: Vec<u16> = utf16_units(raw).take_while(|&u| u != 0).collect();
        String::from_utf16(&units).map_err(|_| LinkError::Malformed)
    }
}

fn utf16_units(raw: &[u8]) -> impl Iterator<Item = u16> + '_ {
    raw.as_chunks::<2>()
        .0
        .iter()
        .copied()
        .map(u16::from_le_bytes)
}

/// Parses a `.lnk` file.
pub fn parse_lnk(bytes: &[u8]) -> Result<ShellLink, LinkError> {
    let r = Reader { bytes };
    if r.u32(0).map_err(|_| LinkError::NotALink)? != 0x4C
        || r.slice(4, 16).ok() != Some(&LINK_CLSID[..])
    {
        return Err(LinkError::NotALink);
    }
    let flags = r.u32(20)?;
    let mut link = ShellLink {
        darwin: flags & HAS_DARWIN_ID != 0,
        ..ShellLink::default()
    };
    let mut at = HEADER_SIZE;

    if flags & HAS_LINK_TARGET_ID_LIST != 0 {
        at = at
            .checked_add(2 + usize::from(r.u16(at)?))
            .ok_or(LinkError::Malformed)?;
    }

    if flags & HAS_LINK_INFO != 0 {
        let size = r.usize32(at)?;
        if flags & FORCE_NO_LINK_INFO == 0 {
            link.local_path = link_info_path(&r, at, size)?;
        }
        at = at.checked_add(size).ok_or(LinkError::Malformed)?;
    }

    let unicode = flags & IS_UNICODE != 0;
    let mut read_string = |present: bool| -> Result<Option<String>, LinkError> {
        if !present {
            return Ok(None);
        }
        let count = usize::from(r.u16(at)?);
        let start = at + 2;
        let value = if unicode {
            let raw = r.slice(start, count * 2)?;
            let units: Vec<u16> = utf16_units(raw).collect();
            at = start + count * 2;
            String::from_utf16(&units).map_err(|_| LinkError::Malformed)?
        } else {
            let raw = r.slice(start, count)?;
            at = start + count;
            if !raw.is_ascii() {
                return Err(LinkError::Malformed);
            }
            String::from_utf8_lossy(raw).into_owned()
        };
        Ok(Some(value).filter(|s| !s.is_empty()))
    };
    read_string(flags & HAS_NAME != 0)?;
    read_string(flags & HAS_RELATIVE_PATH != 0)?;
    link.working_dir = read_string(flags & HAS_WORKING_DIR != 0)?;
    link.arguments = read_string(flags & HAS_ARGUMENTS != 0)?;
    link.icon_location = read_string(flags & HAS_ICON_LOCATION != 0)?;

    // Extra data: size-prefixed blocks, ended by a terminal block whose size is under 4.
    // Windows always writes the terminal block, so its absence means a truncated file.
    loop {
        let size = r.usize32(at)?;
        if size < 4 {
            break;
        }
        if size < 8 {
            return Err(LinkError::Malformed);
        }
        if r.u32(at + 4)? == ENVIRONMENT_BLOCK && size >= 0x314 {
            let unicode_target = r.utf16_fixed(at + 8 + 260, 520)?;
            let target = if unicode_target.is_empty() {
                r.ascii_z(at + 8).ok()
            } else {
                Some(unicode_target)
            };
            link.env_target = target.filter(|t| !t.is_empty());
        }
        at = at.checked_add(size).ok_or(LinkError::Malformed)?;
    }
    Ok(link)
}

/// `LocalBasePath` (+ `CommonPathSuffix`) from a `LinkInfo` structure at `base`.
fn link_info_path(r: &Reader<'_>, base: usize, size: usize) -> Result<Option<String>, LinkError> {
    let field = |offset: usize| -> Result<usize, LinkError> {
        let value = r.usize32(base + offset)?;
        if value >= size {
            return Err(LinkError::Malformed);
        }
        Ok(value)
    };
    let header_size = r.usize32(base + 4)?;
    let info_flags = r.u32(base + 8)?;
    if info_flags & VOLUME_ID_AND_LOCAL_BASE_PATH == 0 {
        return Ok(None);
    }
    if header_size >= 0x24 {
        let local = field(0x1C)?;
        let suffix = field(0x20)?;
        if local != 0 {
            let mut path = r.utf16_z(base + local)?;
            if suffix != 0 {
                path.push_str(&r.utf16_z(base + suffix)?);
            }
            return Ok(Some(path));
        }
    }
    let mut path = r.ascii_z(base + field(0x10)?)?;
    let suffix = field(0x18)?;
    if suffix != 0 {
        path.push_str(&r.ascii_z(base + suffix)?);
    }
    Ok(Some(path))
}

/// Replaces `%NAME%` with `env(NAME)`. `None` if a variable is unknown or a `%` is unpaired.
fn expand_env(value: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(rest.get(..start)?);
        let after = rest.get(start + 1..)?;
        let end = after.find('%')?;
        let name = after.get(..end)?;
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_()".contains(c))
        {
            return None;
        }
        out.push_str(&env(name)?);
        rest = after.get(end + 1..)?;
    }
    out.push_str(rest);
    Some(out)
}

/// Splits a Windows argument string the way the Microsoft C runtime does
/// (`CommandLineToArgvW` for arguments after the program name): whitespace separates
/// arguments outside quotes; `2n` backslashes before `"` give `n` backslashes and toggle
/// quoting; `2n+1` give `n` and a literal `"`; `""` inside quotes is a literal `"`.
pub fn split_command_line(line: &str) -> Result<Vec<String>, LinkError> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' if !quoted => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '\\' => {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                started = true;
                if chars.peek() == Some(&'"') {
                    current.extend(std::iter::repeat_n('\\', count / 2));
                    if count % 2 == 1 {
                        chars.next();
                        current.push('"');
                    }
                } else {
                    current.extend(std::iter::repeat_n('\\', count));
                }
            }
            '"' => {
                started = true;
                if quoted && chars.peek() == Some(&'"') {
                    chars.next();
                    current.push('"');
                } else {
                    quoted = !quoted;
                }
            }
            other => {
                started = true;
                current.push(other);
            }
        }
        if args.len() > MAX_ARGS || current.len() > MAX_ARG_LEN {
            return Err(LinkError::BadArguments);
        }
    }
    if started {
        args.push(current);
    }
    if args.len() > MAX_ARGS {
        return Err(LinkError::BadArguments);
    }
    Ok(args)
}

/// Builds `.lnk` bytes for tests. Mirrors the layout `parse_lnk` reads, and is checked
/// against a shortcut written by Windows itself in `tests/detection.rs`.
#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    #[derive(Default)]
    pub(crate) struct Lnk<'a> {
        pub unicode_path: Option<&'a str>,
        pub ascii_path: Option<&'a str>,
        pub env_target: Option<&'a str>,
        pub arguments: Option<&'a str>,
        pub working_dir: Option<&'a str>,
        pub icon: Option<&'a str>,
        pub darwin: bool,
        pub id_list: bool,
    }

    fn utf16z(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    pub(crate) fn build(l: &Lnk<'_>) -> Vec<u8> {
        let mut flags = IS_UNICODE;
        let has_info = l.unicode_path.is_some() || l.ascii_path.is_some();
        for (on, bit) in [
            (l.id_list, HAS_LINK_TARGET_ID_LIST),
            (has_info, HAS_LINK_INFO),
            (l.working_dir.is_some(), HAS_WORKING_DIR),
            (l.arguments.is_some(), HAS_ARGUMENTS),
            (l.icon.is_some(), HAS_ICON_LOCATION),
            (l.darwin, HAS_DARWIN_ID),
        ] {
            if on {
                flags |= bit;
            }
        }
        let mut out = Vec::new();
        out.extend(0x4Cu32.to_le_bytes());
        out.extend(LINK_CLSID);
        out.extend(flags.to_le_bytes());
        out.resize(HEADER_SIZE, 0);
        if l.id_list {
            out.extend(4u16.to_le_bytes());
            out.extend([2, 0, 0, 0]);
        }
        if has_info {
            let header = if l.unicode_path.is_some() { 0x24 } else { 0x1C };
            let ascii = format!("{}\0", l.ascii_path.unwrap_or(""));
            let suffix_at = header + ascii.len();
            let unicode_at = suffix_at + 1;
            let mut info = Vec::new();
            let total = unicode_at + l.unicode_path.map_or(0, |p| utf16z(p).len() + 2);
            for v in [total, header, 1, 0, header, 0, suffix_at] {
                info.extend(u32::try_from(v).unwrap().to_le_bytes());
            }
            if let Some(p) = l.unicode_path {
                let suffix_unicode_at = unicode_at + utf16z(p).len();
                info.extend(u32::try_from(unicode_at).unwrap().to_le_bytes());
                info.extend(u32::try_from(suffix_unicode_at).unwrap().to_le_bytes());
            }
            info.extend(ascii.as_bytes());
            info.push(0);
            if let Some(p) = l.unicode_path {
                info.extend(utf16z(p));
                info.extend([0, 0]);
            }
            assert_eq!(info.len(), total);
            out.extend(info);
        }
        for s in [l.working_dir, l.arguments, l.icon].into_iter().flatten() {
            out.extend(
                u16::try_from(s.encode_utf16().count())
                    .unwrap()
                    .to_le_bytes(),
            );
            out.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
        }
        if let Some(t) = l.env_target {
            let mut block = Vec::new();
            block.extend(0x314u32.to_le_bytes());
            block.extend(ENVIRONMENT_BLOCK.to_le_bytes());
            block.resize(8 + 260, 0);
            let mut wide: Vec<u8> = t.encode_utf16().flat_map(u16::to_le_bytes).collect();
            wide.resize(520, 0);
            block.extend(wide);
            out.extend(block);
        }
        out.extend(0u32.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::fixture::{Lnk, build};
    use super::*;

    fn env(name: &str) -> Option<String> {
        match name {
            "ProgramFiles" => Some(r"C:\Program Files".into()),
            "windir" => Some(r"C:\Windows".into()),
            _ => None,
        }
    }

    fn app(l: &Lnk<'_>, display: &str) -> Result<Option<DetectedApp>, LinkError> {
        shortcut_app(&build(l), display, &format!("{display}.lnk"), &env)
    }

    #[test]
    fn reads_unicode_path_arguments_and_working_dir() {
        let a = app(
            &Lnk {
                unicode_path: Some(r"C:\Program Files\Édit\edit.exe"),
                ascii_path: Some(""),
                arguments: Some(r#"--profile "Work Stuff" C:\x\\"#),
                working_dir: Some(r"%ProgramFiles%\Édit"),
                icon: Some(r"C:\Program Files\Édit\edit.ico"),
                id_list: true,
                ..Lnk::default()
            },
            "Édit",
        )
        .unwrap()
        .unwrap();
        assert_eq!(a.name, "Édit");
        assert_eq!(
            a.target,
            LaunchTarget::Program {
                path: r"C:\Program Files\Édit\edit.exe".into(),
                args: vec!["--profile".into(), "Work Stuff".into(), r"C:\x\\".into()],
                cwd: Some(r"C:\Program Files\Édit".into()),
            }
        );
        assert_eq!(a.icon, Some(r"C:\Program Files\Édit\edit.ico".into()));
    }

    #[test]
    fn reads_ascii_path_and_prefers_environment_block() {
        let ascii = app(
            &Lnk {
                ascii_path: Some(r"C:\Tools\a.exe"),
                ..Lnk::default()
            },
            "A",
        )
        .unwrap()
        .unwrap();
        assert!(
            matches!(ascii.target, LaunchTarget::Program { ref path, .. } if path == Path::new(r"C:\Tools\a.exe"))
        );

        let env_only = app(
            &Lnk {
                env_target: Some(r"%windir%\system32\notepad.exe"),
                ..Lnk::default()
            },
            "Notepad",
        )
        .unwrap()
        .unwrap();
        assert!(
            matches!(env_only.target, LaunchTarget::Program { ref path, .. }
            if path == Path::new(r"C:\Windows\system32\notepad.exe"))
        );
    }

    #[test]
    fn skips_non_exe_uninstallers_and_advertised_shortcuts() {
        let path = |p| Lnk {
            unicode_path: Some(p),
            ascii_path: Some(""),
            ..Lnk::default()
        };
        assert_eq!(app(&path(r"C:\App\readme.txt"), "Readme"), Ok(None));
        assert_eq!(app(&path(r"C:\App\unins000.exe"), "App"), Ok(None));
        assert_eq!(app(&path(r"C:\App\remove.exe"), "Uninstall App"), Ok(None));
        let darwin = Lnk {
            darwin: true,
            ..path(r"C:\App\app.exe")
        };
        assert_eq!(app(&darwin, "App"), Ok(None));
    }

    #[test]
    fn rejects_unknown_variables_relative_targets_and_id_list_only() {
        assert_eq!(
            app(
                &Lnk {
                    env_target: Some(r"%NOPE%\a.exe"),
                    ..Lnk::default()
                },
                "A"
            ),
            Err(LinkError::UnknownVariable)
        );
        assert_eq!(
            app(
                &Lnk {
                    ascii_path: Some(r"Tools\a.exe"),
                    ..Lnk::default()
                },
                "A"
            ),
            Err(LinkError::NoTarget)
        );
        assert_eq!(
            app(
                &Lnk {
                    id_list: true,
                    ..Lnk::default()
                },
                "A"
            ),
            Err(LinkError::NoTarget)
        );
    }

    #[test]
    fn rejects_non_links_and_every_truncation() {
        assert_eq!(parse_lnk(b"MZ not a link"), Err(LinkError::NotALink));
        let full = build(&Lnk {
            unicode_path: Some(r"C:\a.exe"),
            ascii_path: Some(""),
            arguments: Some("-x"),
            env_target: Some(r"C:\a.exe"),
            id_list: true,
            ..Lnk::default()
        });
        assert!(parse_lnk(&full).is_ok());
        // Any cut must fail, not panic or return a partial link.
        for len in 0..full.len() {
            assert!(parse_lnk(&full[..len]).is_err(), "truncated to {len}");
        }
    }

    #[test]
    fn rejects_out_of_range_link_info_offsets() {
        let mut bytes = build(&Lnk {
            ascii_path: Some(r"C:\a.exe"),
            ..Lnk::default()
        });
        let local_base_path_offset = HEADER_SIZE + 0x10;
        bytes[local_base_path_offset..local_base_path_offset + 4]
            .copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
        assert_eq!(parse_lnk(&bytes), Err(LinkError::Malformed));
    }

    #[test]
    fn splits_like_the_c_runtime() {
        let split = |s| split_command_line(s).unwrap();
        assert_eq!(split(r#"a "b c" d"#), ["a", "b c", "d"]);
        assert_eq!(split(r#""a\"b""#), [r#"a"b"#]);
        assert_eq!(split(r#"a\\\"b"#), [r#"a\"b"#]);
        assert_eq!(split(r#""a\\" b"#), [r"a\", "b"]);
        assert_eq!(split(r"C:\dir\ x"), [r"C:\dir\", "x"]);
        assert_eq!(split(r#""say ""hi""""#), [r#"say "hi""#]);
        assert_eq!(split(r#""""#), [""]);
        assert_eq!(split("  "), Vec::<String>::new());
        assert_eq!(
            split_command_line(&"x ".repeat(MAX_ARGS + 1)),
            Err(LinkError::BadArguments)
        );
    }

    #[test]
    fn expands_known_variables_only() {
        assert_eq!(
            expand_env(r"%windir%\x", &env),
            Some(r"C:\Windows\x".into())
        );
        assert_eq!(expand_env("plain", &env), Some("plain".into()));
        assert_eq!(expand_env("%missing%", &env), None);
        assert_eq!(expand_env("100%", &env), None);
    }

    #[test]
    fn scan_walks_vendor_folders_and_dedupes_targets() {
        let tmp = tempfile::tempdir().unwrap();
        let user = tmp.path().join("user");
        let machine = tmp.path().join("machine");
        std::fs::create_dir_all(user.join("Vendor")).unwrap();
        std::fs::create_dir_all(&machine).unwrap();
        let lnk = |p| {
            build(&Lnk {
                unicode_path: Some(p),
                ascii_path: Some(""),
                ..Lnk::default()
            })
        };
        std::fs::write(user.join("Vendor/Editor.lnk"), lnk(r"C:\Editor\editor.exe")).unwrap();
        std::fs::write(machine.join("Editor.lnk"), lnk(r"c:\editor\EDITOR.exe")).unwrap();
        std::fs::write(machine.join("Broken.lnk"), b"junk").unwrap();
        std::fs::write(machine.join("Help.lnk"), lnk(r"C:\Editor\help.chm")).unwrap();

        let scan = scan(&[user, machine], &env);
        let found: Vec<_> = scan
            .apps
            .iter()
            .map(|a| (a.key.as_str(), a.name.as_str()))
            .collect();
        assert_eq!(found, [("Vendor/Editor.lnk", "Editor")]);
        assert_eq!(scan.skipped, 1);
    }
}
