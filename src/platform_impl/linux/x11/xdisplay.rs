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

/// Error triggered by xlib.
#[derive(Debug, Clone)]
pub struct XError {
    pub description: String,
    pub error_code: u8,
    pub request_code: u8,
    pub minor_code: u8,
}

impl Error for XError {
    #[inline]
    fn description(&self) -> &str {
        &self.description
    }
}

impl fmt::Display for XError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            formatter,
            "X error: {} (code: {}, request code: {}, minor code: {})",
            self.description, self.error_code, self.request_code, self.minor_code
        )
    }
}

/// Error returned if this system doesn't have XLib or can't create an X connection.
#[derive(Clone, Debug)]
pub enum XNotSupported {
    /// Failed to load one or several shared libraries.
    LibraryOpenError(ffi::OpenError),
    /// Connecting to the X server with `XOpenDisplay` failed.
    XOpenDisplayFailed, // TODO: add better message
}

impl From<&ffi::OpenError> for XNotSupported {
    #[inline]
    fn from(err: &ffi::OpenError) -> XNotSupported {
        XNotSupported::LibraryOpenError(err.clone())
    }
}

impl Error for XNotSupported {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            XNotSupported::LibraryOpenError(_) => "Failed to load one of xlib's shared libraries",
            XNotSupported::XOpenDisplayFailed => "Failed to open connection to X server",
        }
    }

    #[inline]
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            XNotSupported::LibraryOpenError(ref err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for XNotSupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str(self.description())
    }
}
