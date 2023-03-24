use crate::*;

use std::convert::TryFrom;

//分类：OOD、server、pc、路由器、android mobile、android pad、android watch、Android  TV
//    iOS mobile、iOS pad、iOS watch、
//    智能音箱
//    浏览器
//    IoT 传感器
//    智能家具设备
#[repr(u8)]
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum DeviceCategory {
    OOD = 0,
    Server = 1,
    PC = 2,
    Router = 3,
    AndroidMobile = 4,
    AndroidPad = 5,
    AndroidWatch = 6,
    AndroidTV = 7,
    IOSMobile = 8,
    IOSPad = 9,
    IOSWatch = 10,
    SmartSpeakers = 11,
    Browser = 12,
    IoT = 13,
    SmartHome = 14,
    VirtualOOD = 15,
    Unknown = 255,
}

impl TryFrom<u8> for DeviceCategory {
    type Error = BuckyError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0u8 => Ok(DeviceCategory::OOD),
            1u8 => Ok(DeviceCategory::Server),
            2u8 => Ok(DeviceCategory::PC),
            3u8 => Ok(DeviceCategory::Router),
            4u8 => Ok(DeviceCategory::AndroidMobile),
            5u8 => Ok(DeviceCategory::AndroidPad),
            6u8 => Ok(DeviceCategory::AndroidWatch),
            7u8 => Ok(DeviceCategory::AndroidTV),
            8u8 => Ok(DeviceCategory::IOSMobile),
            9u8 => Ok(DeviceCategory::IOSPad),
            10u8 => Ok(DeviceCategory::IOSWatch),
            11u8 => Ok(DeviceCategory::SmartSpeakers),
            12u8 => Ok(DeviceCategory::Browser),
            13u8 => Ok(DeviceCategory::IoT),
            14u8 => Ok(DeviceCategory::SmartHome),
            15u8 => Ok(DeviceCategory::VirtualOOD),
            v @ _ => {
                error!("unknown device category: {}", v);
                Err(BuckyError::from(BuckyErrorCode::InvalidFormat))
            }
        }
    }
}

impl std::fmt::Display for DeviceCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceCategory::OOD => write!(f, "OOD"),
            DeviceCategory::Server => write!(f, "Server"),
            DeviceCategory::PC => write!(f, "PC"),
            DeviceCategory::Router => write!(f, "Router"),
            DeviceCategory::AndroidMobile => write!(f, "AndroidMobile"),
            DeviceCategory::AndroidPad => write!(f, "AndroidPad"),
            DeviceCategory::AndroidWatch => write!(f, "AndroidWatch"),
            DeviceCategory::AndroidTV => write!(f, "AndroidTV"),
            DeviceCategory::IOSMobile => write!(f, "IOSMobile"),
            DeviceCategory::IOSPad => write!(f, "IOSPad"),
            DeviceCategory::IOSWatch => write!(f, "IOSWatch"),
            DeviceCategory::SmartSpeakers => write!(f, "SmartSpeakers"),
            DeviceCategory::Browser => write!(f, "Browser"),
            DeviceCategory::IoT => write!(f, "IoT"),
            DeviceCategory::SmartHome => write!(f, "SmartHome"),
            DeviceCategory::VirtualOOD => write!(f, "VirtualOOD"),
            DeviceCategory::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDescContent {
    unique_id: UniqueId,
}

impl DeviceDescContent {
    pub fn new(unique_id: UniqueId) -> Self {
        Self { unique_id }
    }

    pub fn unique_id(&self) -> &UniqueId {
        &self.unique_id
    }
}

impl DescContent for DeviceDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Device.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = Option<Area>;
    type AuthorType = SubDescNone;
    type PublicKeyType = PublicKey;
}

