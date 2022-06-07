use crate::codec as cyfs_base;
use crate::*;
use serde::Serialize;

use std::convert::TryFrom;
use std::str::FromStr;

#[repr(u8)]
#[derive(Clone, Debug, Eq, PartialEq, RawEncode, RawDecode, Serialize)]
pub enum OODWorkMode {
    Standalone = 0,
    ActiveStandby = 1,
}

impl OODWorkMode {
    pub fn as_str(&self) -> &str {
        match &self {
            Self::Standalone => "standalone",
            Self::ActiveStandby => "active-standby",
        }
    }
}

impl std::fmt::Display for OODWorkMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for OODWorkMode {
    type Err = BuckyError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "standalone" => Self::Standalone,
            "active-standby" => Self::ActiveStandby,
            _ => {
                let msg = format!("unknown OODWorkMode: {}", value);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct PeopleDescContent {}

impl PeopleDescContent {
    pub fn new() -> Self {
        Self {}
    }
}

impl DescContent for PeopleDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::People.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = Option<Area>;
    type AuthorType = SubDescNone;
    type PublicKeyType = PublicKey;
}

#[derive(Clone, Debug)]
pub struct PeopleBodyContent {
    ood_work_mode: Option<OODWorkMode>,
    ood_list: Vec<DeviceId>,
    name: Option<String>,
    icon: Option<FileId>,
}

impl BodyContent for PeopleBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

// body使用protobuf编解码
impl TryFrom<protos::PeopleBodyContent> for PeopleBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::PeopleBodyContent) -> BuckyResult<Self> {
        let mut ret = Self {
            ood_work_mode: None,
            ood_list: ProtobufCodecHelper::decode_buf_list(value.take_ood_list())?,
            name: None,
            icon: None,
        };

        if value.has_ood_work_mode() {
            ret.ood_work_mode = Some(OODWorkMode::from_str(value.get_ood_work_mode())?);
        }

        if value.has_name() {
            ret.name = Some(value.take_name());
        }
        if value.has_icon() {
            ret.icon = Some(ProtobufCodecHelper::decode_buf(value.take_icon())?);
        }

        Ok(ret)
    }
}

impl TryFrom<&PeopleBodyContent> for protos::PeopleBodyContent {
    type Error = BuckyError;

    fn try_from(value: &PeopleBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_ood_list(ProtobufCodecHelper::encode_buf_list(&value.ood_list)?);
        
        if let Some(ood_work_mode) = &value.ood_work_mode {
            ret.set_ood_work_mode(ood_work_mode.to_string());
        }
        
        if let Some(name) = &value.name {
            ret.set_name(name.to_owned());
        }
        if let Some(icon) = &value.icon {
            ret.set_icon(icon.to_vec()?);
        }

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(PeopleBodyContent);

impl PeopleBodyContent {
    pub fn new(
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        name: Option<String>,
        icon: Option<FileId>,
    ) -> Self {
        Self {
            ood_work_mode: Some(ood_work_mode),
            ood_list,
            name,
            icon,
        }
    }

    pub fn ood_work_mode(&self) -> OODWorkMode {
        self.ood_work_mode.clone().unwrap_or(OODWorkMode::Standalone)
    }

    pub fn set_ood_work_mode(&mut self, ood_work_mode: OODWorkMode) {
        self.ood_work_mode = Some(ood_work_mode);
    }

    pub fn ood_list(&self) -> &Vec<DeviceId> {
        &self.ood_list
    }

    pub fn ood_list_mut(&mut self) -> &mut Vec<DeviceId> {
        &mut self.ood_list
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|f| f.as_str())
    }
    pub fn icon(&self) -> Option<&FileId> {
        self.icon.as_ref()
    }

    pub fn set_name(&mut self, name: String) {
        self.name = Some(name)
    }
    pub fn set_icon(&mut self, icon: FileId) {
        self.icon = Some(icon)
    }
}

pub type PeopleType = NamedObjType<PeopleDescContent, PeopleBodyContent>;
pub type PeopleBuilder = NamedObjectBuilder<PeopleDescContent, PeopleBodyContent>;

pub type PeopleDesc = NamedObjectDesc<PeopleDescContent>;
pub type PeopleId = NamedObjectId<PeopleType>;
pub type People = NamedObjectBase<PeopleType>;

impl PeopleDesc {
    pub fn people_id(&self) -> PeopleId {
        PeopleId::try_from(self.calculate_id()).unwrap()
    }
}

impl People {
    pub fn new(
        owner: Option<ObjectId>,
        ood_list: Vec<DeviceId>,
        public_key: PublicKey,
        area: Option<Area>,
        name: Option<String>,
        icon: Option<FileId>,
    ) -> PeopleBuilder {
        let desc_content = PeopleDescContent::new();

        let body_content = PeopleBodyContent::new(OODWorkMode::Standalone, ood_list, name, icon);

        PeopleBuilder::new(desc_content, body_content)
            .option_owner(owner)
            .option_area(area)
            .public_key(public_key)
    }

