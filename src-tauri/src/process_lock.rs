use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use fs2::FileExt;

use crate::error::AppResult;

pub(crate) fn exclusive(path: &Path) -> AppResult<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    FileExt::lock_exclusive(&file)?;
    Ok(file)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn shared_existing(path: &Path) -> AppResult<Option<File>> {
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    FileExt::lock_shared(&file)?;
    Ok(Some(file))
}

#[cfg(target_os = "windows")]
pub(crate) struct NamedMutexGuard {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl Drop for NamedMutexGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::ReleaseMutex};

        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn named_workspace_mutex(identity: &str) -> AppResult<NamedMutexGuard> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, WaitForSingleObject, INFINITE},
    };

    let name = format!("Local\\Marktree-Workspace-{identity}");
    let wide_name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr()) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let wait = unsafe { WaitForSingleObject(handle, INFINITE) };
    if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
        unsafe {
            CloseHandle(handle);
        }
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(NamedMutexGuard { handle })
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use std::{
        sync::mpsc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn named_workspace_mutex_coordinates_the_first_reader_and_writer() {
        let identity = format!(
            "test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let first = named_workspace_mutex(&identity).unwrap();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let second_identity = identity.clone();
        let second = std::thread::spawn(move || {
            let _guard = named_workspace_mutex(&second_identity).unwrap();
            acquired_tx.send(()).unwrap();
        });

        assert!(acquired_rx
            .recv_timeout(Duration::from_millis(150))
            .is_err());
        drop(first);
        acquired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        second.join().unwrap();
    }
}
