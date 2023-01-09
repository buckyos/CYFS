use super::category::*;
use super::request::*;
use crate::acl::*;
use crate::base::*;
use crate::crypto::*;
use crate::ndn::*;
use crate::non::*;
use crate::SelectFilter;
use cyfs_base::*;

struct ExpReservedTokenTranslatorHelper;

impl ExpReservedTokenTranslatorHelper {
    /*
    fn trans_router(
        token: &str,
        router: &RouterHandlerRequestRouterInfo,
    ) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "router.source" => ExpTokenEvalValue::from_string(&router.source),
            "router.target" => ExpTokenEvalValue::from_opt_string(&router.target),
            "router.direction" => ExpTokenEvalValue::from_opt_string(&router.direction),

            "router.next_hop" => ExpTokenEvalValue::from_opt_string(&router.next_hop),
            "router.next_direction" => ExpTokenEvalValue::from_opt_string(&router.next_direction),

            _ => {
                return None;
            }
        };

        Some(ret)
    }
    */

    // response的token都以resp.开头
    fn is_response_token(token: &str) -> bool {
        token.starts_with("resp.")
    }

    fn to_response_token(token: &str) -> String {
        format!("resp.{}", token)
    }

    fn from_response_token(token: &str) -> &str {
        token.trim_start_matches("resp.")
    }

