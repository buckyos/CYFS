![CYFS Logo](./doc/logos/CYFS_logo.png)
[![cyfs-lib-ver](https://img.shields.io/crates/v/cyfs-lib?label=cyfs-lib&style=plastic)](https://crates.io/crates/cyfs-lib)
![rust](https://img.shields.io/badge/rustc-1.63%2B-green?style=plastic)
![liscence](https://img.shields.io/crates/l/cyfs-base?style=plastic)
![Downloads](https://img.shields.io/crates/d/cyfs-base?style=plastic)

****CYFS：Next Generation Protocl Family to Build Web3****    
+ CYFS is short for CYberFileSystem
+ CYFS Website: [https://www.cyfs.com/](https://www.cyfs.com) ,[cyfs://cyfs/index](cyfs://cyfs/index)  
+ Discord：[https://discord.gg/dXDxjXTEjT](https://discord.gg/dXDxjXTEjT)   
+ CYFS White Paper(Coming soon)  
+ Language: [中文](./README.zh-CN.md)

# Introduce

CYFS is the next-generation technology to build real Web3 by upgrading the basic protocol of Web (TCP/IP+DNS+HTTP). It has a subversive architectural design that everyone brings  their own OOD (Owner Online Device) to form a truly decentralized network.
 
We define Web3 as follows:
- Web3 allows everyone to have rights to save data on the Internet, publish data, and get income from data.
- Web3's Internet applications are completely decentralized, developers have the right to publish and update applications, and users have the right to use them permanently.
- Web3's data has been determined ownership when generated, and use Trust Link to link together to form the 'data-ownership network', which is the future semantic web that allows data to show its true value.
 
After 6 years of research and development, we have achieved the above goals on the `CYFS testnet`. For the detailed design of the theory and practice of CYFS, please refer to the relevant documents. Open source of CYFS is an important milestone for CYFS, marking that its design has been basically stable and has entered an open community-driven R&D stage. We look forward to having more people join us. We need your passion and strength. Let’s build  Web3  that belongs to ourselves together.    

This is the repository of the CYFS protocol stack on Github. Although we have built a decentralized GitHub (CodeDAO) on the CYFS testnet to realize the `decentralized bootstrapping` of CYFS. But before the CYFS mainnet online, we still decided to stay a little longer in WEB2.0 and use this repository as the main community-driven R&D base. After the CYFS mainnet goes online, the entire Repo will move to CodeDAO.
 
This repository only contains the implementation of the CYFS core system. It does not include the important basic software of the CYFS ecosystem: CyberChat and CYFS Browser, nor does it include SDK implementations for developers of different languages. The following is a list of other important open source projects in the CYFS ecosystem (continuously updated)
 
- [cyfs-ts-sdk](https://github.com/buckyos/cyfs-ts-sdk): cyfs-sdk based on typescript to help typescript developers build Web3 DEC apps.
- CYFS Transmission Lab: A system for the network lab for testing the CYFS protocol (Coming soon)
- CyberChat: CYFS wallet, manage DID, OOD, DECApp and data assets (Coming soon)
- CYFS Browser: A browser based on the Chrome kernel that supports the cyfs:// protocol (Coming soon)
- CodeDAO: Decentralized Github with CYFS (Coming soon)

# Background
![CYFS Architecture](doc/zh-CN/image/CYFS%20Architecture.png)

The above is the overall architecture diagram of CYFS, which can help to establish a macro understanding of the design of CYFS. The core design of CYFS can be decomposed into the following parts

- Design of NamedObject and double-section construction of `cyfs://o/$ownerid/$objid` (Object Link).
- The BDT protocol improves TCP/IP: the network protocol evolves from address-oriented to trusted identity-oriented.
- MetaChain upgrades DNS, realizes decentralized addressing, and realizes Data is Account.
- The Owner deploys its own server OOD (Owner Online Device) on the network, which supports the decentralized accessibility of cyfs://
- Design proofs of storage and proofs of service based on the `game-based consensus` theory of `dispute before going on the chain`, and realize the decentralized high reliability and high availability of cyfs://
- The off-chain delivery of computing resources is separated from on-chain matching, and an evolvable economic model is used to build decentralized shared cloud computing
- Data property rights analysis based on the input and results of trusted computing, to realize the evolution from on-chain smart contracts to off-chain Data Exchage Contract (DEC).
- The Service of Web3 DEC App is installed on everyone's OOD, realizing the decentralization of application services

What key problems are these designs designed to solve? (We believe that "finding the right problem is half done~") You can read "[CYFS Architecture](doc/en/CYFS%20Architecture.pptx)" and "CYFS Whitepaper" (coming soon), the amount of content comparison Great, you can understand it with practice~

## Start your Web3 journey with “Hello CYFS”
We strongly recommend that you read the "Hello CYFS" series of articles in full to get a complete initial experience of CYFS (the whole process can be completed within 1 hour):
1. ["Compile and deploy DIY-OOD"](doc/en/Hello_CYFS/0.%20Compile%20and%20deploy%20DIY%20OOD.md)
2. ["Create Your DID"](doc/en/Hello_CYFS/1.%20Create%20your%20DID.md)
3. ["Publish File and Download"](doc/en/Hello_CYFS/2.%20Publish%20the%20file%20and%20download%20it.md)
4. ["Tips your friend"](doc/en/Hello_CYFS/3.%20Tips%20your%20friend.md)
5. ["Publish Website and View"](doc/en/Hello_CYFS/4.%20Publish%20the%20web3%20site%20and%20view%20it.md)

You can also use the following fast progress to have a first experience of "doing nothing without understand and save time"~

## Advance CYFS DEC App develpe tutorial is online
["Decentralized message board development"](doc/en/Decentralized_message_board_development/)
# Quick Start

The following will build your own Web3 environment by compiling the CYFS source code, and complete the basic experience of Web3 in this environment:
- Publish a folder to the Web3 world and get a permanent address of cyfs://o/$ownerid/$dirid/index.html which can be accessed by CYFS Browser.
- Use the `cyfs get` command to download the files in the folder you just published.

Note that this environment is normally not isolated, but can be added to the CYFS network (we have planned 3 networks: nightly, beta, relase, currently only nightly is available), so the following tutorial does not include compiling MetaChain with source code +SN part. If you plan to build a complete system in an independent environment and need to compile and deploy MetaChain super nodes, please read the document "MetaChain Construction" (Coming soon).

## Compile
Prepare an idle Linux host, preferably a VPS, with more than 2G of memory.
Preparation:
- Node.js 14 or above
- rustc 1.63 or above

Run the following command to compile the OOD System.

```shell
cd ${cyfs_sources_dir}/src
npm i
node ../scripts/build-standalone.js
````
During the compilation process of the script, the `${cyfs_root}` folder needs to be created. If the current system or current user does not have permission to create this folder, you need to manually create the `${cyfs_root}` folder in advance, and assign read and write permissions to the current user. (In the official environment, all components and data of the OOD System will be installed to `${cyfs_root}`)

`${cyfs_root}` path:
- Windows: `c:\cyfs`
- MacOS: `~/Library/cyfs`
- Other systems: `/cyfs`

Compile successfully generates `ood-installer`

## Install the newly compiled OOD System
Before installation, you need to prepare the depends:
- Node.js 14 and above
- MongoDB 4.4 version, configured to boot, use the default port, no authentication (using SQLite as the object storage engine can not rely on MongoDB, the subsequent installation script will support the selection of the storage engine)
- The latest version of docker-ce, configured to start at boot

Find the newly compiled ood-installer in the src directory and execute
````
./ood-installer --target solo
````
After a minintes , the installation is complete.

## Activate OOD
In order to experience the process, the cli tool is used here to complete the D.I.D creation process. **D.I.D created based on the cli tool can only be used for testing purposes! **

1. Install cyfs-tool: use the command line `npm i -g cyfs-tool-nightly` to install the nightly version of the cyfs-tool tool
2. Generating sets of identities
   > Use the command `cyfs desc -s <save_path>` to generate a matching identity file and save it in the save_path directory. If save_path is not specified, it defaults to ~/.cyfs_profile
3. Bind OOD
   > After the identity is generated, copy the two files `<save_path>/ood.desc` and `<save_path>/ood.sec` to `${cyfs_root}/etc/desc` on the OOD machine and rename it to `device. desc` and `device.sec`
4. Bind CYFS-Runtime
   > After the identity is generated, copy the two files `<save_path>/runtime.desc` and `<save_path>/runtime.sec` to `${cyfs_runtime_root}/etc/desc` on the CYFS browser machine and rename them to ` device.desc` and `device.sec`

`${cyfs_runtime_root}` specific path:
- Windows: `%appdata%/cyfs`
- Mac OS: `~/Library/Application Support/cyfs`

## Publish your first Web3 website

First prepare the `www` directory of your website, let's first experience publishing static websites to Web3, and follow-up documents for the construction of dynamic websites will be introduced.

use command
````
cyfs upload <dir_path> -e ood -t ood
````
Add the file pointed to by the local <dir_path> to the OOD.
The command execution is complete, the local `www` directory has been uploaded to OOD and the unique URL to the Web3.0 website has been generated (the end of the command execution).
The link is `cyfs O-Link`, which looks like this `cyfs://o/$ownerid/$objid`, where $objid is the ContentId of the directory.

## Browse the website just released
Use the command on any machine with cyfs-tool installed
````
cyfs get cyfs://o/$ownerid/$objid/index.html
````
You can download the just-released official website.

Any machine with a `cyfs browser` installed can use the cyfs browser to open `cyfs://o/$ownerid/$objid/index.html` and browse the website just released.
For the download of `cyfs browser`, see [here](./download.md)

# Code guide
Through the above process, you have a basic understanding of the design and use of CYFS. Although the design of CYFS is basically stable, we still have a lot of code to write. We are very eager for your help, but certainly not too much energy to write documentation (details are in the source code~). Here we do a minimalist code introduction, hoping to help you understand the implementation of CYFS faster.

According to the architecture, we know that the core of cyfs:// is the construction and acquisition of Object Linke, and the premise of acquisition is to upload data to OOD at least. The process is as follows:
1. Start the local protocol stack (cyfs-rutnime)
2. Calculate the hash of all files in the directory locally, and construct a FileObject with the current PeopleId as the Owner
3. Generate a Map structure (directory structure) locally, and construct a MapObject with the current PeopleId as the Owner,
   At this time, cyfs:// has been constructed, but the cyfs:// cannot be accessed at this time
4. Add the above named objects and named data to the local stack
5. Initiate CYFS PUT operation to OOD: save the MapObject to OOD and set the access permission to public
6. Let OOD start MapObject Prepare and save a named data on OOD
7. MapObject Prepare is completed on OOD, cyfs:// can be accessed

Then the process of using `cyfs get` to obtain is as follows:

1. Start the local protocol stack cyfs-runtime
2. Use the HTTP protocol to initiate an HTTP GET request to cyfs-runtime
3. cyfs-runtime checks whether the object exists in the local cache
4. cyfs-runtime initiates a NamedObject query request (the following behaviors are not serial)
    - 4.1 Query NamedObject from OOD
    - 4.2 OOD query local, whether NamedObject exists
    - 4.3 OOD queries MetaChain, whether NamedObject exists
    - 4.4 OOD queries whether the NamedObject exists on the previous hop device according to the Reference information in get
    - 4.5 OOD queries the configuration of Object's Owner Zone through MetaChain
    - 4.6 OOD is configured through Zone, connected to NamedObject's OOD, or connected to NamedObject' Cache, and query NamedObject
5. After getting the ChunkId, cyfs-runtime calls the Channel interface (NDN semantic interface) of the BDT to request the Chunk
    - 5.1 For the first, small Chunk, get it directly from the associated OOD
    - 5.2 For the second Chunk, it will try to get it from the previous hop (Reference OOD)
    - 5.3 BDT will try to perform multi-source search and multi-source download based on fountain code based on the context information of the application layer
    - 5.4 The router can identify the Chunk request packets sent by the BDT, intercept and forward them, and further optimize the overall load of the network
    - 5.5 Only OOD will upload Chunk
6. When the first Chunk of the FileObject is ready (passed authentication), the HTTP GET request in step 1 starts to return data

(The above process is shown in the figure below)
![get-from-cyfs](doc/en/image/get-from-cyfs.png)

> For the overall design of the CYFS protocol, please refer to [Introduction to CYFS Protocol.md](doc/en/Introduction%20to%20CYFS%20Protocol.md)

After understanding the logic of the above process, you can read the relevant code according to the following guidelines.

## Upload
1. Start the local protocol stack: [util.ts: create_stack()](https://github.com/buckyos/cyfs-ts-sdk/blob/master/src/tool/lib/util.ts)
2. Construct FileObject: [file_recorder.rs: FileRecorder.add_file()](src/component/cyfs-stack/src/trans_api/local/file_recorder.rs)
3. Construct ObjectMap: [publish_manager.rs: PublishLocalDirTask.publish()](src/component/cyfs-stack/src/trans_api/local/publish_manager.rs)
4. Add the above Named-Objects and Named-Data to the local protocol stack: [file_recorder.rs: FileRecorder.record_file_chunk_list()](src/component/cyfs-stack/src/trans_api/local/file_recorder.rs)
5. Initiate a CYFS PUT operation to the OOD: save the MapObject to the OOD and set the access permission to public: [upload.ts: upload_obj()](https://github.com/buckyos/cyfs-ts-sdk/blob/master/src/tool/actions/upload.ts)
6. Let OOD start MapObject Prepare and save Named-data(Chunks) on OOD: [upload.ts: run()](https://github.com/buckyos/cyfs-ts-sdk/blob/master/src/tool/actions/upload.ts)

## Get
1. Start the local protocol stack: [util.ts: create_stack()](https://github.com/buckyos/cyfs-ts-sdk/blob/master/src/tool/lib/util.ts)
2. Initiate an HTTP GET request using the HTTP protocol
3. cyfs-runtime checks whether the object exists in the local cache
4. cyfs-runtime initiates NamedObject query requirements (the following behaviors are usually not serial)
    - 4.1 Query NamedObject from OOD
    - 4.2 OOD query local, whether NamedObject exists
    - 4.3 OOD queries MetaChain, whether NamedObject exists
    - 4.4 OOD queries whether the NamedObject exists on the previous hop device according to the Reference information in GET
    - 4.5 OOD queries the configuration of Object's Owner Zone through MetaChain
    - 4.6 OOD is configured through Zone, connected to NamedObject's OOD, or connected to NamedObject' Cache, and query NamedObject
5. After get the ChunkId, cyfs-runtime calls the Channel interface (NDN semantic interface) of the BDT to request the Chunk
    - 5.1 For the first and small Chunk, get it directly from the associated OOD
    - 5.2 For the second Chunk, it will try to get it from the previous-jump (Reference OOD)
    - 5.3 BDT will try to perform multi-source search and multi-source download based on fountain code based on the context information of the application layer
    - 5.4 The router can identify the Chunk request packets sent by the BDT, intercept and forward them, and further optimize the overall load of the network
    - 5.5 Only OOD will upload Chunk
6. When the first chunk of the FileObject is ready and verified, the HTTP GET request in step 1 starts to return data

# DecApp development practice
The following series of articles describes how to implement a message board DecApp based on CYFS. this is a long series of articles detailing the general process of developing a DecApp
1. [Introduction](./doc/en/Decentralized_message_board_development/1.Introduction.md)
2. [DEC App basic principle](doc/en/Decentralized_message_board_development/2.DEC_App_basic_principle.md)
3. [Implement your own message service](doc/en/Decentralized_message_board_development/3.Implement_your_own_message_service.md)
4. [Run and debug your own message service](doc/en/Decentralized_message_board_development/4.Run_and_debug_your_own_message_service.md)
5. [Extended reading: Further explanation of the principles and introduction of some tools](doc/en/Decentralized_message_board_development/5.Extended_reading.md)
6. [Implement the front end](doc/en/Decentralized_message_board_development/6.Implement_the_front_end.md)
7. [Send it to your friends and understand the CYFS permission system](doc/en/Decentralized_message_board_development/7.Send_it_to_your_friends_and_understand_the_CYFS_permission_system.md)
8. [Use the emulator to test the app across zones](doc/en/Decentralized_message_board_development/8.Use_the_emulator_to_test_the_app_across%20zones.md)
9. [Extended reading: Decentralized, trusted data, semantic data, data property rights](doc/en/Decentralized_message_board_development/9.Extended_reading.md)
10. [Add comments](doc/en/Decentralized_message_board_development/10.Add_comments.md)
11. [DEC App data security and high availability](doc/en/Decentralized_message_board_development/11.DEC_App_data_security_and_high_availability.md)
12. [Extended reading: Learn more about data exchange contracts and DAOs](doc/en/Decentralized_message_board_development/12.Extended_reading.md)
13. [Summary](doc/en/Decentralized_message_board_development/13.%20Summary.md)

# Directory Structure
You have learned from the architecture diagram that the project implementation of CYFS is not small. To prevent you from getting lost, this chapter is a small map that will help you get a basic understanding of the CYFS code structure and understand where the key code will be.

- src //source directory, many development commands require this directory to be the current directory for operations
    - service // contains the code of a series of important executable files, which is a good starting point for reading the code
        - gateway //nginx of CYFS system, the most important basic service on OOD
        - cyfs-runtime //CYFS user mode protocol stack implementation, the most common process in development and debugging
        - ood-daemon //The basic daemon of OOD, keep alive and automatically update other basic services
        - app-manager //Installation manager of DEC App
        - chunk-manager // The most primitive and simple thinking and implementation of the concept of CYFS NDN, currently used for the lowest basic services
        - file-manager // The most primitive and simple thinking and implementation of the concept of CYFS NON, currently used for the lowest-level basic services
        - cyfs-runtime // The backend of the CYFS protocol stack used by the CYFS browser, based on the standard CYFS protocol stack, provides some functions that are convenient for the browser to use
    - component
        - cyfs-base //The basic component shared by all projects, here you can see the implementation details of NamedObject
        - cyfs-base-meta // MetaChain's base object definition
        - cyfs-bdt //The implementation of the BDT protocol, be careful of the huge scale when expanding
        - cyfs-core // Definition of core objects in the CYFS system
        - cyfs-chunk // CYFS protocol stack supports NDN protocol
        - cyfs-stack //The implementation of the CYFS protocol should be the largest component in the system
        - cyfs-meta-lib //Related implementation of MetaChain Client
        - cyfs-mobile-stack //Encapsulation of CYFS mobile protocol stack
        - cyfs-noc //NamedObject storage engine implementation (SQLite+MongoDB), which is the component that has the greatest impact on system IO performance
    - test //Test project, only includes the parts that have been sorted out, the code for a large number of network tests of CYFS is in the CYFS Transmission Lab.
    -tools //cyfs tools. The design idea of ​​cyfs toolchain is a top-level script and a series of independent gadgets, taking into account the consistency of cli experience and the independence of project construction
- scripts //Compile script directory, we use js script to replace makefile
- doc //document directory

# Roadmap
CYFS is currently in the state of dev_testnet (nightly), in this state, all components will be affected by the daily build results by default, and we do not consider how the data structure is backward compatible, and the protection of data security is not enough. whole. This version should only be used for development testing and should not be used in production production.

Our next goal is to complete the launch of the testnet (beta). Relatively nightly, beta will specify strict release standards, and will make the data structure as backward compatible as possible, and can confidently protect users' data security. The stability of beta will be able to support the release of beta products~ The main difference from the release version is assets is fake assets, and there are gaps in stability and performance.

We hope to launch the CYFS mainnet within 18 months. There is no definite launch time for the CYFS mainnet. As long as the asset security, stability and performance of the testnet reach the preset goals, we will enter the release channel of the mainnet.

Welcome to follow [#1 issue](https://github.com/buckyos/CYFS/issues/1) to learn more about CYFS Roadmap~

# Contributing
The dev_testnet of CYFS is designed and implemented by Shenzhen BuckyCloud. The core team of Shenzhen BuckyCloud is inherited from the original Xunlei(NASQ:XNET) infrastructure R&D team. We have rich experience in network protocol and infrastructure development, as well as super-large-scale P2P network design and tuning. experience. After CYFS is open source, BuckyCloud has completed its key historical mission. Now we prefer to call ourselves CYFS CoreDevTeam. In the future, it will be an open team organized based on DAO. The main responsibility is to promote the continuous research and development of CYFS. We welcome all engineers to join:

- If you have rich experience in network protocol development, at this moment we need you to help us improve the core protocol together
- If you are very familiar with open source culture and open source community, you can join decentralized github (CodeDAO) with us
- If you have rich experience in blockchain development, MetaChain, which is waiting to be rewritten at the moment, urgently needs you~ We already have an ambitious design, but over the years we have focused on off-chain protocol development, block-chain development resource is very limited
- If your main language does not include typescript and rust, you can help us build SDK for other languages
- If you are very good at the product, you can help us improve the basic experience of CYFS, or create a real Web3 Startup on CodeDAO
- Of course, everyone is welcome to submit bugs, suggestions for product improvement, and help us revise documents.... We guarantee that your contributions will be remembered

We are designing a DAO-NFT-based contribution record system. When this Repo is move to CodeDAO as a whole, CYFS DAO Tokens can be issued according to the share size of contributors (fairness is difficult)

# License
BSD-2-Clause License    
Copyright (c) 2022, CYFS Core Dev Team.


