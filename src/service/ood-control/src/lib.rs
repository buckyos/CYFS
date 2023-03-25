mod controller;
mod device_info;
//mod http_bdt_listener;
mod access_token;
mod app_bind_manager;
mod bind;
mod http_server;
pub mod interface;
mod request;
mod ood_controller;

pub use app_bind_manager::AppBindManager;
pub use controller::*;
pub use interface::{
    ControlInterface, ControlInterfaceAddrType, ControlInterfaceParam, ControlTCPHost,
};
pub use request::*;
pub use ood_controller::*;

#[macro_use]
extern crate log;

#[derive(Clone, Eq, Debug, PartialEq)]
pub enum InterfaceProtocol {
    HttpBdt,
    Http,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OODControlMode {
    // ood-daemon模式
    Daemon = 0,

    // cyfs-runtime模式
    Runtime = 1,

    // 第三方app模式，端口随机
    App = 2,

    // ood-installer mode
    Installer = 3,
}

#[cfg(target_os = "ios")]
mod ios {
    use crate::app_bind_manager::*;
    use async_std::sync::Arc;
    use cyfs_util::{bind_cyfs_root_path, create_log_with_isolate_bdt};
    use cyfs_debug::CombineLogger;
    use cyfs_debug::PanicBuilder;
    use cyfs_debug::{IosLogger, LogCallback};
    use log::*;
    use std::any::Any;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_uchar, c_void};
    use std::str::FromStr;

    #[derive(Debug)]
    #[repr(C)]
    pub struct Result {
        pub result_num: usize,
        pub result_value: *mut *mut c_char,
    }
    use once_cell::sync::OnceCell;

    static APP_BIND_MANAGER: OnceCell<AppBindManager> = OnceCell::new();

    pub const SERVICE_NAME: &str = "ood-control";

