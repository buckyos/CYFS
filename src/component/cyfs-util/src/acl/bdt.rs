use cyfs_base::*;
use std::fmt;

pub struct BdtPutDataInputRequest {
    pub object_id: ObjectId,
    pub length: u64,
    pub source: DeviceId,
    pub referer: Option<String>,
}

impl fmt::Display for BdtPutDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {:?}", self.object_id)?;
        write!(f, ", length: {:?}", self.length)?;
        write!(f, ", source: {:?}", self.source)?;
        if let Some(referer) = &self.referer {
            write!(f, ", referer: {}", referer)?;
        }

        Ok(())
    }
}

pub struct BdtGetDataInputRequest {
    pub object_id: ObjectId,
    pub source: DeviceId,
    pub referer: Option<String>,
}

impl fmt::Display for BdtGetDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {:?}", self.object_id)?;
        write!(f, ", source: {:?}", self.source)?;
        if let Some(referer) = &self.referer {
            write!(f, ", referer: {}", referer)?;
        }

        Ok(())
    }
}

pub struct BdtDeleteDataInputRequest {
    pub object_id: ObjectId,
    pub source: DeviceId,
    pub referer: Option<String>,
}


impl fmt::Display for BdtDeleteDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {:?}", self.object_id)?;
        write!(f, ", source: {:?}", self.source)?;
        if let Some(referer) = &self.referer {
            write!(f, ", referer: {}", referer)?;
        }

        Ok(())
    }
}