impl RawEncode for DeviceDescContent {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        let size = self.unique_id.raw_measure(purpose).map_err(|e| {
            log::error!("DeviceDescContent::raw_measure error:{}", e);
            e
        })?;
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                format!("[raw_encode] not enough buffer for DeviceDescContent, except {}, actual {}", size, buf.len()),
            ));
        }

        let buf = self.unique_id.raw_encode(buf, purpose).map_err(|e| {
            log::error!("DeviceDescContent::raw_encode error:{}", e);
            e
        })?;

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for DeviceDescContent {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (unique_id, buf) = UniqueId::raw_decode(buf).map_err(|e| {
            log::error!("DeviceDescContent::raw_decode error:{}", e);
            e
        })?;
        Ok((Self { unique_id }, buf))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceBodyContent {
    endpoints: Vec<Endpoint>,
    sn_list: Vec<DeviceId>,
    passive_pn_list: Vec<DeviceId>,
    name: Option<String>,
    bdt_version: Option<u8>,
}

// body使用protobuf编解码
impl BodyContent for DeviceBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl std::fmt::Display for DeviceBodyContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeviceBodyContent:{{name:{:?},endpoints:[", self.name()).map_err(|e| {
            log::error!("DeviceBodyContent::fmt error:{}", e);
            e
        })?;
        for ep in self.endpoints() {
            write!(f, "{},", ep).map_err(|e| {
                log::error!("DeviceBodyContent::fmt endpoint error:{}", e);
                e
            })?;
        }
        write!(f, "],sn_list:[").map_err(|e| {
            log::error!("DeviceBodyContent::fmt sn_list error:{}", e);
            e
        })?;
        for id in self.sn_list() {
            write!(f, "{},", id).map_err(|e| {
                log::error!("DeviceBodyContent::fmt sn_list error:{}", e);
                e
            })?;
        }
        write!(f, "]}}")
    }
}

impl Default for DeviceBodyContent {
    fn default() -> Self {
        Self {
            endpoints: Vec::new(),
            sn_list: Vec::new(),
            passive_pn_list: Vec::new(),
            name: None,
            bdt_version: None,
        }
    }
}

impl DeviceBodyContent {
    pub fn new(
        endpoints: Vec<Endpoint>,
        sn_list: Vec<DeviceId>,
        passive_pn_list: Vec<DeviceId>,
        name: Option<String>,
        bdt_version: Option<u8>,
    ) -> Self {
        Self {
            endpoints,
            sn_list,
            passive_pn_list,
            name,
            bdt_version,
        }
    }

    pub fn endpoints(&self) -> &Vec<Endpoint> {
        &self.endpoints
    }

    pub fn sn_list(&self) -> &Vec<DeviceId> {
        &self.sn_list
    }

    pub fn passive_pn_list(&self) -> &Vec<DeviceId> {
        &self.passive_pn_list
    }

    pub fn mut_endpoints(&mut self) -> &mut Vec<Endpoint> {
        &mut self.endpoints
    }

    pub fn mut_sn_list(&mut self) -> &mut Vec<DeviceId> {
        &mut self.sn_list
    }

    pub fn mut_passive_pn_list(&mut self) -> &mut Vec<DeviceId> {
        &mut self.passive_pn_list
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|f| f.as_str())
    }
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name
    }

    pub fn bdt_version(&self) -> Option<u8> {self.bdt_version}
    pub fn set_bdt_version(&mut self, bdt_version: Option<u8>) {self.bdt_version = bdt_version}
}

// protos编解码
impl TryFrom<&DeviceBodyContent> for protos::DeviceBodyContent {
    type Error = BuckyError;

    fn try_from(value: &DeviceBodyContent) -> BuckyResult<Self> {
        let mut ret = protos::DeviceBodyContent::new();

        ret.set_endpoints(ProtobufCodecHelper::encode_buf_list(&value.endpoints)?);
        ret.set_sn_list(ProtobufCodecHelper::encode_buf_list(&value.sn_list)?);
        ret.set_passive_pn_list(ProtobufCodecHelper::encode_buf_list(
            &value.passive_pn_list,
        )?);

        if let Some(name) = &value.name {
            ret.set_name(name.to_owned());
        }

        if let Some(bdt_version) = value.bdt_version {
            ret.set_bdt_version(bdt_version as u32);
        }

        Ok(ret)
    }
}

