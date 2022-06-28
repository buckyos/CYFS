# Connect Stream and Tunnel
Both establishing `Stream` and `Tunnel` requires a handshake process; For package merging future is introduced, handshake packets of `Stream` and `Tunnel` can be combined in one package box, establishing can be completed in one handshake, called the `first time connection`.

The life cycle of a `Tunnel` will be longer than that of a `Stream`. After the `Stream` is closed, the Tunnel may still be valid. Later session objects directly use the existing `Tunnel` without merging the `Tunnel` handshake process, called a `secondary connection`.

There may be one or more udp/tcp tunnels available between devices, `Stream` implemention differs on udp /tcp tunnel as link layer:
+ If there are available tcp tunnels, `Stream` is implemented to create a new tcp stream, transmits encrypted streaming data, tcp stream itself has achieved a reliable transmission guarantee;
+ If there are only available udp tunnels, `Stream`  is implemented to send and receive `SessionData` packets on udp tunnel(same as tcp stream sends and receives ip packets), `Stream` has to implement sending and receiving queues/sliding windows/congestion control to achieve reliable transmission;

# Build Action
To split connection attempts of multiple udp/tcp tunnels connecting in a `Tunnel` connection process, reduce the implementation complexity of the composited state machine, we abstract each independent tunnel connection attempt as a `Build Action` object. 

With this design, a `Tunnel` connection process is combining various actions, each action is trying to connect a udp/tcp tunnel; 

One `Build Action` has following states:
+ Connecting1: sends a connection request, waits response from remote;
+ PreEstablish: connection requests reaches remote;
+ Connecting2: selects appropriate one of PreEstabish stated actions, calls function `continue_connect` make action entering Connecting2 state;
+ Established: The udp/tcp tunnel corresponding to the action enters the Active state;
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

## Actions Built in BDT
### SynUdpTunnel action
```rust
// tunnel/buidler/actions.rs: SynUdpTunnel
pub struct SynUdpTunnel(Arc<SynUdpTunnelImpl>);
```
Making udp hole-punching attempts, regularly sends a package box combining a `SynTunnel` plus a stream sync, from a local udp socket to a remote udp ip address;
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
We simplify NAT hole-punching process as: both devices regularly sending packet from udp socket to remote at same time. If a udp packet received from remote, udp tunnel is actived.

### ConnectPackageStream action
```rust
// tunnel/builder/connect_stream/package.rs
pub struct ConnectPackageStream(Arc<ConnectPackageStreamImpl>);
```

Connecting `Stream` from udp tunnels. When action is in the Connecting1 state, regularly sends the `SessionData` packet with sync flag to remote through the udp tunnels. 

When remote receives `SessionData` with sync flag packet from the udp tunnels, it will reply the a `SessionData` packet with both sync and ack flag on udp tunnels;

### ConnectTcpStream action
```rust
// tunnel/buidler/connect_stream/tcp.rs
#[derive(Clone)]
pub struct ConnectTcpStream(Arc<ConnectTcpStreamImpl>);
```
Connecting `Stream` from a tcp tunnel. If remote device listening a tcp port, try to connect a tcp stream; when tcp stream established, action enters PreEstablish state.

Correspondingly, remove device accepts a tcp stream, but no data read yet,  temporarily reserved for a timeout period to wait for subsequent packets.
```rust
// interface/tcp.rs: AcceptInterface::accept
let (box_type, box_buf) = future::timeout(timeout, receive_box(&socket, &mut recv_buf)).await??;
```

When calling function `continue_connect` on action, TcpSynConnection packet is sent on tcp stream as handshake, enters the Connecting2 state;
```rust
pub struct TcpSynConnection {
    pub sequence: TempSeq,
    pub result: u8,
    pub to_vport: u16,
    pub from_session_id: IncreaseId,
    pub from_device_id: DeviceId,
    pub to_device_id: DeviceId,
    pub proxy_device_id: Option<DeviceId>,
    pub from_device_desc: Device,
    pub reverse_endpoint: Option<Vec<Endpoint>>,
    pub payload: TailedOwnedData,
}

pub struct TcpAckConnection {
    pub sequence: TempSeq,
    pub to_session_id: IncreaseId,
    pub result: u8,
    pub to_device_desc: Device,
    pub payload: TailedOwnedData,
}

pub struct TcpAckAckConnection {
    pub sequence: TempSeq,
    pub result: u8,
}
```
Correspondingly, remote device receives TcpSynConnection on reserved tcp stream, replies a TcpAckConnection packet, remote's `Stream` object established. 
```rust
// tunnel/builder/accept_stream.rs: AcceptStreamBuilder::on_package
if let Ok(ack) = builder.wait_confirm().await.map(|s| s.tcp_syn_ack.clone()) {
    let _ = match interface.confirm_accept(vec![DynamicPackage::from(ack)]).await {
        Ok(_) => builder.building_stream().as_ref().establish_with(StreamProviderSelector::Tcp(interface.socket().clone(), interface.key().clone()), builder.building_stream()),
```
Action receives the TcpAckConnection on connected tcp stream, enters the established state.