    fn trans_request_source(token: &str, source: &RequestSourceInfo) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "source.protocol" => ExpTokenEvalValue::from_string(&source.protocol),
            "source.dec_id" => ExpTokenEvalValue::from_string(&source.dec),
            "source.zone" => ExpTokenEvalValue::from_opt_string(&source.zone.zone),
            "source.device" => ExpTokenEvalValue::from_opt_string(&source.zone.device),
            "source.zone_category" => ExpTokenEvalValue::from_string(&source.zone.zone_category),
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_non_input_request_common(
        token: &str,
        common: &NONInputRequestCommon,
    ) -> Option<ExpTokenEvalValue> {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_request_source(token, &common.source)
        {
            return Some(v);
        }


        let ret = match token {
            "req_path" => ExpTokenEvalValue::from_opt_glob(&common.req_path),
            "level" => ExpTokenEvalValue::from_string(&common.level),
            "target" => ExpTokenEvalValue::from_opt_string(&common.target),
            "flags" => ExpTokenEvalValue::U32(common.flags),
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_bdt_interest_referer(
        token: &str,
        referer: &BdtDataRefererInfo
    ) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "target" => ExpTokenEvalValue::from_opt_string(&referer.target),
            "object_id" => ExpTokenEvalValue::from_string(&referer.object_id),
            "inner_path" => ExpTokenEvalValue::from_opt_glob(&referer.inner_path),
            "dec_id" => ExpTokenEvalValue::from_string(&referer.object_id),
            "req_path" => ExpTokenEvalValue::from_opt_glob(&referer.inner_path),
            "referer_object" => {
                if referer.referer_object.len() > 0 {
                    ExpTokenEvalValue::from_glob_list(&referer.referer_object)
                } else {
                    ExpTokenEvalValue::None
                }
            }, 
            "flags" => ExpTokenEvalValue::U32(referer.flags),
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_ndn_input_request_common(
        token: &str,
        common: &NDNInputRequestCommon,
    ) -> Option<ExpTokenEvalValue> {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_request_source(token, &common.source)
        {
            return Some(v);
        }

        let ret = match token {
            "req_path" => ExpTokenEvalValue::from_opt_glob(&common.req_path),
            "level" => ExpTokenEvalValue::from_string(&common.level),
            "referer_object" => {
                if common.referer_object.len() > 0 {
                    ExpTokenEvalValue::from_glob_list(&common.referer_object)
                } else {
                    ExpTokenEvalValue::None
                }
            }
            "target" => ExpTokenEvalValue::from_opt_string(&common.target),
            "flags" => ExpTokenEvalValue::U32(common.flags),
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_crypto_input_request_common(
        token: &str,
        common: &CryptoInputRequestCommon,
    ) -> Option<ExpTokenEvalValue> {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_request_source(token, &common.source)
        {
            return Some(v);
        }

        let ret = match token {
            "req_path" => ExpTokenEvalValue::from_opt_glob(&common.req_path),
            "target" => ExpTokenEvalValue::from_opt_string(&common.target),
            "flags" => ExpTokenEvalValue::U32(common.flags),
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_object(token: &str, object: Option<&AnyNamedObject>) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "obj_type" => match object {
                Some(v) => ExpTokenEvalValue::U16(v.obj_type()),
                None => ExpTokenEvalValue::None,
            },
            "object.dec_id" => match object {
                Some(v) => ExpTokenEvalValue::from_opt_string(v.dec_id()),
                None => ExpTokenEvalValue::None,
            },
            "object.author" => match object {
                Some(v) => ExpTokenEvalValue::from_opt_string(&v.author()),
                None => ExpTokenEvalValue::None,
            },
            "object.owner" => match object {
                Some(v) => ExpTokenEvalValue::from_opt_string(&v.owner()),
                None => ExpTokenEvalValue::None,
            },

            _ => return None,
        };

        Some(ret)
    }

    fn trans_object_info(token: &str, info: Option<&NONObjectInfo>) -> Option<ExpTokenEvalValue> {
        let object_id = match info {
            Some(info) => Some(&info.object_id),
            None => None,
        };
        if let Some(v) = Self::trans_object_id(token, object_id) {
            return Some(v);
        }

        let object = match info {
            Some(info) => info.object.as_deref(),
            None => None,
        };
        if let Some(v) = Self::trans_object(token, object) {
            return Some(v);
        }

        None
    }

    fn trans_select_filter(token: &str, filter: &SelectFilter) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "filter.obj_type" => match filter.obj_type.to_owned() {
                Some(v) => ExpTokenEvalValue::U16(v),
                None => ExpTokenEvalValue::None,
            },
            "filter.obj_type_code" => match filter.obj_type_code.as_ref() {
                Some(code) => ExpTokenEvalValue::U16(code.into()),
                None => ExpTokenEvalValue::None,
            },

            "filter.dec_id" => ExpTokenEvalValue::from_opt_string(&filter.dec_id),
            "filter.owner_id" => ExpTokenEvalValue::from_opt_string(&filter.owner_id),
            "filter.author_id" => ExpTokenEvalValue::from_opt_string(&filter.author_id),
            "filter.flags" => match filter.flags.to_owned() {
                Some(v) => ExpTokenEvalValue::U32(v),
                None => ExpTokenEvalValue::None,
            },
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_area(token: &str, area: Option<&Area>) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "area.country" => match area {
                Some(area) => ExpTokenEvalValue::U16(area.country),
                None => ExpTokenEvalValue::None,
            },
            "area.carrier" => match area {
                Some(area) => ExpTokenEvalValue::U8(area.carrier),
                None => ExpTokenEvalValue::None,
            },
            "area.city" => match area {
                Some(area) => ExpTokenEvalValue::U16(area.city),
                None => ExpTokenEvalValue::None,
            },
            "area.inner" => match area {
                Some(area) => ExpTokenEvalValue::U8(area.inner),
                None => ExpTokenEvalValue::None,
            },
            _ => {
                return None;
            }
        };

        Some(ret)
    }

    fn trans_object_id(token: &str, object_id: Option<&ObjectId>) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "object_id" => match object_id {
                Some(object_id) => ExpTokenEvalValue::from_string(object_id),
                None => ExpTokenEvalValue::None,
            },
            "obj_type_code" => match object_id {
                Some(object_id) => ExpTokenEvalValue::U16(object_id.obj_type_code().into()),
                None => ExpTokenEvalValue::None,
            },
            "obj_category" => match object_id {
                Some(object_id) => ExpTokenEvalValue::from_string(&object_id.object_category()),
                None => ExpTokenEvalValue::None,
            },
            _ => {
                if token.starts_with("area.") {
                    let area = if let Some(object_id) = object_id {
                        let info: ObjectIdInfo = object_id.info();
                        if let Some(area) = info.into_area() {
                            Some(area)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(v) = Self::trans_area(token, area.as_ref()) {
                        return Some(v);
                    }

                    unreachable!();
                }

                return None;
            }
        };

        Some(ret)
    }

    fn trans_bucky_error(token: &str, error: &BuckyError) -> Option<ExpTokenEvalValue> {
        let ret = match token {
            "error.code" => ExpTokenEvalValue::U32(error.code().into()),
            "error.msg" => ExpTokenEvalValue::String(error.msg().to_owned()),
            _ => return None,
        };

        Some(ret)
    }
}

impl ExpReservedTokenTranslator for NONPutObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_non_input_request_common(token, &self.common)
        {
            return v;
        }

        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_info(token, Some(&self.object))
        {
            return v;
        }

        unreachable!("unknown router put_object reserved token: {}", token);
    }
}

