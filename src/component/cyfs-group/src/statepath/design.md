```
/--groups // for manager；通过决议以后构造/更新的Group对象放在这里，更新步骤
|   |   // 1.得到一个创建/更新一个Group的决议（旧成员一定量的投票+所有新成员签名）
|   |   //      形成决议的方式可以是合约，也可以是超送用`DEC框架`实现的DEC
|   |   // 2.跟friend管理一样，用决议设定到系统更新Group信息
|   |   // 3.更新Group下所有r-path的本地Group版本，并达成共识；这里主要要同步ood-list
|   |--list-->Set<GroupId>
|   |--option-->GroupOption
|
|--${group-id}
|   |--${dec-id}
|       |--.dec-state-->ObjectId // for dec；各Group的dec状态放这里
|       |   // APP控制的实体状态，通常是个map-id
|       |   // 最终在APP看到的${r-path}结构是这级物理结构的相对路径
|       |   // 其他内部逻辑隐藏掉
|       |   // 每个${r-path}管理范围内是串行的
|       |   // 不同${r-path}范围内的操作是并行的
|       |   // 且不同${r-path}之间是并列的，不能嵌套
|       |--.link // 区块链结构，记录状态变更链条
|           |--${r-path}
|               |--group-blob-->BLOB(Group)
|               |--users
|               |   |--${user-id}
|               |       |--xxx
|               |--last-vote-round-->u64 // 最后一次投票的 轮次
|               |--last-qc-->GroupQuorumCertificate // 最后一次被确认的共识证明
|               |
|               |--range-->(${first_height}, ${header_height}) // 保留的历史block序列号区间
|               |--str(${height})->block
|               |
|               |--prepares // Prepare状态的block
|               |   |--${block.id}
|               |       |--block
|               |       |--result-state-->ObjectId(result-state)
|               |--pre-commits // pre-commit状态的block
|               |   |--${block.id}
|               |       |--block
|               |       |--result-state-->ObjectId(result-state)
|               |
|               |--finish-proposals
|               |   |--flip-time-->Timestamp // 取block时间戳
|               |   |--over-->Set<ObjectId>
|               |   |--adding-->Set<ObjectId>
```

```
// .group结构
/--${group-id}
    |--ObjectId(.group)
        |--.update
            |--voting
            |   |--${proposal-id}
            |       |--proposal-->GroupUpdateProposal
            |       |--decides-->Set<decide-proposal>
            |--latest-version-->GroupUpdateProposal // Chunk(Encode(group))
            |--str(version-seq)-->GroupUpdateProposal // Chunk(Encode(group))
            |--str(group-hash)-->GroupUpdateProposal
```

member 同步结构

```
|--${/} // config by the DecAPP
    |--${group-id}
        |--${r-path}
            |--state-->ObjectId // the latest state
            |--block-->Block // the hightest block
            |--qc-->qc-block // the qc for the ${block}
```
