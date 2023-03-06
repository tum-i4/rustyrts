use libc::{c_char, mkfifo};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{from_reader, to_writer};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Error, Result};
use std::marker;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub struct Fifo<T> {
    path: PathBuf,
    _phantom: marker::PhantomData<T>,
}

impl<T> Fifo<T> {
    pub fn new(path: PathBuf) -> Result<Self> {
        let os_str = path.clone().into_os_string();
        let slice = os_str.as_bytes();
        let mut bytes = Vec::with_capacity(slice.len() + 1);
        bytes.extend_from_slice(slice);
        bytes.push(0);
        std::fs::remove_file(&path).unwrap_or_default();
        if unsafe { mkfifo((&bytes[0]) as *const u8 as *const c_char, 0o644) } != 0 {
            Err(Error::last_os_error())
        } else {
            Ok(Fifo {
                path,
                _phantom: marker::PhantomData,
            })
        }
    }

    pub fn open(self) -> Result<FifoReadHandle<T>> {
        let pipe = OpenOptions::new().read(true).open(&self.path)?;

        Ok(FifoReadHandle {
            path: self.path,
            read: pipe,
            _phantom: marker::PhantomData,
        })
    }
}

pub struct FifoReadHandle<T> {
    path: PathBuf,
    read: File,
    _phantom: marker::PhantomData<T>,
}

impl<'de, T> FifoReadHandle<T>
where
    T: DeserializeOwned,
{
    pub fn recv(&mut self) -> Result<T> {
        let reader = BufReader::new(&self.read);
        let result = from_reader(reader)?;
        Ok(result)
    }
}

impl<T> Drop for FifoReadHandle<T> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct FifoWriteHandle<T> {
    write: File,
    _phantom: marker::PhantomData<T>,
}

impl<T> FifoWriteHandle<T>
where
    T: Serialize,
{
    pub fn send(&mut self, t: T) -> Result<()> {
        let writer = BufWriter::new(&self.write);
        to_writer(writer, &t)?;
        Ok(())
    }
}

impl<T> FifoWriteHandle<T> {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let pipe = OpenOptions::new().write(true).open(path.as_ref())?;
        Ok(FifoWriteHandle {
            write: pipe,
            _phantom: marker::PhantomData,
        })
    }
}
