# IM 核心对象简介

# 好友与好友列表
```
FrinedObject {
    
}
```
/system/friendlist/$peopleIdA -> FriendObjectForAlice
/system/friendlist/groupnameA/$peopleIdA -> FriendObjectForAlice 
/system/friendlist/groupnameB/ -> cyfs://r/$admin_zone/$appid/$group_x //引用外部组，软链接模式 


## DEC 添加好友
好友列表的数据产权完全属于用户
添加好友列表的可信过程，原理上是说明好友的相互信：A告诉C：“B的好友列表里有A”，A告诉C：“A的好友列表里有B”是没有意义的。但从实践上说，任何用户都没有展示自己好友列表的需求
A展示最近的一条B发来的信息，估计是最简单的实现上面目的的方法

/$root_state/friends/reqesut_add_friend
- friend_id
- group_name
- message

resp:
- 添加成功
- 等待验证
- 被拒绝 (有理由，可以再加)
- 被拉黑（以后都不要加了）


### Alice好友列表完全丢失的处理
1. Bob给Alice发消息前，需要Bob重新加Alice好友（如果Bob能进一步展示一条Alice曾经发过的消息，那么Alice会自动通过好友添加请求）
2. Alice添加Bob为好友的请求会直接返回成功


### 通过扩展实现自定义好友验证
运行应用处理添加好友的附加信息，实现“自定义问题的答案”

## 查找好友逻辑
输入信息，得到hash,在链上寻找挂了自己hash的人
hash的缺点是不能做模糊匹配。比如不能基于nickname的一部分去搜索。
update_userinfo_hash() //必须是唯一的
find_by_hash()

//-----------------------------------------------------------------
# MsgObject

```
MsgObject {
    from : ObjectId,
    to :ObjectId,
    author : ObjectId,
    text_content : String, //必须有短摘要，如果无法展示Content则展示text_content
    msg_type : MsgType,//消息的类型。普通，回复，引用，转发等待
    reply_msg : MsgObjectId,//引用的msgobject
    content : ObjectId,//可以以任意对象为Msg内容，content可以是另一个MsgObject里代表转发
}
```



# Session Object
也是一个不会传输出去的内部对象。用来记录一组MsgObject

```
SessionObject {

}
```

/sessions/$session_idA/
/sessions/$session_idB/
/sessions/$session_idC/

## Session DEC过程

发送消息 
```
function send_msg(msg:MessageObject) {
    http_post("/r/$to/im/")
}
```


# GroupObject
代表一个聊天组（包含多个成员）