impl ExpReservedTokenTranslator for NONPutObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "result" => ExpTokenEvalValue::from_string(&self.result),
            "object_update_time" => {
                ExpTokenEvalValue::U64(self.object_update_time.to_owned().unwrap_or(0))
            }
            "object_expires_time" => {
                ExpTokenEvalValue::U64(self.object_expires_time.to_owned().unwrap_or(0))
            }

            _ => ExpTokenEvalValue::None,
        }
    }
}

impl ExpReservedTokenTranslator for NONGetObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "inner_path" => ExpTokenEvalValue::from_opt_glob(&self.inner_path),

            _ => {
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
                {
                    return v;
                }

                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_non_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }

                unreachable!(
                    "unknown router get_object request reserved token: {}",
                    token
                );
            }
        }
    }
}

impl ExpReservedTokenTranslator for NONGetObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_info(token, Some(&self.object))
        {
            return v;
        }

        ExpTokenEvalValue::None
    }
}

impl ExpReservedTokenTranslator for NONPostObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_non_input_request_common(token, &self.common)
        {
            return v;
        }

        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_info(token, Some(&self.object))
        {
            return v;
        }
        unreachable!(
            "unknown router post_object request reserved token: {}",
            token
        );
    }
}

impl ExpReservedTokenTranslator for NONPostObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_info(token, self.object.as_ref())
        {
            return v;
        }

        ExpTokenEvalValue::None
    }
}

impl ExpReservedTokenTranslator for NONSelectObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_non_input_request_common(token, &self.common)
        {
            return v;
        }

        if let Some(v) = ExpReservedTokenTranslatorHelper::trans_select_filter(token, &self.filter)
        {
            return v;
        }

        unreachable!(
            "unknown router select_object request reserved token: {}",
            token
        );
    }
}

impl ExpReservedTokenTranslator for NONSelectObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if self.objects.is_empty() {
            return ExpTokenEvalValue::None;
        }

        // TODO 支持多表达式

        let first = &self.objects[0];
        if let Some(ret) =
            ExpReservedTokenTranslatorHelper::trans_object_info(token, first.object.as_ref())
        {
            return ret;
        }

        ExpTokenEvalValue::None
    }
}

impl ExpReservedTokenTranslator for NONDeleteObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if token == "inner_path" {
            return ExpTokenEvalValue::from_opt_glob(&self.inner_path);
        }
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
        {
            return v;
        }
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_non_input_request_common(token, &self.common)
        {
            return v;
        }

        unreachable!("unknown router delete_object reserved token: {}", token);
    }
}

impl ExpReservedTokenTranslator for NONDeleteObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_info(token, self.object.as_ref())
        {
            return v;
        }

        ExpTokenEvalValue::None
    }
}

// put_data
impl ExpReservedTokenTranslator for NDNPutDataInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_ndn_input_request_common(token, &self.common)
        {
            return v;
        }

        if let Some(v) =
            ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
        {
            return v;
        }

        unreachable!("unknown ndn put_data reserved token: {}", token);
    }
}

impl ExpReservedTokenTranslator for NDNPutDataInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "result" => ExpTokenEvalValue::from_string(&self.result),
            _ => ExpTokenEvalValue::None,
        }
    }
}

// get_data
impl ExpReservedTokenTranslator for NDNGetDataInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "inner_path" => ExpTokenEvalValue::from_opt_glob(&self.inner_path),
            _ => {
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_ndn_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
                {
                    return v;
                }

                unreachable!("unknown ndn get_data reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for NDNGetDataInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "attr" => match &self.attr {
                Some(v) => ExpTokenEvalValue::U32(v.flags()),
                None => ExpTokenEvalValue::None,
            },
            "length" => ExpTokenEvalValue::U64(self.length),
            _ => {
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
                {
                    return v;
                }

                ExpTokenEvalValue::None
            }
        }
    }
}

