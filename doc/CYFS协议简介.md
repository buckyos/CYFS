# CYFS协议设计简介
CYFS 协议是对HTTP协议的整体升级，会尽量保持HTTP的近似语义。     
核心流程是：
> CYFS DEC App <--cyfs@http--> cyfs-rutnime <--cyfs@bdt--> gateway <--cyfs@http--> CYFS DEC Service    

实际在网络中运行的cyfs@bdt协议并不会被DEC App的客户端和服务器直接使用，这个设计让cyfs@bdt协议的实现细节对应用开发者透明，让我们能有空间进行持续迭代，同时也能降低开发者的学习和使用门槛。

cyfs@http协议会被DEC App开发者使用，因此起设计应是简洁易懂且长期稳定的。

# 命名对象格式设计
- TODO,编写一个简介。让阅读本文的读者能有一个基本的概念（保持每篇文章的基本独立性）


# GET Object(NamedObject) 获取命名对象
`cyfs://o[/$ownerid]/$objectid[/$innerpath][?mode=object&format=json]` (这种格式的URL为 ObjectLink URL)
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

# GET ObjectList 批量获取对象
1.传入完整的ObjectLIst
2.传入ObjectMapId
3.传入旧Object Map Id和新Object Map Id,返回增量部分


# GET RootState Link
`cyfs://r/$ownerid/$appid/$root_state_path[?mode=object&format=json]`
RootState总是指向一个NamedObject,其默认行为于直接GET对应的 Object Linke一致。
当mode=objectid时，返回的body-type为空
Resp中会有RootState的当前版本号，可以基于版本号去拉当前版本的RootState
对一组RootState GET行为，可以在参数中传入一个共同的sessionid号，此时会基于相同版本的RootState来返回GET

# GET App 内页
`cyfs://app/$decid[/$dirid]/$inner_path[?parms]`
也许 `cyfs://$decid.a/$dirid]/$inner_path[?parms]` 会更好？

# PUT NamedObject
`cyfs://o/$ownerid/$appid`
`cyfs://r/$ownerid/$appid/$root_state_path`

基本语义时希望ownerid能保存该NamedObject
如果PUT成功，那么通过对应的 Object Link或Root State Link可以GET回刚刚PUT的NamedObject.

# PUT ChunkData
`cyfs://o/$ownerid/$appid`


# POST (CALL) API(DEC)

# WS Event
- 订阅RootState改变
- 订阅Object改变

# 一些旧设计的思考

## 广播
- 广播PUT
- 广播GET

## 对象路由


