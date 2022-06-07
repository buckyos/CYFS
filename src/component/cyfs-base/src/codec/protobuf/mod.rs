pub mod protobuf_helper;
pub(crate) mod protos {
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}

pub use protobuf_helper::*;
pub use protos::EmptyContent;

#[cfg(test)]
mod test {
    use crate::DeviceId;
    use crate::codec::RawConvertTo;
    use super::protos::*;
    use protobuf::Message;
    use std::str::FromStr;

    #[test]
    fn test_codec() {
        let mut v = EmptyContentV1::new();
        v.set_name("纳斯赛博伯".into());


        let size = v.compute_size() as usize;
        let mut buf = Vec::with_capacity(size);
        buf.resize(size, 0);

        let mut stream = ::protobuf::CodedOutputStream::bytes(&mut buf);
        v.write_to(&mut stream).unwrap();

        println!("len={}, buf={:?}", size, buf);

        let mut v = PeopleBodyContent::new();
        let ood = DeviceId::from_str("5hLXAcNqgiGWe1AK3PyQoV1EEdXKGhs2trb9bCJpS4e7").unwrap();
        v.mut_ood_list().push(ood.to_vec().unwrap());
        v.set_name("纳斯赛博伯".into());
        v.set_ood_work_mode("standalone".to_owned());

        let size = v.compute_size() as usize;
        let mut buf = Vec::with_capacity(size);
        buf.resize(size, 0);

        let mut stream = ::protobuf::CodedOutputStream::bytes(&mut buf);
        v.write_to(&mut stream).unwrap();

        println!("len={}, buf={:?}", size, buf);
    }
}
