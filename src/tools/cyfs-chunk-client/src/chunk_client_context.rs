use http_types::{Url};
use cyfs_bdt::StreamGuard;
use cyfs_base::*;

pub enum DeviceTarget{
    Local,
    LocalProxy,
    Remote,
}

pub enum ChunkNodeType{
    Source,
    Cache,
}

pub struct ChunkSourceContext{
    pub bdt_stream: Option<StreamGuard>,
    pub peer_id: DeviceId, 
    pub end_point: String,
    pub vport: u16, 
}

pub struct ChunkCacheContext{
    pub bdt_stream: Option<StreamGuard>,
    pub peer_id: DeviceId, 
    pub end_point: String,
    pub vport: u16, 
}

fn get_vport(end_point:&str)->u16{
    let url = Url::parse(end_point.as_ref()).unwrap();
    url.port_or_known_default().unwrap_or(80)
}

pub trait ChunkClientContext {
    fn get_peer_id(&self)->&DeviceId;

    fn get_end_point(&self)->&str;

    fn get_bdt_stream(self)->Option<StreamGuard>;
}

impl ChunkClientContext for ChunkSourceContext {
    fn get_peer_id(&self)->&DeviceId{
        &self.peer_id
    }

    fn get_end_point(&self)->&str{
        &self.end_point
    }

    fn get_bdt_stream(self)->Option<StreamGuard>{
        self.bdt_stream
    }
}

impl ChunkClientContext for ChunkCacheContext {
    fn get_peer_id(&self)->&DeviceId{
        &self.peer_id
    }

    fn get_end_point(&self)->&str{
        &self.end_point
    }

    fn get_bdt_stream(self)->Option<StreamGuard>{
        self.bdt_stream
    }
}


impl ChunkSourceContext {
    pub fn source_http_local(peer_id:&DeviceId)->Self{

        let (end_point,vport) = Self::get_source_endpoint(&peer_id.to_string(), DeviceTarget::Local);

        ChunkSourceContext{
            bdt_stream: None,
            peer_id: peer_id.clone(),
            end_point,
            vport,
        }
    }

    pub fn source_http_local_proxy(peer_id:&DeviceId)->Self{

        let (end_point,vport) = Self::get_source_endpoint(&peer_id.to_string(), DeviceTarget::LocalProxy);

        ChunkSourceContext{
            bdt_stream: None,
            peer_id: peer_id.clone(),
            end_point,
            vport,
        }
    }

    pub fn source_http_remote(peer_id:&DeviceId)->Self{
        let (end_point,vport) = Self::get_source_endpoint(&peer_id.to_string(), DeviceTarget::Remote);

        ChunkSourceContext{
            bdt_stream: None,
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn source_http_bdt_local(peer_id:&DeviceId, bdt_stream: StreamGuard)->Self{
        
        let (end_point,vport) = Self::get_source_endpoint(&peer_id.to_string(), DeviceTarget::Local);

        ChunkSourceContext{
            bdt_stream: Some(bdt_stream),
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn source_http_bdt_local_proxy(peer_id:&DeviceId, bdt_stream: StreamGuard)->Self{
        
        let (end_point,vport) = Self::get_source_endpoint(&peer_id.to_string(), DeviceTarget::LocalProxy);

        ChunkSourceContext{
            bdt_stream: Some(bdt_stream),
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn source_http_bdt_remote(peer_id:&DeviceId, bdt_stream: StreamGuard)->Self{

        let (end_point,vport) = Self::get_source_endpoint(&peer_id.to_string(), DeviceTarget::Remote);

        ChunkSourceContext{
            bdt_stream: Some(bdt_stream),
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    fn get_source_endpoint(_peer_id:&str, target:DeviceTarget)->(String, u16){
        let end_point = match target {
            DeviceTarget::Local=>format!("http:127.0.0.1:{}", CHUNK_MANAGER_PORT),
            DeviceTarget::LocalProxy=>format!("http:127.0.0.1:{}", ACC_SERVICE_PORT),
            DeviceTarget::Remote=>format!("http://www.cyfs.com/chunk_manager"),
        };

        let vport = get_vport(&end_point);

        (end_point,vport)
    }
}

impl ChunkCacheContext {
    pub fn cache_http_local(peer_id:&DeviceId)->Self{
        let (end_point,vport) = Self::get_cache_endpoint(&peer_id.to_string(), DeviceTarget::Local);

        ChunkCacheContext{
            bdt_stream: None,
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn cache_http_local_proxy(peer_id:&DeviceId)->Self{
        let (end_point,vport) = Self::get_cache_endpoint(&peer_id.to_string(), DeviceTarget::LocalProxy);

        ChunkCacheContext{
            bdt_stream: None,
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn cache_http_remote(peer_id:&DeviceId)->Self{

        let (end_point,vport) = Self::get_cache_endpoint(&peer_id.to_string(), DeviceTarget::Remote);

        ChunkCacheContext{
            bdt_stream: None,
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn cache_http_bdt_local(peer_id:&DeviceId, bdt_stream: StreamGuard)->Self{

        let (end_point,vport) = Self::get_cache_endpoint(&peer_id.to_string(), DeviceTarget::Local);

        ChunkCacheContext{
            bdt_stream: Some(bdt_stream),
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn cache_http_bdt_local_proxy(peer_id:&DeviceId, bdt_stream: StreamGuard)->Self{

        let (end_point,vport) = Self::get_cache_endpoint(&peer_id.to_string(), DeviceTarget::LocalProxy);

        ChunkCacheContext{
            bdt_stream: Some(bdt_stream),
            peer_id: peer_id.clone(),
            end_point,
            vport
        }
    }

    pub fn cache_http_bdt_remote(peer_id:&DeviceId, bdt_stream: StreamGuard)->Self{

        let (end_point,vport) = Self::get_cache_endpoint(&peer_id.to_string(), DeviceTarget::Remote);

        ChunkCacheContext{
            bdt_stream: Some(bdt_stream),
            peer_id: peer_id.clone(),
            end_point: end_point,
            vport: vport
        }
    }

    fn get_cache_endpoint(_peer_id:&str, target:DeviceTarget)->(String, u16){
        let end_point = match target {
            DeviceTarget::Local=>format!("http:127.0.0.1:{}", CACHE_MINER_PORT),
            DeviceTarget::LocalProxy=>format!("http:127.0.0.1:{}", ACC_SERVICE_PORT),
            DeviceTarget::Remote=>format!("http://www.cyfs.com/cache_miner"),
        };

        let vport = get_vport(&end_point);

        (end_point,vport)
    }
}