### AcceptReverseTcpStream action
```rust
// tunnel/builder/connect_stream/tcp.rs
pub struct AcceptReverseTcpStream(Arc<AcceptReverseTcpStreamImpl>);
```
Accept reversed tcp stream from the remote. If device is listening a local tcp port, remote devices may connect a tcp stream with `TcpAckConnection` packet to this tcp port when connecting `Tunnel`. 
```rust
// tunnel/builder/accept_stream.rs: AcceptStreamBuilder::reverse_tcp_stream
let tcp_interface = tcp::Interface::connect(/*local_ip, */remote_ep, remote_device_id.clone(), remote_device_desc, aes_key, stack.config().tunnel.tcp.connect_timeout).await
            .map_err(|err| { 
                tunnel.mark_dead(tunnel.state());
                err
            })?;
let tcp_ack = self.wait_confirm().await.map(|ack| ack.tcp_syn_ack.clone())
    .map_err(|err| { 
        let _ = tunnel.connect_with_interface(tcp_interface.clone())
    })?;
let resp_box = tcp_interface.confirm_connect(&stack, vec![DynamicPackage::from(tcp_ack)], stack.config().tunnel.tcp.confirm_timeout).await
    .map_err(|err| {
        tunnel.mark_dead(tunnel.state());
        err
    })?;
```
When action accepts a tcp stream, reads a TcpAckConnection packet, enters PreEstablish state. When `continue_connect` function called on this action, sends TcpAckAckConnection on the tcp stream, enters the Established state.
```rust
// tunnel/builder/accept_stream.rs: AcceptStreamBuilder::reverse_tcp_stream
let ack_ack: &TcpAckAckConnection = resp_packages[0].as_ref();
let _ = tunnel.pre_active(remote_timestamp);

match ack_ack.result {
    TCP_ACK_CONNECTION_RESULT_OK => {
        stream.as_ref().establish_with(StreamProviderSelector::Tcp(tcp_interface.socket().clone(), tcp_interface.key().clone()), stream)
    }, 
```

# Connecting Stream
The default strategy of combining actions is:
1. Traverse local and remote endpoints, 
+ If remote contains a static public ip address, try to connect it directly: create SynUdpTunnel action and ConnectPackageStream action to udp endpoint, create ConnectTcpStream action to tcp endpoint;
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::build
 let actions = if let Some(remote) = build_params.remote_desc.as_ref() {
        self.explore_endpoint_pair(remote, first_box.clone(), |ep| ep.is_static_wan())
    } else {
        vec![]
    };
```
+ If remote has no static public ip address, send `SnCall` packet to SN node on which remote device has pinged to, and create actions for all local/remote endpoint pairs;
```rust
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
        let mut buf = vec![0u8; 2048];
        let b = first_box.raw_encode_with_context(&mut buf, &mut context, &None).unwrap();
        let len = 2048 - b.len();
        buf.truncate(len);
        buf
    }).await?;

    Ok(self.explore_endpoint_pair(&remote, first_box, |_| true))
```
2. Monitor the state of all actions created, call `continue_connect ` function on the first action that entered PreEstablish state;
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::wait_action_pre_establish
fn wait_action_pre_establish<T: 'static + ConnectStreamAction>(&self, action: T) 
```
3. Wait for `Stream` to enter Established state, complete connecting tunnel process.
```rust
// tunnel/builder/connect_stream.rs: ConnectStreamBuilder::sync_state_with_stream
fn sync_state_with_stream(&self)
```

