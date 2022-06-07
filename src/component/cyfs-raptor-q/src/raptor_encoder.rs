
use crate::encoder::*;
use crate::base::*;
use rand::{Rng};

//
// Raptor编码:
// * 从编码piece做【LT解码】，得到中间编码raptor_pieces
// * 从raptor_pieces做【LT编码】获得分片编码数据
//
#[allow(non_snake_case)]
pub struct RaptorEncoder {
    encoder: Option<Encoder>,
    config: ObjectTransmissionInformation,
    packets: Vec<(Vec<EncodingPacket>, usize)>,
    // packets: Vec<EncodingPacket>,
    source_packets_count: usize,
    source_packets_per_block: usize,
}


#[allow(non_snake_case)]
impl RaptorEncoder {
    pub fn new(K:u32, piece_size:u16)->Result<Self,i32>{
        Ok(Self{
            encoder: None,
            config: ObjectTransmissionInformation::with_defaults(
                (piece_size as u64) * (K as u64),
                piece_size,
            ),
            packets: Vec::new(),
            source_packets_count: 0,
            source_packets_per_block: 0,
        })
    }

    pub fn precode(&mut self, data: Vec<u8>)->Result<(), i32>{
        self.encoder = Some(Encoder::new(
            &data,
            self.config
        ));

        let encoder = self.encoder.as_ref().unwrap();
        
        let mut i = 0usize;
        for encoder in encoder.get_block_encoders().iter() {
            let source_packets = encoder.source_packets();
            self.source_packets_count += source_packets.len();
            self.packets.push((source_packets, i));
            i+=1;
        }
        self.source_packets_per_block = self.source_packets_count / self.config.source_blocks() as usize;

        Ok(())
    }

    pub fn extend_piece_size() -> usize {
        4
    }

    pub fn encode_piece_size(&self) -> usize {
        self.config.symbol_size() as usize + 4
    }

    pub fn encode_raw(&self, seq: u32, buf: &mut [u8]) -> Result<usize, i32>{
        // 随机选一个source block FIXME，提高性能
        let mut rng = rand::thread_rng();
        let source_block_index = rng.gen_range(0,self.packets.len());

        // 取出预编码的source_packets信息
        let (_source_packets, source_block_encoder_index) = self.packets.get(source_block_index).unwrap();
        // let source_packet_count = source_packets.len();
        
        //if (seq as usize) < source_packet_count {
        //    // 使用source_packet
        //    //println!("use source_package seq:{}", source_block_index);
        //    let packet = &source_packets[seq as usize];
        //    let size = packet.serialize_with(buf);
        //    Ok(size)
        //}else{
            // 或者生成一个seq偏移的修复编码
            //println!("use fcc package seq:{}", source_block_index);
            let source_encoder = self.encoder.as_ref().unwrap().get_block_encoder_at(source_block_encoder_index.clone());
            let packet = &source_encoder.repair_packets(seq, 1)[0];
            let size = packet.serialize_with(buf);
            Ok(size)
        //}
    }

    // pub fn precode(&mut self, data: Vec<u8>)->Result<(), i32>{
    //     self.encoder = Some(Encoder::new(
    //         &data,
    //         self.config
    //     ));

    //     self.packets.extend(self.encoder.as_ref().unwrap().get_encoded_packets(42));

    //     Ok(())
    // }

    // pub fn extend_piece_size() -> usize {
    //     4
    // }

    // pub fn encode_piece_size(&self) -> usize {
    //     self.config.symbol_size() as usize + 4
    // }

    // pub fn encode_raw(&self, seq: u32, buf: &mut [u8]) -> Result<usize, i32>{
    //     let index = (seq as usize) % self.packets.len();
    //     let packet = self.packets.get(index).unwrap();
    //     let size = packet.serialize_with(buf);
    //     Ok(size)
    // }
}