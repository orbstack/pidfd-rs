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

fn kill_one_entry(entry: Result<DirEntry, io::Error>, signal: Signal) -> Result<Option<PidFd>, Box<dyn Error>> {
    let filename = entry?.file_name();
    if let Ok(pid) =  filename.to_str().unwrap().parse::<i32>() {
        // skip pid 1
        if pid == 1 {
            return Ok(None);
        }
        
        // skip kthreads (they won't exit)
        if is_process_kthread(pid)? {
            return Ok(None);
        }

        // open a pidfd before killing, then kill via pidfd for safety
        let pidfd = PidFd::open(pid)?;
        pidfd.kill(signal)?;
        Ok(Some(pidfd))
    } else {
        Ok(None)
    }
}

fn broadcast_signal(signal: Signal) -> nix::Result<Vec<PidFd>> {
    // freeze to get consistent snapshot and avoid thrashing
    kill(Pid::from_raw(-1), Signal::SIGSTOP)?;

    // can't use kill(-1) because we need to know which PIDs to wait for exit
    // otherwise unmount returns EBUSY
    let mut pids = Vec::new();
    match fs::read_dir("/proc") {
        Ok(entries) => {
            for entry in entries {
                match kill_one_entry(entry, signal) {
                    Ok(Some(pid)) => {
                        pids.push(pid);
                    },
                    Err(e) => {
                        println!(" !!! Failed to read /proc entry: {}", e);
                    },
                    _ => {},
                }
            }
        },
        Err(e) => {
            println!(" !!! Failed to read /proc: {}", e);
        },
    }

    // always make sure to unfreeze
    kill(Pid::from_raw(-1), Signal::SIGCONT)?;
    Ok(pids)
}

async fn wait_for_pidfds_exit(pidfds: Vec<PidFd>, timeout: Duration) -> Result<(), Box<dyn Error>> {
    let futures = pidfds.into_iter()
        .map(|pidfd| {
            async move {
                let _guard = pidfd.wait().await?;
                Ok::<(), tokio::io::Error>(())
            }
        })
        .collect::<Vec<_>>();

    let results = tokio::time::timeout(timeout, futures::future::join_all(futures)).await?;
    for result in results {
        if let Err(err) = result {
            return Err(InitError::PollPidFd(err).into());
        }
    }

    Ok(())
}
