#![allow(dead_code)]

use crate::constants::{
    DIR_DYNAMIC, DIR_GENERAL, DIR_STATIC, ENDING_CHANGES, ENDING_CHECKSUM, ENDING_CHECKSUM_CONST,
    ENDING_CHECKSUM_VTBL, ENDING_GRAPH, ENDING_TEST, ENDING_TRACE, ENV_TARGET_DIR,
};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::{
    fs::{read_to_string, DirEntry, OpenOptions},
    path::Path,
};
use std::{hash::Hash, str::FromStr};

#[cfg(unix)]
use crate::constants::ENDING_PROCESS_TRACE;

#[derive(Debug, Clone, Copy)]
pub enum CacheKind {
    Static,
    Dynamic,
    General,
}

impl CacheKind {
    pub fn map(self, path_buf: PathBuf) -> PathBuf {
        let mut path_buf = path_buf;
        let path = match self {
            CacheKind::Static => DIR_STATIC,
            CacheKind::Dynamic => DIR_DYNAMIC,
            CacheKind::General => DIR_GENERAL,
        };
        path_buf.push(path);
        path_buf
    }
}

pub fn get_cache_path(kind: CacheKind) -> Option<PathBuf> {
    let path_buf = PathBuf::from(std::env::var(ENV_TARGET_DIR).ok()?);
    Some(kind.map(path_buf))
}

pub enum CacheFileParsingError {
    FoundDirectory,
    InvalidFileName,
    InvalidKind,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum CacheFileKind {
    Tests,
    Changes,
    Checksums(ChecksumKind),
    Graph,
    Traces,

    #[cfg(unix)]
    ProcessTraces,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChecksumKind {
    Checksum,
    VtblChecksum,
    ConstChecksum,
}

impl AsRef<str> for ChecksumKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::Checksum => ENDING_CHECKSUM,
            Self::VtblChecksum => ENDING_CHECKSUM_VTBL,
            Self::ConstChecksum => ENDING_CHECKSUM_CONST,
        }
    }
}

impl FromStr for ChecksumKind {
    type Err = CacheFileParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ENDING_CHECKSUM => Ok(Self::Checksum),
            ENDING_CHECKSUM_VTBL => Ok(Self::VtblChecksum),
            ENDING_CHECKSUM_CONST => Ok(Self::ConstChecksum),
            _ => Err(CacheFileParsingError::InvalidKind),
        }
    }
}

impl AsRef<str> for CacheFileKind {
    fn as_ref(&self) -> &str {
        match self {
            Self::Tests => ENDING_TEST,
            Self::Changes => ENDING_CHANGES,
            Self::Checksums(kind) => kind.as_ref(),
            Self::Graph => ENDING_GRAPH,
            Self::Traces => ENDING_TRACE,

            #[cfg(unix)]
            Self::ProcessTraces => ENDING_PROCESS_TRACE,
        }
    }
}

impl FromStr for CacheFileKind {
    type Err = CacheFileParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ENDING_TEST => Ok(Self::Tests),
            ENDING_CHANGES => Ok(Self::Changes),
            ENDING_GRAPH => Ok(Self::Graph),
            ENDING_TRACE => Ok(Self::Traces),

            #[cfg(unix)]
            ENDING_PROCESS_TRACE => Ok(Self::ProcessTraces),

            _ => FromStr::from_str(s).map(Self::Checksums),
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct CacheFileDescr<'data> {
    pub crate_name: &'data str,
    pub compile_mode: Option<&'data str>,
    pub doctest_name: Option<&'data str>,
    pub kind: CacheFileKind,
}

impl<'data> CacheFileDescr<'data> {
    pub fn new(
        crate_name: &'data str,
        compile_mode: Option<&'data str>,
        doctest_name: Option<&'data str>,
        kind: CacheFileKind,
    ) -> Self {
        Self {
            crate_name,
            compile_mode,
            doctest_name,
            kind,
        }
    }

    pub fn apply(self, buf: &mut PathBuf) {
        let mut file_name = String::new();
        if let Some(mode) = self.compile_mode {
            file_name += mode;
            file_name += "_";
        }
        file_name += self.crate_name;
        if let Some(doctest) = self.doctest_name {
            file_name += "_";
            file_name += doctest;
        }

        buf.push(file_name);
        buf.set_extension(self.kind.as_ref());
    }
}

impl<'data> TryFrom<&'data Path> for CacheFileDescr<'data> {
    type Error = CacheFileParsingError;

    fn try_from(value: &'data Path) -> Result<Self, Self::Error> {
        let mut path_str = value
            .file_stem()
            .ok_or(CacheFileParsingError::FoundDirectory)?
            .to_str()
            .ok_or(CacheFileParsingError::InvalidFileName)?;

        let compile_mode = path_str.split_once('_').map(|(compile_mode, remainder)| {
            path_str = remainder;
            compile_mode
        });

        let doctest_name = path_str
            .rsplit_once('_')
            .map(|(remainder, crate_id)| {
                path_str = remainder;
                crate_id
                    .strip_suffix(']')
                    .ok_or(CacheFileParsingError::InvalidFileName)
            })
            .transpose()?;

        let crate_name = path_str;

        let ending = value
            .extension()
            .and_then(|os| os.to_str())
            .ok_or(CacheFileParsingError::InvalidFileName)?;
        let kind = ending.parse()?;

        Ok(Self {
            crate_name,
            compile_mode,
            doctest_name,
            kind,
        })
    }
}

