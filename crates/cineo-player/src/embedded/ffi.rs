//! The libmpv client and render API, loaded at runtime (ADR-0014).
#![allow(unsafe_code, reason = "FFI to libmpv; ADR-0014, docs/SECURITY.md")]

use std::ffi::{CStr, CString, c_char, c_int, c_ulong, c_void};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tracing::{debug, info};

#[repr(C)]
pub(crate) struct Handle {
    _private: [u8; 0],
}

#[repr(C)]
pub(crate) struct RenderContext {
    _private: [u8; 0],
}

#[repr(C)]
#[allow(dead_code, reason = "C layout; not every field is read")]
struct RawEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
#[allow(dead_code, reason = "C layout; not every field is read")]
struct RawProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct RawEndFile {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
#[allow(dead_code, reason = "C layout; not every field is read")]
struct RawLogMessage {
    prefix: *const c_char,
    level: *const c_char,
    text: *const c_char,
    log_level: c_int,
}

#[repr(C)]
pub(crate) struct RenderParam {
    pub(crate) kind: c_int,
    pub(crate) data: *mut c_void,
}

#[repr(C)]
pub(crate) struct OpenGlInitParams {
    pub(crate) get_proc_address:
        Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void>,
    pub(crate) get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
pub(crate) struct OpenGlFbo {
    pub(crate) fbo: c_int,
    pub(crate) w: c_int,
    pub(crate) h: c_int,
    pub(crate) internal_format: c_int,
}

const EVENT_NONE: c_int = 0;
const EVENT_SHUTDOWN: c_int = 1;
const EVENT_LOG_MESSAGE: c_int = 2;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const EVENT_PROPERTY_CHANGE: c_int = 22;

pub(crate) const FORMAT_STRING: c_int = 1;
pub(crate) const FORMAT_FLAG: c_int = 3;
pub(crate) const FORMAT_DOUBLE: c_int = 5;

const END_FILE_EOF: c_int = 0;
const END_FILE_ERROR: c_int = 4;

pub(crate) const RENDER_PARAM_INVALID: c_int = 0;
pub(crate) const RENDER_PARAM_API_TYPE: c_int = 1;
pub(crate) const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub(crate) const RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub(crate) const RENDER_PARAM_FLIP_Y: c_int = 4;
pub(crate) const RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;

pub(crate) const RENDER_API_TYPE_OPENGL: &CStr = c"opengl";

const API_MAJOR: c_ulong = 2;

type UpdateFn = unsafe extern "C" fn(*mut c_void);

pub(crate) struct Lib {
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    create: unsafe extern "C" fn() -> *mut Handle,
    initialize: unsafe extern "C" fn(*mut Handle) -> c_int,
    terminate_destroy: unsafe extern "C" fn(*mut Handle),
    set_option_string: unsafe extern "C" fn(*mut Handle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(*mut Handle, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(*mut Handle, *mut *const c_char) -> c_int,
    command_async: unsafe extern "C" fn(*mut Handle, u64, *mut *const c_char) -> c_int,
    observe_property: unsafe extern "C" fn(*mut Handle, u64, *const c_char, c_int) -> c_int,
    request_log_messages: unsafe extern "C" fn(*mut Handle, *const c_char) -> c_int,
    wait_event: unsafe extern "C" fn(*mut Handle, f64) -> *mut RawEvent,
    pub(crate) render_context_create:
        unsafe extern "C" fn(*mut *mut RenderContext, *mut Handle, *mut RenderParam) -> c_int,
    pub(crate) render_context_set_update_callback:
        unsafe extern "C" fn(*mut RenderContext, Option<UpdateFn>, *mut c_void),
    pub(crate) render_context_update: unsafe extern "C" fn(*mut RenderContext) -> u64,
    pub(crate) render_context_render:
        unsafe extern "C" fn(*mut RenderContext, *mut RenderParam) -> c_int,
    pub(crate) render_context_free: unsafe extern "C" fn(*mut RenderContext),
    _library: libloading::Library,
}

pub(crate) fn lib() -> Result<&'static Lib, String> {
    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    LIB.get_or_init(load).as_ref().map_err(Clone::clone)
}

#[cfg(target_os = "linux")]
const NAMES: &[&str] = &["libmpv.so.2", "libmpv.so"];
#[cfg(target_os = "macos")]
const NAMES: &[&str] = &["libmpv.2.dylib", "libmpv.dylib"];
#[cfg(windows)]
const NAMES: &[&str] = &["libmpv-2.dll", "mpv-2.dll"];
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
const NAMES: &[&str] = &[];

fn candidates() -> Vec<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from));
    let mut paths: Vec<PathBuf> = beside
        .iter()
        .flat_map(|dir| NAMES.iter().map(|name| dir.join(name)))
        .collect();
    paths.extend(NAMES.iter().map(PathBuf::from));
    paths
}

