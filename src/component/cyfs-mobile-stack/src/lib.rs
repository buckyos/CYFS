mod non_stub;

#[cfg(target_os = "ios")]
mod ios {
    use std::os::raw::{c_char, c_int};
    use std::ffi::CStr;
    use cyfs_util::{bind_cyfs_root_path, create_log_with_isolate_bdt};
    use cyfs_debug::CombineLogger;
    use cyfs_debug::{IosLogger, LogCallback, CyfsLoggerBuilder, PanicBuilder};
    use crate::non_stub::{NonStub, NON_STUB};
    use log::*;

    #[no_mangle]
    pub extern "C" fn resetNetwork(
        addr: *const c_char,
    ) {
        if let Some(stub) = NON_STUB.get() {
            let addr = unsafe {CStr::from_ptr(addr)}.to_string_lossy().to_string();
            async_std::task::block_on(async {
                stub.update_network(addr.as_str()).await;
            })
        } else {
            warn!("call resetNetwork before stack start! ignore.")
        }
    }

    #[no_mangle]
    pub extern "C" fn restartInterface(
    ) {
        if let Some(stub) = NON_STUB.get() {
            async_std::task::block_on(async {
                stub.restart_interface().await;
            })
        } else {
            warn!("call resetNetwork before stack start! ignore.")
        }
    }

    #[no_mangle]
    pub extern "C" fn start(
        cbase_path: *const c_char,
        cnon_addr: *const c_char,
        cws_addr: *const c_char,
        cbdt_port: c_int,
        cloglevel: *const c_char,
        cwifi_addr: *const c_char,
        log: LogCallback
    ) {
        let base_path = unsafe {CStr::from_ptr(cbase_path)}.to_string_lossy().to_string();
        let non_addr = unsafe {CStr::from_ptr(cnon_addr)}.to_string_lossy().to_string();
        let ws_addr = unsafe {CStr::from_ptr(cws_addr)}.to_string_lossy().to_string();
        let loglevel = unsafe {CStr::from_ptr(cloglevel)}.to_string_lossy().to_string();
        let wifi_addr = unsafe {CStr::from_ptr(cwifi_addr)}.to_string_lossy().to_string();
        bind_cyfs_root_path(&base_path);

        let module_log = CyfsLoggerBuilder::new_service("cyfsstack")
            .level(&loglevel)
            .console(&loglevel)
            .enable_bdt(Some(&loglevel), Some(&loglevel))
            .build()
            .unwrap();

        if CombineLogger::new()
            .append(Box::new(IosLogger::new(&loglevel, log)))
            .append(module_log.into())
            .start() {
            PanicBuilder::new("cyfs-sdk", "cyfsstack").build().start();
        }

        // 输出环境信息，用以诊断一些环境问题
        for argument in std::env::args() {
            info!("arg: {}", argument);
        }

        // info!("current exe: {:?}", std::env::current_exe());
        info!("current dir: {:?}", std::env::current_dir());

        info!("current version: {}", cyfs_base::get_version());

        for (key, value) in std::env::vars() {
            info!("env: {}: {}", key, value);
        }

        let mut acc_stub = NonStub::new("device", cbdt_port as u16, &non_addr, &ws_addr);

        async_std::task::spawn(async move{
            acc_stub.update_network(&wifi_addr).await;

            if let Err(e) = acc_stub.init().await {
                log::error!("init stack err {}", e);
                return;
            }
            NON_STUB.set(acc_stub);

            // async_std::future::pending::<()>().await;
        });
    }
}

#[cfg(target_os = "android")]
mod android {
    use cyfs_base::{NamedObject, OwnerObjectDesc, ObjectDesc, PeopleId, ObjectId, IpAddr};
    use jni::objects::{GlobalRef, JClass, JString, JValue};
    use jni::sys::{jint};
    use jni::{JNIEnv, JavaVM};
    use std::ffi::c_void;
    use cyfs_util::{bind_cyfs_root_path, create_log_with_isolate_bdt};
    use cyfs_debug::{str_to_log_level, CombineLogger, CyfsLoggerBuilder, PanicBuilder};
    use log::*;
    use async_std::sync::Arc;
    use crate::non_stub::{NonStub, NON_STUB};
    use std::str::FromStr;

    #[no_mangle]
    #[allow(non_snake_case)]
    unsafe fn JNI_OnLoad(jvm: JavaVM, _reserved: *mut c_void) -> jint {
        let env: JNIEnv = jvm.get_env().unwrap();
        let jni_version = env.get_version().unwrap();
        let version: jint = jni_version.into();
        //info!("JNI_OnLoad end");
        version
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_Stack_resetNetwork(
        env: JNIEnv,
        _class: JClass,
        addr: JString,
    ) {
        if let Some(stub) = NON_STUB.get() {
            let addr: String = env.get_string(addr).unwrap().into();
            async_std::task::block_on(async {
                stub.update_network(addr.as_str()).await;
            })
        } else {
            warn!("call resetNetwork before stack start! ignore.")
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_Stack_restartInterface(
        env: JNIEnv,
        _class: JClass
    ) {
        if let Some(stub) = NON_STUB.get() {
            async_std::task::block_on(async {
                stub.restart_interface().await;
            })
        } else {
            warn!("call resetNetwork before stack start! ignore.")
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_Stack_start(
        env: JNIEnv,
        _class: JClass,
        base_path: JString,
        non_addr: JString,
        ws_addr: JString,
        bdt_port: jint,
        loglevel: JString,
        wifi_addr: JString,
    ) {
        let base_path: String = env.get_string(base_path).unwrap().into();
        let loglevel: String = env.get_string(loglevel).unwrap().into();
        let wifi_addr: String = env.get_string(wifi_addr).unwrap().into();
        let non_addr: String = env.get_string(non_addr).unwrap().into();
        let ws_addr: String = env.get_string(ws_addr).unwrap().into();

        bind_cyfs_root_path(&base_path);


        let android_logger = android_logger::AndroidLogger::new(
            android_logger::Config::default()
                .with_min_level(str_to_log_level(&loglevel))
                .with_tag("cyfsstack"),
        );

        let module_log = CyfsLoggerBuilder::new_service("cyfsstack")
            .level(&loglevel)
            .console(&loglevel)
            .enable_bdt(Some(&loglevel), Some(&loglevel))
            .build()
            .unwrap();

        if CombineLogger::new()
            .append(Box::new(android_logger))
            .append(module_log.into())
            .start() {
            PanicBuilder::new("cyfs-sdk", "cyfsstack").build().start();
        }

        // 输出环境信息，用以诊断一些环境问题
        for argument in std::env::args() {
            info!("arg: {}", argument);
        }

        // info!("current exe: {:?}", std::env::current_exe());
        info!("current dir: {:?}", std::env::current_dir());

        info!("current version: {}", cyfs_base::get_version());

        for (key, value) in std::env::vars() {
            info!("env: {}: {}", key, value);
        }

        let mut acc_stub = NonStub::new("device", bdt_port as u16, &non_addr, &ws_addr);

        async_std::task::block_on(async {
            acc_stub.update_network(&wifi_addr).await;

            if let Err(e) = acc_stub.init().await {
                log::error!("init stack err {}", e);
                return;
            }
            NON_STUB.set(acc_stub);

            async_std::future::pending::<()>().await;
        })
    }
}
