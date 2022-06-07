pub const APP_ACL_CONFIG: &str = r#"
[app]
allow-get-app = {action = "in-get-object", res = "/core/400", access = "accept"}
allow-get-app-ext = {action = "in-get-object", res = "/core/406", access = "accept"}
"#;

pub const PERF_ACL_CONFIG: &str = r#"
[perf]
perf-upload-filter = { action = "*-put-object", res = "/core/600", group = {location = "outer"}, access = "accept" }
"#;
