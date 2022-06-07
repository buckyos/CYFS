
use cyfs_base::*;

#[derive(Clone, Debug)]
pub struct ChunkDownloadConfig {
    pub prefer_source: DeviceId, 
    pub force_stream: bool, 
    pub second_source: Option<DeviceId>, 
    pub more_source: Vec<DeviceId>, 
    pub referer: Option<String>
}

impl ChunkDownloadConfig {
    pub fn force_stream(source: DeviceId) -> Self {
        Self {
            prefer_source: source, 
            force_stream: true, 
            second_source: None, 
            more_source: vec![],
	        referer: None
        }
    }

    pub fn from(source: Vec<DeviceId>) -> Self {
        let prefer_source = source[0].clone();
        let second_source = if source.len() == 2 {
            let src = source[1].clone();
            if src.eq(&prefer_source) {
                None
            } else {
                Some(src)
            }
        } else {
            None
        };
        let more_source = if source.len() > 2 {
            Vec::from(&source[1..])
        } else {
            vec![]
        };
        let force_stream = {
            if second_source.is_some() || more_source.len() > 0 {
                false
            } else {
                true
            }
        };

        ChunkDownloadConfig {
            prefer_source,
            force_stream,
            second_source,
            more_source,
            referer: None,
        }
    }
}
