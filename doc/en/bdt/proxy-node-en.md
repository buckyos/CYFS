# PN support

## PN Server
In order to allow devices that cannot be connected in theory to be connected, or for better link performance, BDT can choose to use Proxy Node to communicate with other devices.
From the direction, it can be divided into:
+ Active proxy: When the device is connected to other devices, it actively uses the Proxy Node as an optional link;
+ Passive proxy: When the device is connected by other devices, it informs other devices that Proxy Node can be used as an optional link;

Some features of BDT proxy:
+ tcp/ip proxy requirements include anonymity, hiding the real ip address after using the proxy, and the need to hide the real DeviceDesc in the BDT network is not a discussion in the category of Proxy, because the construction cost of DeviceDesc is much lower than the construction cost of ip address, It can be achieved by constructing a new DeviceDesc; and with the full encryption feature of BDT, Proxy Node cannot perform protocol-level redirection without obtaining the key between Deivce;
+ The proxy channel (ProxyTunnel) and the native Tcp/Udp channel (TcpTunnel/UdpTunnel) can be concurrent in mechanism, and may not be the only path on the link. TunnelContainer can customize the strategy for the selection of channels;
+ The characteristics of ProxyTunnel are more similar to UdpTunnel, based on BDT Package Box granularity forwarding, similar to the granularity of tcp/ip proxy and ip datagram; whether ProxyTunnel uses tcp or udp in implementation, it cannot guarantee the streaming characteristics, otherwise it will It is necessary to implement a proxy with stream granularity (optional requirement?); that is, if there are only devices connected by ProxyTunnel, TcpStream cannot be established, but PackageStream is established based on ProxyTunnel;
+ For the above reasons, the implementation of ProxyTunnel should be udp first, and the implementation of ProxyTunnel based on tcp should also implement correct limited caching and packet loss;

### ProxyNode implementation
The implementation of ProxyNode is roughly divided into two parts: CommandTunnel and ProxyTunnel;

#### CommandTunnel
The command channel will respond to the SynProxyTunnel packet from the Device and reply to the AckProxyTunnel packet; the endpoint list part of the connect info in the DeviceObject of the ProxyNode is the endpoint of the ProxyNode command channel, which can have both udp and tcp command channels;
It should be possible to limit the ProxyNode to only one ipv4 udp and one ipv4 tcp command channel; if the ipv6 command channel can be connected to prove the original ipv6 connectivity of the device, no proxy is required;
#### ProxyTunnel
The package box from LN and RN is transparently forwarded on the proxy channel; the proxy channel uses a different udp port or tcp socket instance from the command channel; after the proxy channel is connected, it notifies the device of the endpoint of the proxy channel by replying to AckProxyTunnel to the device;
The life cycle of the proxy channel can be designed to be released when there is no traffic at either end for a period of time, and there is no need to implement the ping logic on both sides on the ProxyNode, because the keep-alive logic in the TunnelContainer will generate a ping at the Tunnel layer; when the TunnelCotainer When the ProxyTunnel is released, the ping is no longer available or the ping is lost after one end fails;

### Establish connection via PN

#### Active Proxy
1. LN sends SnCall to SN, the DeviceObject of LN will not contain the active proxy of LN, but the active proxy will be included in the pn_list field of SnCall;
``` rust
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
2. The RN receives the SnCalled packet, where the pn_list field contains the active ProxyNode of the LN;
``` rust
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
3. LN sends SynProxy to ProxyNode at the same time, seq and SnCall are the same;
``` rust
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
4. RN sends SynProxy to ProxyNode, seq is the same as SnCalled;
``` rust
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
5. After the ProxyNode receives the SynProxy from both ends, it creates an alternate channel and returns AckProxy to both ends respectively;
``` rust
//pn\service\command.rs ：CommandTunnel::ack_proxy
    pub(super) fn ack_proxy(
        &self, 
        proxy_endpoint: BuckyResult<SocketAddr>,  
        syn_proxy: &SynProxy, 
        to: &SocketAddr, 
        key: &AesKey) -> BuckyResult<()>
``` 
6. After LN/RN receive LN/RN's AckProxy, it sends the first package in the original builder process on ProxyTunnel;
```
// tunnel\builder\connect_tunnel\builder.rs : SynProxyTunnel
impl OnPackage<AckProxy, &DeviceId> for SynProxyTunnel {
	 fn on_package(&self, ack: &AckProxy, _proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> 
}
```
7. After ProxyTunnel receives the resp package in the builder process, ProxyTunnel is marked as available, and the builder's policy decides whether to use ProxyTunnel as the default tunnel;
``` rust
//tunnel\container.rs:TunnelContainer : create_tunnel
	pub fn create_tunnel<T: 'static + Tunnel + Clone>(
        &self, 
        ep_pair: EndpointPair, 
        proxy: ProxyType) -> Result<T, BuckyError>
```

#### Passive Proxy 
	
