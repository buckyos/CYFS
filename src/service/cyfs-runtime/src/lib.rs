#[macro_use]
extern crate log;

mod proxy;
mod runtime;
mod stack;
mod mime;
mod anonymous;
mod file_cache;
mod ipfs_proxy;
mod ipfs_stub;

// use once_cell::sync::OnceCell;
// use crate::runtime::CyfsRuntime;
// pub static RUNTIME_STUB: OnceCell<CyfsRuntime> = OnceCell::new();

#[cfg(target_os = "android")]
mod android {
    use cyfs_base::{NamedObject, OwnerObjectDesc, ObjectDesc, PeopleId, ObjectId, IpAddr};
    use cyfs_util::{bind_cyfs_root_path, create_log_with_isolate_bdt};
    use jni::objects::{GlobalRef, JClass, JString, JValue};
    use jni::sys::{jint};
    use jni::{JNIEnv, JavaVM};
    use crate::stack::PROXY_PORT;
    use std::ffi::c_void;
    use cyfs_debug::{str_to_log_level, CombineLogger};
    use log::*;
    use async_std::sync::Arc;
    use std::str::FromStr;
    use cyfs_debug::PanicBuilder;
    use crate::runtime::CyfsRuntime;
    use super::stack::{UpdateStackNetworkParams, CyfsStackInsConfig};

    use once_cell::sync::OnceCell;
    // use crate::runtime::CyfsRuntime;
    static RUNTIME_STUB: OnceCell<CyfsRuntime> = OnceCell::new();

    static mut CLASS_LOADER: Option<GlobalRef> = None;

    pub const SERVICE_NAME: &str = ::cyfs_base::CYFS_RUNTIME_NAME;

    #[no_mangle]
    pub extern "system" fn Java_org_chromium_chrome_browser_init_CYFSRuntime_start(
        env: JNIEnv,
        _class: JClass,
        base_path: JString,
    ) {
        let base_path: String = env.get_string(base_path).unwrap().into();
        bind_cyfs_root_path(&base_path);


        let loglevel : &str = "debug";
        let android_logger = android_logger::AndroidLogger::new(
            android_logger::Config::default()
                .with_min_level(str_to_log_level(&loglevel))
                .with_tag("cyfsruntime"),
        );

        let module_log = create_log_with_isolate_bdt("cyfsruntime", Some(&loglevel), Some(&loglevel)).unwrap();

        if CombineLogger::new()
            .append(Box::new(android_logger))
            .append(Box::new(module_log))
            .start() {
                PanicBuilder::new("cyfs-runtime", SERVICE_NAME).build().start();
            }

        let stack_config = CyfsStackInsConfig {
            is_mobile_stack: true,
            anonymous: false,
            random_id: false,
            proxy_port: PROXY_PORT,
        };

        async_std::task::block_on(async {
            let mut runtime = CyfsRuntime::new(stack_config);
            if let Err(e) = runtime.start().await {
                error!("cyfs runtime init failed: {}", e);
                return;
            }
            RUNTIME_STUB.set(runtime);

            async_std::future::pending::<()>().await;
        })
    }

    #[no_mangle]
    pub extern "system" fn Java_org_chromium_chrome_browser_init_CYFSRuntime_resetNetwork(
        env: JNIEnv,
        _class: JClass,
        ip_addr: JString,
    ) {
        if let Some(stub) = RUNTIME_STUB.get() {
            let ip_addr: String = env.get_string(ip_addr).unwrap().into();
            let network_params = UpdateStackNetworkParams {
                ip_v4: ip_addr,
                ip_v6: None
            };
            async_std::task::block_on(async {
                warn!("reset resetNetwork.");
                stub.update_network(network_params);
            })
        } else {
            warn!("call resetNetwork before stack start! ignore.")
        }
    }
}
