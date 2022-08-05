# PackageBox
为了能够在一个链路层数据包中，尽量多的嵌入BDT协议包，BDT引入了PackageBox的设计；
一个PackageBox对应链路层上的一次封包；在udp，package box编码成一个udp datagram；在tcp， package box定义了数据流上的包边界和分包逻辑；

多个BDT协议包合并入PackageBox，以达成在链路层上的一次封包中发送多个BDT协议包；最常见的场景是：在一对 udp链路可联通的device D1 和 D2 之间首次连接Stream，合并了Tunnel的连接过程， 两个过程有各自的三次握手，在逻辑上应当包含两个独立的三次握手：
```
1. udp datagram D1 -> D2 Tunnel Sync
2. udp datagram D2 -> D1 Tunnel Ack
3. udp datagram D1 -> D2 Tunnel AckAck
4. udp datagram D1 -> D2 Stream Sync 
5. udp datagram D2 -> D1 Stream Ack
6. udp datagram D1 -> D2 Stream AckAck
```
一共需要6次串行udp QA；如果合并这两个三次握手，实际上的变成
```
1. udp datagram D1 -> D2 Tunnel Sync, Stream Sync 
2. udp datagram D2 -> D1 Tunnel Ack, Stream Ack
3. udp datagram D1 -> D2 Tunnel AckAck, Stream AckAck 
```

# 协议包的编码
按照上述需求，我们设计单个协议包的编码格式如下：

类型码 8bits| 标记位 16bits| [字段值...]
类型码标识协议包类型；
标记位16bits，每个bit可用来标记一个开关值：一般用来标记某字段值是否被编码；
之后每个要被编码的字段值编码；实现了raw encode/raw decode 的类型；包括

# 包合并
把bdt协议包合并入同一个package box，还引入了一个机会：合并入同package box的协议包如果有相同的字段值，只实际编码一次该值，通过一个较小的标记位来标记这些字段引用了该值；通过包合并可以大大缩小package box的编码长度，package box的最大编码长度受到链路层MTU的限制；
在引入包合并机制后，对一组 [协议包...] P[]的编码流程如下：
1. 对第一个包P[0]，编码写入每一个字段值；
2. 对后续包P[N]的每一个可合并字段F，如果P[N][F]==P[0][F],设置F对应编码标记位为0，跳过编码；否则设置对应标记位为1，写入编码P[N][F];

对应的一组[协议包...] P[]的解码流程如下：
1. 解码第一个包P[0]的每一个字段值;
2. 对后续包P[N]的每一个可合并字段F，如果对应标记位为0，置P[N][F]=P[0][F];否则解码出V, 置P[N][F]=V；


# ExchangeKey
package box 还应当是加密的，天然的device object包含一对非堆成加密密钥，在tunnel上使用对端的共钥加密所有bdt 协议包当然可以满足需求，但是这显然是低效，还对私钥不安全；类似tls，首先随机生成对称密钥，通过非堆成密钥来安全的将对称密钥通告对端；此后使用对称密钥加密后续包；
在两端device不存在双方已知的对称密钥时，首先通过通过exchange通告密钥；
```rust
pub struct Exchange {
    pub sequence: TempSeq,
    pub seq_key_sign: Signature,
    pub from_device_id: DeviceId,
    pub send_time: Timestamp,
    pub from_device_desc: Device,
}
```
exchange 包总是应当被编码在package box 第一个包， 包含exchange 包的package box编码格式如下:
经公钥加密的密钥K | exchange包 | [协议包...]

编码包含exchange包的package box的过程如下:
1. 本段生成新的随机对称密钥K， 通过一个双向映射的数据结构 map device id <-> key, 关联对端device id 和 K； 
2. 使用对端device object中的public key 加密对称密钥K，写入package box的头部；
3. 生成递增的 exchange的sequence 值；
4. 使用本端device object的secret签名 K 和 sequence，写入exchange的seq_key_sign值；
5. 编码exchange包写入package box；
6. 编码要合并的其他协议包写入package box；
7. 使用K加密 exchange 和 协议包部分；

对应的，对端device通过链接层封包接受到一个包含exchange的package box时，解码package box 的过程如下:
1. 使用本端的secret解码出对称密钥K；
2. 使用K解码 exchange 和 合并的其他协议包；
3. 用本端的secret校验exchange.seq_key_sign值；
3. 通过一个双向映射的数据结构 map device id <-> key, 关联对端device id 和 K；

# key mix hash
在通过exchange完成向对端device的堆成密钥K的通告之后，再次向该device发送package box时，可以继续使用对称密钥K，不需要包含exchange；

使用K加密的package box 编码格式如下:
hash(K) | [协议包...]

编码包含使用K加密的package box的过程如下:
1. 使用一种双方约定的hash算法，写入 hash(k) 到package box的头部；
2. 编码要合并的协议包写入package box；
3. 使用K加密协议包部分；

对应的，对端device通过链接层封包接受到一个包含exchange的package box时，解码package box 的过程如下:
1. 双向映射的数据结构 map device id <-> key 中搜索 hash(K)， 返回K 和 device id；
2. 使用K解码合并的协议包；

# 明文package box
从BDT的整体设计需求来看，为逻辑数据设计的Stream和Datagram 都应当是全加密的，Tunnel的连接过程也应当是全加密的。
所以明文package box的实现依然可以依赖 通过密文package box交换的key，以及双向映射的数据结构 map device id <-> key;
当通过package box完成了密钥K的交换后，可以借用package box编码中 hash(K)的第一个bit标识package box是否加密；约定新的hash' 算法为， hash'(K) = hash(k) & 0x7f, 使 hash'(K) 的第一个bit总是0； 使用K加密的package box的编码的第一个bit也会总是0；那么编码明文package box的如下：
1. 写入hash'(K) 到package box的头部；
2. 编码要合并的协议包写入package box；
3. 将package box的第一个bit设置为1；

# 解码package box
综合上面的所有情况，从一段封包数据D解码package box的流程如下：
1. 读取D的前sizeof(hash)值为H；
2. H' = H & 0x7f 置第一个bit为0；
3. 双向映射的数据结构 map device id <-> key 中搜索 H'， 
+ 如果存在返回K 和 device id，如果H的第一个bit是0，使用K解码D剩余数据；
+ 否则认为package box包含exchange包，尝试使用本端的secret解码出对称密钥K, 使用K解码D剩余数据；

# udp封包package box
在udp 链路上封包package box，不需要额外的字段，将package box的编码数据作为一段udp datagram； 

# tcp封包package box
在tcp 流上封包package box，我们简单定义 signed int 16 H作为分隔包的包头，H=package box编码的长度；tcp流上分包出package box的逻辑简单为：
1. 读出2byte，得到H；
2. 读出|H| byte D，从D解码package box；

tcp stream的两端限定了device，在tcp stream上发送的第一个package box应当有上述package box的完整编码以完成密钥K的交换； 之后，可以使用K加密这条tcp stream的后续package box，所以后续package box并不需要在头部放入hash'(K)；相应的，明文package box借用package box 的第一个bit作为加密标志的逻辑就不再可行，但是我们借用 H的第一个bit也就是signed int 16的符号位作为加密标志, 综上所述，

tcp stream 解码第一个package box的流程为：
1. 读出2byte，得到H；
2. 读出|H| byte D；
3. 从D解码package box，并且得到密钥K；

之后的流程为:
1. 读出2byte，得到H；
2. 读出|H| byte D；
3. 如果H > 0, 用K解密D得到D'，从D'解码[协议包...];
4. 如果H < 0, 从D解码[协议包...];