1. LN sends SnCall to SN, RN caches DeviceObject on SN through SnPing;
``` rust
pub struct SnPing {
     //The keepalive package of ln and sn
     pub seq: TempSeq, //sequence number
     pub sn_peer_id: DeviceId, //sn's device id
     pub from_peer_id: Option<DeviceId>, //sender device id
     pub peer_info: Option<Device>, //sender device information
     pub send_time: Timestamp, //send time
     pub contract_id: Option<ObjectId>, // contract file object id
     pub receipt: Option<ReceiptWithSignature>, //List of services provided by the client
}
```
2. The LN receives the SnCallResp returned by the SN, where the DeviceObject of the RN contains the passive pn_list of the RN;
``` rust
// cyfs-base/src/objects/device.rs::DeviceBodyContent
pub struct DeviceBodyContent {
    endpoints: Vec<Endpoint>,
    sn_list: Vec<DeviceId>,
    passive_pn_list: Vec<DeviceId>,
    name: Option<String>,
}
```
3. LN sends SynProxy to passive ProxyNode of RN;
``` rust
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
4. RN receives the SnCalled packet, and then sends SynProxy to its passive ProxyNode;
``` rust
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
5. After the ProxyNode receives the SynProxy from both ends, it creates an alternate channel and returns AckProxy to both ends respectively;
``` rust
//pn\service\command.rs ：CommandTunnel::ack_proxy
    pub(super) fn ack_proxy(
        &self, 
        proxy_endpoint: BuckyResult<SocketAddr>,  
        syn_proxy: &SynProxy, 
        to: &SocketAddr, 
        key: &AesKey) -> BuckyResult<()>
```
6. After LN/RN receives AckProxy, it sends the first package in the original builder process on ProxyTunnel;
``` rust
// tunnel\builder\connect_tunnel\builder.rs : SynProxyTunnel
impl OnPackage<AckProxy, &DeviceId> for SynProxyTunnel {
	 fn on_package(&self, ack: &AckProxy, _proxy: &DeviceId) -> Result<OnPackageResult, BuckyError> 
}
```
7. After ProxyTunnel receives the resp package in the builder process, ProxyTunnel is marked as available, and the builder's policy decides whether to use ProxyTunnel as the default tunnel;
``` rust
//tunnel\container.rs:TunnelContainer : create_tunnel
	pub fn create_tunnel<T: 'static + Tunnel + Clone>(
        &self, 
        ep_pair: EndpointPair, 
        proxy: ProxyType) -> Result<T, BuckyError>
```
### PN Server forwards datagram
The behavior of ProxyTunnel is basically the same as that of UdpTunnel. Based on the Session object of TunnelContainer, ProxyTunnel is used in the same way as UdpTunnel. After LN and RN establish a tunnel through PN, a ProxyInterface will be allocated to the device to forward the received datagram data.
+ UDP Socket to receive datagram data
``` rust
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
+ UDP Socket forwards datagram data
``` rust
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

### UDP implementation
Monitor the Udp Endpoint accessible from the external network as the command channel;
After receiving the SynProxyTunnel with the same seq at both ends of the LN/RN on the command channel, in the available local udp point, bind the new udp socket as the proxy channel, and reply AckProxyTunnel.endpoint=Some (the external network endpoint of the proxy channel) to LN/RN;
LN/RN establishes ProxyTunnel after receiving AckProxyTunnel on the local udp socket, and the package box sent through ProxyTunnel will be sent to the assigned proxy channel;
After the proxy recv from to the udp datagram on the udp socket of the proxy channel (without decoding), it sends the remote ep to the opposite end on the same udp socket;

Here are some details:
+ The device should only send packets to the proxy from a certain udp socket. This can be simply implemented as a udp socket that knows the external network address (that is, the udp socket that passes the sn ping of the device. If there are multiple, select ipv4 determine one of the);
+ A udp proxy channel on the Proxy only needs to hold two different Endpoints, LN/RN, and it is impossible to distinguish which Device these two Endpoints belong to; because: the packets sent and received on the proxy channel are transparent to the ProxyNode, and Undecodable; when SynProxyTunnel is received on the command channel, the mapping relationship between the remote endpoint and DeviceId seen on the udp socket of the command channel can be determined, but NAT may cause the device to send packets to the proxy channel and be mapped to another remote endpoint. ; The two external network endpoints that can be connected to the proxy at the same time should not be the same, otherwise there will be IP conflicts; based on the above guarantees, the logic of the proxy channel can be simply implemented to receive packets from one remote endpoint and send it to another remote endpoint. ;
+ The implementation of NAT will also cause the proxy channel to receive the packets forwarded by ProxyTunnel before the device sends the first package box to ProxyTunnel; therefore, the ProxyTunnel still needs to go through the hole punching process to ensure that the packets at both ends can communicate with each other through the proxy channel. touch;

### TCP implementation (not yet implemented)
Listen to the Tcp Endpoint accessible from the external network as the Listener of the command channel;
Both ends of the LN/RN initiate a tcp connect return to the command channel Listener, and the first package sent on the tcp socket is SynProxyTunnel;
After the ProxyNode accepts two tcp sockets with the same seq in the two SynProxyTunnels on the listener of the command channel, it creates a proxy channel to hold the two tcp sockets, and replies that the first resp package is AckProxyTunnel;
After the device receives the first resp package on the tcp socket, the ProxyTunnel is connected, and then the package box can be sent on the ProxyTunnel;
After the ProxyNode receives the package box on the tcp socket of the proxy channel, it forwards it to another tcp socket;
ProxyNode's tcp proxy channel and Device's TcpProxyTunnel should correctly implement caching and packet loss policies to deal with packet loss and retransmission;

### Service Proof (not yet implemented)
BDT Proxy service proof

#### PN related package field definition
The SynProxy packet is a request from Device to Proxy to create a proxy tunnel; AckProxy is the reply from Proxy to Device to SynProxy;
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