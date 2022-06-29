# 连接流程和协议设计
建立Stream 和 Tunnel的过程，都需要握手过程；在引入了包合并流程之后，在一个package box中合并Stream和Tunnel的握手包，可以在一次握手中，同时完成Stream 和 Tunnel的连接，称为首次连接；Tunnel的生命周期会长于Stream，在Stream close之后，Tunnel依然可能有效，Session 对象可以直接使用已经存在的Tunnel，不在合并Tunnel握手过程，称为二次连接；

本端到远端可能存在一个或者多个可用的udp/tcp tunnel；bdt stream的实现包括两种实现
+ 如果两端存在可用的tcp tunnel，bdt stream实现为创建一条新的tcp stream，在tcp stream传输加密后的流式数据，tcp stream本身已经实现了可靠性传输保证；
+ 如果两端只存在可用的udp tunnel，bdt stream实现为在udp tunnel上收发 protocol::package::Session 数据包（类比tcp stream收发 ip报文），并实现收发队列/滑动窗口/拥塞控制，以实现可靠传输；

# Build Action
为了隔离同时进行的多种 udp/tcp Tunnel的连接尝试，降低混合状态机的实现复杂度， 我们把每个独立的Tunnel的连接尝试抽象为一个Build action对象；
build tunnel的过程就是：组合各种action，尝试连通到远端的一个或者多个udp/tcp tunnel;每个action的执行过程是尝试连通一个udp/tcp tunnel；并基于udp/tcp tunnel创建tcp/udp stream；
action的状态机为：
+ Connecting1 发出连接请求，等待对端返回；
+ PreEstablish 对端可达，但是尚未回复连接请求；
+ Connecting2 本端选择处于PreEstabish的action中合适的一个，调用continue_connect进入Connecting2
+ Establish action对应的udp/tcp tunnel 进入Active状态,并且Stream连通，此时build tunnel完成
```rust
// tunnel/builder/connect_stream/action.rs
pub enum ConnectStreamState {
    Connecting1, 
    PreEstablish,
    Connecting2,  
    Establish, 
    Closed
}

pub trait ConnectStreamAction: BuildTunnelAction {
    fn clone_as_connect_stream_action(&self) -> DynConnectStreamAction;
    fn as_any(&self) -> &dyn std::any::Any;
    fn state(&self) -> ConnectStreamState;
    async fn wait_pre_establish(&self) -> ConnectStreamState;
    async fn continue_connect(&self) -> Result<(), BuckyError>;
}
```

## stream连接中的action包括：
### SynUdpTunnel action
```rust
// tunnel/buidler/actions.rs: SynUdpTunnel
pub struct SynUdpTunnel(Arc<SynUdpTunnelImpl>);
```
用于udp 打洞尝试，定时从本端的一个udp tunnel向远端的一个udp socket 发送合并了 protocol::package::SynTunnel 和 sync stream 的协议包；

```rust
pub struct SynTunnel {
    pub from_device_id: DeviceId,
    pub to_device_id: DeviceId,
    pub sequence: TempSeq,
    pub from_container_id: IncreaseId,
    pub from_device_desc: Device,
    pub send_time: Timestamp,
}
```
根据NAT穿透的原理，可以通过udp NAT穿透连通的设备，同时以一定间隔向对端监听的udp端口发包实现穿透，当收到来自远端的udp 包时，udp tunnel可用；
### ConnectPackageStream action
```rust
// tunnel/builder/connect_stream/package.rs
pub struct ConnectPackageStream(Arc<ConnectPackageStreamImpl>);
```
用于在udp tunnel上创建udp stream;

当action处于Connecting1状态中，定时通过udp tunnel向远端发送protocol::package::SessionData 数据包作为sync stream；

