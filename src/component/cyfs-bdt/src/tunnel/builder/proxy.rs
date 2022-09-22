use log::*;
use std::{
    sync::RwLock, 
};
use async_std::{
    sync::Arc, 
    task,  
    future
};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
    interface::{udp::{Interface, PackageBoxEncodeContext}}, 
    history::keystore
};
use super::super::*;
use super::{
    action::{SynUdpTunnel}
};

struct SynProxyState {
    seq: TempSeq, 
    first_box: Arc<PackageBox>
}

enum State {
    Init, 
    SynProxy(SynProxyState), 
    SynTunnel, 
    Canceled(BuckyErrorCode)
}

struct SynProxyTunnelImpl {
    tunnel: TunnelContainer, 
    proxy: ProxyType, 
    local: Interface, 
    remote: Endpoint,  
    state: RwLock<State>
}

#[derive(Clone)]
struct SynProxyTunnel(Arc<SynProxyTunnelImpl>);

impl std::fmt::Display for SynProxyTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SynProxyTunnel{{tunnel:{}, proxy:{:?}}}", self.0.tunnel, self.0.proxy)
    }
}


impl SynProxyTunnel {
    fn new(
        tunnel: TunnelContainer, 
        proxy: ProxyType, 
        local: Interface, 
        remote: Endpoint) -> Self {
        Self(Arc::new(SynProxyTunnelImpl {
            tunnel, 
            proxy, 
            local, 
            remote, 
            state: RwLock::new(State::Init)
        }))
    }

    fn tunnel(&self) -> &TunnelContainer {
        &self.0.tunnel
    }

    fn proxy(&self) -> &ProxyType {
        &self.0.proxy
    }

    fn remote(&self) -> &Endpoint {
        &self.0.remote
    }

    fn local(&self) -> &Interface {
        &self.0.local
    }

    fn send_box_to_proxy(&self, pkg_box: &PackageBox, interface: &Interface) -> BuckyResult<usize> {
        let mut context = PackageBoxEncodeContext::default();
        interface.send_box_to(&mut context, pkg_box, self.remote())
    }

    async fn syn_proxy(
        &self, 
        seq: TempSeq, 
        first_box: Arc<PackageBox>, 
        proxy_desc: DeviceDesc, 
        remote_timestamp: Timestamp
    ) -> BuckyResult<()> {
        let _ =  {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                State::Init => {
                    *state = State::SynProxy(SynProxyState {
                        seq, 
                        first_box: first_box.clone()
                    });
                    info!("{} begin syn proxy with key {} seq {:?}", self, first_box.mix_key().mix_hash(None), seq);
                    Ok(())
                }, 
                _ => {
                    debug!("{} ignore syn proxy for tunnel's not init, seq={:?}", self, seq);
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's not init"))
                }
            }
        }?;
        
        let stack = self.tunnel().stack();
        let proxy_id = proxy_desc.device_id();
        let key_stub = stack.keystore().create_key_proxy(&proxy_desc, true);

        let syn_proxy = SynProxy {
            protocol_version: 0, 
            stack_version: 0, 
            seq,
            to_peer_id: self.tunnel().remote().clone(),
            to_peer_timestamp: remote_timestamp, 
            from_peer_info: stack.local().clone(), 
            key_hash: first_box.mix_key().mix_hash(None),
            mix_key: first_box.mix_key().clone(),
        };

        // 生成第一个package box
        let mut syn_box = PackageBox::encrypt_box(
            proxy_id.clone(), 
            key_stub.enc_key.clone(), 
            key_stub.mix_key.clone());

        trace!("syn_proxy enck={} mixk={}", key_stub.enc_key.to_hex().unwrap(), key_stub.mix_key.to_hex().unwrap());

        if let keystore::EncryptedKey::Unconfirmed(encrypted) = key_stub.encrypted {
            let mut exchg = Exchange::from((&syn_proxy, encrypted, key_stub.mix_key));
            let _ = exchg.sign(stack.keystore().signer()).await?;
            syn_box.push(exchg);
        }
        syn_box.push(syn_proxy);
        
        let action = self.clone();
        match future::timeout(self.tunnel().config().udp.connect_timeout, async move {
            loop {
                {
                    let state = &*action.0.state.read().unwrap();
                    if let State::SynProxy(_) = state {

                    } else {
                        debug!("{} stop send SynProxy with seq {:?} for not syn proxy", action, seq);
                        break;
                    }
                };
                debug!("{} send SynProxy with seq {:?}", action, seq);
                let _ = action.send_box_to_proxy(&syn_box, action.local());
                let _ = future::timeout(action.tunnel().config().udp.holepunch_interval, future::pending::<()>()).await.err();
            }
        }).await {
            Ok(_) => {
                // do nothing
            }, 
            Err(_) => {
                //FIXME: pn miner如果实现了keystore的持久化，这里的逻辑可能不是必须的
                //  加入pn miner因为异常退出了，keystore里面缓存的key肯能丢失，这里reset掉之前的key
                stack.keystore().reset_peer(&proxy_id);
            }
        }

        Ok(())
    }
}



