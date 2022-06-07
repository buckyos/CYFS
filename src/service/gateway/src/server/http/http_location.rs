use http_types::{Method, Request, Url};

use cyfs_base::BuckyError;

#[derive(Clone, Copy, Debug)]
pub(super) enum HttpLocationPathMode {
    Prefix,
    Equal,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum HttpProxyPassMode {
    // 路径连接
    Join,

    // 直接转发
    Pass,

    // 直接拼接
    Contact,
}

#[derive(Clone, Debug)]
pub(super) struct HttpStreamLocation {
    mode: HttpLocationPathMode,
    path: String,

    method: Vec<Method>,

    // proxy_pass 相关字段
    proxy_pass_mode: HttpProxyPassMode,
    origin_proxy_pass: String,
    proxy_pass: Option<Url>,
}

impl HttpStreamLocation {
    pub fn new() -> HttpStreamLocation {
        HttpStreamLocation {
            mode: HttpLocationPathMode::Equal,
            path: String::from(""),
            method: Vec::new(),
            proxy_pass_mode: HttpProxyPassMode::Pass,
            origin_proxy_pass: String::from(""),
            proxy_pass: None,
        }
    }

    pub fn try_match(&self, req: &Request) -> Option<String> {
        let path = req.url().path();

        if !self.method_test(req) {
            return None;
        }

        match self.mode {
            HttpLocationPathMode::Equal => {
                if self.path == path {
                    if let Ok(target_url) = self.make_target(req.url()) {
                        return Some(target_url);
                    }
                }
            }
            HttpLocationPathMode::Prefix => {
                if path.starts_with(&self.path) {
                    if let Ok(target_url) = self.make_target(req.url()) {
                        return Some(target_url);
                    }
                }
            }
        }

        None
    }

    fn method_test(&self, req: &Request) -> bool {
        let method = req.method();
        let ret = self.method.iter().find(|&&x| x == method);
        return ret.is_some();
    }

    pub fn load_method(&mut self, value: &str) -> Result<(), BuckyError> {
        let methods: Vec<&str> = value.split(" ").collect();
        for method in methods {
            let method_item = match method.to_uppercase().as_str() {
                "GET" => Some(Method::Get),
                "POST" => Some(Method::Post),
                "PUT" => Some(Method::Put),
                "DELETE" => Some(Method::Delete),
                "HEAD" => Some(Method::Head),
                _ => {
                    error!("unsupport method: {}", method);
                    None
                }
            };

            if method_item.is_some() {
                self.method.push(method_item.unwrap());
            }
        }

        Ok(())
    }

    pub fn load_proxy_pass(&mut self, value: &str) -> Result<(), BuckyError> {
        let value = format!("http://{}", value);
        let url = Url::parse(&value).map_err(|e| {
            let msg = format!("parse proxy_pass url error! url={}, {}", value, e);
            error!("{}", msg);
            BuckyError::from(msg)
        })?;

        self.origin_proxy_pass = value;

        if url.path() == "/" {
            if self.origin_proxy_pass.ends_with("/") {
                // proxypass = 127.0.0.1/
                self.proxy_pass_mode = HttpProxyPassMode::Join;
            } else {
                // proxypass = 127.0.0.1
                self.proxy_pass_mode = HttpProxyPassMode::Pass;
            }
        } else {
            if self.origin_proxy_pass.ends_with("/") {
                // proxypass = 127.0.0.1/test/
                self.proxy_pass_mode = HttpProxyPassMode::Join;
            } else {
                // proxypass = 127.0.0.1/test
                self.proxy_pass_mode = HttpProxyPassMode::Contact;
            }
        }

        self.proxy_pass = Some(url);

        Ok(())
    }

    fn make_target(&self, url: &Url) -> Result<String, BuckyError> {
        let target_url;
        let proxy_pass = self.proxy_pass.as_ref().unwrap();
        match self.proxy_pass_mode {
            HttpProxyPassMode::Join => {
                let left = &url.path()[self.path.len()..];
                // info!("left={}", left);
                let mut v = proxy_pass.join(left)?;
                v.set_query(url.query());
                target_url = v.as_str().to_owned();
            }
            HttpProxyPassMode::Pass => {
                let mut v = url.clone();

                if let Err(_) = v.set_scheme(self.proxy_pass.as_ref().unwrap().scheme()) {
                    let msg = format!(
                        "set_scheme for url error! url={}, proxy_pass={}",
                        v, proxy_pass
                    );
                    error!("{}", msg);
                }

                if let Err(e) = v.set_host(self.proxy_pass.as_ref().unwrap().host_str()) {
                    let msg = format!(
                        "set_host for url error! url={}, proxy_pass={}, err={}",
                        v, proxy_pass, e
                    );
                    error!("{}", msg);
                }

                if let Err(_) = v.set_port(self.proxy_pass.as_ref().unwrap().port()) {
                    let msg = format!(
                        "set_port for url error! url={}, proxy_pass={}",
                        v, proxy_pass
                    );
                    error!("{}", msg);
                }

                target_url = v.as_str().to_owned();
            }
            HttpProxyPassMode::Contact => {
                let left = &url.path()[self.path.len()..];
                target_url = self.proxy_pass.as_ref().unwrap().as_str().to_owned() + left;
            }
        }

        info!(
            "make http target: proxy_pass={}, url={}, mode={:?}, target_url={}",
            self.origin_proxy_pass,
            url.as_str(),
            self.proxy_pass_mode,
            target_url
        );
        Ok(target_url)
    }
}

pub(super) struct HttpLocationManager {
    location_list: Vec<Box<HttpStreamLocation>>,
}

impl HttpLocationManager {
    pub fn new() -> HttpLocationManager {
        HttpLocationManager {
            location_list: Vec::new(),
        }
    }

    pub fn load(&mut self, location_list: &Vec<toml::Value>) -> Result<(), BuckyError> {
        assert_eq!(self.location_list.len(), 0);

        for v in location_list {
            let node = v.as_table();
            if node.is_none() {
                continue;
            }

            let item = self.load_location(node.unwrap())?;
            self.location_list.push(Box::new(item));
        }

        Ok(())
    }

    fn load_location(
        &mut self,
        location_node: &toml::value::Table,
    ) -> Result<HttpStreamLocation, BuckyError> {
        let mut location = HttpStreamLocation::new();
        for (k, v) in location_node {
            match k.as_str() {
                "type" => match v.as_str() {
                    Some("=") => {
                        location.mode = HttpLocationPathMode::Equal;
                    }
                    Some("prefix") => {
                        location.mode = HttpLocationPathMode::Prefix;
                    }
                    _ => {
                        error!("unknown location type: {:?}", v);
                    }
                },
                "method" => {
                    location.load_method(v.as_str().unwrap_or(""))?;
                }
                "path" => {
                    location.path = v.as_str().unwrap_or("").to_string();
                }
                "proxy_pass" => {
                    let proxy_pass = v.as_str().unwrap_or("");
                    let proxy_pass = ::base::VAR_MANAGER.translate_addr_str(proxy_pass)?;

                    location.load_proxy_pass(&proxy_pass)?;
                }
                _ => {
                    error!("unknown location filed: {}", k.as_str());
                }
            }
        }

        Ok(location)
    }

    pub fn search(&self, req: &Request) -> Option<String> {
        for item in &self.location_list {
            if let Some(target_url) = item.try_match(req) {
                return Some(target_url);
            }
        }

        None
    }
}
