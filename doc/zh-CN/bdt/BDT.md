# 功能

# 稳定性

# 关键指标（可能有性）

# Tunnel

## BDT版本兼容 OK

## 加密和MixHash 
MixHash的日志优化，15分钟的粒度，日期获取可以从MetaChain获得
指标考虑：MixHash错误的概率？
### 新的PN支持和验证
主要是集中在PN的服务证明和服务发现上

## 去中心PN
### PN协议和PN实现
TODO：PN能实现真实节点隐藏么？
TODO：开发远程桌面的难度？
TODO：基于PN的两条网络能实现流量特征的消除么？每个人都要被动PN，每个人都可以成为PN

+ 主动代理和被动代理；
+ 基于Package Box头部Hash的匹配转发；
+ 对AesKey透明；对Package Box内容透明；对编解码透明；
+ 只支持udp链路代理；
+ 要兼容MixHash实现；

### PN服务证明
+ 添加额外的ProxyControl Packet；
+ 在PN Tunnel生效期间；PN定期通过ProxyControl要求两端提供传输证明；
+ 两端聚合通过PN Tunnel收到的流量，通过ProxyControl返回签名的流量证明；

### PN服务发现
+ PN DHT 发现 

## 连接 *
逻辑简化，把一些无效的逻辑给删除
连接的流程文档，强化灰盒测试
指标：连接成功率和平均连接延迟

## 明确CYFS的去中心Boot问题
ConfigList(手工更新) -> DHT自动更新ConfigList -> 能从BTC上读到ConfigList -> 得到MetaChain

### 本地有效EndPoint的智能选择

## DHT改进 
DHT的目的是用在BOOT流程里使用DHT去发现节点
传统DHT只用于MetaChain的节点发现，MetaChain可用后，可用区块链特性实现进一步的，更大规模的服务发现（共识列表）
     DHT不能用于存储，只能用于Cache，D-LRU才是正确名字
+ Device引入Area：异或逻辑距离反映链路距离的特性 OK
+ Device引入PoW：提高污染攻击门槛 ： Device闲置时应该用PoW来提高Device的难度信用
+ 用作广域网服务发现 

## 去中心SN
stack可以不依赖SN上线 ** 

### SN服务证明 待重构
有一个实现，需要重新梳理;
1.  SnPing 证明:
+ SnPingResp要求下次Ping发送服务证明； 
2. SnCall 证明（目前缺失)：
+ SnCalled要求在SnCalledResp嵌入证明；

### SN服务发现 待重构
+ MetaChain的超节点，都是SN
+ MetaChain共识
+ SN DHT发现（删除）
+ 共识列表：专门进行区块链服务发现的机制
删除）
+ 共识列表发现

## 被动两跳网络/多路TcpTunnel （TODO有测试么？）
1. A->B的流量，超过阈值的部分，会主动走代理；对Session透明；MixHash提供了协议的浑浊，代理提供了链路的浑浊；
2. 并发多个TcpTunnel提高桌面端抢带宽能力;

+ 依赖TunnelContainer多链路并发；
+ 重新定义builder的策略：总是并发一条Proxy链路
+ 与基于Endpoint pair的TunnelContainer的抽象冲突？
+ DefaultTunnel的逻辑可定制： 所有Session对象（包括NDN）并不只是通过单一的default tunnel发送packet；以一定的策略在并发的链路中选择其中一个或者多个；

## MTU探测
TODO：找刀哥
动态MTU，大MTU检测与支持

# Stream
## FastQA
在能使用FastQA的场景下尽量使用（NON）
指标：使用率
TODO：CYFS 体系里最常用的FirstQuestion是啥?

+ FirstQuestion的measure逻辑：首次连接是合并的PackageBox要嵌入SnCall；二次连接的时候又不会，FirstQuestion的可用大小非常有限；如果能依赖大MTU，可以有效解决这个问题；同时又要保证语义上的一致(放不放的下FirstQuestion时，应用接口应该是一致的，否则很不好用)；

## TCP Stream
实现Review:固有延时问题
TODO：我们为什么要做
    统计学问题：CYFS体系里多少比例的Stream会是TCPStream

+ 类TLS实现，以record粒度加密stream，相当于把stream又加上包边界，合并和negle timer可能导致固有的延迟；
+ TcpTunnel绑定了 aes key；如果连接时间非常长，不能更换aes key；保留tcp socket重新交换 aes key的机制；

## 移动设备友好
TODO:统计学上，长连接有哪些？是否需要移动友好?
     是不是运营商已经把这个事情给搞定了？

## 拥塞控制 （长期优化）
目前已经都切到BBR，目前能跑满（） 
TODO：我们现在的实现的实验室数据
      迭代优化是以后工作的主题

1.拥塞算法可选择
+ 在Stack粒度的可选：指定当前stack的所有stream；
+ 在Stream粒度的可选： 在SessionData的Syn中声明prefer；在Ack中确认；考虑向下兼容性（支持迭代的基础设施）；
2.Ledbat和BBR
+ BDT的实现中 BBR的效果是优于Ledbat的；

# Datagram
TODO：和可变MTU关系密切

## 广播支持（局域网 和 广域网）
无需求无场景

# 资源占用的理论分析
内存 + Timer

# CacheNode和流量证明
完成开发，尽早集成

# NDN
写一篇如何正确使用，正确使用的文章开始
组织一次专题讨论
