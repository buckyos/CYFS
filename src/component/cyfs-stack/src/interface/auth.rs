use crate::app::AuthenticatedAppList;
use super::http_server::HttpRequestSource;
use cyfs_base::*;

#[derive(Clone)]
pub struct InterfaceAuth {
    auth_app_list: AuthenticatedAppList,
}

impl InterfaceAuth {
    pub(crate) fn new(auth_app_list: AuthenticatedAppList) -> Self {
        Self {
            auth_app_list,
        }
    }

    pub fn check_dec(&self, dec_id: &ObjectId, source: &HttpRequestSource)-> BuckyResult<()> {
        let addr = match source {
            HttpRequestSource::Remote((device_id, _)) => {
                device_id.to_string()
            }
            HttpRequestSource::Local(addr) => {
                addr.ip().to_string()
            }
        };

        let dec_id_str = dec_id.to_string();
        self.auth_app_list.check_auth(&dec_id_str, &addr)
    }

    pub fn check_option_dec(&self, dec_id: Option<&ObjectId>, source: &HttpRequestSource)-> BuckyResult<()> {
        match  dec_id {
            Some(dec_id) => self.check_dec(dec_id, source,),
            None => {
                let msg = format!("request's dec_id not specified! source={:?}", source);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
        }
    }
}