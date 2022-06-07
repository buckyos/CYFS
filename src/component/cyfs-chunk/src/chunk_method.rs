pub mod method{
    pub const CREATE_CHUNK_DELEGATE: &'static str = "create_chunk_delegate";
    pub const QUERY_CHUNK_DELEGATE: &'static str = "query_chunk_delegate";
    pub const DELEGATE_CHUNK: &'static str = "delegate_chunk";
    pub const SET_CHUNK: &'static str = "set_chunk";
    pub const GET_CHUNK: &'static str = "get_chunk";
    pub const GET_CHUNK_CACHE: &'static str = "get_chunk_cache";
    pub const REDIRECT_CHUNK: &'static str = "redirect_chunk";
    pub const PROOF_CHUNK: &'static str = "proof_chunk";
    pub const REDEEM_CHUNK_PROOF: &'static str = "redeem_chunk_proof";
}

pub mod method_path{
    pub const CREATE_CHUNK_DELEGATE: &'static str = "/create_chunk_delegate";
    pub const QUERY_CHUNK_DELEGATE: &'static str = "/query_chunk_delegate";
    pub const DELEGATE_CHUNK: &'static str = "/delegate_chunk";
    pub const SET_CHUNK: &'static str = "/set_chunk";
    pub const GET_CHUNK: &'static str = "/get_chunk";
    pub const GET_CHUNK_CACHE: &'static str = "/get_chunk_cache";
    pub const REDIRECT_CHUNK: &'static str = "/redirect_chunk";
    pub const PROOF_CHUNK: &'static str = "/proof_chunk";
    pub const REDEEM_CHUNK_PROOF: &'static str = "/redeem_chunk_proof";
}