impl TryFrom<protos::DeviceBodyContent> for DeviceBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::DeviceBodyContent) -> BuckyResult<Self> {
        let mut ret = Self {
            endpoints: ProtobufCodecHelper::decode_buf_list(value.take_endpoints())?,
            sn_list: ProtobufCodecHelper::decode_buf_list(value.take_sn_list())?,
            passive_pn_list: ProtobufCodecHelper::decode_buf_list(value.take_passive_pn_list())?,
            name: None,
            bdt_version: None,
        };

        if value.has_name() {
            ret.name = Some(value.take_name());
        }

        if value.has_bdt_version() {
            ret.bdt_version = Some(value.get_bdt_version() as u8);
        }

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(DeviceBodyContent);

pub type DeviceType = NamedObjType<DeviceDescContent, DeviceBodyContent>;
pub type DeviceBuilder = NamedObjectBuilder<DeviceDescContent, DeviceBodyContent>;

pub type DeviceId = NamedObjectId<DeviceType>;
pub type DeviceDesc = NamedObjectDesc<DeviceDescContent>;
pub type Device = NamedObjectBase<DeviceType>;

impl DeviceDesc {
    pub fn device_id(&self) -> DeviceId {
        DeviceId::try_from(self.calculate_id()).unwrap()
    }

    pub fn unique_id(&self) -> &UniqueId {
        &self.content().unique_id
    }
}

impl Device {
    pub fn new(
        owner: Option<ObjectId>,
        unique_id: UniqueId,
        endpoints: Vec<Endpoint>,
        sn_list: Vec<DeviceId>,
        passive_pn_list: Vec<DeviceId>,
        public_key: PublicKey,
        area: Area,
        category: DeviceCategory,
    ) -> DeviceBuilder {
        let desc_content = DeviceDescContent::new(unique_id);

        let body_content = DeviceBodyContent::new(endpoints, sn_list, passive_pn_list, None, None);
        let mut real_area = area.clone();
        real_area.inner = category as u8;

        DeviceBuilder::new(desc_content, body_content)
            .public_key(public_key)
            .area(real_area)
            .option_owner(owner)
    }

    pub fn connect_info(&self) -> &DeviceBodyContent {
        self.body().as_ref().unwrap().content()
    }

    pub fn mut_connect_info(&mut self) -> &mut DeviceBodyContent {
        self.body_mut().as_mut().unwrap().content_mut()
    }

    pub fn name(&self) -> Option<&str> {
        self.body().as_ref().unwrap().content().name()
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .set_name(name)
    }

    pub fn bdt_version(&self) -> Option<u8> {
        self.body().as_ref().unwrap().content().bdt_version()
    }

    pub fn set_bdt_version(&mut self, bdt_version: Option<u8>) {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .set_bdt_version(bdt_version)
    }

    pub fn category(&self) -> BuckyResult<DeviceCategory> {
        match DeviceCategory::try_from(self.desc().area().as_ref().unwrap().inner) {
            Ok(category) => Ok(category),
            Err(_) => {
                Ok(DeviceCategory::Unknown)
            }
        }
    }

