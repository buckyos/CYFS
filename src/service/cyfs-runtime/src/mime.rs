use cyfs_lib::NDNAction;

use http_types::{mime, Mime, Url};
use mime_sniffer::{HttpRequest, MimeTypeSniffer};
use std::borrow::Cow;
use std::str::FromStr;

pub(crate) struct MimeHelper;

impl MimeHelper {
    fn mime_from_ext(ext: &str) -> Option<Mime> {
        match ext.to_lowercase().as_ref() {
            "html" => Some(mime::HTML),
            "js" | "mjs" | "jsonp" => Some(mime::JAVASCRIPT),
            "json" => Some(mime::JSON),
            "css" => Some(mime::CSS),
            "svg" => Some(mime::SVG),
            "xml" => Some(mime::XML),
            "png" => Some(mime::PNG),
            "jpg" => Some(mime::JPEG),
            "ico" => Some(mime::ICO),
            _ => None,
        }
    }

    fn try_set_mime_from_ext(url: &Url, resp: &mut ::http_types::Response) -> bool {
        let filename = match url.path_segments() {
            Some(p) => match p.last() {
                Some(v) => v,
                None => return false,
            },
            None => return false,
        };

        let parts: Vec<&str> = filename.split('.').collect();
        if parts.len() < 2 {
            return false;
        }

        match parts.last() {
            Some(ext) => match Self::mime_from_ext(ext) {
                Some(m) => {
                    info!("mime from ext: {} -> {}", filename, m);
                    resp.set_content_type(m);
                    true
                }
                None => {
                    warn!("unknown request file ext: {}", ext);
                    false
                }
            },
            None => {
                warn!("request file without ext: {}", filename);
                false
            }
        }
    }

    pub(crate) async fn try_set_mime(url: Url, resp: &mut ::http_types::Response) {
        // 只有ndn get_data请求才需要猜测content-type
        match resp.header(cyfs_base::CYFS_NDN_ACTION).map(|h| h.as_str()) {
            Some(v) => {
                if v != &NDNAction::GetData.to_string() {
                    return;
                }
            }
            None => {
                return;
            }
        }

        // 根据扩展名来判断
        // ndn的get_data的inner_path会反馈在url path上
        if Self::try_set_mime_from_ext(&url, resp) {
            return;
        }

        // 根据内容猜测
        // read some content and try sniff the mime
        use async_std::io::ReadExt;
        let mut body = resp.take_body().into_reader();
        let body_len = resp.len();
        let mut content: Vec<u8> = vec![0; 512];
        let read_content_len;
        match body.read(&mut content).await {
            Ok(bytes) => {
                if bytes == 0 {
                    warn!("sniff mime read resp body but got empty! url={}", url);
                    return;
                }

                read_content_len = bytes;
            }
            Err(e) => {
                error!("sniff read resp body but error! url={}, {}", url, e);
                resp.set_body(http_types::Body::from_reader(body, body_len));
                return;
            }
        }

        let str_url = url.to_string();

        let mut count = 0;
        let mut type_hint = Cow::Borrowed("*/*");
        let mut result = None;
        loop {
            if count > 5 {
                warn!("sniff mime more then 5 times! url={}", str_url);
                break;
            }
            count += 1;

            let hreq = HttpRequest {
                content: &content,
                url: &str_url,
                type_hint: &type_hint,
            };
            match hreq.sniff_mime_type() {
                Some(v) => {
                    if let Some(prev) = &result {
                        if prev == v {
                            break;
                        }
                    }

                    result = Some(v.to_owned());

                    if v.ends_with("/xml") {
                        type_hint = Cow::Owned(v.to_owned());
                        continue;
                    }

                    break;
                }
                None => {
                    warn!("sniff mime from url but not found: {}", url);
                    break;
                }
            };
        }

        if let Some(v) = result {
            match ::tide::http::Mime::from_str(&v) {
                Ok(m) => {
                    info!("sniff mime: {} -> {}", url, m);
                    resp.set_content_type(m);
                }
                Err(e) => {
                    error!("parse mime error! value={}, {}", v, e);
                }
            }
        }

        let new_body = http_types::Body::from_bytes(content[..read_content_len].to_vec());

        let tail_len = match body_len {
            Some(len) => {
                if len >= read_content_len {
                    Some(len - read_content_len)
                } else {
                    error!(
                        "sniff mime but read len is more than content len: {} > {}",
                        read_content_len, len
                    );
                    None
                }
            }
            None => None,
        };

        let tail_body = http_types::Body::from_reader(body, tail_len);
        let new_body = new_body.chain(tail_body);
        resp.set_body(new_body);
    }
}
