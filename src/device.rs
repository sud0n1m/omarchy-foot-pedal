#[allow(unused_imports)]
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::OpenOptionsExt,
    },
    path::{Path, PathBuf},
    ptr,
};
#[link(name = "udev")]
unsafe extern "C" {
    fn udev_new() -> *mut libc::c_void;
    fn udev_unref(p: *mut libc::c_void) -> *mut libc::c_void;
    fn udev_monitor_new_from_netlink(
        p: *mut libc::c_void,
        name: *const libc::c_char,
    ) -> *mut libc::c_void;
    fn udev_monitor_unref(p: *mut libc::c_void) -> *mut libc::c_void;
    fn udev_monitor_filter_add_match_subsystem_devtype(
        p: *mut libc::c_void,
        sub: *const libc::c_char,
        dev: *const libc::c_char,
    ) -> i32;
    fn udev_monitor_enable_receiving(p: *mut libc::c_void) -> i32;
    fn udev_monitor_get_fd(p: *mut libc::c_void) -> i32;
    fn udev_monitor_receive_device(p: *mut libc::c_void) -> *mut libc::c_void;
    fn udev_device_unref(p: *mut libc::c_void) -> *mut libc::c_void;
}
pub struct Monitor {
    context: *mut libc::c_void,
    monitor: *mut libc::c_void,
    fd: RawFd,
}
impl AsRawFd for Monitor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            if !self.monitor.is_null() {
                udev_monitor_unref(self.monitor);
            }
            if !self.context.is_null() {
                udev_unref(self.context);
            }
        }
    }
}
impl Monitor {
    pub fn new() -> io::Result<Self> {
        unsafe {
            let mut result = Self {
                context: udev_new(),
                monitor: ptr::null_mut(),
                fd: -1,
            };
            if result.context.is_null() {
                return Err(io::Error::other("No udev context"));
            }
            result.monitor = udev_monitor_new_from_netlink(result.context, c"udev".as_ptr());
            if result.monitor.is_null()
                || udev_monitor_filter_add_match_subsystem_devtype(
                    result.monitor,
                    c"hidraw".as_ptr(),
                    ptr::null(),
                ) < 0
                || udev_monitor_enable_receiving(result.monitor) < 0
            {
                return Err(io::Error::other("No udev monitor"));
            }
            result.fd = udev_monitor_get_fd(result.monitor);
            if result.fd < 0 {
                return Err(io::Error::other("No udev fd"));
            }
            let mut address: libc::sockaddr_nl = std::mem::zeroed();
            let mut len = std::mem::size_of_val(&address) as libc::socklen_t;
            if libc::getsockname(
                result.fd,
                &mut address as *mut _ as *mut libc::sockaddr,
                &mut len,
            ) < 0
                || address.nl_groups & 2 == 0
            {
                return Err(io::Error::other("Processed udev notifications unavailable"));
            }
            Ok(result)
        }
    }
    pub fn drain(&self) -> io::Result<()> {
        unsafe {
            for _ in 0..32 {
                *libc::__errno_location() = 0;
                let device = udev_monitor_receive_device(self.monitor);
                if device.is_null() {
                    let e = io::Error::last_os_error();
                    return if matches!(e.raw_os_error(), Some(0 | libc::EAGAIN)) {
                        Ok(())
                    } else {
                        Err(e)
                    };
                }
                udev_device_unref(device);
            }
            Ok(())
        }
    }
}
#[cfg(test)]
thread_local! { pub static TEST_DEVICE: std::cell::RefCell<Option<PathBuf>> = const {std::cell::RefCell::new(None)}; }
#[cfg(test)]
pub fn find() -> Option<PathBuf> {
    TEST_DEVICE.with_borrow(Clone::clone)
}
#[cfg(not(test))]
pub fn find() -> Option<PathBuf> {
    for entry in fs::read_dir("/sys/class/hidraw").ok()?.flatten() {
        let Ok(resolved) = entry.path().canonicalize() else {
            continue;
        };
        for parent in resolved.ancestors() {
            if let (Ok(v), Ok(p)) = (
                fs::read_to_string(parent.join("idVendor")),
                fs::read_to_string(parent.join("idProduct")),
            ) {
                if v.trim() == "0fd9" && p.trim() == "0086" {
                    return Some(Path::new("/dev").join(entry.file_name()));
                }
                break;
            }
        }
    }
    None
}
pub fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
}
pub fn read(file: &File) -> io::Result<[bool; 3]> {
    let mut b = [0u8; 64];
    let n = unsafe { libc::read(file.as_raw_fd(), b.as_mut_ptr().cast(), b.len()) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    if n < 8 {
        return Err(io::Error::other("Pedal disconnected"));
    }
    Ok([b[4] != 0, b[5] != 0, b[6] != 0])
}