当远端从udp tunnel上收到sync stream时，会向udp tunnel回复protocol::package::SessionData数据包作为 ack sync stream；
```rust
// tunnel/builder/accept_stream.rs: AcceptPackageStream
impl OnPackage<SessionData> for AcceptPackageStream {
    fn on_package(&self, pkg: &SessionData, _: Option<()>) -> Result<OnPackageResult, BuckyError> {
        if pkg.is_syn() {
            // 如果已经confirm了，立即回复ack
            let builder = AcceptStreamBuilder::from(&self.0.builder);
            if let Some(syn_ack) = builder.confirm_syn_ack().map(|c| c.package_syn_ack.clone()) {
                debug!("{} send session data with ack", self);
                let packages = vec![DynamicPackage::from(syn_ack.clone())];
                let _ = builder.building_stream().as_ref().tunnel().send_packages(packages);
```

当action从远端收到ack sync stream时，进入PreEstablish状态;

调用continue_connect时，向远端发送SessionData数据包(可能包含第一段write stream的数据)作为ack ack sync stream；udp stream进入Establish状态；

同样当远端接收到ack ack sync stream时，udp stream进入Establish状态；
```rust
    // tunnel/builder/accept_stream.rs: AcceptPackageStream::on_package
    let stream = AcceptStreamBuilder::from(&self.0.builder).building_stream().clone();
    let _ = stream.as_ref().establish_with(StreamProviderSelector::Package(self.0.remote_id, Some(pkg.clone())), &stream);
```

+ ConnectTcpStream action
```rust
// tunnel/buidler/connect_stream/tcp.rs
#[derive(Clone)]
pub struct ConnectTcpStream(Arc<ConnectTcpStreamImpl>);
```
用于从tcp tunnel上创建bdt tcp stream;如果远端监听了tcp 端口，尝试向tcp 端口发起tcp stream connect；如果tcp stream连通，此时action进入PreEstablish状态；

对应的当远端tcp listener accept到tcp stream时，因为尚且没有在tcp stream read到任何包，在一个超时时间内暂时保留该tcp stream等待后续包；
```rust
// interface/tcp.rs: AcceptInterface::accept
let (box_type, box_buf) = future::timeout(timeout, receive_box(&socket, &mut recv_buf)).await??;
```

调用continue_connect时，在tcp stream上发送protocol::pacakge::TcpSynConnection作为bdt tcp stream握手包,进入Connecting2状态；

当远端在tcp stream上收到TcpSynConnection时，在tcp stream上回复protocol::package::TcpAckConnection，bdt tcp stream进入establish状态；
```rust
// tunnel/builder/accept_stream.rs: AcceptStreamBuilder::on_package
if let Ok(ack) = builder.wait_confirm().await.map(|s| s.tcp_syn_ack.clone()) {
    let _ = match interface.confirm_accept(vec![DynamicPackage::from(ack)]).await {
        Ok(_) => builder.building_stream().as_ref().establish_with(StreamProviderSelector::Tcp(interface.socket().clone(), interface.key().clone()), builder.building_stream()),
```

action在从 tcp stream上收到TcpAckConnection时，进入establish状态，bdt tcp stream进入establish状态；

### AcceptReverseTcpStream action
```rust
// tunnel/builder/connect_stream/tcp.rs
pub struct AcceptReverseTcpStream(Arc<AcceptReverseTcpStreamImpl>);
```
用于接收来自远端的tcp stream 反连的action；

如果本端监听了tcp端口，对端在收到本端的connect stream请求时（来SnCalled),向本端的tcp端口发起 tcp stream connect，在tcp stram上发送 protocol::package::TcpAckConnection包；
```rust
// tunnel/builder/accept_stream.rs: AcceptStreamBuilder::reverse_tcp_stream
let tcp_interface = tcp::Interface::connect(/*local_ip, */remote_ep, remote_device_id.clone(), remote_device_desc, aes_key, stack.config().tunnel.tcp.connect_timeout).await
            .map_err(|err| { 
                tunnel.mark_dead(tunnel.state());
                debug!("{} reverse tcp stream to {} {} connect tcp interface failed for {}", self, remote_device_id, remote_ep, err);
                err
            })?;
let tcp_ack = self.wait_confirm().await.map(|ack| ack.tcp_syn_ack.clone())
    .map_err(|err| { 
        let _ = tunnel.connect_with_interface(tcp_interface.clone());
        debug!("{} reverse tcp stream to {} {} wait confirm failed for {}", self, remote_device_id, remote_ep, err);
        err
    })?;
let resp_box = tcp_interface.confirm_connect(&stack, vec![DynamicPackage::from(tcp_ack)], stack.config().tunnel.tcp.confirm_timeout).await
    .map_err(|err| {
        tunnel.mark_dead(tunnel.state());
        err
    })?;
```

