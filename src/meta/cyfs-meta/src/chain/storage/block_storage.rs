use std::path::{PathBuf};
use std::fs::{create_dir};
use cyfs_base::*;
use cyfs_base_meta::*;
use std::io::Read;

pub struct BlockStorage {
    dir: PathBuf
}


impl BlockStorage {
    pub fn new(dir: PathBuf) -> BuckyResult<Self> {
        if !dir.exists() {
            create_dir(dir.as_path()).unwrap();
        }

        Ok(BlockStorage {
            dir: dir
        })
    }

    pub fn has_block(&self, hash: &BlockHash) -> bool {
        let file_path = self.dir.join(hash.to_string());
        file_path.as_path().exists()
    }

    pub async fn load_block(&self, hash: &BlockHash) -> BuckyResult<Block> {
        let mut file_path = self.dir.join(hash.to_string());
        if !file_path.exists() {
            file_path = self.dir.join(hash.to_hex().unwrap());
        }

        let ret = match std::fs::File::open(file_path.as_path()) {
            Ok(mut file) => {
                let mut buf = Vec::<u8>::new();
                if let Err(e) = file.read_to_end(&mut buf) {
                    Err(BuckyError::from(e))
                } else {
                    Ok(buf)
                }
            },
            Err(e) => {
                Err(BuckyError::from(e))
            },
        }.map_err(|err| {
            log::error!("load_block file:{} err:{}", file_path.to_str().unwrap(), &err);
            err
        })?;

        let context = NamedObjectContext::clone_from_slice(ret.as_slice())?;
        if context.obj_type() == BlockDescContentV1::obj_type() {
            let block = BlockV1::clone_from_slice(ret.as_slice())
                .or_else(|e| {
                    log::error!("load_block file:{} err:{}", file_path.to_str().unwrap(), e);
                    Err(e)
                })?;
            let _block_body = block.body().as_ref().unwrap().content();
            let block_desc = block.desc();
            let mut new_receipts = Vec::new();
            for receipt in block.receipts() {
                new_receipts.push(receipt.into())
            }

            Ok(BlockBuilder::new(BlockDescContent::V1(block_desc.clone()), BlockBody::V1(block.clone(), new_receipts)).build())
        } else {
            let block = Block::clone_from_slice(ret.as_slice())
                .or_else(|e| {
                    log::error!("load_block file:{} err:{}", file_path.to_str().unwrap(), e);
                    Err(e)
                })?;
            Ok(block)
        }

    }

    pub fn save_block(&self, block: &Block) -> BuckyResult<()> {
        let file_path = self.dir.join(block.header().hash().to_string());
        block.encode_to_file(file_path.as_path(), false).or_else(|e| {
            log::error!("save_block file:{} err:{}", file_path.to_str().unwrap(), e);
            Err(e)
        })?;
        Ok(())
    }

    pub fn get_block_size(_hash: &BlockHash) -> BuckyResult<u64> {
        unimplemented!()
    }

    pub async fn get_tx_from_block(&self, hash: &BlockHash, index: i64) -> BuckyResult<(MetaTx, Receipt)> {
        let block = self.load_block(hash).await?;
        let tx_list = block.transactions();
        let tx_ret: Option<&MetaTx> = tx_list.get(index as usize);
        let receipt_list = block.receipts();
        let receipt_ret: Option<&Receipt> = receipt_list.get(index as usize);
        match tx_ret {
            Some(tx) => {
                match receipt_ret {
                    Some(receipt) => {
                        Ok((tx.clone(), receipt.clone()))
                    }
                    None => {
                        Err(BuckyError::new(BuckyErrorCode::NotFound, "NotFound"))
                    }
                }
            }
            None => {
                Err(BuckyError::new(BuckyErrorCode::NotFound, "NotFound"))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::chain::BlockStorage;
    use std::path::PathBuf;
    use cyfs_base::ObjectId;
    use std::str::FromStr;

    #[test]
    fn test() {
        async_std::task::block_on(async move {
            let storage = BlockStorage::new(PathBuf::from("C:\\Users\\wugren\\Desktop")).unwrap();
            storage.load_block(&ObjectId::from_str("9cfBkPsuNy5h9TqFJB9puyiP58B7ZV9mJs1YueZHKJr6").unwrap()).await.unwrap();
        })
    }
}
