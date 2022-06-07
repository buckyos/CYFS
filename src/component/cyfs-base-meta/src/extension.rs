use cyfs_base::*;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct MetaExtensionTx {
    pub extension_id: MetaExtensionType,
    pub tx_data: Vec<u8>,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, RawEncode, RawDecode, Eq, PartialEq)]
pub enum MetaExtensionType {
    DSG,
}