当本端accept到来自远端的tcp stream，并且read tcp stream得到 TcpAckConnection包时，action进入PreEstablish状态；

在action上调用continue_connect, 在tcp stream上发送 protocol::package::TcpAckAckConnection, action进入establish状态， bdt tcp stream进入establish状态；

远端在tcp stream上接收到TcpAckAckConnection后， bdt tcp stream进入establish状态；
```rust
// tunnel/builder/accept_stream.rs: AcceptStreamBuilder::reverse_tcp_stream
let ack_ack: &TcpAckAckConnection = resp_packages[0].as_ref();
let _ = tunnel.pre_active(remote_timestamp);

match ack_ack.result {
    TCP_ACK_CONNECTION_RESULT_OK => {
        stream.as_ref().establish_with(StreamProviderSelector::Tcp(tcp_interface.socket().clone(), tcp_interface.key().clone()), stream)
    }, 
```


# stream连接发起端
build tunnel组合action的策略可以不断优化或者由开发者定制，以当前的默认实现为例：
+ 第一次遍历本端和远端endpoint组合，如果远端endpoint包含固定外网地址，尝试进行直连; 对远端的外网udp端口创建 SynUdpTunnel action 和 ConnectPackageStream action；对远端的外网tcp端口创建 ConnectTcpStream action；
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::build
 let actions = if let Some(remote) = build_params.remote_desc.as_ref() {
        self.explore_endpoint_pair(remote, first_box.clone(), |ep| ep.is_static_wan())
    } else {
        vec![]
    };
```

+ 如果远端没有固定外网地址，向远端上线的sn 发起sn call，并为所有的endpoint组合创建action；
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::call_sn
let remote = stack.sn_client().call(
    &vec![],  
    tunnel.remote(),
    &sn, 
    true, 
    true,
    false,
    |sn_call| {
        let mut context = udp::PackageBoxEncodeContext::from((tunnel.remote_const(), sn_call));
        //FIXME 先不调用raw_measure_with_context
        //let len = first_box.raw_measure_with_context(&mut context).unwrap();
        let mut buf = vec![0u8; 2048];
        let b = first_box.raw_encode_with_context(&mut buf, &mut context, &None).unwrap();
        //buf[0..b.len()].as_ref()
        let len = 2048 - b.len();
        buf.truncate(len);
        buf
    }).await?;

    Ok(self.explore_endpoint_pair(&remote, first_box, |_| true))
```

+ 监听创建的所有action的状态，对第一个进入PreEstablish状态的action调用continue_connect，尝试使用该action创建bdt stream；
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::wait_action_pre_establish
fn wait_action_pre_establish<T: 'static + ConnectStreamAction>(&self, action: T) {
        // 第一个action进入establish 时，忽略其他action，builder进入pre establish， 调用 continue connect
```

+ 等待bdt stream进入Establish状态，build tunnel流程完成；
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::sync_state_with_stream
fn sync_state_with_stream(&self)
```

