pub mod empty_content;
pub(crate) mod standard_objects;

pub use empty_content::*;
pub(crate) use standard_objects::*;


#[cfg(test)]
mod test {
    use crate::DeviceId;
    use crate::codec::RawConvertTo;
    use super::empty_content::*;
    use super::standard_objects::*;
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