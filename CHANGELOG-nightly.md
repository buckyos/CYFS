## [1.0.0.718] -- 2023/2/21
1b0cf118 Change sync services packages from parallel to serial
638848a2 Remove two spawn task using for cancel on timeout
70837da7 Fix error op_env type check in requestor
e3f55f6e Improve timeout and retry strategy for repo download in ood-daemon service
9c3c63ab Meta db initialization switch to transaction mode
1e4e4277 Modify: add some statistic logs for ndn
94d8f7a4 Merge branch '104-appmanager-suppport-local-app-repo-store'
34580770 Modify: add some statistic logs for ndn
7522c0fe Email Sender support starttls
10d27023 App Manager support install app from local repo
2a3c1dc1 ood-installer support sync app repo
51f0be28 WIP: App Manager Support Local Repo
e6392f5b WIP： App Manager reflator app install logic
e65fe061 WIP： AppManager add repo type, use zip_extract to extract app service package

## [1.0.0.715] -- 2023/2/20
e51af4f2 Set docker api timeout to 300s
b213c3fe Fix: tcp not update active time when recv packet
24553a9c Improve the object layer strategy detection of after pass the first layer rpath access verification
574cfddf OOD-daemon will now report an error and exit if it fails to load the repo
0ab51327 Add version output in process alive check log
14c004b6 Fix meta-stat email format
4daab080 App-manager report app status to ood-daemon
c09ddea9 Fix field names of ood status
f3449363 Fix: monitor create desc when desc not existed on chain
a4e0e755 Fix: stream break when loss fin ack packet
0d2393f3 Fix: monitor create desc error
ba302dd4 Merge remote-tracking branch 'origin/107-support-multiple-global-states-in-cyfs-stack' into main
3d70797d Add group state test case
63e3e712 Fix a assert error when global state is init
61a5eff1 Cyfs monitor use generated identify to test metachain read and write
3d4aefc9 Improve the use of u64::MAX in two places
ee5323ea Delete some useless logs
90d9b5c8 meta spv support storage anbd query objects by object id, body hash
1101829c Add isolate_id method for GlobalStateRawProcessor
e3765b61 Add global state raw processor for use by external components of cyfs-stack
ff03c824 Add the persistence mechanism of all the global state list
b4c4de04 Refactor GlobalStateManager to support multiple global-states
937c7dbe Merge branch '79-cyfs-stack-should-support-interfaces-for-ndn-upload-task'
b4284830 Add exists methods into collection for to check if exists
5bcc2984 Merge branch '79-cyfs-stack-should-support-interfaces-for-ndn-upload-task'
da09ed21 Merge branch '79-cyfs-stack-should-support-interfaces-for-ndn-upload-task'
412d03ff Merge branch '99-recycle-unused-bdt-tunnel-instances'
979773da Fix: implement stream reserving 2*msl state with lru cache
25c53b07 Refactory: export private methods with pub(crate)
114f1d1b Fix: bdt unit test sn with ipv6 endpoints
fdde1948 Merge branch '95-ipv6-support-refactory'
1b91764a Merge remote-tracking branch 'origin/101-ood-status-query-an-aggregation-services' into main
8d522ec7 Merge remote-tracking branch 'origin/96-singleopenvstub-support-constructing-directory-tree-structure-and-support-directory-tree-traversal' into main
1875de3d Add object and name for meta in cyfs-stack
3e12b1be Fix cyfs-perf deps Improve the format of some tomls
e83890fc Add noc meta cache to optimize update last access write operation
2ff85f4d Publish cyfs-core new version
d917d648 Bump cyfs-core version
0dd49104 Fix cyfs-core deps
220c7ab2 Fix compile error
dd2f2f17 Publish new cargo packages
fb63c476 Bump cargo package version and dependences
89917227 Merge branch 'main' into beta
75aea584 Merge remote-tracking branch 'origin/101-ood-status-query-an-aggregation-services' into main
8f60a00b Merge remote-tracking branch 'origin/96-singleopenvstub-support-constructing-directory-tree-structure-and-support-directory-tree-traversal' into main
2ebd1e20 Add object and name for meta in cyfs-stack
4d5a9e90 Fix cyfs-perf deps Improve the format of some tomls
2121a11e Add noc meta cache to optimize update last access write operation
078b27d3 Publish cyfs-core new version
019b09d9 Bump cyfs-core version
51e4365b Fix cyfs-core deps
a04d6e88 Fix compile error
038b75b4 Publish new cargo packages
279ce4bd Bump cargo package version and dependences
026c1383 Merge branch 'main' into beta
ccb044db Integrate ood-daemon services status
4af47a92 Improve ServiceState fields spelling
f9eaed01 Add service status related def
ec1ba569 ood-daemon adds independent status http server
5bc02595 Add service status support to ood-daemon
ff14b0d1 Add external http server support to ood control
287c9468 Fix app-manager compile error
08249fa5 Cyfs Monitor restore case interval time after success
c6c372dc Add noc test cases
b176d6a5 Fix the reserved names as blob and chunk dir names's bug on windows for the missing part
97bd7bb9 Print warning when decapp missing status or stop scripts
9f85c599 Stream shutdown success at closed state
71ffede0 Fix: fix reset key logic
ec2194ec Integrate ood-daemon services status
2b30769b Improve ServiceState fields spelling
eb881d46 Add service status related def
5dc9c661 ood-daemon adds independent status http server
b2882ff5 Add service status support to ood-daemon
cfc87989 Add external http server support to ood control
d54866c2 Fix app-manager compile error
f0822e79 Cyfs Monitor restore case interval time after success
ab37ea21 Add noc test cases
fa1cc5c0 Fix the reserved names as blob and chunk dir names's bug on windows for the missing part
13cdfe46 Print warning when decapp missing status or stop scripts
5ad5d183 Fix: bdt unit test sn with ipv6 endpoints
9607470d Merge branch '95-ipv6-support-refactory'
a8ba267a Merge remote-tracking branch 'origin/101-ood-status-query-an-aggregation-services' into main
5c2a8d77 Merge remote-tracking branch 'origin/96-singleopenvstub-support-constructing-directory-tree-structure-and-support-directory-tree-traversal' into main
c88a5e76 Add object and name for meta in cyfs-stack
598df4a8 Fix cyfs-perf deps Improve the format of some tomls
1952c7ee Add noc meta cache to optimize update last access write operation
ba181cef Publish cyfs-core new version
139a9ab6 Bump cyfs-core version
11c7205a Fix cyfs-core deps
76f871ea Fix compile error
30f99d3d Publish new cargo packages
4228e263 Bump cargo package version and dependences
34cb38f4 Merge branch 'main' into beta
87956987 Integrate ood-daemon services status
086dcd8a Improve ServiceState fields spelling
51f135c6 Add service status related def
0bd37768 ood-daemon adds independent status http server
933ed8cd Add isolate_path_env stub test cases
1be055d0 Add service status support to ood-daemon
d3e16c4d Add native ioslate_path_env component test cases and fix some test warnings
25fa3e85 Add external http server support to ood control
60b04789 Add ioslate path env stub
4b32f2b0 Optimize objectmap path to support non-transactional mode
5a6aa9b7 Integrate isolate path env into global-state services
6ccd4ef9 Add isolate path env impl
c78ae028 Fix app-manager compile error
429bddd8 Cyfs Monitor restore case interval time after success
9dd5aac3 Add noc test cases
c7132f26 Fix the reserved names as blob and chunk dir names's bug on windows for the missing part
cbf80ee3 Print warning when decapp missing status or stop scripts
2451a583 Fix: package enter write closed state correctly
0d4443ff Panic manager supports multiple bug reporters
f6749bb1 Stream shutdown success at closed state
f1a4cc60 Create a directory if it does not exist when downloading to a local file
28a37154 Improve cyfs-stack's control strategy for task manager resume_task
a63d6854 Fix: fix reset key logic
b7a3e3c9 Add dingtalk_bug_report method to PanicBuilder for external use
445fc501 Optimize some logs output
cc70a84b Add init-ood-daemon param to ood-installer
28fbed65 Remove cyfs-bdt's dependence on os-id lib
18d1534d Improve the ndn request logs output
8538be10 Add req_path suport to front o protocol for acl
1a621713 Send start notify when cyfs-monitor started
e8752b2a Improve the problem that the front r protocol cannot correctly handle data req_path for acl
4abdcf6b Use search instead of get for device in context
c8ad90a8 Add target device prompt info to access reject msg for better debug
4cc08abb Improve cyfs stack test cases
9cf2aafe Improve some logs output about chunk cache
b30d8cd6 Fix: chunk list task download split chunks dumplicatedly
c2570da0 Improve the problem that the backslash path in Windows format cannot be correctly recognized when the download task processes the local file path on nix system
b7ecdbc7 Fix: using channel got reserved correctly
a8a9a3b9 Fix: cyfs-monitor report, use sn-online-test to test sn server status
c3964a38 impl: log udp socket's recv loop thread os id
cb239140 Improve some logs output
67b2aed5 Add seperate sn-online-test program
b6e22b0f Fix: cyfs stack resp interest with NotFound if chunk not exists on default
e1bbe10d Fix: Add some log for dectecting zero speed source
d3535025 Fix: modify resend piece control send time
04629f0f Fix: channel ignore estimate request for sequence lost
11bfb5a5 Correct the use of sync mechanism on bdt download context
d2d9ff8f Bdt acl callback can correctly handle empty referer now
a2ebb0df Changed to auto cancel strategy for ndn task with context param
d653c967 Fix the bug that the device_list param of trans.create_task cannot correctly find the device object
a2cb418a Remove useless ood_resolver field from ndn service relate codes
236abdbb Merge remote-tracking branch 'origin/101-ood-status-query-an-aggregation-services' into main
cb0cba8a Merge remote-tracking branch 'origin/96-singleopenvstub-support-constructing-directory-tree-structure-and-support-directory-tree-traversal' into main
dce764a9 Add object and name for meta in cyfs-stack
e0dac397 Fix: tcp stream don't reset tunnel when break
5af0b5fb Fix: implement stream reserving 2*msl state with lru cache
3da42793 Fix cyfs-perf deps Improve the format of some tomls
9867ad29 Add noc meta cache to optimize update last access write operation
e11ff3ef Publish cyfs-core new version
95a5c99d Bump cyfs-core version
c168c17a Fix cyfs-core deps
427050c6 Fix compile error
2412d747 Integrate ood-daemon services status
aa5ff976 Publish new cargo packages
0249ba77 Bump cargo package version and dependences
96e3bb69 Merge branch 'main' into beta
0493c935 Improve ServiceState fields spelling
5a88c564 Add service status related def
00bf2764 ood-daemon adds independent status http server
c013612e Fix: bdt mem device cache limited size
39de9529 Refactory: export private methods with pub(crate)
7785029d Fix: package enter write closed state correctly
101db063 Stream shutdown success at closed state
3fca43d8 Create a directory if it does not exist when downloading to a local file
ac0e4f2a Improve cyfs-stack's control strategy for task manager resume_task
4d8bfacc Fix: fix reset key logic
96e1c1e6 Panic manager supports multiple bug reporters
8d9b7929 Add dingtalk_bug_report method to PanicBuilder for external use
7825a4ea Optimize some logs output
fa6867c6 Add init-ood-daemon param to ood-installer
9aa4489c Remove cyfs-bdt's dependence on os-id lib
66b856fc Improve the ndn request logs output
c0622388 Add req_path suport to front o protocol for acl
cc24f8e0 Send start notify when cyfs-monitor started
10ace371 Improve the problem that the front r protocol cannot correctly handle data req_path for acl
6c59337d Use search instead of get for device in context
ef0f880d Add target device prompt info to access reject msg for better debug
beb0e7be Add service status support to ood-daemon
edc1d1f6 Add external http server support to ood control
e5e9a3a5 Refactory: stream release tunnel guard when closed
009c3c8a Refactory: export private methods with pub(crate)
8e21764a Refactory: export private methods with pub(crate)
743c2fcf Impl: recycle unused tunnel
b4eb9f85 Add isolate_path_env stub test cases
b0c6fd63 Add native ioslate_path_env component test cases and fix some test warnings
3f14f978 Impl: add a test for sn ipv6 ping
ff916188 Add ioslate path env stub
dd437a1f Impl: sn support ipv6 ping
b69709df Optimize objectmap path to support non-transactional mode
0de3fc0c Integrate isolate path env into global-state services
edacc65a Add isolate path env impl
29fed496 Remove complie warning
f3f66501 Impl: Add transfered and recursion close method to ndn task
027e9ae4 Impl: Add transfered and recursion close method to ndn task
6a1f7ed1 Rebase: rebase to 74-integrate-bdt-task-context-and-group
aa682520 Refactory: impl trans api for close upload group
260cbdf8 Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
c2bd5612 Refactory: impl trans api for close upload group
99bbebfc Refactory: impl trans api for upload group
c19064b5 Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
2f47bcc0 Fix: reserve or recycle channels when all reference released
ae3f02eb Impl: cancel task with user defined error
82692c64 Fix: download task progress/speed
bc3ba88e Rebase: rebase to 74-integrate-bdt-task-context-and-group
58d6d195 Refactory: impl trans api for close upload group
5dad1539 Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
30aa07ae Fix: reserve or recycle channels when all reference released
acaa42fb Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
09076f70 Add uptime and boot_time into get_system_info
2b81e62a Fix: correctly wait dapp process
3bbbcfc3 Add total trans bytes field in get_system_info Added network card deduplication in some environments
80fa0863 Publish new cargo packages
ae40d694 Fix cyfs-util publish error
ee957a29 Bump cargo packages version
89341b52 Fix: retry sn list when offline
1370e83c Improve ndn test case with get_data
3584395e Improve the resp's return logic of get_data with error prompt
a3a5172e Use of http requestor for get_data and put_data methods as default
0347e50e Error of reading resp is returned correctly in ws mode
87d26ad6 Merge remote-tracking branch 'origin/main' into main
0df70f89 Sync projects for trans and stack object relate apis's changes
2ef0d178 Fix a compile error with requestor config and some warnings
4b716f44 Merge remote-tracking branch 'origin/80-add-surf-based-http-requestor' into main
1a15a767 Fix: trigger chunk downloader's on_drain method
e9faea63 Add some logs
a8dedd60 Add serialization support for storage base on global state
6effd696 Add log for named-data-client chunk reading
9edf7c31 Merge branch '74-integrate-bdt-task-context-and-group'
3fcdbf0c Improve ood-daemon's upgrade detection and restart logic
e0323ea1 Add host mode support to browser sanbox
91e519ec Also check app exclude list when install or start app
66acc69e Merge pull request #88 from buckyos/fix-container-dns
f1a63712 Ood-installer support only start ood-daemon
6514acec Fix AppManager startup param
ee80ce8c Fix AppManager config path, load config after log init
cb2ac1ee Mixed the usage of different requestor config for zone simulator stacks
be10f64b Add shared stack cache logic to zone simulator
5da765af Remove shared stack support and cache from cyfs-stack-loader
c2708f59 Improve open shared stack relate logic and add requestor config params
6150617a Temporarily disable all ipv6 addresses for bdt stack
95654ae5 App Manager AppSource logic
e849439c AppManager supports more detailed configuration
715e851e WIP： add new config for app manager
2ded38db Modify the way to start file uploader
0a50ed5f Improve the update state relate logic for service in ood-daemon service
c7de6539 Fix: avoid duplicate hole punching process when retry sn call
e828e7b5 Merge remote-tracking branch 'origin/fix-container-dns'
f8f4381b Add --startup-mode for ood-daemon startup script
c7bf12f2 Add startup-mode to ood-daemon service with network verify on system startup
b9aca01d Add active check in daemon update and state check loop
dc6aa3d9 Add timeout for repo in ood-daemon service
20ec54d1 Start the monitor service synchronously on startup
2e66ff4a Add service_list cache mechanism for meta config repo
d758151b Remove the local status of meta config repo in ood-daemon service
f99caadc Fix: retry sn list when builder not establish in time
b143379f Integrate surf requestor into shared stack
7eed812f App-tool and app-tool-ex return error correctly
09f79e9a Remove redundant service requestors constructor
cd8588de Merge remote-tracking branch 'origin/75-add-de-initialization-mechanism-for-sharedobjectstack' into main
f5873108 Add surf based requestor
889d7f81 Adjust the file directory structure of requestsors in cyfs-lib
e7e9781e Fix some compile errors and warnings
3aa50d32 Remove the useless base project
4de7e33d Add app-manager.toml config file support to ood-installer tool
9317c01d Remove the useless old configuration of acl.toml
5af28985 Improve the error status of build relate tasks
a762fecf Merge branch 'fix-container-dns' into beta
b3231559 Fix compile
832e7a20 Fix container dns: when resolv contain 127.0.0.53, mount the systemd resolv file instead
b2471088 Fix container dns, when resolv contain 127.0.0.53
bde55b1a Merge branch 'main' into beta
4dd330fe Merge branch 'main' into beta
35cabb3f Merge branch 'main' into beta
bd9326c9 File upload tool only support windows & mac for now.
01a46434 Open upload tool with "open" cmd on Mac
b45d377f Fix：ping client doesn't update local device
b36e728e Fix：ping client doesn't update local device
d9bd6698 Remove handler and event manager's none mode
980b5392 Add some field compatibility with the old version's data on new db format
6541571a Fix: pannic on time escape of call session
c0ceda78 Improve ws session and manager stop-related timing logic
f30cc9c8 Fix: some warning with while let
473e56d6 Add delayed start mechanism for router handler and events
74e30500 Fix: downloading speed/progress for chunk_list/file task use percent
172c5c9e Add shared stack stop test case
d670b458 Improve ws and event manager stop strategy
5841a4fb Improve ws session and ws client stop strategy
b62c204a Refactor ws session related logic and improve packet parser logic
706f06e6 Add stop method to shared stack with requestors
b45ddcf1 Enable none mode for handler and event system in shared stack
a1d67489 Optimize the use of requestor by the service component and adopt the shared mode
6acc40fa Remove http handler support for shared stack
5c3abc42 Add stop method to handler and event manager
b24935d7 Add stop method for ws client
6f55517f Publish new cargo packages
8c933ffc Fix cyfs-util publish error
bf75d29e Bump cargo packages version
bce2a745 Merge branch 'main' into beta
acbd636a Refactory: Add track chunk method option to publish dir/file methods
a022c2cc Fix the bug that search context will enter an infinite loop
a8a57c27 Improve the result of router get_object with flash flags is true
9f3d4e8d Fix the bug for decode context path param without fix
4236102e Add router support for group relate methods Add target check for publish_file method
3d758fcd Merge branch 'main' into beta
0843bbba Fix container dns: when resolv contain 127.0.0.53, mount the systemd resolv file instead (#87)
387362a2 Rebase: rebase to 74-integrate-bdt-task-context-and-group
8bdf227b Refactory: impl trans api for close upload group
18ed9311 Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
3f52efd9 Impl: cancel task with user defined error
9627d57a Fix: download task progress/speed
e0292e40 Fix: download task progress/speed
addd1976 Refactory: impl trans api for close upload group
dcb4395e Refactory: impl trans api for upload group
db6750fd Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
9f10434a Fix: reserve or recycle channels when all reference released
3ffb8c7e Refactor the file task and chunk task and use a consistent impls at the lower layer
603b5377 Impl: cancel task with user defined error
e66ab510 Fix: download task progress/speed
538a7111 Rebase: rebase to 74-integrate-bdt-task-context-and-group
f720dac7 Refactory: impl trans api for close upload group
a3d86db0 Refactory: impl trans api for upload group
31a93c96 Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
f2287d89 Fix query_tasks error and add some logs for task
8ed9f7f2 Sync context manager modifications with bdt
7d6ceaa8 Refactor the context state session manager with source and chunk as unique index
50fb0345 Refactor the download task to better support state switching and task release
a9fe0654 Fix the read logic error for SharedMemChunk
2f89e98d Refactory: change context update_at to async function
f9bbc140 Refactory: chunk downloader triggers context's on_drain when all existing sources tried
4643e4d5 Refactory: add update at property for download context
da234ea1 Fix: reserve or recycle channels when all reference released
a63f74ea Router handler support empty string as filter and will treat as none
5475581f Improve the local file & chunk writers logs
3758c0a0 Refactor the file task and chunk task and use a consistent impls at the lower layer
680f18e2 Add chunk type support for verify_file_task
f237b25b Add state manager with context for ndn task to cancel task on source error
d46a8abb Refactory: add method on_drain for download context
b88929fd Impl: cancel task with user defined error
81c297f0 Sync test cases with interface modification
9ead4762 Add access support for build_dir_from_object_map source object's access
078811fa Add get_object_map_ex with access field return for object_map relate cache
2e1d1924 Add access param support for publish_file and build_file relate methods
eb62b863 Add access support to put_object_map relate methods
dd1caba5 Add some logs for task manager
2b843a47 Fix: download task progress/speed
db13d228 Improve some logs output
f00b77c2 Add from_str support for access string
7c92e607 Add alias for trans output requests
c08c3e36 Improve the log and error codes for local file relate codes
7821234f Use insert_chunk instead of update_chunk_state for chunk state updater
91f0177a Improve insert_chunk logic with update state when already exists for ndc module
ddd8f8e6 Improve some logs output for ndn relate codes
4f637a9d Refactory: interest reponse upload field tokens
1c12c499 Fix: download task finish directly when total length is zero
a43ef9db Fix: download task progress/speed
e94e9560 Fix: download task progress/speed
307c10b5 Refactory: impl trans api for close upload group
a9071c13 Refactory: impl trans api for upload group
763791ba Refactory: upload handler with group_path filter; upload handler response with upload from path
96603e3c Refactory: impl download task progress
191497e7 Fix: finish download leaf task
311aba18 Refactory: upload task group cancel/close
38d8d707 Refactory: Add logic leaf download task
f0141ccc Refactory: chunk downloader send interest with group_path field
6291029a Refactory: chunk downloader not mergable
1684b91f Fix: pannic on udp call
0cf29489 Merge branch 'main' into bdt-beta
5bd73033 Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
7805227c Merge branch '74-integrate-bdt-task-context-and-group' into bdt-beta
a4dbe34f Merge branch '56-cyfs-stack-support-no-sn-online' into bdt-beta
8611caff Fix: add log for ping clients
71af78d7 Merge branch '56-cyfs-stack-support-no-sn-online' into bdt-beta
7a1d4b82 Merge branch '56-cyfs-stack-support-no-sn-online' into bdt-beta
5540f89e Fix: the status of udp stream
e5307fac Fix: cc loss bytes in stream write
229dbc6a Add reset_sn_list() function in Stack's ineterface.
b469a288 Upload get-nearest SN Server in client call. The build-tunnel-params's remote-sn is priority over get-nearest sn-list.
a564d9ee Merge branch 'cyfs-bdt-task-group-impl' into bdt-beta

## [1.0.0.713] -- 2023/2/11
ab8a2a6c Fix app-manager compile error
cd7eb6b4 Cyfs Monitor restore case interval time after success
7a03fdb3 Add noc test cases
bb596b85 Fix the reserved names as blob and chunk dir names's bug on windows for the missing part
20d14f19 Print warning when decapp missing status or stop scripts

## [1.0.0.711] -- 2023/2/11
2c798622 Fix: package enter write closed state correctly
be780df7 Stream shutdown success at closed state
21aef793 Create a directory if it does not exist when downloading to a local file
b6b7e7f8 Improve cyfs-stack's control strategy for task manager resume_task
c0f775cc Fix: fix reset key logic
c90f6ae9 Panic manager supports multiple bug reporters
07f2717d Add dingtalk_bug_report method to PanicBuilder for external use
db4acc4b Optimize some logs output
fb13f654 Add init-ood-daemon param to ood-installer
8f8e88c2 Remove cyfs-bdt's dependence on os-id lib
c05e88b7 Improve the ndn request logs output
7e0c03b0 Add req_path suport to front o protocol for acl
e3d042a0 Send start notify when cyfs-monitor started
2b3a433e Improve the problem that the front r protocol cannot correctly handle data req_path for acl
11e6ed60 Use search instead of get for device in context
06498f24 Add target device prompt info to access reject msg for better debug
7c753b20 Improve cyfs stack test cases
851f1a04 Improve some logs output about chunk cache
7cce9079 Fix: chunk list task download split chunks dumplicatedly

## [1.0.0.709] -- 2023/2/7
3005944e Improve the problem that the backslash path in Windows format cannot be correctly recognized when the download task processes the local file path on nix system
0a004af9 Fix: using channel got reserved correctly
1bfe7b55 Fix: cyfs-monitor report, use sn-online-test to test sn server status
8aa6a426 impl: log udp socket's recv loop thread os id
a1d30562 Improve some logs output
4bf5e29c Add seperate sn-online-test program
82e71892 Fix: cyfs stack resp interest with NotFound if chunk not exists on default
7af6ce69 Fix: Add some log for dectecting zero speed source
d593800f Fix: modify resend piece control send time
b5c41727 Fix: channel ignore estimate request for sequence lost
21a3cb11 Correct the use of sync mechanism on bdt download context
42b7029e Bdt acl callback can correctly handle empty referer now
52709bc8 Changed to auto cancel strategy for ndn task with context param
acde7c48 Remove complie warning
6834b67a Fix the bug that the device_list param of trans.create_task cannot correctly find the device object
4c3370f4 Remove useless ood_resolver field from ndn service relate codes

## [1.0.0.708] -- 2023/2/6
4e1122df Fix: the status of udp stream
58c545d7 Improve the handling of non.get_object when the target object cannot be found in (dir or objectmap) + inner_path mode
78904db7 Switch cyfs-ndc and cyfs-tracker-cache db to use SqliteConnectionHolder
932f1c61 Add SqliteConnectionHolder for general db connection management Switch cyfs-noc db to use SqliteConnectionHolder
5f1db008 impl: add a download context strategy to cancel source when zero speed
044bc59c Fix: ndn timer triggered correctly
d6bccc0f The read and write mode of Meta db is separated to support read-only in special cases such as disk is full etc

## [1.0.0.707] -- 2023/2/3
0ac77054 Fix the coding stability problem of Dir about HashMap
85cdd9cb Change some Mutex to cyfs-debug's checked Mutex in ood-daemon
5a448ccd Fix the deadlock bug that may be caused by service state detection
24177f29 Add test cases for config repo in ood-daemon project
8c7465ea Fix the coding stability problem of AppList and AppLocalStatus about HashMap
47d820ae Add sort by name strategy for device-config.toml
df39550f Fix the init error of trans store when the db dir is missing
bad1a48d Add inner path support for SingleOpEnvStub
cec8822c Add inner_path support for single_op_env.load method
26c68baa Fix some warning and add some comments
dc3b6e74 Update cyfs-bdt-ext deps
e977dcc9 Move LocalDeviceManager from cyfs-stack-loader to cyfs-util Remove the dependency on cyfs-stack-loader of ood-daemon and ood-installer
b09291cb Add stop_all support to stop all the services for ood-daemon
e031b4b5 Remove useless codes in cyfs-stack project
efb2431c Merge remote-tracking branch 'origin/92-more-convenient-and-suitable-to-use-cyfs-bdt' into main
170eedcf Move group and data relate codes to cyfs-bdt-ext project
259c46c4 Switched cyfs-stack to rely on cyfs-bdt-ext
965803d5 Move relate components to cyfs-bdt-ext project
00f5b35b Add cyfs-bdt-ext project
50036402 Add uptime and boot_time into get_system_info
a166c72d Fix: correctly wait dapp process
20ddf1d0 Add total trans bytes field in get_system_info Added network card deduplication in some environments
ec08c584 Publish new cargo packages
b6db6f9b Fix cyfs-util publish error
cd204231 Bump cargo packages version
cd120a75 Fix: retry sn list when offline
338c31b8 Improve ndn test case with get_data
9ee1a6d4 Improve the resp's return logic of get_data with error prompt
87b0693e Use of http requestor for get_data and put_data methods as default
6f4d47a0 Error of reading resp is returned correctly in ws mode
114c040a Merge remote-tracking branch 'origin/main' into main
a5c5da0e Sync projects for trans and stack object relate apis's changes
b51738f3 Fix a compile error with requestor config and some warnings
d7950257 Fix: trigger chunk downloader's on_drain method
95abf250 Merge remote-tracking branch 'origin/80-add-surf-based-http-requestor' into main
fd22bdec Add some logs
4a459fd3 Add serialization support for storage base on global state
068cde9a Add log for named-data-client chunk reading
cd0f3c5f Fix a compile error
ab7cf9e1 Merge branch '74-integrate-bdt-task-context-and-group'
63a37de6 Improve ood-daemon's upgrade detection and restart logic
6850633e Add host mode support to browser sanbox
d4d2639d Improve the error status of build relate tasks
7aa93795 Fix the bug that search context will enter an infinite loop
3ff1412f Improve the result of router get_object with flash flags is true
1ff3ff2e Fix the bug for decode context path param without fix
6bd563c9 Integrate surf requestor into shared stack
dc6d5ebb Remove redundant service requestors constructor
066ebd0e Add surf based requestor
066fe787 Also check app exclude list when install or start app
32b62e13 Adjust the file directory structure of requestsors in cyfs-lib
4ed6ddd2 Merge pull request #88 from buckyos/fix-container-dns
7404ab2e Merge branch 'fix-container-dns' into beta
93c35af4 Add router support for group relate methods Add target check for publish_file method
647b53ba Fix query_tasks error and add some logs for task
37c9bd9c Sync context manager modifications with bdt
78dd780c Refactor the context state session manager with source and chunk as unique index
f05bcd8b Refactor the download task to better support state switching and task release
c028b5bc Fix the read logic error for SharedMemChunk
f50bf84b Refactory: change context update_at to async function
7a2582f6 Refactory: chunk downloader triggers context's on_drain when all existing sources tried
475de4ad Refactory: add update at property for download context
d0200673 Fix: reserve or recycle channels when all reference released
eda71af7 Ood-installer support only start ood-daemon
a491c000 Fix AppManager startup param
d2fa72e0 Fix AppManager config path, load config after log init
409c38f9 Router handler support empty string as filter and will treat as none
51d8dc8f Mixed the usage of different requestor config for zone simulator stacks
70e1e833 Add shared stack cache logic to zone simulator
37cae961 Remove shared stack support and cache from cyfs-stack-loader
eb77a092 Improve open shared stack relate logic and add requestor config params
00b8cf65 Improve the local file & chunk writers logs
e0f14387 Refactor the file task and chunk task and use a consistent impls at the lower layer
e138e06c Add chunk type support for verify_file_task
3fb583e0 Add state manager with context for ndn task to cancel task on source error
200c8922 Refactory: add method on_drain for download context
72e2bb30 Impl: cancel task with user defined error
a0413319 Sync test cases with interface modification
473334df Add access support for build_dir_from_object_map source object's access
973718a1 Add get_object_map_ex with access field return for object_map relate cache
4a547ca3 Add access param support for publish_file and build_file relate methods
78d8f93e Add access support to put_object_map relate methods
750e65d5 Add some logs for task manager
58d90843 Fix container dns: when resolv contain 127.0.0.53, mount the systemd resolv file instead
b398e932 Fix: download task finish directly when total length is zero
2b415e37 Fix: download task progress/speed
54965e6c Improve some logs output
f62cb4df Refactory: impl download task progress
bbec1935 Fix: finish download leaf task
fd230411 Refactory: upload task group cancel/close
4c2f95b4 Refactory: Add logic leaf download task
eab160ec Refactory: chunk downloader send interest with group_path field
50db1d5c Refactory: chunk downloader not mergable
062bf19e Add from_str support for access string
2243fa7b Add alias for trans output requests
ebb67790 Improve the log and error codes for local file relate codes
ff7dd4c1 Use insert_chunk instead of update_chunk_state for chunk state updater
0525944c Improve insert_chunk logic with update state when already exists for ndc module
fc2d8b18 Improve some logs output for ndn relate codes
128a7cc5 Fix: some warning with while let
f1890ba3 Add chunk exists check for ndn get_data cache mechanism
18c94cf2 Improve some error process in std::io poll relate methods
06f03a6a Improve the conversion between BuckyError and std::io::Error
e801b468 Improve put_chunk for local chunk cache
23a61570 Add cache mechanism to ndn get_data
9055ec35 Refactory: add group_path field to interest packet
1093046b Add range support for ndn get_data with chunk
e3797e31 Add group test case
5061b962 Refactory: poll split read
e700a047 Use {dec_id}/ as default group when no group param is specified
14339d1a Remove dec_id field of context object Add global context support for context
6922dfa0 Put_context is adjusted to depend on context manager Add access field for put_context request
968e5e6d Fix context path fix error & add test case
b70f7990 Fix error when context manager parse context string with object_id
de18c885 Add ndn context test cases Fix compile errors of trans relate methods
8687e171 Fix ndn referer param encode error
be8f06b3 Add debug output for changed of context
abcc02d8 Remove context_id field for query tasks request
21421faf Fix: build error for cyfs-stack
4f62dbad Fix: build error for cyfs-stack
6d6ccad3 Improve resp process for trans & sync api methods
3eb6ca28 Refactor put_context and get_context methods for trans
5ad7e7f1 Improve the clone_from_slice method for ObjectId
9ece347b Refactor context support for trans
7340db07 Refactor ndn api target related codes to support context
458ad1f6 Refactor context to support target mode
9ea97744 Add context field for ndn get_data request
4a0aa1b6 Adapt to bdt new context relate params and types
b51eed19 Add cache and path search category to context manager
f0427489 Add dec_id field for context object, and add strict limit to context_path field
3441fdb0 Refactory: rename ChunkEncodeDesc => ChunkCodecDesc; declare context.source_of as async function;
04db0d46 Add context holder generator for context manager
6081a426 Add context manager and holder core impls
23eb4af0 Refactor the TransContext core object
73d7dcb1 Add dec limit for trans task group relate methods
abda8cfc Improve the param of TransOutputProcessor methods
a3678c4e Improve the trans.query_tasks request path
0fb35dbb Add task group relate methods for trans api
c78b3568 Add a common impl of serde code for JsonCodec
70366e60 Refactor get_task_state method's response to add group support
2d335eff Add group field for DownloadTaskState
ba606857 Add task group for ndn get_data resp
bbb16276 Improve the url query parse for referer and group params
5fae309c Add task group support for NDN/Trans modules
71414249 Rename the task range reader
a5107424 Optimize the ChunkWriter write method
b5ba81fb DirLoader switch to reply on chunk_store_reader instead of local_data_manager
89df6fcc NDN local_data_manager's get relate impls switch to reply on target_data_manager instead
bdfabe75 Fix: split read test
12610350 Fix: cyfs stack ndn event apply to new bdt ndn interface
816e7f6b Refactor the param model of ndc/tracker/chunk_manager
a397bd52 Adapt to bdt's new download stream model
2d5eb444 Impl: split read for download task reader
6cd9410e Impl: Add Single source context
95c8554a Merge branch 'cyfs-bdt-task-group-impl' into 74-integrate-bdt-task-context-and-group
5a1f23af Merge branch 'cyfs-bdt-task-group-impl' into 74-integrate-bdt-task-context-and-group
6111f439 Fix: Release chunk cache and downloader with weak reference
ffc56abe Refactory：create seperate downloader when context is not mergable
1ff8388e Refactory： add loading state to chunk cache
8e3590a2 Refactory: split chunk downloader from chunk cache
f2a3672f Fix: call cc on_loss with error loss count value
1dd8bae3 Fix: Donwload task path logic
4f3a92c4 Fix: return download task speed
371792c0 Fix: use BuckyError instead of BuckyErrorCode in some state
167664c9 Fix: add chunk_list_writer method to TrackedChunkStore
b4a67578 Merge branch 'cyfs-bdt-task-group-impl' into bdt-beta
2e42dae0 Fix: stream chunk encoder always read cache with sync reader
bb954d5f Impl: Unit test for upload from path
8b7aa861 Refactory: Add start_upload_from_cache method
81014478 Refactory: Move ndn default event handler to utils
def00192 Refactory: remove default ndc/tracker from bdt stack
67f29271 Refactory: Taskgroup makesure path
896b39ba Impl: File raw cache for chunk
bc043adc In SN statistic, add endpoint's statistic.
ecb58708 Call sn support MTU_LARGE
2cf4567b Add Mutlity SN。
2321e02c Fix: tcp tunnel send piece buffer with mtu
2ac27f33 Refactory: add DownloadContext trait
6832e73e Refactory: add DownloadContext trait
330a8940 Support large mtu and question&answer
e03962a7 Merge remote-tracking branch 'origin/bdt-beta' into bdt-beta
d1cc885e Merge remote-tracking branch 'origin/bdt-beta' into bdt-beta
06d89662 Fix: chunk list task reader
f4d20813 Fix: close/finish download task group
5bd2ec64 Fix: task.reader return unexpected not found error
7d2e6f6f Fix:panic on download reader
de837c47 Refactory: write chunk from task.reader
677e444b Refactory: Reader of download task
c0a479e3 Refactory: Upload session wait establish
22e9cc0f Refactory: download/upload differs in tcp/udp tunnel
638a8fc4 Refactory: download/upload differs in tcp/udp tunnel
23251581 Fix: bind tunnel with upload/download session
70292ce4 Fix： cache packages in tunnel.build_send, send them when tunnel actived
197e241a Merge branch 'ndn-cache' into bdt-beta
51d5b714 Merge branch 'ndn-cache' into bdt-beta
ea1dae46 Merge remote-tracking branch 'origin/bdt-beta' into bdt-beta
16d25537 Merge branch 'ndn-cache' into bdt-beta
c6bf684a log decrypt aeskey error info
79bceeea Fix: chunk list task reader
bc380433 Fix: close/finish download task group
61714a7b Fix: task.reader return unexpected not found error
97c955d7 Fix:panic on download reader
1e95e4da Refactory: write chunk from task.reader
9ad964fb Refactory: Reader of download task
d8bf33a0 Refactory: Upload session wait establish
06f3affc Refactory: download/upload differs in tcp/udp tunnel
2ef44f99 Refactory: download/upload differs in tcp/udp tunnel
8519534c Fix: bind tunnel with upload/download session
b2925e95 Fix： cache packages in tunnel.build_send, send them when tunnel actived
7235f3dc Merge branch 'ndn-cache' into bdt-beta
04e2a5a0 Merge branch 'ndn-cache' into bdt-beta
eaa0b650 Merge remote-tracking branch 'origin/bdt-beta' into bdt-beta
3d711110 Merge branch 'ndn-cache' into bdt-beta
8c8b6039 log decrypt aeskey error info
43ccf15c Impl: Cancel download chunk task
6efb6eca Test: download from uploader test ok
439635f5 Merge branch 'ndn-cache' into bdt-beta
0e08b03a Refactory: download/upload session support reverse step stream
96ffaef7 Refactory: impl download/upload session with new cache
292cf4ae Refactory: impl download/upload session with new cache
d38e31ba Refactory: impl download/upload session with new cache
07b40c65 Refactory: impl download/upload session with new cache
d525a649 Refactory: implement memory raw cache
e07a4b0e Refactory: chunk downloader with stream cache

## [1.0.0.703] -- 2023/1/13
cfd654ac Temporarily disable all ipv6 addresses for bdt stack

## [1.0.0.702] -- 2023/1/12
bb72348e App Manager AppSource logic
413edbad AppManager supports more detailed configuration
7b0704ec WIP： add new config for app manager
2eb6dce1 Modify the way to start file uploader
6076a69f Improve the update state relate logic for service in ood-daemon service
44fbe781 Fix: avoid duplicate hole punching process when retry sn call
ea48e120 Merge remote-tracking branch 'origin/fix-container-dns'
56f22e21 Add --startup-mode for ood-daemon startup script
cf1f363c Add startup-mode to ood-daemon service with network verify on system startup
56b755e6 Add active check in daemon update and state check loop
b7943988 Fix compile
2b355960 Fix container dns, when resolv contain 127.0.0.53
d6b5bb06 Add timeout for repo in ood-daemon service
628649f0 Start the monitor service synchronously on startup
de1e9960 Add service_list cache mechanism for meta config repo
a84bfafe Remove the local status of meta config repo in ood-daemon service
a807f171 Fix: retry sn list when builder not establish in time
5639e004 App-tool and app-tool-ex return error correctly
26b234cb Merge remote-tracking branch 'origin/75-add-de-initialization-mechanism-for-sharedobjectstack' into main
f136c262 Fix some compile errors and warnings
a4ece723 Remove the useless base project
2d216910 Add app-manager.toml config file support to ood-installer tool
e111f3b2 Remove the useless old configuration of acl.toml
eae9d604 Merge branch 'main' into beta
d5463188 Merge branch 'main' into beta
b34744c3 Merge branch 'main' into beta
180ae196 Remove handler and event manager's none mode
d10953b2 Improve ws session and manager stop-related timing logic
8b84ef3f Add delayed start mechanism for router handler and events
c13cc029 Add shared stack stop test case
faf04a09 Improve ws and event manager stop strategy
c5d02ab9 Improve ws session and ws client stop strategy
007247e2 Refactor ws session related logic and improve packet parser logic
b0463c83 Add stop method to shared stack with requestors
1b80f43a Enable none mode for handler and event system in shared stack
8ec0888b Optimize the use of requestor by the service component and adopt the shared mode
d9d69051 Remove http handler support for shared stack
bb0e005e Add stop method to handler and event manager
1a2ada09 Add stop method for ws client
77ea26c6 Fix： cyfs-client correct get sn list from local stack
e29cd15c Merge branch 'main' into beta
0bb5dcd7 Sync service list method use the lock for services in ood-daemon
19b338e6 Improve some logs output
7e8cb267 Improve cmd status check return value and compare mechanism
fc800657 Improve the agent check for the browser disable mode
67a2179e NamedDataClient use init config, use GetData instead of GetDataWithMeta
e160a62a Improve the get_data logic of chunk_manager
9f7263bc Add memory cache for chunk_manager get_data_with_meta Improve the get_chunk_data resp body mechanism and remove the chunk_id verify temporarily
270778a9 Fix: stream pool use sn list in device cache
b1abf319 Fix: ignore sn list in build param's remote desc
ff05f5ff Fix: beta use correct built-in sn list
02fe10ef Fix: Add logs for build params info
26989db3 Merge branch 'main' into beta
440b90f9 Merge branch 'main' into beta
c377c61f Merge branch 'main' into beta
b0fe871d Merge branch 'main' into beta
ef8a124a Merge branch 'main' into beta
71e9e52b Merge branch 'main' into beta
2fa6be68 Fix: disable cyfs_debug::mutex check in some fraquently called functions