# stream连接监听端
在通过SNCall发起的首次连接时，连接监听端会首先收到来自SN Server转发的SNCalled,解包SNCalled payload中的SynTunnel + SessionData，创建被动 bdt stream 从 StreamListener返回处于PreEstablish状态的stream，应用层应当在stream上调用confirm以继续连接；
```rust
// tunnel/builder/accept_stream.rs AcceptStreamBuilder::on_called
fn on_called(&self, called: &SnCalled, caller_box: PackageBox) -> Result<(), BuckyError>
```
和发起段同样，为了进行打洞尝试，监听端也需要组合action配合发起端；
build tunnel组合action的策略可以不断优化或者由开发者定制，以当前的默认实现为例：
+ 向请求端监听的tcp端口发起tcp stream connect，尝试连通 bdt tcp stream；
+ 向请求端监听的udp端口发送打洞包，尝试建立udp tunnel；
```rust
// tunnel/builder/accept_stream.rs AcceptStreamBuilder::build
async fn build(&self, caller_box: PackageBox, active_pn_list: Vec<DeviceId>) -> Result<(), BuckyError> {

    for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_tcp()) {
        // let local_ip = *local_ip;
        let remote_ep = *remote_ep;
        let builder = self.clone();
        let remote_deviceid = syn_tunnel.from_device_id.clone();
        let remote_constinfo = syn_tunnel.from_device_desc.desc().clone();
        let remote_timestamp = syn_tunnel.from_device_desc.body().as_ref().unwrap().update_time();
        let aes_key = caller_box.key().clone();
        task::spawn(async move {
            let _ = builder.reverse_tcp_stream(
                /*local_ip, */
                remote_ep, 
                remote_deviceid, 
                remote_constinfo, 
                remote_timestamp, 
                aes_key).await
                .map_err(|e| {
                    debug!("{} reverse tcp stream to {} failed for {}", builder, remote_ep, e);
                    e
                });
        });
    }


    for udp_interface in net_listener.udp() {
        for remote_ep in connect_info.endpoints().iter().filter(|ep| ep.is_udp() && ep.is_same_ip_version(&udp_interface.local())) {
            if let Ok(tunnel) = stream.as_ref().tunnel().create_tunnel(EndpointPair::from((udp_interface.local(), *remote_ep)), ProxyType::None) {
                SynUdpTunnel::new(
                    tunnel, 
                    first_box.clone(), 
                    stream.as_ref().tunnel().config().udp.holepunch_interval);      
            }    
        }  
    }
}
```

# 二次连接
当TunnelContainer已经处于Active状态时（首次Stream连接通过build tunnel达到的结果），发起stream connect，此时不再需要build tunnel的流程，因为TunnelContainer中的 udp/tcp tunnel已经Acitve或者处于PreActive， 可以通过tunnel 发送bdt stream的连接请求而不需要通过SN Server中转；
bdt stream的两个实现中，应当是tcp 优先的，含义是如果同时有 udp和tcp tunnel可用，优先建立bdt tcp stream；
```rust
//tunnel/container.rs TunnelContainer::select_stream_connector
pub(crate) async fn select_stream_connector(
        &self,
        build_params: BuildTunnelParams,  
        stream: StreamContainer) -> BuckyResult<StreamConnectorSelector> {
        
    match &mut state.tunnel_state {
                TunnelStateImpl::Active(active) => {
                    let cur_timestamp = active.remote_timestamp;
                    if let Some(selector) = Self::select_stream_connector_by_exists(
                            cur_timestamp, 
                            &state.tunnel_entries) {
```
如果只有udp tunnel可用，通过udp tunnel向远端发送SessionData（包含syn connection flag）;远端回复SessionData(包含ack syn connection flag)；本端回复SessionData完成三次握手，建立PackageStream;
```rust
// stream/container.rs connector::PackageProvider
impl Provider for PackageProvider {
    fn begin_connect(&self, stream: &StreamContainer) {
```
如果有主动tcp tunnel可用，向远端的tcp端口发起tcp stream connect，并在tcp stream上发送TcpSynConnection；远端accept tcp stream后，回复TcpAckConnection；建立bdt tcp stream；
```rust
// stream/container.rs connector::TcpProvider
impl Provider for TcpProvider {
    fn begin_connect(&self, stream: &StreamContainer) {
```
如果tcp tunnel中只有被动tcp tunnel可用；需要通过tunnel发送bdt tcp stream连接请求；如果TunnelContainer中只有被动tcp tunnel可用并且处于PreActive状态，还要首先通过SN Server使tcp tunnel进入Active状态以发送straem连接请求；
连接端首先通过tunnel发送TcpSynConnectioin, TcpSynConnection.reverse_endpionts被指定为本地监听的tcp 端口；
```rust
// stream/container.rs connector::ReverseProvider
impl Provider for ReverseProvider {
    fn begin_connect(&self, stream: &StreamContainer)
```
监听端从tunnel上收到TcpSynConnection时，向reverse_endpoints指定的tcp端口发起tcp stream connect，并且发送TcpAckConnection;
连接端accept tcp stream，并且在tcp stream上read到TcpAckConnection时，bdt tcp stream可用，并回复TcpAckAckConnection；


