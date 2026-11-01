//! File-content totals; depth-first iteration keeps memory independent of entry count.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};
static READY: AtomicBool = AtomicBool::new(false);
pub fn ready() -> bool {
    READY.swap(false, Ordering::Relaxed)
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Total {
    pub bytes: u64,
    pub files: u64,
    pub skipped: u64,
}
fn link(info: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if info.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    info.file_type().is_symlink()
}
pub fn directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !link(&m))
}
pub fn scan(path: &Path, cancel: &AtomicBool) -> Option<Total> {
    let mut total = Total::default();
    let mut stack = Vec::new();
    match fs::read_dir(path) {
        Ok(entries) => stack.push(entries),
        Err(_) => total.skipped += 1,
    }
    while !stack.is_empty() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let next = stack.last_mut().unwrap().next();
        match next {
            None => {
                stack.pop();
            }
            Some(Err(_)) => total.skipped += 1,
            Some(Ok(entry)) => match fs::symlink_metadata(entry.path()) {
                Err(_) => total.skipped += 1,
                Ok(info) if link(&info) => {}
                Ok(info) if info.is_dir() => match fs::read_dir(entry.path()) {
                    Ok(entries) => stack.push(entries),
                    Err(_) => total.skipped += 1,
                },
                Ok(info) if info.is_file() => {
                    total.bytes = total.bytes.saturating_add(info.len());
                    total.files = total.files.saturating_add(1);
                }
                Ok(_) => {} // Never open devices, sockets or FIFOs.
            },
        }
    }
    Some(total)
}
impl Total {
    pub fn text(&self) -> String {
        let value = format!("{} bytes ({} files)", self.bytes, self.files);
        if self.skipped == 0 {
            value
        } else {
            format!("{value}; INCOMPLETE: {} unreadable entries", self.skipped)
        }
    }
}
pub fn synchronous(path: &Path) -> String {
    scan(path, &AtomicBool::new(false))
        .unwrap_or_default()
        .text()
}
struct Worker {
    cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Option<Total>>,
}
#[derive(Default)]
pub struct Job {
    path: Option<PathBuf>,
    worker: Option<Worker>,
    result: Option<Total>,
}
impl Job {
    pub fn request(&mut self, path: Option<PathBuf>, force: bool) {
        if force || path != self.path {
            if let Some(worker) = &self.worker {
                worker.cancel.store(true, Ordering::Relaxed);
            }
            self.path = path;
            self.result = None;
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(worker) = &self.worker {
            match worker.rx.try_recv() {
                Ok(result) => {
                    if !worker.cancel.load(Ordering::Relaxed) {
                        self.result = result;
                        changed = true;
                    }
                    self.worker = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.worker.is_none() && self.result.is_none() {
            if let Some(path) = self.path.clone() {
                let cancel = Arc::new(AtomicBool::new(false));
                let token = cancel.clone();
                let (tx, rx) = mpsc::sync_channel(1);
                // At most one outstanding scan per UI, including cancelled scans.
                thread::Builder::new()
                    .name("filex-folder-size".into())
                    .stack_size(128 * 1024)
                    .spawn(move || {
                        let _ = tx.send(scan(&path, &token));
                        READY.store(true, Ordering::Relaxed);
                    })
                    .ok();
                self.worker = Some(Worker { cancel, rx });
            }
        }
        changed
    }
    pub fn text(&self) -> Option<String> {
        self.path.as_ref().map(|_| {
            self.result
                .as_ref()
                .map_or_else(|| "Calculating folder contents...".into(), Total::text)
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn totals_include_nested_hidden_and_empty_files_without_following_links() {
        let root = std::env::temp_dir().join(format!("filex-size-{}", std::process::id()));
        fs::create_dir_all(root.join(".hidden/deep")).unwrap();
        fs::write(root.join("a"), b"abc").unwrap();
        fs::write(root.join(".hidden/deep/b"), b"12345").unwrap();
        fs::write(root.join("empty"), b"").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, root.join("cycle")).unwrap();
        }
        assert_eq!(
            scan(&root, &AtomicBool::new(false)),
            Some(Total {
                bytes: 8,
                files: 3,
                skipped: 0
            })
        );
        assert_eq!(scan(&root, &AtomicBool::new(true)), None);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unavailable_directory_is_incomplete_not_zero_success() {
        let path = std::env::temp_dir().join(format!("filex-size-missing-{}", std::process::id()));
        let result = scan(&path, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.skipped, 1);
        assert!(result.text().contains("INCOMPLETE"));
    }
    #[test]
    fn worker_retargets_and_refreshes_without_stale_results() {
        let root = std::env::temp_dir().join(format!("filex-size-worker-{}", std::process::id()));
        fs::create_dir_all(root.join("child")).unwrap();
        fs::write(root.join("child/a"), b"123").unwrap();
        let mut job = Job::default();
        job.request(Some(root.clone()), false);
        job.poll();
        job.request(Some(root.join("child")), false);
        let wait = |job: &mut Job| {
            for _ in 0..300 {
                job.poll();
                if job.result.is_some() {
                    return;
                }
                thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("scan timeout");
        };
        wait(&mut job);
        assert_eq!(job.result.as_ref().unwrap().bytes, 3);
        fs::write(root.join("child/a"), b"123456").unwrap();
        job.request(Some(root.join("child")), true);
        wait(&mut job);
        assert_eq!(job.result.as_ref().unwrap().bytes, 6);
        job.request(None, false);
        assert!(job.text().is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
