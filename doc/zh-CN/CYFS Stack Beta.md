# cyfs-base
一大堆无状态的计算组件，可以构造完整的测试
## 可扩展密码学算法
性能优化：能正确使用CPU指令优化(AES-NI)
### SHA256算法 ：SHA256 30年内会完蛋么？
### 签名结果带类型，与签名算法 OK 
### 密钥对与密钥对算法 OK 目前用secp256k1

### Base58编码与xxx编码（大小写不敏感编码）
### 密码学组件

## NamedObject 编码体系
- ObjId 是否需要修改？ObjId里编码信息，以对应极小的对象(id as value)
- ObjDesc with Common Headers OK， 
- ObjBody OK 
- ObjLinks 应用使用不够简单
- 签名列表

稳定编码与数据类型
编码后的大小限制（64KB or 128KB)
标准对象是没有可扩展性的？（版本升级之类）


### 标准对象 
- 常用对象 OK  
- 单签名有权对象 OK 
- 多签名有权对象(最复杂)
- 代理对象 
- 合约对象
- Context对象
- DID标准支持

### AppObject
在ObjDesc和ObjBody中使用ProtoBuf

### CoreObject
CoreOBject一般从App Object发展而来，目前没有CoreObject
广泛流行的App Object,会由委员会确定，编制成CoreObject，赋予Core Object Type Id
委员会会说明Core Object Id与AppObject的关系（如标准化后无需改动），以提升应用程序的兼容度。
委员会扩展CoreObject后，会更新表格 CoreObjectType <-> AppObject

### MutBody的使用
MutBody与UTXO 和MetaChain有关

## 构建文件系统:File,Map,Dir,Chunk
Dir相当于Zip,可以简化使用和实现
提升小文件的传输性能：Chunk合并 range@chunkid

### cyfs URL定义和解析器
对现在的实现进行一次check
主要是对name的解析
cyfs://lzc/www/index.html


# NOC 
## NOC中对MutBody进行跟踪
## ContextDB：让NDN工作的更好

# ChunkManager 
通过FileMap提升内存使用效率 OK 
Chunk相关Reader的正确使用,Chunk传输流程的端到端内存使用的精细优化
核心指标：ChunkManager的内存占用

# DEC RootSate (核心OOD组件)
## ObjectMap的实现 （Map,Set）, Array未支持
## Map的构造 （关键性能) 
## Soft Link 还有一点
## cyfs:// r link OK 
## 事务支持 OK 
## Diff （Map的增量改变）OK，需要在场景中验证
## GC支持 TODO
## 事件系统？ 设计讨论

# ACL-Rules 
提升使用体验和默认权限的正确逻辑
# Access-log TODO

# Gateway,RPC (cys-stack) 分层的Rview
一台OOD上运行多个协议栈

## NON
## NDN

# AppManager 
## docker套docker的问题 需要确认一些
## 浏览器里的沙盒问题

# Backup与Recover
基于MetaChain和DSG网络，完成OOD上所有数据在去中心网络中的备份和还原
增量备份

----------------------------------------------------------------------------
# 通过MetaChain保存对象
保存Object:ObjectId -> ObjectDesc with 最新的MutBody 
保存Path: $ownerid/$app_root_id/inner_path  或者 $ownerid/语义组件/inner_path -> objectId




