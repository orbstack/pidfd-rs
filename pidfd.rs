/*
Copyright (c) 2023 Orbital Labs, LLC <license@orbstack.dev>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

use std::os::fd::{OwnedFd, FromRawFd, AsRawFd, RawFd};

use nix::{libc::{syscall, SYS_pidfd_open, PIDFD_NONBLOCK, SYS_pidfd_send_signal, siginfo_t}, sys::signal::Signal};
use tokio::io::unix::{AsyncFd, AsyncFdReadyGuard};

pub struct PidFd(AsyncFd<OwnedFd>);

impl PidFd {
    pub fn open(pid: i32) -> std::io::Result<Self> {
        let fd = unsafe { syscall(SYS_pidfd_open, pid, PIDFD_NONBLOCK) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd as _) };
        let fd = AsyncFd::new(fd)?;
        Ok(Self(fd))
    }

    pub fn kill(&self, signal: Signal) -> nix::Result<()> {
        let res = unsafe { syscall(SYS_pidfd_send_signal, self.as_raw_fd(), signal, std::ptr::null::<*const siginfo_t>(), 0) };
        if res < 0 {
            return Err(nix::Error::last());
        }

        Ok(())
    }

    pub async fn wait(&self) -> tokio::io::Result<AsyncFdReadyGuard<OwnedFd>> {
        self.0.readable().await
    }
}

impl AsRawFd for PidFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}
