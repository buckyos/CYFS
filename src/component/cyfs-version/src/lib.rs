pub fn get_version() -> &'static str {
    &VERSION
}

fn get_version_impl() -> String {
    let channel_ver = cyfs_base::get_channel().get_ver();
    format!("1.0.{}.{}-{} ({})", channel_ver, env!("VERSION"), cyfs_base::get_channel(), env!("BUILDDATE"))
}

pub fn check_cmd_and_exec(service_name: &str) -> cyfs_util::process::ProcessAction {
    check_cmd_and_exec_ext(service_name, service_name)
}

pub fn check_cmd_and_exec_ext(service_name: &str, mutex_name: &str) -> cyfs_util::process::ProcessAction {
    let about = format!("{} ood service for cyfs system", service_name);
    let app = clap::App::new(&format!("{}", service_name))
        .version(get_version())
        .about(&*about);

    let app = cyfs_util::process::prepare_args(app);
    let matches = app.get_matches();

    cyfs_util::process::check_cmd_and_exec_with_args_ext(service_name, mutex_name, &matches)
}

lazy_static::lazy_static! {
    static ref VERSION: String = get_version_impl();
}
