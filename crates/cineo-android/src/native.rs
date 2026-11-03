#![allow(unsafe_code)]

use std::ptr::NonNull;
use std::sync::{Mutex, PoisonError};

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject};
use jni::refs::Global;
use jni::sys::jlong;
use tracing::warn;

static SURFACE: Mutex<Option<Global<JObject<'static>>>> = Mutex::new(None);

#[unsafe(no_mangle)]
pub(crate) extern "system" fn Java_io_github_mdmrk_cineo_Native_init<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    context: JObject<'caller>,
) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("cineo")
            .with_max_level(log::LevelFilter::Info),
    );
    env.with_env(|env| -> jni::errors::Result<()> {
        rustls_platform_verifier::android::init_with_env(env, context)?;
        let vm = env.get_java_vm()?;
        match NonNull::new(vm.get_raw().cast()) {
            Some(vm) => {
                if let Err(err) = cineo_player::embedded::set_java_vm(vm) {
                    warn!(%err, "hardware decoding will not have the JavaVM");
                }
            }
            None => warn!("the JavaVM pointer is null"),
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub(crate) extern "system" fn Java_io_github_mdmrk_cineo_Native_attachSurface<'caller>(
    mut env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    surface: JObject<'caller>,
) -> jlong {
    env.with_env(|env| -> jni::errors::Result<jlong> {
        let global = env.new_global_ref(&surface)?;
        let wid = i64::try_from(global.as_obj().as_raw().expose_provenance()).unwrap_or(0);
        *SURFACE.lock().unwrap_or_else(PoisonError::into_inner) = Some(global);
        Ok(wid)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub(crate) extern "system" fn Java_io_github_mdmrk_cineo_Native_releaseSurface<'caller>(
    _env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) {
    SURFACE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
}
