mod app_config;
mod def;

pub use app_config::*;
pub use def::*;

#[macro_use]
extern crate log;

#[cfg(test)]
mod tests {
    use crate::*;
    //use log::*;

    #[test]
    fn test_app_config() {
        cyfs_debug::CyfsLoggerBuilder::new_app("test-app-config")
            .level("trace")
            .console("trace")
            .enable_bdt(Some("warn"), Some("warn"))
            .build()
            .unwrap()
            .start();

        let app_config = AppManagerConfig::new();
        log::info!("host_mode: {:?}", app_config.host_mode())
    }
}