// delete_data
impl ExpReservedTokenTranslator for NDNDeleteDataInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "inner_path" => ExpTokenEvalValue::from_opt_glob(&self.inner_path),
            _ => {
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_ndn_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
                {
                    return v;
                }

                unreachable!("unknown ndn delete_data reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for NDNDeleteDataInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            _ => {
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_id(token, Some(&self.object_id))
                {
                    return v;
                }

                ExpTokenEvalValue::None
            }
        }
    }
}

// sign_object
impl ExpReservedTokenTranslator for CryptoSignObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "crypto_flags" => ExpTokenEvalValue::U32(self.flags),
            _ => {
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_crypto_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_info(token, Some(&self.object))
                {
                    return v;
                }

                unreachable!("unknown crypto sign_object reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for CryptoSignObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "result" => ExpTokenEvalValue::from_string(&self.result),
            _ => {
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_info(token, self.object.as_ref())
                {
                    return v;
                }

                ExpTokenEvalValue::None
            }
        }
    }
}

// verify_object
impl ExpReservedTokenTranslator for CryptoVerifyObjectInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "sign_type" => ExpTokenEvalValue::from_string(&self.sign_type),
            "sign_object_type" => ExpTokenEvalValue::from_string(&self.sign_object),
            _ => {
                if token.starts_with("sign_object") {
                    let sub_token = token.trim_start_matches("sign_object.");

                    match &self.sign_object {
                        VerifyObjectType::Object(sign_object) => {
                            if let Some(v) = ExpReservedTokenTranslatorHelper::trans_object_id(
                                sub_token,
                                Some(&sign_object.object_id),
                            ) {
                                return v;
                            }

                            let sign_object = sign_object.object.as_ref().map(|v| v.as_ref());
                            if let Some(v) = ExpReservedTokenTranslatorHelper::trans_object(
                                sub_token,
                                sign_object,
                            ) {
                                return v;
                            }

                            unreachable!("unknown crypto verify_object reserved token: {}", token);
                        }
                        _ => {
                            return ExpTokenEvalValue::None;
                        }
                    }
                }

                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_crypto_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }
                if let Some(v) =
                    ExpReservedTokenTranslatorHelper::trans_object_info(token, Some(&self.object))
                {
                    return v;
                }

                unreachable!("unknown crypto verify_object reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for CryptoVerifyObjectInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "valid" => ExpTokenEvalValue::Bool(self.result.valid),
            _ => ExpTokenEvalValue::None,
        }
    }
}

// encrypt_data
impl ExpReservedTokenTranslator for CryptoEncryptDataInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "encrypt_type" => ExpTokenEvalValue::String(self.encrypt_type.to_string()),
            "crypto_flags" => ExpTokenEvalValue::U32(self.flags),
            _ => {
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_crypto_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }

                unreachable!("unknown crypto encrypt_data reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for CryptoEncryptDataInputResponse {
    fn trans(&self, _token: &str) -> ExpTokenEvalValue {
        ExpTokenEvalValue::None
    }
}

// decrypt_data
impl ExpReservedTokenTranslator for CryptoDecryptDataInputRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "decrypt_type" => ExpTokenEvalValue::String(self.decrypt_type.to_string()),
            "crypto_flags" => ExpTokenEvalValue::U32(self.flags),
            _ => {
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_crypto_input_request_common(
                    token,
                    &self.common,
                ) {
                    return v;
                }

                unreachable!("unknown crypto decrypt_data reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for CryptoDecryptDataInputResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "result" => ExpTokenEvalValue::from_string(&self.result),
            _ => {
                ExpTokenEvalValue::None
            }
        }
    }
}

