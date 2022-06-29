# PN支持

## PN Server
为了让理论上不可联通的设备可联通，或者为了更好的链路性能，BDT 可以选择通过使用Proxy Node代理来和其他设备联通。
从方向上，可以分成：
+ 主动代理：设备在和其他设备连接时，主动通过Proxy Node作为可选的链路；
+ 被动代理：设备在被其他设备连接时，告知其他设备可以使用Proxy Node作为可选的链路；

BDT代理的一些特征：
+ tcp/ip代理需求包括匿名性，使用代理后隐藏真实的ip地址，在BDT网络中隐藏真实的DeviceDesc的需求并不是Proxy范畴的讨论，因为DeviceDesc的构造成本远低于ip地址的构造成本，可以通过构造新的DeviceDesc来实现；并且BDT的全加密特性，Proxy Node也无法在不获取Deivce之间密钥的前提下，进行协议级别的重定向；
+ 代理通道（ProxyTunnel）和原生的Tcp/Udp通道（TcpTunnel/UdpTunnel)在机制上是可以并发的，可以不是链路上的唯一路径，TunnelContainer在通道的选择上可以自定义策略；
+ ProxyTunnel的特征更类似于UdpTunnel，基于BDT Package Box粒度的转发，类似tcp/ip代理和ip数据报的粒度；无论ProxyTunnel在实现上是使用tcp还是udp，都不能保证流式的特性，否则就需要实现stream粒度的代理（可选的需求？）；也就是说，如果只有ProxyTunnel联通的Device之间，并不能建立TcpStream，而是基于ProxyTunnel建立PackageStream；
+ 因为上述的原因，ProxyTunnel的实现应当是udp优先，基于tcp的ProxyTunnel实现也应当实现正确的有限缓存和丢包；
### ProxyNode的实现
ProxyNode的实现大体上是两部分：CommandTunnel和ProxyTunnel；
#### CommandTunnel
命令通道上会响应来自Device的SynProxyTunnel包，并且回复AckProxyTunnel包；ProxyNode的DeviceObject中的connect info中的endpoint list部分，就是ProxyNode命令通道的endpoint，可以同时有 udp 和 tcp 命令通道；
应当可以限制ProxyNode 只有 ipv4 udp 和 ipv4 tcp命令通道各一个；如果ipv6命令通道可连通证明device本来的ipv6联通性，那就不需要proxy；
#### ProxyTunnel
代理通道上透明转发来自LN和RN的package box；代理通道使用和命令通道不同的udp端口或者tcp socket实例；代理通道连通后，通过回复AckProxyTunnel到device来通知device代理通道的endpoint；
代理通道的生命周期可以设计为，当一段时间内任何一端没有流量后释放，不需要显示的在ProxyNode上实现对两边的ping逻辑，因为TunnelContainer中的保活逻辑会在Tunnel层产生ping;当TunnelCotainer释放ProxyTunnel时，不再ping或者一端失效后，ping丢失；

### 通过PN建立连接

