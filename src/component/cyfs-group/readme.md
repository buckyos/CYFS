# 模块说明

支持群组产权数据相关需求

# 方案简介

1. 抛弃目前的 SimpleGroup 标准对象，提供一个可以包含多个 People 对象的 Group 标准对象，Group 对象可以支持动态配置相关属性（成员、权力等）
2. 提供 GroupState 结构，在 Group 的各成员 OOD 上保存其所属 Group 的 r-path 状态信息，类似个人产权的 RootState 设计，但更新机制不同，需要 Group 成员之间通过共识协议保持一致
3. 共识协议目前采用 BFT(HotStuff?)
4. 向 Group 发起的请求(Post/Get)，能自动寻址到其成员，并投递
5. 对从 Group 获取到的信息，Group 提供方法验证（主要是验证签名的过程）
6. 支持 cyfs://r/${groupid}/${decid}/${r-path}
7. 提供对 GroupState 的访问权限控制 ACL
8. 提供操作 Group 的权限配置 Group-ACL
