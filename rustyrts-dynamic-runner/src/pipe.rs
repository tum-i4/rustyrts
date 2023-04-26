#![cfg(target_family = "unix")]

use libc::{c_int, pipe};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{from_reader, to_writer};
use std::fs::File;
use std::io::{BufReader, BufWriter, Error, Result};
use std::marker;
use std::os::fd::FromRawFd;

pub fn create_pipes<T>() -> Result<(ReadHandle<T>, WriteHandle<T>)>
where
    T: DeserializeOwned + Serialize,
{
    let fds: [c_int; 2] = [0, 0];
    // SAFETY: Just a call to libc
    if unsafe { pipe(&fds[0] as *const c_int as *mut c_int) } == 0 {
        Ok((ReadHandle::new(fds[0]), WriteHandle::new(fds[1])))
    } else {
        Err(Error::last_os_error())
    }
}

pub struct ReadHandle<T> {
    read: File,
    _phantom: marker::PhantomData<T>,
}

impl<'de, T> ReadHandle<T>
where
    T: DeserializeOwned,
{
    pub fn new(fd: c_int) -> Self {
        Self {
            // SAFETY: fd has been created by pipe() and is a valid file descriptor
            read: unsafe { File::from_raw_fd(fd) },
            _phantom: marker::PhantomData,
        }
    }

    pub fn recv(&mut self) -> Result<T> {
        let reader = BufReader::new(&self.read);
        let result = from_reader(reader)?;
        Ok(result)
    }
}

pub struct WriteHandle<T> {
    write: File,
    _phantom: marker::PhantomData<T>,
}

impl<T> WriteHandle<T>
where
    T: Serialize,
{
    pub fn new(fd: c_int) -> Self {
        Self {
            // SAFETY: fd has been created by pipe() and is a valid file descriptor
            write: unsafe { File::from_raw_fd(fd) },
            _phantom: marker::PhantomData,
        }
    }

    pub fn send(&mut self, t: T) -> Result<()> {
        let writer = BufWriter::new(&self.write);
        to_writer(writer, &t)?;
        Ok(())
    }
}
