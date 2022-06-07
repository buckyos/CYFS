use cyfs_util::GatewayRegister;

const META_STRING: &str = r#"
{
    "block": "server",
    "listener": [
      {
        "type": "bdt",
        "stack": "default",
        "vport": 80
      }
    ],
    "server_name": "_",
    "location": [
      {
        "type": "prefix",
        "path": "/chunk_manager/",
        "method": "get post",
        "proxy_pass": "127.0.0.1:${port}/"
      }
    ]
}
"#;

pub fn register() {
  let id = "chunk_manager";
  let server_type = "http";

  let meta_str = META_STRING.replace("${port}", &::cyfs_base::CHUNK_MANAGER_PORT.to_string());

  if let Err(e) = GatewayRegister::register(id.to_owned(), server_type.to_owned(), meta_str) {
    error!("register to gateway error! err={}", e);
  }
}