    pub fn ood_work_mode(&self) -> OODWorkMode {
        self.body().as_ref().unwrap().content().ood_work_mode()
    }

    pub fn set_ood_work_mode(&mut self, ood_work_mode: OODWorkMode) {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .set_ood_work_mode(ood_work_mode)
    }

    pub fn ood_list(&self) -> &Vec<DeviceId> {
        self.body().as_ref().unwrap().content().ood_list()
    }

    pub fn ood_list_mut(&mut self) -> &mut Vec<DeviceId> {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .ood_list_mut()
    }

    pub fn name(&self) -> Option<&str> {
        self.body().as_ref().unwrap().content().name()
    }
    pub fn icon(&self) -> Option<&FileId> {
        self.body().as_ref().unwrap().content().icon()
    }

    pub fn set_name(&mut self, name: String) {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .set_name(name)
    }
    pub fn set_icon(&mut self, icon: FileId) {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .set_icon(icon)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    // use std::str::FromStr;


    // 必须要跑的测试用以，用以测试新老编码的编码兼容性
    const OLD_LIST: &'static [&'static str] = &[
        //"0002520e002f3536f92b62c00000000000010030818902818100b81a66cc846de21354844cd3e3df991f45f4747c05b488027eacfb520ef3b3d4c2e58f2b75ad90bcf4a436a95ccd2cdd1b3bc0155fb92945524ec33c1ff263852288b44a75f430ebd9e3cdaffef59c075a3f16d43c3df30b1fdf167d696ec2f5ddbe0b1b69ab5397e1d46f2224cec8629dbd1e5e5fd9e569dde12077edf9c0c10203010001000000000000000000000000000000000000000000000000000000002f3536f92b62c0000140630a2044000000006c7e539b49ee3ad1cb1765b1afb344fecde8719416da84be391ad3123370656f706c652d3572344d59664637676355375a6a427a735079574748434e50734d3376514a7639324a506965516a576b6979220a7374616e64616c6f6e650100ff002f3536f92f2eac0061e7aac92e3ee196a9a1c278507da287d0ba872eff6de6abd7eeba3877d7c3e795c1cbe8ea524f58203c31f11a80e802afc5b00a5d5ceb9ce7d9fd2b6328dfeafd2bbfb9bb0217f378d4b04f5f230206573cf67a53b6298a62e2e86eda7d7769d93d64f550b85160d0f6c12b79a5b83aaf4a8c3a716a2b622bd059941f66dec70100ff002f3536f92f2fe900adb22dffd705deb589c0973a722f27d537cc2be48a365e90bc11007b4b6337dd402f550c9634043a2f25e723248aa1bfaa1b7880e6880789dc124ab801fd41b28551775ecb4b3bbdf6183a27c771a15a7459e1a809db89207cd1c9e66b9c80487dfef57ca5aa547063778bbf6ad9310df39e0de5dcfff6a11796c0e1f6723dd9",
        "0002500e0000000000010030818902818100e0252144cac6aa8493f252c1c7d288afd9d01f04430a24f19bbd1f0fec428278b149f3b748e26a532c7e238dcdde6fb60d3820727f53b7ae090ce1bb04f637d43aea4551043a06535ded73e6a7de845e6a6187cfcd4def56b841fd098afc0671f659bfbabd1fbceb268b6fa0f47b8c7e3cb698a2d6ba120e54b6df9064c889ed0203010001000000000000000000000000000000000000000000000000000000002f3b2f6e3acd900001390a2045c40d30000cd65e863aa69f59d818f2090e3fa3b3d646dadc87568f773da50c12096275636b7962616c6c220a7374616e64616c6f6e650100ff002f3b2f6e3ad56000c661eeddb115b2b1cc8f6a0abe871b83c3108379c766303457676e1beca4836220767e72cac8fa53b3addff7686a604ee5537b4593d7ef363dcfd827dace32a51fb46c527330d6cb1a2bf37a4fcc4d6bae9ed5acf0289c7f6fa3e957c4a11362bf237746a253edd574c59acf85705d36747dd83ac65acd0995c201e76be7db5f0100ff002f3b2f6e3e09b000bb6a8d783a3be84580323e7e70b7aeb0c277c60c09705218fe77eb5a527df5cfdfe738d05c0661c9e06f59c883d7b314d4709d56675cecdde81bbb72dc692c5413c9a39f81aaf7fd928319f7bd4183132ab3c383b6e39a924a87ba1608133cbd2a6bdfeb2e613752971d24c944ed666ed5d50c9a77177ab4077143c5354f90c2",
        "0002500e0000000000010030818902818100e0252144cac6aa8493f252c1c7d288afd9d01f04430a24f19bbd1f0fec428278b149f3b748e26a532c7e238dcdde6fb60d3820727f53b7ae090ce1bb04f637d43aea4551043a06535ded73e6a7de845e6a6187cfcd4def56b841fd098afc0671f659bfbabd1fbceb268b6fa0f47b8c7e3cb698a2d6ba120e54b6df9064c889ed0203010001000000000000000000000000000000000000000000000000000000002f3b2f6e3acd9000013f0a2045c40d30000cd65e863aa69f59d818f2090e3fa3b3d646dadc87568f773da50c120fe7bab3e696afe8b59be58d9ae4bcaf220a7374616e64616c6f6e650100ff002f3b2f6e3ad56000c661eeddb115b2b1cc8f6a0abe871b83c3108379c766303457676e1beca4836220767e72cac8fa53b3addff7686a604ee5537b4593d7ef363dcfd827dace32a51fb46c527330d6cb1a2bf37a4fcc4d6bae9ed5acf0289c7f6fa3e957c4a11362bf237746a253edd574c59acf85705d36747dd83ac65acd0995c201e76be7db5f0100ff002f3b2f6e3e09b000bb6a8d783a3be84580323e7e70b7aeb0c277c60c09705218fe77eb5a527df5cfdfe738d05c0661c9e06f59c883d7b314d4709d56675cecdde81bbb72dc692c5413c9a39f81aaf7fd928319f7bd4183132ab3c383b6e39a924a87ba1608133cbd2a6bdfeb2e613752971d24c944ed666ed5d50c9a77177ab4077143c5354f90c2",
    ];