#### 主动Proxy
1. LN 向SN发SnCall，LN的DeviceObject中不会包含LN的主动Proxy，而是在SnCall的pn_list字段中包含主动Proxy；
```
// protocol/package.rs
pub struct SnCall {
    pub seq: TempSeq,
    pub sn_peer_id: DeviceId,
    pub to_peer_id: DeviceId,
    pub from_peer_id: DeviceId,
    pub reverse_endpoint_array: Option<Vec<Endpoint>>,
    pub active_pn_list: Option<Vec<DeviceId>>,
    pub peer_info: Option<Device>,
    pub send_time: Timestamp,
    pub payload: SizedOwnedData<SizeU16>,
    pub is_always_call: bool,
}
```
2. RN 收到SnCalled包，其中pn_list字段包含 LN的主动ProxyNode；
```
pub struct SnCalled {
    pub seq: TempSeq,
    pub sn_peer_id: DeviceId,
    pub to_peer_id: DeviceId,
    pub from_peer_id: DeviceId,
    pub reverse_endpoint_array: Vec<Endpoint>,
    pub active_pn_list: Vec<DeviceId>,
    pub peer_info: Device,
    pub call_seq: TempSeq,
    pub call_send_time: Timestamp,
    pub payload: SizedOwnedData<SizeU16>,
}
```
3. LN 同时向ProxyNode发送SynProxy，seq和SnCall的相同；
```
//tunnel\builder\connect_tunnel\builder.rs
async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
	for proxy in stack.proxy_manager().active_proxies() {
		let _ = proxy_buidler.syn_proxy(ProxyType::Active(proxy)).await;
	}
}
//tunnel\builder\connect_stream\builder.rs
async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
	for proxy in stack.proxy_manager().active_proxies() {
		let _ = proxy_buidler.syn_proxy(ProxyType::Active(proxy)).await;
	}
}

```
4. RN 向ProxyNode发送SynProxy， seq和SnCalled的相同；
```
//tunnel\builder\connect_tunnel\builder.rs
async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
	for proxy in stack.proxy_manager().active_proxies() {
		let _ = proxy_buidler.syn_proxy(ProxyType::Active(proxy)).await;
	}
}
//tunnel\builder\connect_stream\builder.rs
async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
	for proxy in stack.proxy_manager().active_proxies() {
		let _ = proxy_buidler.syn_proxy(ProxyType::Active(proxy)).await;
	}
}
```
5. ProxyNode在收到两端的SynProxy后，创建代替通道，分别向两端返回AckProxy；
```
//pn\service\command.rs ：CommandTunnel::ack_proxy
    pub(super) fn ack_proxy(
        &self, 
        proxy_endpoint: BuckyResult<SocketAddr>,  
        syn_proxy: &SynProxy, 
        to: &SocketAddr, 
        key: &AesKey) -> BuckyResult<()>
``` 
6. LN/RN收到AckProxy后，在ProxyTunnel上发送原builder流程中的first package；
```
// tunnel\builder\connect_tunnel\builder.rs : SynProxyTunnel
impl OnPackage<AckProxy, &DeviceId> for SynProxyTunnel {
	 fn on_package(&self, ack: &AckProxy, _proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> 
}
```
7. 在ProxyTunnel收到builder流程中的resp package后，ProxyTunnel标记为可用状态，builder的策略决定是否使用ProxyTunnel作为default tunnel；
```
//tunnel\container.rs:TunnelContainer : create_tunnel
	pub fn create_tunnel<T: 'static + Tunnel + Clone>(
        &self, 
        ep_pair: EndpointPair, 
        proxy: ProxyType) -> Result<T, BuckyError>
```
#### 被动Proxy建立连接流程
	
