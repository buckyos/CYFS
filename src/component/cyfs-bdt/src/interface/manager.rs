use log::*;
use std::{collections::BTreeSet, iter::FromIterator, sync::RwLock};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{
    stack::WeakStack
};
use super::{udp, tcp};


#[derive(Clone)]
pub struct Config {
    pub udp: udp::Config
}



struct NetListenerImpl {
    local: DeviceId, 
    udp: Vec<udp::Interface>, 
    tcp: Vec<tcp::Listener>, 
    // ip_set: BTreeSet<IpAddr>, 
    // ep_set: BTreeSet<Endpoint>, 
}

impl std::fmt::Display for NetListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NetListener:{{local:{}}}", self.0.local)
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
pub enum UpdateOuterResult {
    None, 
    Update, 
    Reset
}

#[derive(Clone)]
pub struct NetListener(Arc<NetListenerImpl>);

impl NetListener {
    pub fn open(
        local: DeviceId, 
        config: &Config, 
        endpoints: &[Endpoint], 
        port_mapping: Option<Vec<(Endpoint, u16)>>
    ) -> BuckyResult<Self> {
        let ep_len = endpoints.len();
        if ep_len == 0 {
            let err = BuckyError::new(BuckyErrorCode::InvalidParam, "no endpoint");
            warn!("NetListener{{local:{}}} bind failed for {}", local, err);
            return Err(err);
        }

        let mut listener = NetListenerImpl {
            local: local.clone(), 
            udp: vec![], 
            tcp: vec![], 
            // ip_set: BTreeSet::new(), 
            // ep_set: BTreeSet::new(), 
        };
        let mut port_mapping = port_mapping.unwrap_or(vec![]);

        let mut ep_index = 0;

        while ep_index < ep_len {
            let ep = &endpoints[ep_index];
            let ep_pair = if ep.is_mapped_wan() {
                let local_index = ep_index + 1;
                let ep_pair = if local_index == ep_len {
                    Err(BuckyError::new(BuckyErrorCode::InvalidParam, format!("mapped wan endpoint {} has no local endpoint", ep)))
                } else {
                    let local_ep = &endpoints[local_index];
                    if !(local_ep.is_same_ip_version(ep) 
                        && local_ep.protocol() == ep.protocol()
                        && !local_ep.is_static_wan()) {
                        Err(BuckyError::new(BuckyErrorCode::InvalidParam, format!("mapped wan endpoint {} has invalid local endpoint {}", ep, local_ep)))
                    } else {
                        Ok((*local_ep, Some(*ep)))
                    }
                };
                ep_index = local_index;
                ep_pair
            } else {
                Ok((*ep, None))
            };
            ep_index += 1;

            if ep_pair.is_err() {
                let err = ep_pair.unwrap_err();
                warn!("NetListener{{local:{}}} bind on {:?} failed for {:?}", local, ep, err);
                continue;
            }

            let (local, out) = ep_pair.unwrap();
          
            let r = match ep.protocol() {
                Protocol::Udp => {
                    let mapping_port = {
                        let mut found_index = None;
                        for (index, (src_ep, _)) in port_mapping.iter().enumerate() {
                            if *src_ep == *ep {
                                found_index = Some(index);
                                break;
                            }
                        }
                        found_index.map(|index| {
                            let (_, dst_port) = port_mapping.remove(index);
                            dst_port
                        })
                    };
                    udp::Interface::bind(local, out, mapping_port, config.udp.clone()).map(|i| {
                        listener.udp.push(i);
                        ep
                    })
                },
                Protocol::Tcp => {
                    let mapping_port = {
                        let mut found_index = None;
                        for (index, (src_ep, _)) in port_mapping.iter().enumerate() {
                            if *src_ep == *ep {
                                found_index = Some(index);
                                break;
                            }
                        }
                        found_index.map(|index| {
                            let (_, dst_port) = port_mapping.remove(index);
                            dst_port
                        })
                    };
                    tcp::Listener::bind(local, out, mapping_port).map(|l| {
                        listener.tcp.push(l);
                        ep
                    })
                },
                Protocol::Unk => {
                    panic!()
                }
            };

            if let Err(e) = r.as_ref() {
                warn!("NetListener{{local:{}}} bind on {:?} failed for {:?}", local, ep, e);
            } else {
                info!("NetListener{{local:{}}} bind on {:?} success", local, ep);
                // listener.ep_set.insert(*ep);
                // if listener.ip_set.insert(ep.addr().ip()) {
                //     info!("NetListener{{local:{}}} add local ip {:?}", local, ep.addr().ip());
                // }
            }
        }
        Ok(Self(Arc::new(listener)))
    }

