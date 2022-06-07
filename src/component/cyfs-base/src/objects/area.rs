use crate::codec::{RawDecode, RawEncode, RawEncodePurpose, RawFixedBytes};
use crate::*;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
// use crate::{ObjectType, ObjectTypeCode, ObjectId};

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Area {
    pub country: u16,
    pub carrier: u8,
    pub city: u16,
    pub inner: u8, //对不同的对象来说有不同的意义，比如device这里就表示 device的设备类型。
                   //分类：OOD、server、pc、路由器、android mobile、android pad、android watch、Android  TV
                   //    iOS mobile、iOS pad、iOS watch、
                   //    智能音箱
                   //    浏览器
                   //    IoT 传感器
                   //    智能家具设备
}

impl Area {
    pub fn new(country: u16, carrier: u8, city: u16, inner: u8) -> Self {
        assert!(country <= 0x1FF);
        assert!(city <= 0x1FFF);
        assert!(carrier <= 0xF);
        Area {
            country,
            carrier,
            city,
            inner,
        }
    }
}

impl FromStr for Area {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let values = s.split(":");
        let mut array: Vec<u16> = vec![];
        for value in values {
            match value.parse::<u16>() {
                Ok(num) => array.push(num),
                Err(_) => {
                    return Err(BuckyError::new(
                        BuckyErrorCode::InvalidFormat,
                        "decode area err",
                    ))
                }
            }
        }
        if array.len() != 4 {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "decode area err",
            ));
        }
        if array[0] > 0x1FF || array[2] > 0x1FFF || array[1] > 0xF {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "decode area err",
            ));
        }

        Ok(Area {
            country: array[0],
            carrier: array[1] as u8,
            city: array[2],
            inner: array[3] as u8,
        })
    }
}

impl Display for Area {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.country, self.carrier, self.city, self.inner
        )
    }
}

impl Default for Area {
    fn default() -> Self {
        Area {
            country: 0,
            carrier: 0,
            city: 0,
            inner: 0,
        }
    }
}

impl RawFixedBytes for Area {
    fn raw_bytes() -> Option<usize> {
        Some(5)
    }
}

impl RawEncode for Area {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(5)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode Area, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        // (国家编码9bits)+(运营商编码4bits)+城市编码(13bits)+inner(8bits) = 34 bit
        // 此处直接用5个bytes
        buf[0] = self.country as u8;
        buf[1] = self.carrier;
        buf[2] = (self.city >> 8) as u8 | (self.country >> 8 << 7) as u8;
        buf[3] = (self.city << 8 >> 8) as u8;
        buf[4] = self.inner;

        Ok(&mut buf[5..])
    }
}

impl<'de> RawDecode<'de> for Area {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for decode Area, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let mut area_code = Self::default();
        area_code.country = buf[0] as u16 | (buf[2] as u16 >> 7 << 8);
        area_code.carrier = buf[1];
        area_code.city = ((buf[2] as u16) << 9 >> 1) | (buf[3] as u16);
        area_code.inner = buf[4];

        Ok((area_code, &buf[5..]))
    }
}

#[cfg(test)]
mod test_area {
    use crate::*;
    use std::str::FromStr;

    #[test]
    fn test() {
        let area = Area::new(333, 14, 3345, 32);
        let buf = area.to_vec().unwrap();
        let tmp_area = Area::clone_from_slice(buf.as_slice()).unwrap();

        assert_eq!(333, tmp_area.country);
        assert_eq!(14, tmp_area.carrier);
        assert_eq!(3345, tmp_area.city);
        assert_eq!(32, tmp_area.inner);
        assert_eq!(area.country, tmp_area.country);
        assert_eq!(area.carrier, tmp_area.carrier);
        assert_eq!(area.city, tmp_area.city);
        assert_eq!(area.inner, tmp_area.inner);

        let area = Area::new(33, 14, 3345, 32);
        let buf = area.to_vec().unwrap();
        let tmp_area = Area::clone_from_slice(buf.as_slice()).unwrap();

        assert_eq!(33, tmp_area.country);
        assert_eq!(14, tmp_area.carrier);
        assert_eq!(3345, tmp_area.city);
        assert_eq!(32, tmp_area.inner);
        assert_eq!(area.country, tmp_area.country);
        assert_eq!(area.carrier, tmp_area.carrier);
        assert_eq!(area.city, tmp_area.city);
        assert_eq!(area.inner, tmp_area.inner);
    }

    #[test]
    fn test_str() {
        let area = Area::from_str("333:14:3345:32").unwrap();
        let buf = area.to_vec().unwrap();
        let tmp_area = Area::clone_from_slice(buf.as_slice()).unwrap();

        assert_eq!(333, tmp_area.country);
        assert_eq!(14, tmp_area.carrier);
        assert_eq!(3345, tmp_area.city);
        assert_eq!(32, tmp_area.inner);
        assert_eq!(area.country, tmp_area.country);
        assert_eq!(area.carrier, tmp_area.carrier);
        assert_eq!(area.city, tmp_area.city);
        assert_eq!(area.inner, tmp_area.inner);
    }

    #[test]
    fn test_object_id() {
        let area = Area::from_str("334:15:3345:32").unwrap();
        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let people: People = People::new(
            None,
            Vec::new(),
            private_key.public(),
            Some(area.clone()),
            None,
            None,
        )
        .build();
        let people_id = people.desc().calculate_id();
        let people_data = people.to_vec().unwrap();
        let new_people = People::clone_from_slice(people_data.as_slice()).unwrap();
        let new_people_id = new_people.desc().calculate_id();
        assert_eq!(people_id, new_people_id);
        let id_info = people_id.info();
        if let ObjectIdInfo::Standard(obj) = id_info {
            let tmp_area = obj.area.unwrap();
            assert_eq!(334, tmp_area.country);
            assert_eq!(15, tmp_area.carrier);
            assert_eq!(3345, tmp_area.city);
            assert_eq!(32, tmp_area.inner);
            assert_eq!(area.country, tmp_area.country);
            assert_eq!(area.carrier, tmp_area.carrier);
            assert_eq!(area.city, tmp_area.city);
            assert_eq!(area.inner, tmp_area.inner);
        } else {
            assert!(false);
        }

        let area = Area::from_str("3333:10:3345:32");
        assert!(area.is_err());
        let area = Area::from_str("333:17:3345:32");
        assert!(area.is_err());
        let area = Area::from_str("333:15:8192:32");
        assert!(area.is_err());
        let area = Area::from_str("333:15:133345:256");
        assert!(area.is_err());
    }
}