1. LN 向SN发SnCall,RN 通过SnPing在SN上线缓存DeviceObject;
```
pub struct SnPing {
    //ln与sn的keepalive包
    pub seq: TempSeq,                          //序列号
    pub sn_peer_id: DeviceId,                  //sn的设备id
    pub from_peer_id: Option<DeviceId>,        //发送者设备id
    pub peer_info: Option<Device>,             //发送者设备信息
    pub send_time: Timestamp,                  //发送时间
    pub contract_id: Option<ObjectId>,         //合约文件对象id
    pub receipt: Option<ReceiptWithSignature>, //客户端提供的服务清单
}
```
2. LN 收到SN返回的SnCallResp其中RN的DeviceObject中包含RN的被动pn_list;
```
// cyfs-base/src/objects/device.rs::DeviceBodyContent
pub struct DeviceBodyContent {
    endpoints: Vec<Endpoint>,
    sn_list: Vec<DeviceId>,
    passive_pn_list: Vec<DeviceId>,
    name: Option<String>,
}
```
3. LN 向RN的被动ProxyNode发送SynProxy;
```
//tunnel\builder\connect_tunnel\builder.rs
async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
	for proxy in remote.connect_info().passive_pn_list().iter().cloned() {
		let _ = proxy_buidler.syn_proxy(ProxyType::Passive(proxy)).await;
	}
}
//tunnel\builder\connect_stream\builder.rs
async fn call_sn(&self, sn: Device, first_box: Arc<PackageBox>) -> Result<Vec<DynBuildTunnelAction>, BuckyError> {
	for proxy in remote.connect_info().passive_pn_list().iter().cloned() {
		let _ = proxy_buidler.syn_proxy(ProxyType::Passive(proxy)).await;
	}
}

```
4. RN 收到SnCalled包，之后向自己的被动ProxyNode发送SynProxy；
```
// tunnel\builder\accept_stream.rs : AcceptStreamBuilder::build
async fn build(&self, caller_box: PackageBox, active_pn_list: Vec<DeviceId>) -> Result<(), BuckyError> {
	for proxy in stack.proxy_manager().passive_proxies() {
		let _ = proxy_builder.syn_proxy(ProxyType::Passive(proxy)).await;
	}
}

// tunnel\builder\accept_tunnel.rs : AcceptTunnelBuilder::build
pub async fn build(&self, caller_box: PackageBox, active_pn_list: Vec<DeviceId>) -> Result<(), BuckyError> {
	for proxy in stack.proxy_manager().passive_proxies() {
		let _ = proxy_builder.syn_proxy(ProxyType::Passive(proxy)).await;
	}
}
```
5. ProxyNode在收到两端的SynProxy后，创建代替通道，分别向两端返回AckProxy；
```
//pn\service\command.rs ：CommandTunnel::ack_proxy
    pub(super) fn ack_proxy(
        &self, 
        proxy_endpoint: BuckyResult<SocketAddr>,  
        syn_proxy: &SynProxy, 
        to: &SocketAddr, 
        key: &AesKey) -> BuckyResult<()>
```
6. LN/RN收到AckProxy后，在ProxyTunnel上发送原builder流程中的first package；
```
// tunnel\builder\connect_tunnel\builder.rs : SynProxyTunnel
impl OnPackage<AckProxy, &DeviceId> for SynProxyTunnel {
	 fn on_package(&self, ack: &AckProxy, _proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> 
}
```
7. 在ProxyTunnel收到builder流程中的resp package后，ProxyTunnel标记为可用状态，builder的策略决定是否使用ProxyTunnel作为default tunnel；
```
//tunnel\container.rs:TunnelContainer : create_tunnel
	pub fn create_tunnel<T: 'static + Tunnel + Clone>(
        &self, 
        ep_pair: EndpointPair, 
        proxy: ProxyType) -> Result<T, BuckyError>
```
### PN Server 转发数据请求
ProxyTunnel的行为跟UdpTunnel基本一致，基于TunnelContainer的Session对象，和使用UdpTunnel一致的方式使用ProxyTunnel；LN和RN通过PN建立Tunnel后，会为设备分配一个ProxyInterface，转发接收到的datagram数据
+ UDP Socket接收datagram数据
```
// pn\service\proxy.rs : ProxyInterface::proxy_loop
fn proxy_loop(&self) {
        info!("{} started", self);
        loop {
            UDP_RECV_BUFFER.with(|thread_recv_buf| {
                let recv_buf = &mut thread_recv_buf.borrow_mut()[..];
                loop {
                    let rr = self.0.socket.recv_from(recv_buf);
                    if rr.is_ok() {
                        let (len, from) = rr.unwrap();
                        let recv = &recv_buf[..len];
                        trace!("{} recv datagram len {} from {:?}", self, len, from);
                        self.on_proxied_datagram(recv, &from);
                    } else {
                        let err = rr.err().unwrap();
                        if let Some(10054i32) = err.raw_os_error() {
                            trace!("{} socket recv failed for {}, ingore this error", self, err);
                        } else {
                            info!("{} socket recv failed for {}, break recv loop", self, err);
                            break;
                        }
                    }
                }
            });
        }
    }


```
+ UDP Socket转发datagram数据
```
// pn\service\proxy.rs : ProxyInterface::on_proxied_datagram
fn on_proxied_datagram(&self, datagram: &[u8], from: &SocketAddr) {
        let proxy_to = match KeyMixHash::raw_decode(datagram) {
            Ok((mut key, _)) => {
                key.as_mut()[0] &= 0x7f;
                if let Some(tunnel) = self.0.tunnels.lock().unwrap().get_mut(&key) {
                    trace!("{} recv datagram of key: {}", self, key);
                    tunnel.on_proxied_datagram(&key, from)
                } else {
                    trace!("{} ignore datagram of key: {}", self, key);
                    None
                }
            }, 
            _ => {
                trace!("{} ignore datagram for invalid key foramt", self);
                None
            }
        };
        if let Some(proxy_to) = proxy_to {
            let _ = self.0.socket.send_to(datagram, &proxy_to);
        }
    }
```

