# CYFS协议设计简介
CYFS 协议是对HTTP协议的整体升级，会尽量保持HTTP的近似语义。     
核心流程是：
> CYFS DEC App <--cyfs@http--> cyfs-rutnime <--cyfs@bdt--> gateway <--cyfs@http--> CYFS DEC Service    

实际在网络中运行的cyfs@bdt协议并不会被DEC App的客户端和服务器直接使用，这个设计让cyfs@bdt协议的实现细节对应用开发者透明，让我们能有空间进行持续迭代，同时也能降低开发者的学习和使用门槛。

cyfs@http协议会被DEC App开发者使用，因此起设计应是简洁易懂且长期稳定的。

# 命名对象格式设计
- TODO,编写一个简介。让阅读本文的读者能有一个基本的概念（保持每篇文章的基本独立性）


# GET Object(NamedObject) 获取命名对象
`cyfs://o[/$ownerid]/$objectid[/$innerpath][?mode=object&format=json]` (这种格式的URL为 ObjectLink URL,或简称为O Link)
获取指定的NamedObjecct，通过Http resp body返回。body的格式为指定格式。
在不传入mode的情况下，GET的行为会根据请求NamedObject的类型进行一些智能的处理.

## 智能处理
- GET FileObject时，并不会返回FileObject本身，而是在cyfs-runtime中完成File的第一个Chunk重建后，返回File Content,Range生效。
- GET DirObject/MapObject时，默认返回可操作的html结构

## 注意
- 在浏览器地址栏中输入o链接时，会根据已安装的DEC App，将打开某类NamedObject的行为转化为应用内页，比如
`cyfs://o[/$ownerid]/$objectid` 指向一个ArticleObject,如果用户安装的某个DEC App注册了ArticleObject的打开方式，那么可能会自动的转换成
`cyfs://a/$decid/show_article.html?owner=$ownerid&objid=$objid`。
如果指向的是一个AppObject,且本地并未安装该DEC App（该DEC APP是存在的）,那么浏览器会提示先安装DEC App。（按我们的要求，正式的DEC App都会在MetaChain上发布，所以很容易判断是否存在）。

- 注意失败扩散: 在DEC Client看来，`GET NamedObject是有多个可以并行的获取逻辑`的，DEC Service也有。如果DEC Client与OOD的连接良好，应让运行在OOD上的DEC Service进行足够的错误重试。只有在无法连接OOD时，DEC Client才会进行多路重试。这可以减少网络里错误重试的总量。

## Step1: CYFS DEC App(浏览器) <-> cyfs-runtime@local (cyfs@http)
这是最常见的请求，所以其接口逻辑为向local cyfs-runtime 平凡的发起一个HTTP GET请求。
按这个设计，当cyfs-runtime绑定本地的80端口时，如用户在HOST中把o配置为127.0.0.1（或cyfs-runtime绑定的本地virtual IP）,那么可以在传统浏览器中直接用
`http://o/$ownerid/$objectid` 打开。

Reqeust:
```
GET http://o/$ownerid/$objectid/$innerpath?mode=object&format=json HTTP/1.1
[cyfs-from:$deviceid] // 如果填写，说明App希望用指定身份发起请求
[cyfs-context-id:$contextid] //应用逻辑上处在同一个事件中的GET应尽量填写成一个id，能传递给底层以提高传输性能
[cyfs-decid:$decid] //发起请求的decid。
*Reference : $Reference //让传统浏览器自动填写contextid的方法。涉及到底层的两跳传输
*Range: $Range // 当$objectid为FileObjectId 或 ChunkId时，Range有效。
Accept: *
```

