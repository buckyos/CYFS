![CYFS Logo](./doc/logos/CYFS_logo.png)

***CYFS：Next Generation Protocl Family to Build Web3***    
+ CYFS Website: [https://www.cyfs.com/](https://www.cyfs.com) ,[cyfs://cyfs/index_en.html](cyfs://cyfs/index_en.html)  
+ Discord：[https://discord.gg/dXDxjXTEjT](https://discord.gg/dXDxjXTEjT)   
+ CYFS White Paper(Coming soon)  

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

# Quick Start

The following will build your own Web3 environment by compiling the CYFS source code, and complete the basic experience of Web3 in this environment:
- Publish a folder to the Web3 world and get a permanent address of cyfs://o/$ownerid/$dirid/index.html which can be accessed by CYFS Browser.
- Use the `cyfs get` command to download the files in the folder you just published.

Note that this environment is normally not isolated, but can be added to the CYFS network (we have planned 3 networks: nightly, beta, relase, currently only nightly is available), so the following tutorial does not include compiling MetaChain with source code +SN part. If you plan to build a complete system in an independent environment and need to compile and deploy MetaChain super nodes, please read the document "MetaChain Construction" (Coming soon).

## Compile
This basic experience requires the following independent components, all of which are open source and can be compiled independently. You can also mix the official version with your own compiled version
- OOD: You can refer to the article [Hello CYFS 0: Compile and deploy DIYOOD](doc/zh-CN/Hello_CYFS/0. Compile and deploy DIYOOD.md), compile and deploy DIYOOD from source code
- Over-delivery: used to manage user identities, bind your own OOD and CYFS Browser, and can currently be downloaded from the official website
- CYFS Browser: Access the website published by yourself or others through the cyfs link, which can currently be downloaded from the official website
- CYFS Tool: Node.js-based command line tool that provides upload and get commands. Currently installable via `npm i -g cyfs-tool-nightly`

## Use the component just compiled
On a machine with CYFS browser installed and activated, install CYFS Tool:

Use the command `cyfs upload <file_path> -t ood` to upload the file pointed to by the local <file_path> to OOD.

Use the command `cyfs get <object link> -s <save_path>` to download the file to the local through the cyfs-runtime protocol stack

For more specific instructions and the meaning of the parameters, you can refer to the article [Hello CYFS 2: Release files and download](doc/zh-CN/Hello_CYFS/2.%E5%8F%91%E5%B8%83%E6%96 %87%E4%BB%B6%E5%B9%B6%E4%B8%8B%E8%BD%BD.md)

Publish a static website by publishing a folder using the command `cyfs upload <folder_path> -t ood`. This command outputs a cyfs link. Fill in the link in the address bar of the CYFS browser, you can view the website you just published through the CYFS browser

For more specific instructions, you can refer to the article [Hello CYFS 3: Publish the website and view](doc/zh-CN/Hello_CYFS/3.%E5%8F%91%E5%B8%83%E7%BD%91%E7% AB%99%E5%B9%B6%E6%9F%A5%E7%9C%8B.md)

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
    4.1 Query NamedObject from OOD
    4.2 OOD query local, whether NamedObject exists
    4.3 OOD queries MetaChain, whether NamedObject exists
    4.4 OOD queries whether the NamedObject exists on the previous hop device according to the Reference information in get
    4.5 OOD queries the configuration of Object's Owner Zone through MetaChain
    4.6 OOD is configured through Zone, connected to NamedObject's OOD, or connected to NamedObject' Cache, and query NamedObject
5. After getting the ChunkId, cyfs-runtime calls the Channel interface (NDN semantic interface) of the BDT to request the Chunk
    5.1 For the first, small Chunk, get it directly from the associated OOD
    5.2 For the second Chunk, it will try to get it from the previous hop (Reference OOD)
    5.3 BDT will try to perform multi-source search and multi-source download based on fountain code based on the context information of the application layer
    5.4 The router can identify the Chunk request packets sent by the BDT, intercept and forward them, and further optimize the overall load of the network
    5.5 Only OOD will upload Chunk
6. When the first Chunk of the FileObject is ready (passed authentication), the HTTP GET request in step 1 starts to return data

(The above process is shown in the figure below)
![get-from-cyfs](doc/en/image/get-from-cyfs.png)

> For the overall design of the CYFS protocol, please refer to [Introduction to CYFS Protocol.md](doc/en/Introduction%20to%20CYFS%20Protocol.md)

After understanding the logic of the above process, you can read the relevant code according to the following guidelines.

## Upload
1. Start the local protocol stack: [cyfs-ts-sdk/src/tool/lib/util.ts:304](https://github.com/buckyos/cyfs-ts-sdk/blob/master/src/tool /lib/util.ts#L304)
2. Construct FileObject: [file_recorder.rs:46](src/component/cyfs-stack/src/trans_api/local/file_recorder.rs#L46)
3. Construct ObjectMap: [publish_manager.rs:223](src/component/cyfs-stack/src/trans_api/local/publish_manager.rs#L223)
4. Add the above named objects and named data to the local protocol stack: [file_recorder.rs:257](src/component/cyfs-stack/src/trans_api/local/file_recorder.rs#L257)
5. Initiate a CYFS PUT operation to the OOD: save the MapObject to the OOD and set the access permission to public: [cyfs-ts-sdk/src/tool/actions/upload.ts:35](https://github.com /buckyos/cyfs-ts-sdk/blob/master/src/tool/actions/upload.ts#L35)
6. Let OOD start MapObject Prepare and save a named data on OOD: [cyfs-ts-sdk/src/tool/actions/upload.ts:170](https://github.com/buckyos/cyfs-ts -sdk/blob/master/src/tool/actions/upload.ts#L170)

## Get
1. Start the local protocol stack: [cyfs-ts-sdk/src/tool/lib/util.ts:304](https://github.com/buckyos/cyfs-ts-sdk/blob/master/src/tool /lib/util.ts#L304)
2. Initiate an HTTP GET request using the HTTP protocol
3. cyfs-runtime checks whether the object exists in the local cache
4. cyfs-runtime initiates NamedObject query requirements (the following behaviors are usually not serial)
    4.1 Query NamedObject from OOD
    4.2 OOD query local, whether NamedObject exists
    4.3 OOD queries MetaChain, whether NamedObject exists
    4.4 OOD queries whether the NamedObject exists on the previous hop device according to the Reference information in get
    4.5 OOD queries the configuration of Object's Owner Zone through MetaChain
    4.6 OOD is configured through Zone, connected to NamedObject's OOD, or connected to NamedObject' Cache, and query NamedObject
5. After getting the ChunkId, cyfs-runtime calls the Channel interface (NDN semantic interface) of the BDT to request the Chunk
    5.1 For the first, small Chunk, get it directly from the associated OOD
    5.2 For the second Chunk, it will try to get it from the previous hop (Reference OOD)
    5.3 BDT will try to perform multi-source search and multi-source download based on fountain code based on the context information of the application layer
    5.4 The router can identify the Chunk request packets sent by the BDT, intercept and forward them, and further optimize the overall load of the network
    5.5 Only OOD will upload Chunk
6. When the first chunk of the FileObject is ready and verified, the HTTP GET request in step 1 starts to return data

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