# Tcp Tunnel State
和Udp Tunnel不同，tcp tunnel的active状态分为两种 PreActive 和 Active；通过tcp tunnel创建的Stream对象的同时，额外创建新的tcp stream；在tunnel的首次连接时，联通的tcp stream被Stream持有，而tcp tunnel处于PreActive状态，意为可以通过tcp tunnel对应的 tcp address创建新的tcp stream用于发送packet；当 tcp tunnel的connect接口被调用时，创建额外的tcp stream用于发送packet，此时tcp tunnel进入active状态；

tcp tunnel 还是有方向的， 
一种方向是向对端的监听的tcp port发起连接: connect 直接向对端connect tcp stream；
另一种方向在本地的tcp port监听连接： connect要让对端向本地发起connect tcp stream，首先向SN Node发起SNCall；

# SN Server实现
SN(SuperNode)用于发现device的经过NAT映射的NAT地址，查询device信息，协助NAT穿透；  
SN Server应当部署在有固定公网地址，udp友好的设备上；
## Device在SN上线流程
bdt stack初始化时，本地bind udp 端口之后，首先向SN Server的udp port发送 protocol::package::SNPing 包，
```rust
//sn/client/ping.rs Client::start
fn start(&self)
```
SN Server收到来自device的SNPing包时，记录device的信息，在protocol::package::SNPingResp中返回SN Server观察到的device的外网地址；
```rust
//sn/service/service.rs: SnService::handle_ping
fn handle_ping
```
bdt stack从同udp 端口首次收到来自SN Server的 protocol::package::SNPingResp回复后，在SN server上线成功，bdt stack更新device endpoint list，重新签名之后同步到SN Server上；
```rust
//sn/client/ping.rs Client::on_ping_resp
fn on_ping_resp(&self, resp: &SnPingResp, from: &Endpoint, from_interface: Interface)
```
此后bdt stack定期向SN Server发送SNPing，来自SN Server的SNPingResp；如果bdt stack退出，SN Server在一定时间丢失来自bdt stack的SNPing时，认为该device下线，移除device信息；
```rust
//sn/service/service.rs: SnService::clean_timeout_resource
pub fn clean_timeout_resource(&mut self)
```

## 通过SN连接NAT后的Device
当device首次向NAT后的其他device发包时，需要通过SN Server中转；因为NAT的过滤规则，即便已知device经过NAT映射后的外网地址，也不一定可达；
stream连接端向远端device上线的SN Server发送protocol::package::SNCall包， 在其中的payload中发送SynTunnel和SessionData;
```rust
//sn/client/call.rs: CallManager::call
 pub fn call(&self,
            reverse_endpoints: &[Endpoint],
            remote_peerid: &DeviceId,
            sn: &Device,
            is_always_call: bool,
            is_encrypto: bool,
            with_local: bool,
            payload_generater: impl Fn(&SnCall) -> Vec<u8>
```
SN Server收到来自device的SNCall包时，如果SNCall的目标device在该SN Server上上线，通过protocol::package::SNCalled向目标device转发SNCall中的payload；并向device回复protocol::package::SNCallResp包，在其中包含目标device的信息；
```rust
// sn/service/service.rs: SnService::handle_call
fn handle_call(&mut self, mut call_req: Box<SnCall>, resp_sender: MessageSender, _encryptor: Option<(&AesKey, &DeviceId)>, send_time: &SystemTime)
// sn/service/service.rs: SnService::handle_called_resp
fn handle_called_resp(&self, called_resp: Box<SnCalledResp>, _aes_key: Option<&AesKey>)
```
### 通过SN Server不同NAT类型设备之间的连通性

常见NAT类型说明：
* Full Cone NAT ：内网主机建立一个UDP socket(LocalIP:LocalPort) 第一次使用这个socket给外部主机发送数据时NAT会给其分配一个公网(PublicIP:PublicPort),以后用这个socket向外面任何主机发送数据都将使用这对(PublicIP:PublicPort)。此外，任何外部主机只要知道这个(PublicIP:PublicPort)就可以发送数据给(PublicIP:PublicPort)，内网的主机就能收到这个数据包