    pub fn has_wan_endpoint(&self) -> bool {
        match self.body() {
            Some(body) => {
                for ep in body.content().endpoints() {
                    if ep.is_mapped_wan() {
                        return true;
                    }
                }

                false
            }
            None => false,
        }
    }
}

impl RawMergable for Device {
    fn raw_merge_ok(&self, other: &Self) -> bool {
        self.desc().device_id() == other.desc().device_id()
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use std::str::FromStr;

    use std::convert::TryFrom;
    use hex::encode;
    //use std::path::Path;

    #[test]
    fn test_decode() {
        // let device = "0001580e4800000000857283dc484f7a184c158fa8e2deec97145ca5b8d0fd0bd6de4057e2000d5e02010030818902818100b96dad4eee3ff9ec6b666595e2c3767b2d1f67007147d48962f1ff545e476585ce7b38513d35c6d835f6ccc5b2728de64b569df33f5f0a11c906cf7db6cbcea68e36f0ceb1e485a085991c7a7aaab0f0deafc0b44035a9fec7041f5177ba3fd545f898b8149b287cef15cb7984047114a83245521f3d4947812a9bace47d1d350203010001000000000000000000000000000000000000000000000000001069f84401d28397116f58a580b99d437900002f3e1afce634ec0001408b0a070a721fc0a864100a070c721fc0a864100a1312721f000000000000000000000000000000000a1314721f00000000000000000000000000000000122044000000010f6bb5a1f53156de084c5f116f8c60aba978dbc5752e81cb2ce07f1a2044000000019ab0d16fabaece45f63e77f715e102c925d7db23ff68b572ad4ca52209302d52756e74696d65010000002f3e0f2dd0a37d002f7977a55ef776a8a892b8ab73da95acdb1352825024de84b39fa215ab5a46bf573513251acf4b27927978c75f51cbfc6d0f7914456bb48c382487eb99f1f056d9f72082cb0da823a9a3ebb445aa450be78b0121bcecec311b26ac03612130c4eb2d1b51e90b59c8da5081865456a0a803a2838fdcd43078b08c1db86ddd0f06010000002f3e0f2dd0a47d008ac5f0a2384ab0814b37d45ede8c0f37aea9d44160e274fb9f964b2afd6a29508547a3de179f8fac518d152006209d6702a2ae13e813ca9f67b3fc64cc21a4ffce76062c74af0a12bc54b26d884f321a9b2c2b91dc07db3b2a5dc46f6687d020f460d58b990ecd9dbadf020ca9fd91299b2dd0aebbb4e6913e49d64e6e721b2a";
        // let device = "0001580e4800000000857283dc484f7a184c158fa8e2deec97145ca5b8d0fd0bd6de4057e2000d5e02010030818902818100b96dad4eee3ff9ec6b666595e2c3767b2d1f67007147d48962f1ff545e476585ce7b38513d35c6d835f6ccc5b2728de64b569df33f5f0a11c906cf7db6cbcea68e36f0ceb1e485a085991c7a7aaab0f0deafc0b44035a9fec7041f5177ba3fd545f898b8149b287cef15cb7984047114a83245521f3d4947812a9bace47d1d350203010001000000000000000000000000000000000000000000000000001069f84401d28397116f58a580b99d437900002f3e1ad112fd9c000140690a070a721fc0a864100a1312721f000000000000000000000000000000000a070c721fc0a864100a1314721f00000000000000000000000000000000122044000000010f6bb5a1f53156de084c5f116f8c60aba978dbc5752e81cb2ce07f2209302d52756e74696d65010000002f3e0f2dd0a37d002f7977a55ef776a8a892b8ab73da95acdb1352825024de84b39fa215ab5a46bf573513251acf4b27927978c75f51cbfc6d0f7914456bb48c382487eb99f1f056d9f72082cb0da823a9a3ebb445aa450be78b0121bcecec311b26ac03612130c4eb2d1b51e90b59c8da5081865456a0a803a2838fdcd43078b08c1db86ddd0f06010000002f3e0f2dd0a47d008ac5f0a2384ab0814b37d45ede8c0f37aea9d44160e274fb9f964b2afd6a29508547a3de179f8fac518d152006209d6702a2ae13e813ca9f67b3fc64cc21a4ffce76062c74af0a12bc54b26d884f321a9b2c2b91dc07db3b2a5dc46f6687d020f460d58b990ecd9dbadf020ca9fd91299b2dd0aebbb4e6913e49d64e6e721b2a";
        let old_str = "00015a0e002f4943def944e94800000000e471f8f0e4069b9ec1f04a5e652a9bcb30432d50416468d744a0c700000000000100308189028181009cf42aa4b1c72607dca379fff9f57101521c3b6ef83400eca478083e27542c74a5a3ab4320e7a3e270977747e0e4a86b78304f103557fc8acdb9a5413e6b663fb52baa7b0b86c9513cb805dd776ea72fb6e2a22272363a4976429d20dc819f984a0d2f3ca41d3fa508dd90acad6bb711bb9537371b2e77f9e604873769ea50f3020301000100000000000000000000000000000000000000000000000000100000000000000000000000000000000000002f4943def944eb0001090a07090000000000000100fe002f4943defc32c600079f4ba67290707f9757e5f3ffa9c64c537fbd2b9cf3f13e27c0b7bd3000e93a399044736a0c8b0ce42e0bd783d937a5d53f7e5a92a9f78a2c1054e80a1e6a084d7ea05974cbb02ec3199eb04894b860645c56451b74652fd9e168c5a6e02a446d008f74985ce8efd51827744f7ed3a143c519d0e369b5176dabc44c53e7f22b0100fe002f4943defc59d20007d5c812c7814a6a921d6fdd480c7a07d7e3f5ffcf4f38921732c0c3d46c52173e193d0a7aa0ce28fc29c177dad7350f01587121309140809d8fca75e885eb44b8dc2cce62afd3a2579427182f1b3dc836e87710a17912e9457853e59b2ad1bea38105a8a84a4108de444bae097d7e52545b67dbbb7b31f59bdf653ee5374e69";
        {
            let device = "0001580e4800000000661456d9a5d0503f01cbca7dc66dea0d4de188f44a3751ee66d0230000000000010a027fee89e7e40d2f9544683e480d28575794f56013a49d81b119ce3b84ff29e761000000107e8397450134fdbf16e62a576a32e35900002f4b38abe31967000140680a070a1481c0a838010a070a1481c0a864ed0a070c1481c0a838010a070c1481c0a864ed12204400000001d707019d5593b7e33a6acf5c2beb475df3feffecee8acadb8f0b741a204400000001ecdeb526690e03f1feb00d51796cc7cca0eac8a1f06915780163360100fe002f4b38abc85cb0055c4f938cd5ee2f303909cb4556d5b8033c48e69db17308d01e886e823111002645f936ed188cb9848452b9fec239f5fc8394110421ff440007b51f3e676fa2f10100ff002f4b38abe3197405863ac70379ab5cddec296d5ca292a918815e59741b22bbc8e24765d6feb25e2940c15a24979fe71ba97ffa754c405449ba64cc8a79034b579703f2fd0054b0dd";

            let mut buf = vec![];
            let d = Device::clone_from_hex(&device, &mut buf).unwrap();
            println!("{}", d.desc().public_key().key_type_str());

            d.to_vec().unwrap();
            let id = d.desc().device_id();
            println!("{}", id);
        }

        let mut buf = vec![];
        let device = Device::clone_from_hex(old_str, &mut buf).unwrap();
        let new_device = device.to_vec().unwrap();
        let new_str = encode(new_device);
        for i in 0 .. old_str.len() {
            if old_str.as_bytes()[i] != new_str.as_bytes()[i] {
                println!("{}, {:#x}!={:#x}", i, old_str.as_bytes()[i], new_str.as_bytes()[i]);
            }
        }
        assert_eq!(old_str, new_str);
    }

    #[test]
    fn device_load_test() {
        let root = std::env::current_dir().unwrap();
        let p = root.join("../../../util/peers/sn-miner.desc");
        if p.exists() {
            let mut v = Vec::<u8>::new();
            let (device, _) = Device::decode_from_file(&p, &mut v).unwrap();
            println!("{:?}", device);

            let v = device
                .body_expect("sssss")
                .content()
                .sn_list
                .get(0)
                .unwrap()
                .object_id()
                .as_slice();
            println!("snlist 0 {:?}", v);

            let v = device.desc().public_key().to_vec().unwrap();
            println!("public key {:?}, len:{}", v, v.len());
        }
    }

    #[test]
    fn device() {
        let area = Area::new(0, 0, 0, 0);

        let private_key = PrivateKey::generate_rsa(1024).unwrap();

        let pubic_key = private_key.public();
        let device_public_key = pubic_key.clone();

        // {
        //     let size = pubic_key.raw_measure(purpose).unwrap();
        //     let mut encod_buf = vec![0u8;size];
        //     let buf = pubic_key.raw_encode(& mut encod_buf).unwrap();

        //     let (d,buf) = PublicKey::raw_decode(&encod_buf).unwrap();
        // }

        let endpoints = vec![Endpoint::default()];
        let sn_list = vec![];

        // {
        //     let size = endpoints.raw_measure(purpose).unwrap();
        //     let mut encod_buf = vec![0u8;size];
        //     let buf = endpoints.raw_encode(& mut encod_buf).unwrap();

        //     let (d,buf) = Vec::<Endpoint>::raw_decode(&encod_buf).unwrap();
        // }

        let sn_unique_id = UniqueId::default();
        let _btc_hash_value = Some(HashValue::default());
        let sn_1 = Device::new(
            Some(ObjectId::default()),
            sn_unique_id,
            endpoints,
            sn_list,
            Vec::new(),
            pubic_key,
            area,
            DeviceCategory::Server,
        )
        .build();

        let sn_1_deviceid = sn_1.desc().calculate_id();

        println!("an sn device, sn_1_deviceid:{}", sn_1_deviceid);

        let device_endpoints = vec![Endpoint::default()];
        let device_unique_id = UniqueId::default();
        // let device_sn_list = vec![DeviceId::try_from(sn_1_deviceid).unwrap()];
        let mut device_sn_list = vec![];
        for _i in 0..64 {
            device_sn_list.push(DeviceId::try_from(sn_1_deviceid).unwrap());
        }
        let _btc_hash_value_2 = Some(HashValue::default());

        let device_area = Area::new(0, 5, 0, 1);
        let mut device = Device::new(
            Some(ObjectId::default()),
            device_unique_id,
            device_endpoints,
            device_sn_list,
            Vec::new(),
            device_public_key,
            device_area,
            DeviceCategory::OOD,
        )
        .build();

        //let device_id = device.desc().calculate_id();
        //let body = device.body_mut().as_mut().unwrap();
        //let content = body.content_mut();

        let user_data = vec![0u8; 100];
        let _ = device.body_mut().as_mut().unwrap().set_userdata(&user_data);

        let device_clone = device.clone();
        // let device_clone2 = device.clone();
        println!("before={}", device_clone.desc().device_id());
        // let device_id2 = device_clone.desc().calculate_id();
        // assert_eq!(device_id,device_id2);

        // let p = Path::new("f:\\temp\\device.obj");
        // if p.parent().unwrap().exists() {
        //     device_clone2.encode_to_file(p, false);
        // }

        let size = device.raw_measure(&None).unwrap();
        let mut encod_buf = vec![0u8; size];
        let buf = device.raw_encode(&mut encod_buf, &None).unwrap();
        println!("encode buf rest:{}", buf.len());
        assert_eq!(buf.len(), 0);

        let _ = device.desc().owner();
        let _ = device.desc().area();

        // let (t,buf) = ObjectTypeInfo::raw_decode(& encod_buf).unwrap();
        // println!("a device with sn peer info, device t:{:?}, size:{}, encod_buf:{}, buf:{}", t, size, encod_buf.len(), buf.len());

        let (d, buf) = Device::raw_decode(&encod_buf).unwrap();
        println!(
            "a device with sn peer info, device d:{:?}, buf:{}",
            d,
            buf.len()
        );

        let _id = d.desc().device_id();
        let id_str = _id.to_string();
        println!("id to string:{}", id_str);
        let id_from_str = DeviceId::from_str(&id_str);
        println!("id from str:{:?}", id_from_str);

        println!("[object id] {:?}", _id);
        println!("[object id] {:?}", _id.to_string());

        println!("[object id] {:?}", _id.object_id());
        println!("[object id] {}", _id.object_id().to_string());

        // d.signs_mut().push_body_sign(Signature::default());

        println!("\n\n\n");

        let buf = d.to_vec().unwrap();
        // println!("d: {:?}", &buf[..10]);

        println!("test:{:?}", d.body_expect("xx").content().sn_list());

        let dd = Device::clone_from_slice(&buf).unwrap();
        let buf = dd.to_vec().unwrap();
        println!("dd: {:?}", &buf[..10]);

        let mut dc = dd.clone();
        dc.signs_mut().push_body_sign(Signature::default());

        let buf = dd.to_vec().unwrap();
        println!("dd: {:?}", &buf[..10]);

        let buf = dc.to_vec().unwrap();
        println!("dc: {:?}", &buf[..10]);

        // assert!(false);

        assert!(dc.signs().body_signs().is_some());
    }
}