    #[no_mangle]
    pub extern "C" fn free_result(result: *mut Result) {
        if !result.is_null() {
            let result = unsafe { Box::from_raw(result) };
            let values = unsafe {
                Vec::from_raw_parts(result.result_value, result.result_num, result.result_num)
            };
            for i in 0..result.result_num {
                let _ = unsafe { CString::from_raw(values[i as usize]) };
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn init(cbase_path: *const c_char, cloglevel: *const c_char, log: LogCallback) {
        if APP_BIND_MANAGER.get().is_none() {
            let base_path = unsafe { CStr::from_ptr(cbase_path) }
                .to_string_lossy()
                .to_string();
            let loglevel = unsafe { CStr::from_ptr(cloglevel) }
                .to_string_lossy()
                .to_string();

            bind_cyfs_root_path(&base_path);

            let module_log =
                create_log_with_isolate_bdt("oodcontrol", Some(&loglevel), Some(&loglevel))
                    .unwrap();

            if CombineLogger::new()
                .append(Box::new(IosLogger::new(&loglevel, log)))
                .append(Box::new(module_log))
                .start()
            {
                PanicBuilder::new("oodcontrol", SERVICE_NAME)
                    .build()
                    .start();
            }

            let app_bind_mgr = AppBindManager::new(None);
            APP_BIND_MANAGER.set(app_bind_mgr);
        }
    }

    #[no_mangle]
    pub extern "C" fn wait_bind() {
        async_std::task::block_on(async {
            let manager = APP_BIND_MANAGER.get();
            if manager.is_none() {
                return;
            }
            if manager.unwrap().is_bind() {
                info!("manager already bind, exit.");
                return;
            }

            if let Err(e) = manager.unwrap().wait_util_bind().await {
                error!("app bind manager bind failed: {} ", e);
                return;
            }
        })
    }

    #[no_mangle]
    pub extern "C" fn start() {
        async_std::task::block_on(async {
            let manager = APP_BIND_MANAGER.get();
            if manager.is_none() {
                return;
            }
            if manager.unwrap().is_bind() {
                info!("manager already bind, exit.");
                return;
            }

            if let Err(e) = manager.unwrap().start().await {
                error!("app bind manager init failed: {}", e);
                return;
            }
        })
    }

    #[no_mangle]
    pub extern "C" fn is_bind() -> c_uchar {
        let is_bind = APP_BIND_MANAGER.get().unwrap().is_bind();
        is_bind as c_uchar
    }

    fn new_result_object(values: Vec<String>) -> Result {
        let mut result = Result {
            result_num: 0,
            result_value: std::ptr::null_mut(),
        };
        let mut values = vec![];

        for value in values {
            values.push(CString::new(value.clone()).unwrap().into_raw());
        }
        result.result_num = result.result_value.len();

        values.shrink_to_fit();
        result.result_value = values.as_mut_ptr();
        std::mem::forget(values);

        result
    }

    #[no_mangle]
    pub extern "C" fn get_address_list() -> *const Result {
        let peers = APP_BIND_MANAGER.get().unwrap().get_tcp_addr_list();
        let size: usize = peers.len() as usize;
        let mut params = vec![];
        for peer in peers {
            let ip = peer.ip().to_string();
            let port = peer.port();
            let addr_str = format!("{}:{}", ip, port);
            params.push(addr_str);
        }

        let result = new_result_object(params);

        Box::into_raw(Box::new(result))
    }
}

#[cfg(target_os = "android")]
mod android {
    use crate::app_bind_manager::*;
    use async_std::sync::Arc;
    use cyfs_base::{
        IpAddr, NamedObject, ObjectDesc,
        ObjectId, OwnerObjectDesc, PeopleId,
    };
    use cyfs_util::{bind_cyfs_root_path, create_log_with_isolate_bdt};
    use cyfs_debug::PanicBuilder;
    use cyfs_debug::{str_to_log_level, CombineLogger};
    use jni::objects::{
        AutoArray, AutoLocal, GlobalRef, JByteBuffer, JClass, JList, JObject, JString, JValue,
    };
    use jni::sys::{
        jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jobject, jobjectArray, jshort, jsize,
    };
    use jni::{JNIEnv, JavaVM};
    use log::*;
    use once_cell::sync::OnceCell;
    use std::ffi::c_void;
    use std::str::FromStr;

    static APP_BIND_MANAGER: OnceCell<AppBindManager> = OnceCell::new();

    pub const SERVICE_NAME: &str = "ood-control";
    static STRING_CLASS: &str = "java/lang/String";

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_OodControl_init(
        env: JNIEnv,
        _class: JClass,
        base_path: JString,
        loglevel: JString,
    ) {
        if APP_BIND_MANAGER.get().is_none() {
            let base_path: String = env.get_string(base_path).unwrap().into();
            let loglevel: String = env.get_string(loglevel).unwrap().into();

            bind_cyfs_root_path(&base_path);

            let android_logger = android_logger::AndroidLogger::new(
                android_logger::Config::default()
                    .with_min_level(str_to_log_level(&loglevel))
                    .with_tag("oodcontrol"),
            );

            let module_log =
                create_log_with_isolate_bdt("oodcontrol", Some(&loglevel), Some(&loglevel))
                    .unwrap();

            if CombineLogger::new()
                .append(Box::new(android_logger))
                .append(Box::new(module_log))
                .start()
            {
                PanicBuilder::new("oodcontrol", SERVICE_NAME)
                    .build()
                    .start();
            }

            let app_bind_mgr = AppBindManager::new(None);
            APP_BIND_MANAGER.set(app_bind_mgr);
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_OodControl_start(env: JNIEnv, _class: JClass) {
        async_std::task::block_on(async {
            let manager = APP_BIND_MANAGER.get();
            if manager.is_none() {
                return;
            }
            if manager.unwrap().is_bind() {
                info!("manager already bind, exit.");
                return;
            }

            if let Err(e) = manager.unwrap().start().await {
                error!("app bind manager init failed: {}", e);
                return;
            }
        })
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_OodControl_wait_bind(env: JNIEnv, _class: JClass) {
        async_std::task::block_on(async {
            let manager = APP_BIND_MANAGER.get();
            if manager.is_none() {
                return;
            }
            if manager.unwrap().is_bind() {
                info!("manager already bind, exit.");
                return;
            }

            if let Err(e) = manager.unwrap().wait_util_bind().await {
                error!("app bind manager bind failed: {} ", e);
                return;
            }
        })
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_OodControl_isBind(
        env: JNIEnv,
        _class: JClass,
    ) -> jboolean {
        let is_bind = APP_BIND_MANAGER.get().unwrap().is_bind();
        is_bind as jboolean
    }

    #[no_mangle]
    pub extern "system" fn Java_com_cyfs_OodControl_getAddressList(
        env: JNIEnv,
        _class: JClass,
    ) -> jobjectArray {
        let peers = APP_BIND_MANAGER.get().unwrap().get_tcp_addr_list();
        let size: jsize = peers.len() as jsize;
        let peers_array = env
            .new_object_array(size, STRING_CLASS, JObject::null())
            .unwrap();
        assert!(!peers_array.is_null());

        let mut i: i32 = 0;
        for peer in peers {
            let ip = peer.ip().to_string();
            let port = peer.port();
            let addr_str = env.new_string(format!("{}:{}", ip, port)).unwrap();
            info!("app bind manager addr: {}:{}", ip, port);
            env.set_object_array_element(peers_array, i, addr_str)
                .unwrap();
            env.delete_local_ref(JObject::from(addr_str)).unwrap();
            i += 1;
        }

        return peers_array;
    }
}
