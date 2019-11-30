use std::{collections::HashMap, error::Error, fmt, os::raw::c_int, ptr};

use libc;
use parking_lot::Mutex;

use crate::window::CursorIcon;

use super::ffi;

/// A connection to an X server.
pub struct XConnection {
    pub display: *mut ffi::Display,
    pub x11_fd: c_int,
    pub latest_error: Mutex<Option<XError>>,
    pub cursor_cache: Mutex<HashMap<Option<CursorIcon>, ffi::Cursor>>,
}

unsafe impl Send for XConnection {}
unsafe impl Sync for XConnection {}

pub type XErrorHandler =
    Option<unsafe extern "C" fn(*mut ffi::Display, *mut ffi::XErrorEvent) -> libc::c_int>;

impl XConnection {
    pub fn new(error_handler: XErrorHandler) -> Result<XConnection, XNotSupported> {
        // opening the libraries
        (*ffi::XLIB).as_ref()?;
        (*ffi::XCURSOR).as_ref()?;
        (*ffi::XRANDR_2_2_0).as_ref()?;
        (*ffi::XINPUT).as_ref()?;
        (*ffi::XLIB_XCB).as_ref()?;
        (*ffi::XRENDER).as_ref()?;

        let xlib = syms!(XLIB);
        unsafe { (xlib.XInitThreads)() };
        unsafe { (xlib.XSetErrorHandler)(error_handler) };

        // calling XOpenDisplay
        let display = unsafe {
            let display = (xlib.XOpenDisplay)(ptr::null());
            if display.is_null() {
                return Err(XNotSupported::XOpenDisplayFailed);
            }
            display
        };

        // Get X11 socket file descriptor
        let fd = unsafe { (xlib.XConnectionNumber)(display) };

        Ok(XConnection {
            display,
            x11_fd: fd,
            latest_error: Mutex::new(None),
            cursor_cache: Default::default(),
        })
    }

    /// Checks whether an error has been triggered by the previous function calls.
    #[inline]
    pub fn check_errors(&self) -> Result<(), XError> {
        let error = self.latest_error.lock().take();
        if let Some(error) = error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Ignores any previous error.
    #[inline]
    pub fn ignore_error(&self) {
        *self.latest_error.lock() = None;
    }
}

impl fmt::Debug for XConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(f)
    }
}

impl Drop for XConnection {
    #[inline]
    fn drop(&mut self) {
        let xlib = syms!(XLIB);
        unsafe { (xlib.XCloseDisplay)(self.display) };
    }
}