pub fn read_lines(files: &[DirEntry], file_ending: &str) -> HashSet<String>
where {
    read_lines_filter_map(files, file_ending, |_x| true, |x| x)
}

pub fn read_lines_filter_map<F, M, O>(
    files: &[DirEntry],
    file_ending: &str,
    filter: F,
    mapper: M,
) -> HashSet<O>
where
    F: Fn(&String) -> bool,
    M: std::ops::FnMut(std::string::String) -> O,
    O: Eq + Hash,
{
    let tokens: HashSet<O> = files
        .iter()
        .filter(|path| path.file_name().to_str().unwrap().ends_with(file_ending))
        .flat_map(|path| {
            let content = read_to_string(path.path()).unwrap();
            let lines: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
            lines
        })
        .filter(|line| !line.is_empty())
        .filter(filter)
        .map(mapper)
        .collect();
    tokens
}

/// Computes the location of a file from a closure
/// and overwrites the content of this file
///
/// ## Arguments
/// * `content` - new content of the file
/// * `path_buf` - `PathBuf` that points to the parent directory
/// * `initializer` - function that modifies path_buf - candidates: `get_graph_path`, `get_test_path`, `get_changes_path`
/// * 'append' - whether content should be appended
///
pub fn write_to_file<F, C: AsRef<[u8]>>(content: C, path_buf: PathBuf, initializer: F, append: bool)
where
    F: FnOnce(&mut PathBuf),
{
    let mut path_buf = path_buf;
    initializer(&mut path_buf);

    let mut file = match OpenOptions::new()
        .write(true)
        .append(append)
        .truncate(!append)
        .create(true)
        .open(path_buf.as_path())
    {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    match file.write_all(content.as_ref()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };
}

/// Computes the location of a file from a closure
/// and overwrites the content of this file
///
/// ## Arguments
/// * `content` - new content of the file
/// * `path_buf` - `PathBuf` that points to the parent directory
/// * `initializer` - function that modifies path_buf - candidates: `get_graph_path`, `get_test_path`, `get_changes_path`
///
#[cfg(feature = "fs_lock_file_guard")]
pub fn append_to_file<F, C: AsRef<[u8]>>(content: C, path_buf: PathBuf, initializer: F)
where
    F: FnOnce(&mut PathBuf),
{
    let mut path_buf = path_buf;
    initializer(&mut path_buf);

    let mut file = match OpenOptions::new()
        .append(true)
        .truncate(false)
        .create(true)
        .open(path_buf.as_path())
    {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    let mut lock = file_guard::lock(&mut file, file_guard::Lock::Exclusive, 0, 1)
        .unwrap_or_else(|err| panic!("Failed to lock file: {}", err));

    match lock.write_all(content.as_ref()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };
}

/// Computes the location of a file from a closure
/// and overwrites the content of this file
///
/// ## Arguments
/// * `content` - new content of the file
/// * `path_buf` - `PathBuf` that points to the parent directory
/// * `initializer` - function that modifies path_buf - candidates: `get_graph_path`, `get_test_path`, `get_changes_path`
///
#[cfg(all(unix, feature = "fs_lock_syscall"))]
pub fn append_to_file<F, C: AsRef<[u8]>>(content: C, path_buf: PathBuf, initializer: F)
where
    F: FnOnce(&mut PathBuf),
{
    use std::{arch::asm, os::fd::AsRawFd};

    let mut path_buf = path_buf;
    initializer(&mut path_buf);

    let mut file = match OpenOptions::new()
        .append(true)
        .truncate(false)
        .create(true)
        .open(path_buf.as_path())
    {
        Ok(file) => file,
        Err(reason) => panic!("Failed to create file: {}", reason),
    };

    unsafe {
        let syscall = 73; // __NR_flock
        let fd = file.as_raw_fd();
        let op = 2; // LOCK_EX
        let mut ret: usize;
        asm!(
            "syscall",
            inlateout("rax") syscall as usize => ret,
            in("rdi") fd,
            in("rsi") op,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags)
        );
        if ret != 0 {
            panic!("Failed to lock file {}", path_buf.display());
        }
    }

    match file.write_all(content.as_ref()) {
        Ok(_) => {}
        Err(reason) => panic!("Failed to write to file: {}", reason),
    };

    unsafe {
        let syscall = 73; // __NR_flock
        let fd = file.as_raw_fd();
        let op = 8; // LOCK_UN
        let mut ret: usize;
        asm!(
            "syscall",
            inlateout("rax") syscall as usize => ret,
            in("rdi") fd,
            in("rsi") op,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags)
        );
        if ret != 0 {
            panic!("Failed to unlock file {}", path_buf.display());
        }
    }
}
