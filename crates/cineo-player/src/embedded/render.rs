#![allow(unsafe_code, reason = "FFI to libmpv; ADR-0014, docs/SECURITY.md")]

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Arc;

use super::ffi::{self, Core, OpenGlFbo, OpenGlInitParams, RenderContext, RenderParam};
use crate::PlayerError;

/// Resolves OpenGL function names in the host's context (eframe's
/// `CreationContext::get_proc_address`).
pub type ProcAddress = Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>;

/// Called by mpv, from its own threads, when a new frame should be drawn.
/// It must only schedule a repaint.
pub type OnFrame = Box<dyn Fn() + Send + Sync>;

/// mpv's OpenGL renderer for one playback.
pub struct Renderer {
    context: *mut RenderContext,
    core: Arc<Core>,
    _proc: Box<ProcAddress>,
    _on_frame: Box<OnFrame>,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer").finish_non_exhaustive()
    }
}

// SAFETY: see the thread contract on the type: it moves between threads only
// as part of a paint callback that runs on the GL thread.
unsafe impl Send for Renderer {}
// SAFETY: as above; `&Renderer` gives no access to the context.
unsafe impl Sync for Renderer {}

unsafe extern "C" fn proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: `ctx` is the `Box<ProcAddress>` owned by the renderer, alive
    // while mpv can call this; `name` is a NUL-terminated string.
    unsafe {
        let resolve = &*ctx.cast::<ProcAddress>();
        resolve(CStr::from_ptr(name)).cast_mut()
    }
}

unsafe extern "C" fn frame_ready(ctx: *mut c_void) {
    // SAFETY: `ctx` is the `Box<OnFrame>` owned by the renderer; mpv stops
    // calling this when the render context is freed, before the box is.
    unsafe { (*ctx.cast::<OnFrame>())() };
}

impl Renderer {
    pub(crate) fn new(
        core: Arc<Core>,
        get_proc_address: ProcAddress,
        on_frame: OnFrame,
    ) -> Result<Self, PlayerError> {
        let lib = core.lib();
        let proc = Box::new(get_proc_address);
        let on_frame = Box::new(on_frame);
        let mut init = OpenGlInitParams {
            get_proc_address: Some(proc_address),
            get_proc_address_ctx: std::ptr::from_ref::<ProcAddress>(&proc).cast_mut().cast(),
        };
        let mut params = [
            RenderParam {
                kind: ffi::RENDER_PARAM_API_TYPE,
                data: ffi::RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
            },
            RenderParam {
                kind: ffi::RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: std::ptr::from_mut(&mut init).cast(),
            },
            RenderParam {
                kind: ffi::RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        let mut context = std::ptr::null_mut();
        // SAFETY: valid core handle; `params` is terminated by INVALID and
        // its pointers outlive the call; the proc-address context is boxed
        // and kept in the renderer. The caller holds the GL context current.
        let code = unsafe {
            (lib.render_context_create)(&raw mut context, core.handle(), params.as_mut_ptr())
        };
        if code < 0 || context.is_null() {
            return Err(PlayerError::Render(format!(
                "mpv could not draw with this OpenGL context (error {code})"
            )));
        }
        // SAFETY: valid context; the callback data is boxed and outlives the
        // context (freed in `drop` before the box).
        unsafe {
            (lib.render_context_set_update_callback)(
                context,
                Some(frame_ready),
                std::ptr::from_ref::<OnFrame>(&on_frame).cast_mut().cast(),
            );
        }
        Ok(Self {
            context,
            core,
            _proc: proc,
            _on_frame: on_frame,
        })
    }

    /// Draws the current video frame into framebuffer `fbo` of
    /// `width`×`height` pixels, letterboxed. `fbo` 0 is the window. Never
    /// waits for the frame's display time.
    pub fn draw(&mut self, fbo: u32, width: u32, height: u32) {
        let lib = self.core.lib();
        let (Ok(fbo), Ok(w), Ok(h)) = (
            c_int::try_from(fbo),
            c_int::try_from(width),
            c_int::try_from(height),
        ) else {
            return;
        };
        if w == 0 || h == 0 {
            return;
        }
        let mut target = OpenGlFbo {
            fbo,
            w,
            h,
            internal_format: 0,
        };
        let mut flip: c_int = c_int::from(fbo == 0);
        let mut block: c_int = 0;
        let mut params = [
            RenderParam {
                kind: ffi::RENDER_PARAM_OPENGL_FBO,
                data: std::ptr::from_mut(&mut target).cast(),
            },
            RenderParam {
                kind: ffi::RENDER_PARAM_FLIP_Y,
                data: std::ptr::from_mut(&mut flip).cast(),
            },
            RenderParam {
                kind: ffi::RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: std::ptr::from_mut(&mut block).cast(),
            },
            RenderParam {
                kind: ffi::RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: valid context, used on the GL thread (type contract);
        // `params` is INVALID-terminated and its pointers outlive the calls.
        unsafe {
            let _flags = (lib.render_context_update)(self.context);
            (lib.render_context_render)(self.context, params.as_mut_ptr());
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: valid context, freed once, on the GL thread (type
        // contract), while `core` is still alive.
        unsafe { (self.core.lib().render_context_free)(self.context) };
    }
}