### udp实现
监听本机可外网访问的Udp Endpoint作为命令通道；
在命令通道上收到LN/RN两端seq相同的SynProxyTunnel后, 在可用的本地udp point 端中，bind新的udp socket作为代理通道, 回复AckProxyTunnel.endpoint=Some(代理通道的外网endpoint)到LN/RN;
LN/RN在本地的udp socket收到AckProxyTunnel之后建立ProxyTunnel,通过ProxyTunnel发送的package box都会发向分配的代理通道；
proxy在代理通道的udp socket上 recv from 到udp datagram（不需要解码）后，在相同的udp socket上发向对端的remote ep；

这里有些细节：
+ device应当只从确定的udp socket向proxy的发包，这里可以简单实现为从一个获知了外网地址的udp socket上（也就是跟device的sn ping通过的udp socket，如果有多个，选取ipv4中的确定一个）；
+ Proxy上的一条udp 代理通道只需要持有LN/RN两个不同的Endpoint，并且没法区分这两个Endpoint是归属于哪个Device；因为：代理通道上收发的包对ProxyNode是透明的，并且不可解码；命令通道上收到SynProxyTunnel时，可以确定命令通道的udp socket上看到的remote endpoint和DeviceId的映射关系，但是NAT可能导致device向proxy的代理通道发包时，被映射到另一个remote endpoint；能够同时和代理连通的两个外网endpoint应当不会相同，否则就ip冲突了；基于以上保证，代理通道的逻辑就能简单实现成从一个remote endpoint收到包就发向另一个remote endpoint；
+ NAT的实现也会导致代理通道在device向ProxyTunnel发出第一个package box之前，接收不倒ProxyTunnel转发过来的包；所以在ProxyTunnel上仍然要走打洞流程，保证两端的包可以通过代理通道互相触达；

### tcp实现(暂未实现)
监听本机可外网访问的Tcp Endpoint作为命令通道的Listener；
LN/RN中的两端向命令通道Listener发起tcp connect返回，在tcp socket上发送first package是SynProxyTunnel;
ProxyNode在命令通道的listener上 Accept到两条SynProxyTunnel中seq相同的个tcp socket后，创建代理通道持有这两个tcp socket，并且在上面回复first resp package是AckProxyTunnel；
device在tcp socket上收到first resp package之后，ProxyTunnel连通，之后就可以在ProxyTunnel上发送package box了；
ProxyNode在代理通道的tcp socket上收到package box后，转发到另一个tcp socket；
ProxyNode的tcp代理通道，Device的TcpProxyTunnel 上要正确的实现缓存和丢包策略；以应对丢包和重发；

### 服务证明（暂未实现）
Proxy服务证明
#### PN相关package字段定义
SynProxy包是从Device向Proxy发送的创建proxy tunnel的请求;AckProxy是Proxy向Device对SynProxy的回复；
```rust
// protocol\package.rs 
pub struct SynProxy {
    pub seq: TempSeq,
    pub to_peer_id: DeviceId,
    pub to_peer_timestamp: Timestamp,
    pub from_peer_id: DeviceId,
    pub from_peer_info: Device,
    pub key_hash: KeyMixHash,
}

pub struct AckProxy {
    pub seq: TempSeq,
    pub to_peer_id: DeviceId,
    pub proxy_endpoint: Option<Endpoint>,
    pub err: Option<BuckyErrorCode>,
}
pub struct Datagram {
    pub to_vport: u16,
    pub from_vport: u16,
    pub dest_zone: Option<u32>,
    pub hop_limit: Option<u8>,
    pub sequence: Option<TempSeq>,
    pub piece: Option<(u8, u8)>, // index/count
    pub send_time: Option<Timestamp>,
    pub create_time: Option<Timestamp>, 
    pub author_id: Option<DeviceId>,
    pub author: Option<Device>,
    // pub data_sign: Option<Signature>, 
    pub inner_type: DatagramType,
    pub data: TailedOwnedData, // TailedSharedData<'a>,
}
pub struct SessionData {
    pub stream_pos: u64,
    pub ack_stream_pos: u64,
    pub sack: Option<StreamRanges>,
    pub session_id: IncreaseId,
    pub send_time: Timestamp,
    pub syn_info: Option<SessionSynInfo>,
    pub to_session_id: Option<IncreaseId>,
    pub id_part: Option<SessionDataPackageIdPart>,
    pub payload: TailedOwnedData,
    pub flags: u16,
}
```