Reponse:
```
HTTP/1.1 200 OK //
cyfs-body-type:bin_object | json_object | file_content] //body中的内容类型
cyfs-object-id:$objectid 
cyfs-owner:$owner
[cyfs-remote:$deviceid] //返回结果的deviceid
[MIME:textobject] //MIME控制是否应在GET时传入？FileObject中应有可选字段来要求默认的MIME类型。
[Cache:] //Cache控制使用默认即可，对于有MutBody的对象，应设置为“无cache”。
```
Response Bodys:(根据cyfs-body-type有多种结果）
```
1.二进制的NamedObject
2.FileContent（ChunkData）
3.json的NamedObject
```

注意Response的错误码和行为应尽量与HTTP协议对齐，以让浏览器能有正确的默认反应。下面是cyfs-runtime的错误码
- 200 一切正常
- 404 对象未找到，无法打开



## Step2:cyfs-runtime <-> gateway (cyfs@BDT or BDT)
BDT协议目前对应用透明，所以我们保留了根据应用实践改进性能的机会。比如可以为NamedObject GET定制专门的BDT协议报文。使用HTTP@BDT Stream是目前最稳定的实现。

按上述设计，这一层建立好正确的BDT Stream后，只需原样转发HTTP请求即可。因为BDT自带身份，所以Req中的cyfs-from和Resp中的cyfs-remote字段以删除以减少流量占用。

## Step3:gateway <-> DEC Service (cyfs@http)
正常情况下，DEC Service不应该HANDLE NamedObject 的GET请求。gateway的默认行为会自动的进行NamedObject查找，并返回结果.
默认行为下，对GET NamedObject的权限控制思路为
- Zone内请求全放行，如果Zone内没有OOD会尝试去从其它地方获取。(`请求中的ownerid不一定要等于 OOD's Owner`)。
- Zone外请求，如果请求的Object在OOD上没有，则直接返回404。如果有，则判断该Object的Owner，如果Owner不是OOD's Owner,则返回。如果是，满足下面条件的请求放行：来源于“好友Zone”;NamedObject为Public;有效的ContextId（详见Context管理）

DEC Service可以按SDK里gateway部分的接口，设置GET NamedObject Handler。设置后的基本流程和nginx upstream类似，流程如下:
`gateway->data_firewall->dec_service->data_firewall->gateway`
gateway在把请求转发给DEC Service之前，以及DEC Service完成处理产生Response之后，都会经过数据防火墙的处理。
- 我们现在还未开放GET NamedObject Handler.

# GET Chunk(NamedData) 获取命名数据
`cyfs://o[/$ownerid]/$chunkid`
获取指定的Chunk。基本流程与GET NamedObject 
权限控制上，Chunk如能找到对应的NamedObject，则使用相同的权限控制。如无法找到，则走全通过的权限。
- 没有被引用的任何Chunk，被视作放在OOD的Cache里。为了提高CYFS网络中的数据可用性，Cache Data通常都是可以被访问的。

GET Chunk模式使用NDN模式进行传输。BDT为了提高NDN的传输效能，降低网络的整体负载，有专门设计的协议。这个协议使用明文方式发送Chunk，以方便网络上的路由节点对Chunk数据交换进行干预和优化。如应用需要以端到端加密方式传输Chunk，需要进行配置。

# GET ObjectList 批量获取对象
GET Object协议的设计简单明了，但其QA特性会导致当有大量Object需要获取时，需要消耗大量的冗余流量。因此我们设计了批量获取Object的协议来优化性能。

- 传入完整的ObjectMapId 
当GET Object指向一个ObjectMapId,且mode指定为object_package时，会返回一个ChunkId。ChunkId对应的是ObjectMap的全量打包文件。随后可以使用Get Chunk更高效的获得所有的Map中的Object.可以通过参数要求只返回ObjectMap的MetaData(key-objectid)列表。对于极其巨大的ObjectMap，可以指定层数来减少消耗。对单层特别大的ObjectMap，应从设计上通过其它接口（比如POST）通过查询缩小规模后再进行交换。
`cyfs://o[/$ownerid]/$obj_map_id?mode=chunk`
Request:

ResponseBody (json body):
```
{
    chunkId:xxxx
    key:xxxx //chunk用该key加密过了
}
```
- 传入旧Object Map Id和新Object Map Id,返回增量部分
这是一个类似Git Pull的同步场景，假设Client总是在本地跟踪OOD上的一个ObjectMap的版本，那么DEC Service也拥有Client本地保存的版本，此时可以由DEC Service计算增量部分来提高同步性能。详细细节可以展开，应用研发应充分利用绝大部分NamedObject的不可变特性来优化传输。


# GET RootState
`cyfs://r[/$ownerid]/$decid/$root_state_path[?mode=object&format=json]` (RootState Link URL,或简称R Link)
如果不填写ownerid则是向当前zone发起GET RootState
RootState总是指向一个NamedObject,其默认行为于直接GET对应的 Object Link一致。
当mode=objectid时，返回的body-type为空
Resp中会有RootState的当前版本号，可以基于版本号去拉当前版本的RootState
对一组RootState GET行为，可以在参数中传入一个共同的sessionid号，此时会基于相同版本的RootState来返回GET

## Step1: CYFS DEC App(浏览器) <-> cyfs-runtime@local (cyfs@http)
Reqeust:
```
GET http://r[/$ownerid]/$decid/$root_state_path[?mode=object&format=json] HTTP/1.1
[cyfs-from:$deviceid] // 如果填写，说明App希望用指定身份发起请求
[cyfs-context-id:$contextid] //应用逻辑上处在同一个事件中的GET应尽量填写成一个id，能传递给底层以提高传输性能
[cyfs-decid:$decid] //发起请求的decid。
*Reference : $Reference //让传统浏览器自动填写contextid的方法。涉及到底层的两跳传输
*Range: $Range // 当$objectid为FileObjectId 或 ChunkId时，Range有效。
Accept: *
```

Respone:
```
HTTP/1.1 200 OK //
cyfs-body-type:bin_object | json_object | file_content | none] //body中的内容类型
cyfs-object-id:$objectid 
cyfs-owner:$owner
cyfs-root-version:$version_id
[cyfs-remote:$deviceid] //返回结果的deviceid
[MIME:textobject] //MIME控制是否应在GET时传入？FileObject中应有可选字段来要求默认的MIME类型。
[Cache:] //Cache控制使用默认即可，对于有MutBody的对象，应设置为“无cache”。
```
### 确保访问同一个版本的RootState
- TODO：如果应用需要强保障，在一个页面中多个展示R URL的地方是基于同一个Root Version,如何设计可以方便应用使用。最简单的方法就是用cookie或参数。

### R URL转换成 O URL
基于Response里的信息，我们可以将一个可变的R Link固定(PIN)成O Link。
`cyfs://o/$cyfs-owner/$cyfs-object-id`
`cyfs://o/$cyfs-owner/$cyfs-root-version/$root_state_path`

## Step2 cyfs-runtime <-> gateway (cyfs@BDT)
建立好正确的BDT Stream后，原样转发HTTP请求到目标OOD的gateway。因为BDT自带身份，所以Req中的cyfs-from和Resp中的cyfs-remote字段以删除以减少流量占用。

## Step3:gateway <-> DEC Service (cyfs@http)
gateway的一般HANDLE逻辑如下：（防火墙->dec_service(upstream)）,对于GET RootState，应用如果不设置HANLDER，默认实现会根据防火墙配置返回响应的对象。大部分情况下，应用只需要做好RootState的权限设置：公开/好友可见/私有 即可，不必HANDLE GET RootState请求，但如果有复杂的应用权限控制要做

需要注意的是，用户的管理员权限拥有可以控制RootState的可访问性，及时应用通过HANDLE把一个特定的RootState设置为了公开，用户也可以通过防火墙配置阻止访问该RootState。

本质上说，GET RootState只需要返回ObjectID就足够了，然后client再根据需要使用 GET Object来得到进一步的信息。但在实际实现中，也通常会一步完成。
此时要注意防火墙对GET Object的配置，GET RootState的权限够，GET Object的权限没有，那么就只返回ObjectId.

- TODO： 默认权限表
- TODO： 得到授权的DEC Service Handle别的DEC Service的GET RootState请求。


# GET App 内页
`cyfs://a/$decid[/$dirid]/$inner_path[?parms]`（也许 `cyfs://$decid.a/$dirid]/$inner_path[?parms]` 会更好？）
在浏览器的地址栏输入DEC App内页会触发DEC App的安装（如果DEC App未安装），这导致分享DEC App内页会触发DEC App的安装。
DEC App安装的默认逻辑，会把当前版本(dirid)的所有资源保存到OOD上，所以同版本的DEC App的内页一定是完整的。DEC App是否自动升级完全由用户控制，升级后也会保存上一个版本的资源以方便回退。

DEC App的内页可以正确的decid身份使用cyfs-sdk，非DEC App内页的html中使用cyfs-sdk，将只能以guest decid的权限使用cyfs-sdk.有不少SDK的接口都需要decid身份。

# PUT Object to Cache
`cyfs://o/$ownerid/[$decid]` 
可面向所有的设备发起，基本语义是希望目标设备能保存该NamedObject一段时间（默认是60分钟），该对象可能会在后面的一些流程里被用到（通常是DEC过程）。通常PUT Object不填写目标$decid,因为使用O Link PUT Object的行为都是不会被Handle的（顺序无关性）。

PUT Object to Cache的行为有几个特点
- 幂等性：可以多次PUT同一个NamedObject，而不必担心副作用
- 可Cache性:PUT操作成功后，立刻GET也会必然成功。
- 顺序无关性:虽然Object之间存在Ref关系，但PUT时无所谓顺序。

PUT的协议流程
## Step1: CYFS DEC App(浏览器) <-> cyfs-runtime@local (cyfs@http)
## Step2 cyfs-runtime <-> gateway (cyfs@BDT)
## Step3:gateway <-> NOC / DEC Service  (cyfs@http)

# PUT Object to RootState
`cyfs://r/$ownerid/$decid[/$dirid]/$root_state_path` 
只能向OOD发起。基本语义是将希望Object保存在该root_state_path。如果PUT成功，那么通过对应的Root State Link可以GET回刚刚PUT的NamedObject.
从目前的实践来看，该语义并不常用。decid一般很难开放root_state的直接写入，都是通过RPC Call来事务性的改变dec service的RootState。

# PUT Chunk(NamedData)
`cyfs://o/$ownerid/[$decid]`
TODO：需要思考一下场景（目前看来似乎不需要PUT Chunk，Chunk都是主动拉取的）

# POST Call DEC
`cyfs://r/$ownerid/$decid/$dec_name?d1=objid1&d2=objid2`
要求触发执行一个DEC过程，该DEC过程一般会更新dec service的RootState。

有两种语义：
- 要求执行dec，执行成功后返回{R}。
- 要求验证dec, 验证成功后返回三元组{dec_name,{D},{R}}的签名。

Request
```
POST http://r/$ownerid/$decid/$dec_name?d1=objid1&d2=objid2 HTTP/1.1
[cyfs-from:$deviceid] // 如果填写，说明App希望用指定身份发起请求
[cyfs-decid:$decid] //发起请求的decid。
[cyfs-dec-action:exeucte | verify]
```
POST Call的Body中可以带一组package的named object。但大部分情况下，由DEC Service自行Prepare。


Response
```
HTTP/1.1 200 OK
cyfs-dec-state: complete | prepare | running | wait_verify| failed //本次dec是完成，准备中，正在工作，等待验证，失败
cyfs-dec-finish : $time //dec完成的时间（dec不会重复执行，如果之前已经完成过，会用之前的时间）
cyfs-prepare : objid1,objid2,objid3 ... // 如果处在准备状态的
```
Response Body：如果action是执行，则返回Result ObjectIds.如果是验证，验证通过返回对DEC 三元组的签名。

POST Call DEC的协议流程
## Step1: CYFS DEC App(浏览器) <-> cyfs-runtime@local (cyfs@http)
## Step2 cyfs-runtime <-> gateway (cyfs@BDT)
## Step3:gateway <-> DEC Service  (cyfs@http)
整个流程基本是HTTP POST Request和Response的原样转发,也是DEC Service会主要Handle的请求

## 通过一个例子来理解DEC共识的达成
1. Alice Call Bob's DEC
2. 该DEC的结果涉及到多个Owner，需要做验证
3. Bob先返回未验证的结果
4. Bob、Alice均可向其它Owner发起DEC 验证请求
5. Bob和其它Owner收集到了足够多的验证结果，确定DEC生效，推进后续的RootState改变流程（和Owner相关）。
6. 某个Owner数据丢失，发现没有完整的数据去验证DEC，会主动尝试重建{D}。
7. DEC共识的推进和区块链的BFT共识并不完全相同，实际上允许更多的并行共识推进。


# POST Call API
`cyfs://r/$ownerid/$decid[/$dirid]/$function_name?parm1=$parm1&parm2=$parm2`
一般用来做查询,不会修改dec_app的RootState。可以返回任意数据，与HTTP Post类似。太依赖这个调用会导致系统的单点故障。
DEC Service会主要Handle的一类请求。

# WS Event （TODO：需要设计）
- 通过RootStatePath订阅RootState改变
- 通过ObjectId订阅Object改变
- 通过EventName订阅Event改变（和旧时代一致）

# 一些旧设计的思考

## 广播
- 广播PUT
- 广播GET

## 对象路由
从现在的设计看，有点到点（来源相关）加密的逻辑请求其实没有特别强的路由特性。无非就是 Device->My OOD,Device->My OOD->Other OOD ,OOD->OOD,Device->Other OOD 这4种形态，似乎设计那么灵活的模式了。
而NDN为来源无关的明文不可修改协议，其有一定的路由特性。我们下一个阶段重点提升NDN时，可以在这一块为路由逻辑做更多的支持。