    #[test]
    fn test_codec() {
        for code in OLD_LIST {
            let code = hex::decode(code).unwrap();
            let people = People::clone_from_slice(&code).unwrap();
    
            println!("ood list: {:?}", people.ood_list());
            println!("ood_work_mode: {}, name={:?}", people.ood_work_mode(), people.name());
            let hash = people.body().as_ref().unwrap().raw_hash_value().unwrap();
            println!("desc hash: {}", hash);

            let new_buf = people.to_vec().unwrap();
            assert_eq!(code.len(), new_buf.len());
 
            println!("code: {:?}", code);
            assert_eq!(code, new_buf);
        }
    }

    #[test]
    fn people() {
        let private_key = PrivateKey::generate_rsa(1024).unwrap();

        let pubic_key = private_key.public();

        let mut p = People::new(None, Vec::new(), pubic_key.clone(), None, None, None)
            .no_create_time()
            .build();

        let p2 = People::new(None, Vec::new(), pubic_key, None, None, None)
            .no_create_time()
            .build();

        assert!(p.desc().people_id() == p2.desc().people_id());

        p.set_name("people".to_owned());

        // let path = Path::new("f:\\temp\\people.obj");
        // if path.parent().unwrap().exists() {
        //     p.encode_to_file(path, false);
        // }

        let user_data = vec![0u8; 100];
        let _ = p.body_mut().as_mut().unwrap().set_userdata(&user_data);

        let buf = p.to_vec().unwrap();
        let pp = People::clone_from_slice(&buf).unwrap();

        assert_eq!(p.desc().people_id(), pp.desc().people_id());
        assert_eq!(p.name(), pp.name());
    }
}