    pub fn reset(&self, endpoints: Option<&[Endpoint]>) -> Self {
        if let Some(endpoints) = endpoints {
            let mut all_default = true;
            for ep in endpoints {
                if !ep.is_sys_default() {
                    all_default = false;
                    break;
                }
            }
            //TODO: 支持显式绑定本地ip的 reset
            if !all_default {
                error!("{} reset should be endpoint with default flag", self);
                return self.clone();
            }
        }
       

        fn local_of(former: Endpoint, endpoints: &Option<&[Endpoint]>) -> Option<Endpoint> {
            if let Some(endpoints) = endpoints {
                for ep in *endpoints {
                    if former.is_same_ip_version(ep) 
                        && former.protocol() == ep.protocol() 
                        && former.addr().port() == ep.addr().port() {
                        return Some(*ep);
                    }
                }
            } 
            None
        }

        // let mut ip_set = BTreeSet::new(); 
        // let mut ep_set = BTreeSet::new(); 
        let udp = Vec::from_iter(self.0.udp.iter().map(|udp| {
            if let Some(new_ep) = local_of(udp.local(), &endpoints) {
                // ep_set.insert(new_ep);
                // ip_set.insert(new_ep.addr().ip());
                udp.reset(&new_ep)
            } else {
                // ep_set.insert(new_ep);
                // ip_set.insert(new_ep.addr().ip());
                udp.clone()
            }
        }));

        let tcp = Vec::from_iter(self.0.tcp.iter().map(|tcp| {
            if let Some(new_ep) = local_of(tcp.local(), &endpoints) {
                tcp.reset(&new_ep)
            } else {
                tcp.clone()
            }
            // ep_set.insert(new_ep);
            // ip_set.insert(new_ep.addr().ip());
            
        })); 

        NetListener(Arc::new(NetListenerImpl {
            local: self.0.local.clone(), 
            udp, 
            tcp, 
            // ip_set, 
            // ep_set, 
        }))


    }

    pub fn start(&self, stack: WeakStack) {
        for i in self.udp() {
            i.start(stack.clone());
        } 
        for l in &self.0.tcp {
            l.start(stack.clone());
        }
    }


    pub fn close(&self) {
        for _i in self.udp() {

        }

        for _l in self.tcp() {
            
        }
    }

    pub fn update_outer(&self, ep: &Endpoint, outer: &Endpoint) -> UpdateOuterResult {
        let outer = *outer;
        let mut reseult = UpdateOuterResult::None;
        if let Some(interface) = self.udp_of(ep) {
            let udp_result = interface.update_outer(&outer);
            if udp_result > reseult {
                reseult = udp_result;
            }
            if udp_result > UpdateOuterResult::None {
                if ep.addr().is_ipv6() {
                    for listener in self.tcp() {
                        if listener.local().addr().is_ipv6() {
                            let mut tcp_outer = outer;
                            tcp_outer.set_protocol(Protocol::Tcp);
                            tcp_outer.mut_addr().set_port(listener.local().addr().port());
                            listener.update_outer(&tcp_outer);
                        }
                    }
                } else {
                    for listener in self.tcp() {
                        if let Some(mapping_port) = listener.mapping_port() {
                            if listener.local().is_same_ip_addr(ep) {
                                let mut tcp_outer = outer;
                                tcp_outer.set_protocol(Protocol::Tcp);
                                tcp_outer.mut_addr().set_port(mapping_port);
                                listener.update_outer(&tcp_outer);
                            }
                        }
                    }
                }
            }
        }
        reseult
    }

    pub fn endpoints(&self) -> BTreeSet<Endpoint> {
        let mut ep_set = BTreeSet::new();
        for udp in self.udp() {
            if udp.local().addr().is_ipv4() {
                ep_set.insert(udp.local());
            }
            let outer = udp.outer();
            if outer.is_some() {
                ep_set.insert(outer.unwrap());
            }
        }
        for tcp in self.tcp() {
            if tcp.local().addr().is_ipv4() {
                ep_set.insert(tcp.local());
            }
            let outer = tcp.outer();
            if outer.is_some() {
                ep_set.insert(outer.unwrap());
            }
        }
        ep_set
    }

    
    pub fn udp_of(&self, ep: &Endpoint) -> Option<&udp::Interface> {
        for i in &self.0.udp {
            if i.local() == *ep {
                return Some(i);
            }
        }
        None
    }

    pub fn udp(&self) -> &Vec<udp::Interface> {
        &self.0.udp
    }

    pub fn tcp_of(&self, ep: &Endpoint) -> Option<&tcp::Listener> {
        for i in &self.0.tcp {
            if i.local() == *ep {
                return Some(i);
            }
        }
        None
    }

    pub fn tcp(&self) -> &Vec<tcp::Listener> {  
        &self.0.tcp
    }

    // pub fn ep_set(&self) -> &BTreeSet<Endpoint> {
    //     &self.0.ep_set
    // }

    // pub fn ip_set(&self) -> &BTreeSet<IpAddr> {
    //     &self.0.ip_set
    // }
}


pub struct NetManager {
    listener: RwLock<NetListener>
}

impl NetManager {
    pub fn open(
        local: DeviceId, 
        config: &Config, 
        endpoints: &[Endpoint], 
        tcp_port_mapping: Option<Vec<(Endpoint, u16)>>) -> Result<Self, BuckyError> {
        Ok(Self {
            listener: RwLock::new(NetListener::open(local, config, endpoints, tcp_port_mapping)?)
        })
    }

    pub fn reset(&self, endpoints: &[Endpoint]) -> NetListener {
        let listener = self.listener().reset(Some(endpoints));
        *self.listener.write().unwrap() = listener.clone();
        listener
    } 

    pub fn listener(&self) -> NetListener {
        self.listener.read().unwrap().clone()
    }
}