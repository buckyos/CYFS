use cyfs_base::*;
use crate::*;


#[derive(Clone, Debug)]
pub enum RequestDecType {
    None,

    // dec_id in request header， common fields
    Source,

    // dec_id in request/response's object's dec_id 
    Target,

    Both,
}

impl RequestDecType {
    pub fn new(source: bool, target: bool) -> Self {
        match source{
            true=> match target {
                true => RequestDecType::Both,
                false => RequestDecType::Source,
            }
            false => match target {
                true => RequestDecType::Target,
                false => RequestDecType::None,
            }
        }
    }
}

pub trait RequestDecChecker {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType;
}

impl RequestDecChecker for NONPutObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);
        let target = self.object.object().dec_id().as_ref() == Some(dec_id);

        RequestDecType::new(source, target)
    }
}

impl RequestDecChecker for NONPutObjectInputResponse {
    fn check(&self, _dec_id: &ObjectId) -> RequestDecType {
        RequestDecType::None
    }
}

impl RequestDecChecker for NONGetObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);

        RequestDecType::new(source, false)
    }
}

impl RequestDecChecker for NONGetObjectInputResponse {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let target = self.object.object().dec_id().as_ref() == Some(dec_id);

        RequestDecType::new(false, target)
    }
}


impl RequestDecChecker for NONPostObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);
        let target = self.object.object().dec_id().as_ref() == Some(dec_id);

        RequestDecType::new(source, target)
    }
}

impl RequestDecChecker for NONPostObjectInputResponse {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let target = match &self.object{
            Some(object) => {
                object.object().dec_id().as_ref() == Some(dec_id)
            }
            None => {
                false
            }
        };

        RequestDecType::new(false, target)
    }
}

impl RequestDecChecker for NONSelectObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);
        let target = self.filter.dec_id.as_ref() == Some(dec_id);

        RequestDecType::new(source, target)
    }
}

impl RequestDecChecker for NONSelectObjectInputResponse {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let mut target = false;
        for info in &self.objects {
            if let Some(object) = &info.object {
                target = object.object().dec_id().as_ref() == Some(dec_id);
                if target {
                    break;
                }
            }
        }

        RequestDecType::new(false, target)
    }
}

impl RequestDecChecker for NONDeleteObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);

        RequestDecType::new(source, false)
    }
}

impl RequestDecChecker for NONDeleteObjectInputResponse {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let target = match &self.object{
            Some(object) => {
                object.object().dec_id().as_ref() == Some(dec_id)
            }
            None => {
                false
            }
        };

        RequestDecType::new(false, target)
    }
}


impl RequestDecChecker for NDNPutDataInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);

        RequestDecType::new(source, false)
    }
}

impl RequestDecChecker for NDNPutDataInputResponse {
    fn check(&self, _dec_id: &ObjectId) -> RequestDecType {
        RequestDecType::None
    }
}

impl RequestDecChecker for NDNGetDataInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);

        RequestDecType::new(source, false)
    }
}

impl RequestDecChecker for NDNGetDataInputResponse {
    fn check(&self, _dec_id: &ObjectId) -> RequestDecType {
        RequestDecType::None
    }
}

impl RequestDecChecker for NDNDeleteDataInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);

        RequestDecType::new(source, false)
    }
}

impl RequestDecChecker for NDNDeleteDataInputResponse {
    fn check(&self, _dec_id: &ObjectId) -> RequestDecType {
        RequestDecType::None
    }
}

impl RequestDecChecker for CryptoSignObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);
        let target = self.object.object().dec_id().as_ref() == Some(dec_id);

        RequestDecType::new(source, target)
    }
}

impl RequestDecChecker for CryptoSignObjectInputResponse {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let target = match &self.object{
            Some(object) => {
                object.object().dec_id().as_ref() == Some(dec_id)
            }
            None => {
                false
            }
        };

        RequestDecType::new(false, target)
    }
}

impl RequestDecChecker for CryptoVerifyObjectInputRequest {
    fn check(&self, dec_id: &ObjectId) -> RequestDecType {
        let source = self.common.dec_id.as_ref() == Some(dec_id);
        let target = self.object.object().dec_id().as_ref() == Some(dec_id);

        RequestDecType::new(source, target)
    }
}

impl RequestDecChecker for CryptoVerifyObjectInputResponse {
    fn check(&self, _dec_id: &ObjectId) -> RequestDecType {
        RequestDecType::None
    }
}

impl RequestDecChecker for AclHandlerRequest {
    fn check(&self, _dec_id: &ObjectId) -> RequestDecType {
        RequestDecType::None
    }
}