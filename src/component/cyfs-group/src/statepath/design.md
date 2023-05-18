```
|--${group-id} // one group
|   |--${DecId("shells", ${group-id})}
|   |   |--.shells
|   |       |--.latest-->GroupShell // latest version
|   |       |--${group.version}-->GroupShell // add shells for history versions of group
|   |--${dec-id}  // one dec for a group
|       |--${r-path}
|           |--.dec-state-->ObjectId // for dec；the latest state of all groups
|           |   // one state of a r-path, It's calculated by the app, and it's a map-id in most times
|           |   // Each state change for same ${r-path} is serial
|           |   // Each state change for different ${r-path} is parallel
|           |   // **The process of state change for different ${r-path} should always not be nested, Because the change of each branch will affect the state of the root**
|           |--.link // Blockchain for hotstuff, record the state change chain,Most of the time, application developers do not need to pay attention
|               |--group-blob-->BLOB(Group) // the latest group, it's store as chunk, so, it'll not be updated by different version
|               |--users // info of any user, is useful?
|               |   |--${user-id}
|               |       |--xxx
|               |--last-vote-round-->u64 // the round that I voted last time
|               |--last-qc-->GroupQuorumCertificate
|               |
|               |--range-->(${first_height}, ${header_height}) // the range retained, we can remove some history
|               |--str(${height})->block // commited blocks with any height, QC(Quorum Certificate) by next blocks at least 2
|               |
|               |--prepares // prepare blocks, with QC for pre-block(pre-commit/commited), but not QC by any one
|               |   |--${block.id}
|               |         |--block
|               |         |--result-state-->ObjectId(result-state) // hold the ref to avoid recycle
|               |--pre-commits // pre-commit blocks, with QC for the header block, and is QC by a prepare block
|               |   |--${block.id}
|               |         |--block
|               |         |--result-state-->ObjectId(result-state) // hold the ref to avoid recycle
|               |
|               |--finish-proposals // The proposal is de-duplicated. Proposals that exceed the timeout period are directly discarded, and those within the timeout period are de-duplicated by the list
|               |   |--flip-time-->Timestamp // the timestamp of the first block
|               |   |--recycle-->Set<ObjectId>
|               |   |--adding-->Set<ObjectId>
```