* Restricted Cone NAT ：内网主机建立一个UDP socket(LocalIP:LocalPort) 第一次使用这个socket给外部主机发送数据时NAT会给其分配一个公网(PublicIP:PublicPort),以后用这个socket向外面任何主机发送数据都将使用这对(PublicIP:PublicPort)。此外，如果任何外部主机想要发送数据给这个内网主机，只要知道这个(PublicIP:PublicPort)并且内网主机之前用这个socket曾向这个外部主机IP发送过数据。只要满足这两个条件，这个外部主机就可以用自己的(IP,任何端口)发送数据给(PublicIP:PublicPort)，内网的主机就能收到这个数据包

* Port Restricted Cone NAT ：内网主机建立一个UDP socket(LocalIP:LocalPort) 第一次使用这个socket给外部主机发送数据时NAT会给其分配一个公网(PublicIP:PublicPort),以后用这个socket向外面任何主机发送数据都将使用这对(PublicIP:PublicPort)。此外，如果任何外部主机想要发送数据给这个内网主机，只要知道这个(PublicIP:PublicPort)并且内网主机之前用这个socket曾向这个外部主机(IP,Port)发送过数据。只要满足这两个条件，这个外部主机就可以用自己的(IP,Port)发送数据给(PublicIP:PublicPort)，内网的主机就能收到这个数据包

* Symmetric NAT ：内网主机建立一个UDP socket(LocalIP,LocalPort),当用这个socket第一次发数据给外部主机1时,NAT为其映射一个(PublicIP-1,Port-1),以后内网主机发送给外部主机1的所有数据都是用这个(PublicIP-1,Port-1)； 如果内网主机同时用这个socket给外部主机2发送数据，第一次发送时，NAT会为其分配一个(PublicIP-2,Port-2), 以后内网主机发送给外部主机2的所有数据都是用这个(PublicIP-2,Port-2)，这种NAT无法实现UDP-P2P通信

通过SN服务BDT协议理论上，不同NAT环境机器间建立UDP Tunnel的连通性

| LN NAT/RN NAT            | Public IP | Full Cone NAT | Restricted Cone NAT | Port Restricted Cone NAT | Symmetric NAT |
| ------------------------ | --------- | ------------- | ------------------- | ------------------------ | ------------- |
| Public IP                | 成功      | 成功        | 成功              | 成功                   | 成功        |
| Full Cone NAT            | 成功      | 成功        | 成功              | 成功                   | 成功        |
| Restricted Cone NAT      | 成功      | 成功        | 成功              | 成功                   | 成功        |
| Port Restricted Cone NAT | 成功      | 成功        | 成功              | 成功                   | 失败        |
| Symmetric NAT            | 成功      | 成功        | 成功              | 失败                   | 失败        |


通过SN服务BDT协议理论上，不同NAT环境机器间建立TCP Tunnel的连通性
| LN NAT/RN NAT            | Public IP | Full Cone NAT | Restricted Cone NAT | Port Restricted Cone NAT | Symmetric NAT |
| ------------------------ | --------- | ------------- | ------------------- | ------------------------ | ------------- |
| Public IP                | 成功      | 成功        | 成功              | 成功                   | 成功        |
| Full Cone NAT            | 成功      | 失败        | 失败              | 失败                   | 失败        |
| Restricted Cone NAT      | 成功      | 失败        | 失败              | 失败                   | 失败        |
| Port Restricted Cone NAT | 成功      | 失败        | 失败              | 失败                   | 失败        |
| Symmetric NAT            | 成功      | 失败        | 失败              | 失败                   | 失败        |

UDP Tunnel中 Symmetric NAT 和 Port Restricted Cone NAT 、Symmetric NAT 和 Symmetric NAT 之间无法使用BDT SN打洞;
TCP Tunnel中 NAT设备间无法使用BDT SN打洞;
BDT中使用Proxy Node服务解决该问题，在后续Proxy Server中说明.
