use crate::{EncodingPacket, decoder::*};

//
// Raptor解码的过程
// * 从编码piece做【LT解码】，得到中间编码raptor_pieces
// * 从raptor_pieces做【LT编码】获得原始数据
//
#[allow(non_snake_case)]
pub struct RaptorDecoder {
    decoder: Decoder,
    K: u32,
    piece_size: u16,
}

#[allow(non_snake_case)]
impl RaptorDecoder {
    pub fn new(K:u32, piece_size:u16)->Result<Self,i32>{
        Ok(Self{
            decoder: Decoder::with_defaults(K as u64 * piece_size as u64, piece_size),
            K,
            piece_size,
        })
    }

    pub fn piece_size(&self) -> usize {
        self.piece_size as usize
    }

    // 使用收到的 (seq, Piece ) 解码，如果解码成功则返回Ok(true)
    pub fn decode_raw(&mut self, _seq: u32, piece: Vec<u8>) -> Result<DecodeStatus,i32>{
        let packet = EncodingPacket::deserialize(piece);
        let status = self.decoder.decode(packet);
        Ok(status)
    }

    pub fn retrieve_piece(&self, index: usize, buffer:&mut [u8]) -> Result<bool,i32>{
        self.decoder.retrieve_piece(index, buffer)
    }
}