# Accepting Stream
When device is listening for incoming connection. It will first receive a `SNCalled` packet forwarded from the SN Node, unpacks a package box from `SNCalled`'s payload field,  a `SynTunnel` and `SessionData` with sync flag is combined in box. A incoming `Stream`  in PreEstablish state returned to application interface. Application code should call `confirm` function on `Stream` to continue accepting;
```rust
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

```rust
// tunnel/builder/accept_stream.rs AcceptStreamBuilder::on_called
fn on_called(&self, called: &SnCalled, caller_box: PackageBox) -> Result<(), BuckyError>
```

Correspondingly, to make hole-punching attempt or reverse tcp stream connecting, accepting `Stream` process should create actions on local/remote endpoints. The default strategy of combining actions is:
+ create SynUdpTunnel action to udp endpoint;
+ create ConnectTcpStream action to tcp endpoint;
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

# Secondary Connection
If `Tunnel` has connected in active state before,  to connect a `Stream` no longer needs to connect `Tunnel` as done in `First time Connection`, `Stream` handshake packet can sent from `Tunnel` directly without forwarding by SN node; 
By default, if both udp tunnels and tcp tunnels are active, prefer to use tcp tunnel for `Stream`.
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
```
+ If only the udp tunnels are active, create a ConnectPackageStream action, `Stream` handshakes with `SessionData`;
```rust
// stream/container.rs connector::PackageProvider
impl Provider for PackageProvider {
    fn begin_connect(&self, stream: &StreamContainer) {
```
+ If any tcp tunnel connecting to remote's listening tcp port is active, create a TcpSynConnection action on it;
```rust
// stream/container.rs connector::TcpProvider
impl Provider for TcpProvider {
    fn begin_connect(&self, stream: &StreamContainer) {
```
+ If no tcp tunnel connecting to remote's listening tcp port is active, but a tcp tunnel accepting on local's listening tcp port is active, sends TcpSynConnection packet with `reverse_endpionts` fields set from tcp tunnel; and if tunnel's in pre-active state, tcp tunnel should firstly got actived with help of SN node;
```rust
// stream/container.rs connector::ReverseProvider
impl Provider for ReverseProvider {
    fn begin_connect(&self, stream: &StreamContainer)
```

# Tcp Tunnel State
On udp tunnel, tunnel is actived means device can send receive udp datagram from this udp address pair;
But on tcp tunnel, tunnel has 2 active state, pre-active and active; when `First time Connecition` on `Stream` and `Tunnel` finished, a established tcp stream was used for `Stream` but not `Tunnel`, tcp tunnel is pre-actived, means that a tcp stream connect from this ip address pair will establish, but no tcp stream is established now. To make a pre-actived tunnel entering active state, a tcp stream should establish for this tunnel. Tcp tunnel is also directed: 
+ one direction is connecting to remoted's listening tcp port, connect a tcp stream to remote directly;
+ another direction is accepting on local's listening tcp port, sends a `SNCall` packet to SN node on which remote device has pinged to, and waits revserse tcp stream connection from remote.

# Super Node
SN(Super Node), is used to discover the NAT-mapped address of device, query device information, and assist hole-punching; SN node should be deployed on a static public ip addressed, udp-friendly device.

## ping to SN node
+ Device sends SNPing packet to SN node's udp address;
```rust
pub struct SnPing {
    pub seq: TempSeq,                         
    pub sn_peer_id: DeviceId,                 
    pub from_peer_id: Option<DeviceId>,     
    pub peer_info: Option<Device>,             
    pub send_time: Timestamp,                 
    pub contract_id: Option<ObjectId>,       
    pub receipt: Option<ReceiptWithSignature>, 
}
```
+ SN node receives SNPing Packet from device, records device's local and public addresses, replies device's public address with a SNPingResp packet; 
```rust
pub struct SnPingResp {
    pub seq: TempSeq,                     
    pub sn_peer_id: DeviceId,             
    pub result: u8,                        
    pub peer_info: Option<Device>,        
    pub end_point_array: Vec<Endpoint>,    
    pub receipt: Option<SnServiceReceipt>, 
}
```

## forward connection request
+ To connect a remote device, send SNCall packet to SN node on which remote device has pinged to.
+ When SN node receives a SNCall packet, if SN node has target device's record, forward SNCall packet with a SnCalled packet to target;