impl OnPackage<AckProxy, &DeviceId> for SynProxyTunnel {
    fn on_package(&self, ack: &AckProxy, _proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        let (ep_pair, first_box) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                State::SynProxy(syn_proxy_state) => {
                    if syn_proxy_state.seq != ack.seq {
                        debug!("{} ignore AckProxy for expect seq={:?}, but got {:?}", self, syn_proxy_state.seq, ack.seq);
                        Err(BuckyError::new(BuckyErrorCode::InvalidInput, "sequence mismatch"))
                    } else if ack.proxy_endpoint.is_none() {
                        let err = ack.err.clone().unwrap_or(BuckyErrorCode::Failed);
                        debug!("{} SynProxy=>Canceled({})", self, err);
                        *state = State::Canceled(err);
                        Err(BuckyError::new(BuckyErrorCode::InvalidInput, "no proxy endpoint"))
                    } else {
                        let proxy_endpoint = ack.proxy_endpoint.as_ref().unwrap();
                        let first_box = syn_proxy_state.first_box.clone();
                        info!("{} SynProxy=>SynTunnel", self);
                
                        *state = State::SynTunnel;
                        Ok((EndpointPair::from((self.local().local(), *proxy_endpoint)), first_box))
                    }
                }, 
                _ => {
                    debug!("{} ignore AckProxy for tunnel's not syn proxy, seq={:?}", self, ack.seq);
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's not syn proxy"))
                }
            }
        }?;

        let udp_tunnel: udp::Tunnel = self.0.tunnel.create_tunnel(ep_pair, self.0.proxy.clone())?;
        
        let _ = SynUdpTunnel::new(
            udp_tunnel, 
            first_box, 
            self.tunnel().config().udp.holepunch_interval
        );

        Ok(OnPackageResult::Break)
    }
}


struct ProxyBuilderImpl {
    tunnel: TunnelContainer, 
    remote_timestamp: Timestamp, 
    local: Option<Interface>, 
    seq: TempSeq, 
    first_box: Arc<PackageBox>, 
    actions: RwLock<Vec<SynProxyTunnel>>, 
}

#[derive(Clone)]
pub struct ProxyBuilder(Arc<ProxyBuilderImpl>);

impl std::fmt::Display for ProxyBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProxyBuilder{{tunnel:{}}}", self.0.tunnel)
    }
}


impl ProxyBuilder {
    pub fn new(tunnel: TunnelContainer, remote_timestamp: Timestamp, first_box: Arc<PackageBox>) -> Self {
        let stack = tunnel.stack();
        let net_listener = stack.net_manager().listener();
        let local = net_listener.udp().iter().find(|i| i.local().addr().is_ipv4() && i.outer().is_some()).cloned();
        let syn_tunnel: &SynTunnel = first_box.packages_no_exchange()[0].as_ref();
        Self(Arc::new(ProxyBuilderImpl {
            tunnel, 
            remote_timestamp, 
            local, 
            seq: syn_tunnel.sequence, 
            first_box, 
            actions: RwLock::new(vec![])
        }))
    }

    pub async fn syn_proxy(&self, proxy: ProxyType) -> BuckyResult<()> {
        info!("{} syn proxy {:?}", self, proxy);
        if self.0.local.is_none() {
            let err = BuckyError::new(BuckyErrorCode::ErrorState, "no local interface");
            error!("{} syn proxy failed for {}", self, err);
            return Err(err);
        }
        let stack = self.0.tunnel.stack();
        let proxy_id = proxy.device_id().unwrap();
        let proxy_obj = stack.device_cache().get(proxy_id).await
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "proxy Device not found"))
            .map_err(|err| {
                error!("{} syn proxy {:?} failed for {}", self, proxy, err);
                err
            })?;
        let cmd_endpoint = proxy_obj.connect_info().endpoints().get(0).unwrap().clone();
        assert!(cmd_endpoint.is_udp());
        let action = {
            let action = SynProxyTunnel::new(self.0.tunnel.clone(), proxy.clone(), self.0.local.as_ref().unwrap().clone(), cmd_endpoint);
            let mut actions = self.0.actions.write().unwrap();
            if actions.iter().find(|a| a.proxy().device_id().unwrap().eq(proxy_id)).is_some() {
                let err = BuckyError::new(BuckyErrorCode::AlreadyExists, "action to proxy exists");
                debug!("{} syn proxy {:?} failed for {}", self, proxy, err);
                Err(err)
            } else {
                actions.push(action.clone());
                Ok(action)
            }
        }?;
        {
            let seq = self.0.seq;
            let first_box = self.0.first_box.clone();
            let proxy_desc = proxy_obj.desc().clone();
            let remote_timestamp = self.0.remote_timestamp;
            task::spawn(async move {
                let _ = action.syn_proxy(seq, first_box, proxy_desc, remote_timestamp).await;
            });
            
        }
        Ok(())
    }
}


impl OnPackage<AckProxy, &DeviceId> for ProxyBuilder {
    fn on_package(&self, ack: &AckProxy, proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> {
        if let Some(action) = self.0.actions.read().unwrap().iter().find(|a| a.proxy().device_id().unwrap().eq(proxy)).cloned() {
            action.on_package(ack, proxy)
        } else {
            let err = BuckyError::new(BuckyErrorCode::NotFound, "action not exists");
            debug!("{} ignore AckProxy for action {} not found", self, proxy);
            Err(err)
        }
    }
}