// acl
impl ExpReservedTokenTranslator for AclHandlerRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "protocol" => ExpTokenEvalValue::from_string(&self.protocol),
            "direction" => ExpTokenEvalValue::from_string(&self.action.direction),
            "operation" => ExpTokenEvalValue::from_string(&self.action.operation),

            "device" | "source" | "target" | "your" => {
                ExpTokenEvalValue::from_string(&self.device_id)
            }

            "inner_path" => ExpTokenEvalValue::from_opt_glob(&self.inner_path),
            "dec_id" => ExpTokenEvalValue::from_string(&self.dec_id),
            "req_path" => ExpTokenEvalValue::from_opt_glob(&self.req_path),
            "referer_object" => {
                if let Some(referer_object) = &self.referer_object {
                    if referer_object.len() > 0 {
                        ExpTokenEvalValue::from_glob_list(referer_object)
                    } else {
                        ExpTokenEvalValue::None
                    }
                } else {
                    ExpTokenEvalValue::None
                }
            }

            _ => {
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_object_id(
                    token,
                    self.object.as_ref().map(|item| &item.object_id),
                ) {
                    return v;
                }

                let object = self
                    .object
                    .as_ref()
                    .map(|item| item.object.as_deref())
                    .flatten();
                if let Some(v) = ExpReservedTokenTranslatorHelper::trans_object(token, object) {
                    return v;
                }

                unreachable!("unknown router acl request reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for AclHandlerResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "access" => ExpTokenEvalValue::from_string(&self.access),
            _ => ExpTokenEvalValue::None,
        }
    }
}


// interest
impl ExpReservedTokenTranslator for InterestHandlerRequest {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "chunk" => ExpTokenEvalValue::from_string(&self.chunk), 
            "from" => ExpTokenEvalValue::from_opt_string(&self.from), 
            "from_channel" => ExpTokenEvalValue::from_string(&self.from_channel), 
            _ => {
                if let Some(referer) = &self.referer {
                    if let Some(v) = ExpReservedTokenTranslatorHelper::trans_bdt_interest_referer(token, referer) {
                        return v;
                    }
                }
                unreachable!("unknown router interest request reserved token: {}", token);
            }
        }
    }
}

impl ExpReservedTokenTranslator for InterestHandlerResponse {
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "type" => ExpTokenEvalValue::String(self.type_str().to_owned()), 
            "transmit_to" => ExpTokenEvalValue::from_opt_string(&self.transmit_to().clone()), 
            "err" => {
                if let Some(err) = self.resp_interest().map(|r| r.err.as_u16()) {
                    ExpTokenEvalValue::U32(err as u32)
                } else {
                    ExpTokenEvalValue::None
                }
            }, 
            "redirect" => ExpTokenEvalValue::from_opt_string(&self.resp_interest().and_then(|r| r.redirect.clone())), 
            "redirect_referer_target" => ExpTokenEvalValue::from_opt_string(&self.resp_interest().and_then(|r| r.redirect_referer_target.clone())), 
            _ => {
                unreachable!("unknown router interest response reserved token: {}", token);
            }, 
        }
    }
}

impl<REQ, RESP> ExpReservedTokenTranslator for RouterHandlerRequest<REQ, RESP>
where
    REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + std::fmt::Display,
    RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + std::fmt::Display,
{
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        if ExpReservedTokenTranslatorHelper::is_response_token(token) {
            let token = ExpReservedTokenTranslatorHelper::from_response_token(token);
            if let Some(resp) = &self.response {
                match resp {
                    Ok(resp) => resp.trans(token),
                    Err(e) => {
                        if let Some(ret) =
                            ExpReservedTokenTranslatorHelper::trans_bucky_error(token, e)
                        {
                            return ret;
                        }

                        ExpTokenEvalValue::None
                    }
                }
            } else {
                ExpTokenEvalValue::None
            }
        } else {
            self.request.trans(token)
        }
    }
}

pub struct RouterHandlerReservedTokenList {
    pub put_object: ExpReservedTokenList,
    pub get_object: ExpReservedTokenList,
    pub post_object: ExpReservedTokenList,
    pub select_object: ExpReservedTokenList,
    pub delete_object: ExpReservedTokenList,

    pub get_data: ExpReservedTokenList,
    pub put_data: ExpReservedTokenList,
    pub delete_data: ExpReservedTokenList,