fn load() -> Result<Lib, String> {
    let mut last = String::from("no library name for this platform");
    for path in candidates() {
        // A bare name must reach the system loader as such, not as a relative path.
        if path.components().count() > 1 && !path.exists() {
            continue;
        }
        // SAFETY: loading runs libmpv's initializers. libmpv is a regular C
        // library without load-time side effects beyond its dependencies.
        match unsafe { libloading::Library::new(&path) } {
            Ok(library) => {
                let lib = resolve(library).map_err(|err| format!("{}: {err}", path.display()))?;
                info!(library = %path.display(), "libmpv loaded");
                return Ok(lib);
            }
            Err(err) => {
                debug!(library = %path.display(), %err, "libmpv not loaded");
                last = err.to_string();
            }
        }
    }
    Err(format!("libmpv was not found ({last})"))
}

fn resolve(library: libloading::Library) -> Result<Lib, String> {
    macro_rules! sym {
        ($name:literal) => {
            // SAFETY: the declared type matches the C prototype in mpv's
            // client API 2.x headers; the pointer is copied out and stays
            // valid because `library` is stored in the returned `Lib`.
            *unsafe { library.get($name) }.map_err(|err| err.to_string())?
        };
    }
    let version: unsafe extern "C" fn() -> c_ulong = sym!(b"mpv_client_api_version\0");
    // SAFETY: no arguments, returns a number.
    let version = unsafe { version() };
    if version >> 16 != API_MAJOR {
        return Err(format!(
            "libmpv client API {}.{} is not supported (need {API_MAJOR}.x, mpv 0.35 or newer)",
            version >> 16,
            version & 0xffff
        ));
    }
    info!(api = %format!("{}.{}", version >> 16, version & 0xffff), "libmpv client API");
    Ok(Lib {
        error_string: sym!(b"mpv_error_string\0"),
        create: sym!(b"mpv_create\0"),
        initialize: sym!(b"mpv_initialize\0"),
        terminate_destroy: sym!(b"mpv_terminate_destroy\0"),
        set_option_string: sym!(b"mpv_set_option_string\0"),
        set_property_string: sym!(b"mpv_set_property_string\0"),
        command: sym!(b"mpv_command\0"),
        command_async: sym!(b"mpv_command_async\0"),
        observe_property: sym!(b"mpv_observe_property\0"),
        request_log_messages: sym!(b"mpv_request_log_messages\0"),
        wait_event: sym!(b"mpv_wait_event\0"),
        render_context_create: sym!(b"mpv_render_context_create\0"),
        render_context_set_update_callback: sym!(b"mpv_render_context_set_update_callback\0"),
        render_context_update: sym!(b"mpv_render_context_update\0"),
        render_context_render: sym!(b"mpv_render_context_render\0"),
        render_context_free: sym!(b"mpv_render_context_free\0"),
        _library: library,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub(crate) struct MpvError(pub(crate) String);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    None,
    Flag(bool),
    Double(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    None,
    Shutdown,
    Log {
        level: String,
        prefix: String,
        text: String,
    },
    FileLoaded,
    EndFile {
        eof: Result<bool, String>,
    },
    Property {
        id: u64,
        value: Value,
    },
    Other,
}

pub(crate) struct Core {
    lib: &'static Lib,
    handle: *mut Handle,
    waiting: Mutex<()>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core").finish_non_exhaustive()
    }
}

// SAFETY: "The client API is generally fully thread-safe, unless otherwise
// noted" (client.h). The one exception, `mpv_wait_event`, is serialized by
// `waiting`.
unsafe impl Send for Core {}
// SAFETY: as above.
unsafe impl Sync for Core {}

impl Drop for Core {
    fn drop(&mut self) {
        // SAFETY: `handle` came from `mpv_create` and is destroyed once.
        // Render contexts are already freed (see the type's invariant).
        unsafe { (self.lib.terminate_destroy)(self.handle) };
    }
}

fn cstring(text: &str) -> Result<CString, MpvError> {
    CString::new(text).map_err(|_| MpvError("an argument contains a NUL byte".into()))
}

unsafe fn text(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        // SAFETY: non-NULL and NUL-terminated per the caller's contract.
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

impl Core {
    pub(crate) fn create(lib: &'static Lib) -> Result<Self, MpvError> {
        // SAFETY: no arguments; returns NULL on failure.
        let handle = unsafe { (lib.create)() };
        if handle.is_null() {
            return Err(MpvError("mpv_create failed".into()));
        }
        Ok(Self {
            lib,
            handle,
            waiting: Mutex::new(()),
        })
    }

    pub(crate) fn lib(&self) -> &'static Lib {
        self.lib
    }

    pub(crate) fn handle(&self) -> *mut Handle {
        self.handle
    }

    fn check(&self, code: c_int) -> Result<(), MpvError> {
        if code >= 0 {
            return Ok(());
        }
        // SAFETY: returns a static string for any code.
        Err(MpvError(unsafe { text((self.lib.error_string)(code)) }))
    }

    pub(crate) fn set_option(&self, name: &str, value: &str) -> Result<(), MpvError> {
        let (name, value) = (cstring(name)?, cstring(value)?);
        // SAFETY: valid handle and NUL-terminated strings that outlive the
        // call (mpv copies them).
        self.check(unsafe {
            (self.lib.set_option_string)(self.handle, name.as_ptr(), value.as_ptr())
        })
    }

    pub(crate) fn initialize(&self) -> Result<(), MpvError> {
        // SAFETY: valid handle, initialized once by the caller.
        self.check(unsafe { (self.lib.initialize)(self.handle) })
    }

    pub(crate) fn set_property(&self, name: &str, value: &str) -> Result<(), MpvError> {
        let (name, value) = (cstring(name)?, cstring(value)?);
        // SAFETY: as in `set_option`.
        self.check(unsafe {
            (self.lib.set_property_string)(self.handle, name.as_ptr(), value.as_ptr())
        })
    }

    pub(crate) fn command(&self, args: &[&str]) -> Result<(), MpvError> {
        let (_owned, mut argv) = argv(args)?;
        // SAFETY: `argv` is a NULL-terminated array of strings owned by
        // `_owned`, alive for the call.
        self.check(unsafe { (self.lib.command)(self.handle, argv.as_mut_ptr()) })
    }

    pub(crate) fn command_async(&self, args: &[&str]) -> Result<(), MpvError> {
        let (_owned, mut argv) = argv(args)?;
        // SAFETY: as in `command`; mpv copies the arguments before returning.
        self.check(unsafe { (self.lib.command_async)(self.handle, 0, argv.as_mut_ptr()) })
    }

    pub(crate) fn observe(&self, id: u64, name: &str, format: c_int) -> Result<(), MpvError> {
        let name = cstring(name)?;
        // SAFETY: valid handle and string; `format` is an `mpv_format` value.
        self.check(unsafe { (self.lib.observe_property)(self.handle, id, name.as_ptr(), format) })
    }

    pub(crate) fn request_log_messages(&self, level: &str) -> Result<(), MpvError> {
        let level = cstring(level)?;
        // SAFETY: valid handle and string.
        self.check(unsafe { (self.lib.request_log_messages)(self.handle, level.as_ptr()) })
    }

    pub(crate) fn wait_event(&self, timeout: f64) -> Event {
        let _guard = self
            .waiting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: valid handle; only one thread waits at a time (`_guard`).
        // The result is never NULL and stays valid until the next call,
        // which `_guard` keeps from happening while we copy it.
        unsafe {
            let event = &*(self.lib.wait_event)(self.handle, timeout);
            decode(self.lib, event)
        }
    }
}

type Argv = (Vec<CString>, Vec<*const c_char>);

fn argv(args: &[&str]) -> Result<Argv, MpvError> {
    let owned = args
        .iter()
        .map(|arg| cstring(arg))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pointers: Vec<*const c_char> = owned.iter().map(|s| s.as_ptr()).collect();
    pointers.push(std::ptr::null());
    Ok((owned, pointers))
}

unsafe fn decode(lib: &Lib, event: &RawEvent) -> Event {
    match event.event_id {
        EVENT_NONE => Event::None,
        EVENT_SHUTDOWN => Event::Shutdown,
        EVENT_FILE_LOADED => Event::FileLoaded,
        EVENT_LOG_MESSAGE if !event.data.is_null() => {
            // SAFETY: LOG_MESSAGE carries an `mpv_event_log_message`.
            let log = unsafe { &*event.data.cast::<RawLogMessage>() };
            // SAFETY: the strings are valid until the next wait.
            unsafe {
                Event::Log {
                    level: text(log.level),
                    prefix: text(log.prefix),
                    text: text(log.text),
                }
            }
        }
        EVENT_END_FILE if !event.data.is_null() => {
            // SAFETY: END_FILE carries an `mpv_event_end_file`.
            let end = unsafe { &*event.data.cast::<RawEndFile>() };
            let eof = match end.reason {
                END_FILE_EOF => Ok(true),
                // SAFETY: returns a static string for any code.
                END_FILE_ERROR => Err(unsafe { text((lib.error_string)(end.error)) }),
                _ => Ok(false),
            };
            Event::EndFile { eof }
        }
        EVENT_PROPERTY_CHANGE if !event.data.is_null() => {
            // SAFETY: PROPERTY_CHANGE carries an `mpv_event_property`.
            let property = unsafe { &*event.data.cast::<RawProperty>() };
            // SAFETY: `data` points to a value of `format`, or is NULL.
            let value = unsafe { property_value(property) };
            Event::Property {
                id: event.reply_userdata,
                value,
            }
        }
        _ => Event::Other,
    }
}

unsafe fn property_value(property: &RawProperty) -> Value {
    if property.data.is_null() {
        return Value::None;
    }
    // SAFETY (all arms): `data` points to the C type `format` names.
    unsafe {
        match property.format {
            FORMAT_FLAG => Value::Flag(*property.data.cast::<c_int>() != 0),
            FORMAT_DOUBLE => Value::Double(*property.data.cast::<f64>()),
            FORMAT_STRING => Value::Text(text(*property.data.cast::<*const c_char>())),
            _ => Value::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_with_nul_are_refused() {
        assert!(argv(&["loadfile", "a\0b"]).is_err());
        let (owned, pointers) = argv(&["stop"]).unwrap_or_default();
        assert_eq!(owned.len(), 1);
        assert_eq!(pointers.len(), 2, "NULL-terminated");
        assert!(pointers[1].is_null());
    }
}
