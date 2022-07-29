//
// extern crate chrono;
// extern crate env_logger;
// extern crate log;
//
// pub fn init_log() {
//     use chrono::Local;
//     use std::io::Write;
//
//     let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "debug");
//     env_logger::Builder::from_env(env)
//         .format(|buf, record| {
//             writeln!(
//                 buf,
//                 "[{}] [{}] [{}] {} {}:{}",
//                 Local::now().format("%Y-%m-%d %H:%M:%S"),
//                 record.level(),
//                 record.module_path().unwrap_or("<unnamed>"),
//                 &record.args(),
//                 record.file().unwrap_or("<unnamed>"),
//                 record.line().unwrap_or(0)
//             )
//         })
//         .init();
// }