    pub sign_object: ExpReservedTokenList,
    pub verify_object: ExpReservedTokenList,
    pub encrypt_data: ExpReservedTokenList,
    pub decrypt_data: ExpReservedTokenList,

    pub acl: ExpReservedTokenList,
    pub interest: ExpReservedTokenList, 
}

impl RouterHandlerReservedTokenList {
    fn new() -> Self {
        Self {
            put_object: Self::gen_put_object(),
            get_object: Self::gen_get_object(),
            post_object: Self::gen_post_object(),
            select_object: Self::gen_select_object(),
            delete_object: Self::gen_delete_object(),

            get_data: Self::gen_get_data(),
            put_data: Self::gen_put_data(),
            delete_data: Self::gen_delete_data(),

            sign_object: Self::gen_sign_object(),
            verify_object: Self::gen_verify_object(),
            encrypt_data: Self::gen_encrypt_object(),
            decrypt_data: Self::gen_decrypt_object(),

            acl: Self::gen_acl(),
            interest: Self::gen_interest(), 
        }
    }

    fn add_bdt_interest_referer_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_string("target");
        token_list.add_string("object_id");
        token_list.add_glob("inner_path");
        token_list.add_string("dec_id");
        token_list.add_glob("req_path");
        token_list.add_glob("referer_object");
        token_list.add_u32("flags");
    }

    fn add_non_input_request_common_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_glob("req_path");

        token_list.add_string("source.dec_id");
        token_list.add_string("source.device");
        token_list.add_string("source.zone_category");
        token_list.add_string("source.zone");
        token_list.add_string("source.protocol");

        token_list.add_string("level");
        token_list.add_string("target");
        token_list.add_u32("flags");
    }

    fn add_ndn_input_request_common_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_glob("req_path");
        
        token_list.add_string("source.dec_id");
        token_list.add_string("source.device");
        token_list.add_string("source.zone_category");
        token_list.add_string("source.zone");
        token_list.add_string("source.protocol");

        token_list.add_string("level");
        token_list.add_glob("referer_object");
        token_list.add_string("target");
        token_list.add_u32("flags");
    }

    fn add_crypto_input_request_common_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_glob("req_path");
        
        token_list.add_string("source.dec_id");
        token_list.add_string("source.device");
        token_list.add_string("source.zone_category");
        token_list.add_string("source.zone");
        token_list.add_string("source.protocol");
        
        token_list.add_string("target");
        token_list.add_u32("flags");
    }

    fn add_area_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_u16("area.country");
        token_list.add_u8("area.carrier");
        token_list.add_u16("area.city");
        token_list.add_u8("area.inner");
    }

    fn add_object_id_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_string("object_id");
        token_list.add_u16("obj_type_code");
        token_list.add_string("obj_category");

        Self::add_area_tokens(token_list);
    }

    fn add_object_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_u16("obj_type");
        token_list.add_string("object.dec_id");
        token_list.add_string("object.author");
        token_list.add_string("object.owner");
    }

    fn add_object_info_tokens(token_list: &mut ExpReservedTokenList) {
        Self::add_object_tokens(token_list);
        Self::add_object_id_tokens(token_list);
    }

    fn add_error_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_u32("error.code");
        token_list.add_string("error.msg");
    }

    fn add_filter_tokens(token_list: &mut ExpReservedTokenList) {
        token_list.add_u16("filter.obj_type");
        token_list.add_u16("filter.obj_type_code");
        token_list.add_string("filter.dec_id");
        token_list.add_string("filter.owner_id");
        token_list.add_string("filter.author_id");
        token_list.add_u32("filter.flags");
    }

    // put_object
    fn gen_put_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_non_input_request_common_tokens(&mut token_list);
        Self::add_object_info_tokens(&mut token_list);

        token_list
    }

    fn gen_put_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_string("result");
        token_list.add_u64("object_update_time");
        token_list.add_u64("object_expires_time");

        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_put_object() -> ExpReservedTokenList {
        let mut list = Self::gen_put_object_request();
        list.append(Self::gen_put_object_response());

        list
    }

    // get_object
    fn gen_get_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_glob("inner_path");
        Self::add_non_input_request_common_tokens(&mut token_list);
        Self::add_object_id_tokens(&mut token_list);

        token_list
    }

    fn gen_get_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_object_info_tokens(&mut token_list);
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_get_object() -> ExpReservedTokenList {
        let mut list = Self::gen_get_object_request();
        list.append(Self::gen_get_object_response());

        list
    }

    // post_object
    fn gen_post_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_non_input_request_common_tokens(&mut token_list);
        Self::add_object_info_tokens(&mut token_list);

        token_list
    }

    fn gen_post_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_object_info_tokens(&mut token_list);
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_post_object() -> ExpReservedTokenList {
        let mut list = Self::gen_post_object_request();
        list.append(Self::gen_post_object_response());

        list
    }

    // select
    fn gen_select_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_filter_tokens(&mut token_list);
        Self::add_non_input_request_common_tokens(&mut token_list);

        token_list
    }

    fn gen_select_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_object_info_tokens(&mut token_list);
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_select_object() -> ExpReservedTokenList {
        let mut list = Self::gen_select_object_request();
        list.append(Self::gen_select_object_response());

        list
    }

    // delete_object
    fn gen_delete_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_glob("inner_path");
        Self::add_object_id_tokens(&mut token_list);
        Self::add_non_input_request_common_tokens(&mut token_list);

        token_list
    }

    fn gen_delete_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_object_info_tokens(&mut token_list);
        Self::add_error_tokens(&mut token_list);
        token_list.translate_resp();

        token_list
    }

    fn gen_delete_object() -> ExpReservedTokenList {
        let mut list = Self::gen_delete_object_request();
        list.append(Self::gen_delete_object_response());

        list
    }

    // get_data
    fn gen_get_data_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_glob("inner_path");
        Self::add_ndn_input_request_common_tokens(&mut token_list);
        Self::add_object_id_tokens(&mut token_list);

        token_list
    }

    fn gen_get_data_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_object_id_tokens(&mut token_list);
        token_list.add_u32("attr");
        token_list.add_u32("length");
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_get_data() -> ExpReservedTokenList {
        let mut list = Self::gen_get_data_request();
        list.append(Self::gen_get_data_response());

        list
    }

    // put_data
    fn gen_put_data_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_ndn_input_request_common_tokens(&mut token_list);
        Self::add_object_id_tokens(&mut token_list);
        token_list.add_u32("length");

        token_list
    }

    fn gen_put_data_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_string("result");
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_put_data() -> ExpReservedTokenList {
        let mut list = Self::gen_put_data_request();
        list.append(Self::gen_put_data_response());

        list
    }

    // delete_data
    fn gen_delete_data_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_glob("inner_path");
        Self::add_ndn_input_request_common_tokens(&mut token_list);
        Self::add_object_id_tokens(&mut token_list);

        token_list
    }

    fn gen_delete_data_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_object_id_tokens(&mut token_list);
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_delete_data() -> ExpReservedTokenList {
        let mut list = Self::gen_delete_data_request();
        list.append(Self::gen_delete_data_response());

        list
    }

    // sign_object
    fn gen_sign_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_crypto_input_request_common_tokens(&mut token_list);
        Self::add_object_info_tokens(&mut token_list);

        token_list.add_u32("crypto_flags");

        token_list
    }

    fn gen_sign_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_string("result");
        Self::add_object_info_tokens(&mut token_list);

        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_sign_object() -> ExpReservedTokenList {
        let mut list = Self::gen_sign_object_request();
        list.append(Self::gen_sign_object_response());

        list
    }

    // verify_object
    fn gen_verify_object_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_crypto_input_request_common_tokens(&mut token_list);
        Self::add_object_info_tokens(&mut token_list);

        token_list.add_string("sign_type");

        //// 对枚举类型sign_object的关键字支持

        token_list.add_string("sign_object_type");

        let mut sign_object_token_list = ExpReservedTokenList::new();
        Self::add_object_info_tokens(&mut sign_object_token_list);
        sign_object_token_list.translate("sign_object");

        token_list.append(sign_object_token_list);

        // TODO 对VerifyObjectType::Sign的支持

        token_list
    }

    fn gen_verify_object_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_bool("valid");
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_verify_object() -> ExpReservedTokenList {
        let mut list = Self::gen_verify_object_request();
        list.append(Self::gen_verify_object_response());

        list
    }

    // encrypt_data
    fn gen_encrypt_data_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_crypto_input_request_common_tokens(&mut token_list);
    
        token_list.add_u32("crypto_flags");
        token_list.add_string("encrypt_type");

        token_list
    }

    fn gen_encrypt_data_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();
        token_list.translate_resp();

        token_list
    }

    fn gen_encrypt_object() -> ExpReservedTokenList {
        let mut list = Self::gen_encrypt_data_request();
        list.append(Self::gen_encrypt_data_response());

        list
    }

    // decrypt data
    fn gen_decrypt_data_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        Self::add_crypto_input_request_common_tokens(&mut token_list);
    
        token_list.add_u32("crypto_flags");
        token_list.add_string("decrypt_type");

        token_list
    }

    fn gen_decrypt_data_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();
        token_list.add_string("result");
        token_list.translate_resp();

        token_list
    }

    fn gen_decrypt_object() -> ExpReservedTokenList {
        let mut list = Self::gen_decrypt_data_request();
        list.append(Self::gen_decrypt_data_response());

        list
    }

    // acl
    fn gen_acl_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_string("protocol");
        token_list.add_string("direction");
        token_list.add_string("operation");

        // 下面四个都对应device
        token_list.add_string("device");
        token_list.add_string("source");
        token_list.add_string("target");
        token_list.add_string("your");

        Self::add_object_info_tokens(&mut token_list);
        token_list.add_glob("inner_path");

        token_list.add_string("dec_id");
        token_list.add_glob("req_path");

        token_list.add_glob("referer_object");

        token_list
    }

    fn gen_acl_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_bool("access");
        Self::add_error_tokens(&mut token_list);

        token_list.translate_resp();

        token_list
    }

    fn gen_acl() -> ExpReservedTokenList {
        let mut list = Self::gen_acl_request();
        list.append(Self::gen_acl_response());

        list
    }

    // interest
    fn gen_interest_request() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();
        
        token_list.add_string("chunk");
        token_list.add_string("from");
        Self::add_bdt_interest_referer_tokens(&mut token_list);
        token_list.add_string("from_channel");

        token_list
    }

    fn gen_interest_response() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        token_list.add_string("type");
        token_list.add_string("transmit_to");
        token_list.add_u32("err");
        token_list.add_glob("redirect");
        token_list.add_string("redirect_referer_target");
        
        token_list.translate_resp();

        token_list
    }

    fn gen_interest() -> ExpReservedTokenList {
        let mut list = Self::gen_interest_request();
        list.append(Self::gen_interest_response());

        list
    }


    pub fn select<REQ, RESP>(&self) -> &ExpReservedTokenList
    where
        REQ: Send + Sync + 'static + JsonCodec<REQ> + std::fmt::Display,
        RESP: Send + Sync + 'static + JsonCodec<RESP> + std::fmt::Display,
        RouterHandlerRequest<REQ, RESP>: super::category::RouterHandlerCategoryInfo,
    {
        let category = extract_router_handler_category::<RouterHandlerRequest<REQ, RESP>>();
        match category {
            RouterHandlerCategory::GetObject => &self.get_object,
            RouterHandlerCategory::PutObject => &self.put_object,
            RouterHandlerCategory::PostObject => &self.post_object,
            RouterHandlerCategory::SelectObject => &self.select_object,
            RouterHandlerCategory::DeleteObject => &self.delete_object,

            RouterHandlerCategory::PutData => &self.put_data,
            RouterHandlerCategory::GetData => &self.get_data,
            RouterHandlerCategory::DeleteData => &self.delete_data,

            RouterHandlerCategory::SignObject => &self.sign_object,
            RouterHandlerCategory::VerifyObject => &self.verify_object,
            RouterHandlerCategory::EncryptData => &self.encrypt_data,
            RouterHandlerCategory::DecryptData => &self.decrypt_data,

            RouterHandlerCategory::Acl => &self.acl, 
            RouterHandlerCategory::Interest => &self.interest,
        }
    }
}

lazy_static::lazy_static! {
    pub static ref ROUTER_HANDLER_RESERVED_TOKEN_LIST: RouterHandlerReservedTokenList = RouterHandlerReservedTokenList::new();
}
