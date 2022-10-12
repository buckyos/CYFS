# 说明

本教程基于 cyfs-dapp-cli 脚手架创建的模板项目进行讲解，使用模拟器环境进行开发。

# 准备条件

1. 请确保你已经通过 Cyber Chat 导出自己的身份到电脑上。

- mac 系统，在 ~/.cyfs_profile 目录下分别有 poeple.desc 和 people.sec 文件

- windows 系统，在 C:\Users\<your-conputer-name>\.cyfs_profile 目录下分别有 poeple.desc 和 people.sec 文件

2. 请确保已经在系统中安装好 nodejs 最新稳定版。
3. 全局安装 cyfs-sdk、cyfs-tool 和 cyfs-dapp-cli 脚手架。
   **注意** 如果使用的是 nightly 版本，请安装 cyfs-sdk-nightly、cyfs-tool-nightly
4. 在你觉得合适的文件夹下，使用脚手架创建 dec app 工程模板，命令如下：

```shell
npm i -g cyfs-sdk cyfs-tool cyfs-dapp-cli
cyfs-dapp-cli create hello-demo
cd hello-demo
npm i --force
```

---

_cyfs-dapp-cli 使用注意_

如果你在使用 Cyber Chat 导出自己的身份到电脑上的时候，更改了身份文件的储存路径，在用 cyfs-dapp-cli 创建项目时需要使用 `-o <peoplePath>`来指定身份文件，如 `-o ~/.cyfs_profile/people`，否则会因找不到身份文件而报错。

---

在 hello-demo/src 目录下，有三个子文件夹：

1. common: 前端和 Service 端可以共同复用的代码
2. service: Service 端工程代码
3. www: 前端工程代码

# 最终交互效果

按照本教程完成全部的编码之后，最终交互效果如下：

1. 点击前端页面中的 Hello World 按钮，发起与 Service 端 的 HelloWorld 接口交互的请求，请求携带 HelloRequestObject 自定义请求对象，对象中包含 Jack 字符串
2. 接口处理后，返回 HelloResponseObject 自定义响应对象，其中包含 “Hello Jack, welcome to CYFS world” 字符串
3. 前端接收响应后，弹出内容为 “Hello Jack, welcome to CYFS world” 的弹窗

# 前端部分

www 目录下，包含前端工程的全部代码，是一个规范化的基于 React 的工程项目。在这里，我们只关注以下 3 个文件：

1. www/initialize.ts， 初始化 Stack (SharedCyfsStack 简称 Stack)
2. www/apis/hello.ts，hello world 请求接口模块
3. www/pages/Welcome/Welcome.tsx，Hello World 按钮及触发事件

## 打开并等待 Stack 上线

打开 www/initialize.ts，手动注释已有的 init 函数，手动创建一个新的 init 函数来完成初始化 Stack 工作。
**注意** decId 需要更改为当前应用的 app_id，按照 cyfs.config.json -> app_id 可以找到。
完整代码如下：

```typescript
import * as cyfs from "cyfs-sdk";

export let stack: cyfs.SharedCyfsStack;

export async function init() {
	// 模拟器zone1-ood1的http-port
	const service_http_port = 21000;
	// 模拟器zone1-ood1的ws-port
	const ws_port = 21001;
	// cyfs.config.json -> app_id
	const decId = cyfs.ObjectId.from_base_58(
		"9tGpLNnfhjHxqiTKDCpgAipBVh4qjCDjxYGsUEy5p5EZ"
	).unwrap();
	// 打开模拟器SharedCyfsStack所需参数
	const param = cyfs.SharedCyfsStackParam.new_with_ws_event_ports(
		service_http_port,
		ws_port,
		decId
	);
	if (param.err) {
		console.error(`init SharedCyfsStackParam failed, ${param}`);
		return;
	}
	// 打开SharedCyfsStack
	stack = cyfs.SharedCyfsStack.open(param.unwrap());
	// 等待Stack上线
	while (true) {
		const r = await stack.wait_online(cyfs.JSBI.BigInt(1000000));
		if (r.err) {
			console.error(`wait online err: ${r.val}`);
		} else {
			console.info("online success.");
			break;
		}
	}
}
```

## Hello World 请求接口开发

前端向 Service 端发起请求，需要使用 cyfs.SharedCyfsStack 实例的 non_service().post_object 函数，post_object 函数的入参为包含 common 和 object 属性的对象，返回对象包含一个可选的 object 属性。这里我们重点关注入参对象和返回对象都有的 object 属性，这两个 object 属性值分别是发起请求和响应请求必须携带的对象数据，其类型是 NONObjectInfo，通过代码提示和跳转，可以得到如下信息：

```typescript
post_object(req: NONPostObjectOutputRequest): Promise<BuckyResult<NONPostObjectOutputResponse>>;
// 请求参数对象
interface NONPostObjectOutputRequest {
    common: NONOutputRequestCommon;
    object: NONObjectInfo;
}
// 响应参数对象
interface NONPostObjectOutputResponse {
    object?: NONObjectInfo;
}

class NONObjectInfo {
    constructor(object_id: ObjectId, object_raw: Uint8Array, object?: AnyNamedObject | undefined);
    // ...忽略其他细节
}
```

不难发现，NONObjectInfo 是一个 class，其构造函数入参主要是 object_id 和 object_raw，这两个参数实际上都是指向同一个对象。
说到这里，你可能已经意识到，要发起并处理一个完整的请求，需要分别定义一个请求对象和一个响应对象。

### HelloRequestObject 请求对象和 HelloResponseObject 响应对象

我们给请求对象命名 HelloRequestObject，给响应对象命名 HelloResponseObject。
打开 src/common/objs 文件夹，这个文件夹下存放着我们所需的全部自定义对象文件，让我们打开 obj\*proto.proto 的文件。文件的第一行写着 syntax = "proto3"，这说明我们的 proto 文件使用的是 proto3。

#### proto3

用过 proto3 这个工具的同学应该不会陌生，我们的自定义对象正是使用 proto3 来实现各种自定义对象的编解码。
没有用过这个工具的同学也不用担心，只需要了解 proto3 最基础的用法即可满足绝大部分的开发需求。
这里为大家贴出 proto3 的学习链接：https://developers.google.com/protocol-buffers/docs/proto3

#### 在 proto3 中定义 HelloRequestObject 和 HelloResponseObject 对象

接下来，我们打开 obj_proto.proto 文件，分别找到 HelloRequestObject 对象和 HelloResponseObject 对象的定义，代码如下：

```proto
syntax = "proto3";

message HelloRequestObject {
	string name = 1;
}

message HelloResponseObject {
	string greet = 1;
}

```

- 在 proto3 中，使用 message 关键字即可定义任意对象。

不难看出，我们的请求对象 HelloRequestObject 只有一个 name 属性，其类型为 string，响应对象 HelloResponseObject 也只有一个 greet 属性，类型为 string。

#### 利用 proto3 生成 Typescript 和 Javascript 的对象定义

在利用 proto3 生成 Typescript 和 Javascript 时，不同的系统在操作上有所差异。

- mac 系统，在项目根目录下，执行指令如下：

```shell
npm run proto-mac-pre
npm run proto-mac
```

**注意** 由于是直接执行的 protoc 执行程序，可能会弹窗提示 _无法打开“protoc”，因为无法验证开发者_，需要开发者按照以下路径去设置：
_系统偏好设置_ -> _安全性与隐私_ -> _允许从以下位置下载的 App_ -> 选择 _App Store 和认可的开发者_ -> 点击 _仍然允许_
按照这个路径设置好，重新执行指令即可。

- windows 系统，在项目根目录下，运行如下指令：

```shell
npm run proto-windows
```

运行完毕，在 src/common/objs 文件夹下，生成了 obj_proto_pb.d.ts 和 obj_proto_pb.js 这两个文件。在 obj_proto_pb.d.ts 声明文件中，我们看到了 HelloRequestObject 和 HelloResponseObject 对象的类型定义！

#### 创建 HelloRequestObject 和 HelloResponseObject 对象文件

在 src/common/objs 文件夹下， hello_request_object.ts 和 hello_response_object.ts 分别定义了 HelloRequestObject 对象和 HelloResponseObject 对象，其中包含了 HelloRequestObject 和 HelloResponseObject 对象全部的属性、方法等内容。

- 值得一提的是，几乎所有的自定义对象，都可以以这两个文件中的任意一个为模板进行套用。

关于 hello_request_object.ts 中的内容，我们重点关注 class HelloRequestObject 和 class HelloRequestObjectDecoder。

1. class HelloRequestObject，创建 HelloRequestObject 对象实例时，调用其 create 函数。
2. class HelloRequestObjectDecoder，对 HelloRequestObject 对象解码时使用，解码后得到 HelloRequestObject 对象。

类似的，关于 hello_response_object.ts 中的内容，我们重点关注 class HelloResponseObject 和 class HelloResponseObjectDecoder。

1. class HelloResponseObject，创建 HelloResponseObject 对象实例时，调用其 create 函数。
2. class HelloResponseObjectDecoder，对 HelloResponseObject 对象解码时使用，解码后得到 HelloResponseObject 对象。

### 编写 Hello World 请求 Api

定义好请求和响应对象之后，打开 www/apis/hello.ts，手动创建 helloWorld 函数。
**注意** decId 需要替换为当期应用的 app_id，owner 也需要替换为当前模拟器 1 的 peopleId。
代码如下：

```typescript
import * as cyfs from "cyfs-sdk";
import { stack } from "../initialize";
import { HelloRequestObject } from "@src/common/objs/hello_request_object";
import { HelloResponseObjectDecoder } from "@src/common/objs/hello_response_object";

export async function helloWorld(name: string) {
	// cyfs.config.json -> app_id
	const decId = cyfs.ObjectId.from_base_58(
		"9tGpLNnfhjHxqiTKDCpgAipBVh4qjCDjxYGsUEy5p5EZ"
	).unwrap();
	// mac-> ~/Library/cyfs/etc/zone-simulator/desc_list -> zone1 -> peopleId
	// windows -> C:\cyfs\etc\zone-simulator\desc_list -> zone1 -> peopleId
	const owner = cyfs.ObjectId.from_base_58(
		"5r4MYfFSHP9fBMtzQBw3SvRYGztJYaWyTcTitLGWEEjG"
	).unwrap();
	// 创建 HelloRequestObject 请求对象
	const helloObject = HelloRequestObject.create({
		name,
		decId,
		owner,
	});
	// 发起请求
	const ret = await stack.non_service().post_object({
		common: {
			req_path: "/test/hello",
			dec_id: decId,
			level: cyfs.NONAPILevel.Router,
			flags: 0,
		},
		object: new cyfs.NONObjectInfo(
			helloObject.desc().object_id(),
			helloObject.encode_to_buf().unwrap()
		),
	});
	if (ret.err) {
		console.error(`test api failed, ${ret}`);
		return ret;
	}
	const nonObject = ret.unwrap();
	// 使用 HelloResponseObjectDecoder 解析 HelloResponseObject 对象
	const r = new HelloResponseObjectDecoder().from_raw(
		nonObject.object!.object_raw
	);
	if (r.err) {
		console.error(`test api failed, ${ret}`);
		return ret;
	}
	// 解析后的 HelloResponseObject 对象
	const helloResponseObject = r.unwrap();
	console.log(`test api success, response: ${helloResponseObject?.greet}`);
	// 读取 HelloResponseObject 对象的 greet 值
	alert(helloResponseObject?.greet);
}
```

## Hello World 按钮及触发事件

在 www/pages/Welcome/Welcome.tsx 文件中，为页面中的 Hello World 按钮定义点击事件，代码如下：

```jsx
import { helloWorld } from "@src/www/apis/hello";

<Button onClick={() => helloWorld("Jack")} type="primary">
	Hello World
</Button>;
```

到这里，Hello World 的前端代码已经准备完毕。接下来，我们进入服务端的 Hello World 接口开发。

# Service 部分

为方便单独演示服务端的 Hello World 接口开发，在 src 文件夹下新建 hello_service 文件夹，里面包含 Hello World 服务端所需的全部代码。

## Service 启动程序

在 hello_service 文件夹下创建 app_startup.ts 文件作为 Service 的启动程序，代码如下：
**注意** decId 需要替换为当前应用的 app_id。

```typescript
import * as cyfs from "cyfs-sdk";
import { helloWorldRouter } from "./hello_world";

type postRouter = (
	req: cyfs.RouterHandlerPostObjectRequest
) => Promise<cyfs.BuckyResult<cyfs.RouterHandlerPostObjectResult>>;

class PostRouterReqPathRouterHandler
	implements cyfs.RouterHandlerPostObjectRoutine
{
	private m_router: postRouter;
	public constructor(router: postRouter) {
		this.m_router = router;
	}

	public call(
		param: cyfs.RouterHandlerPostObjectRequest
	): Promise<cyfs.BuckyResult<cyfs.RouterHandlerPostObjectResult>> {
		return this.m_router(param);
	}
}
export async function main() {
	// cyfs.config.json -> app_id
	const decId = cyfs.ObjectId.from_base_58(
		"9tGpLNnfhjHxqiTKDCpgAipBVh4qjCDjxYGsUEy5p5EZ"
	).unwrap();
	// 模拟器zone1-ood1的http-port
	const service_http_port = 21000;
	// 模拟器zone1-ood1的ws-port
	const ws_port = 21001;
	// 打开模拟器SharedCyfsStack所需参数
	const param = cyfs.SharedCyfsStackParam.new_with_ws_event_ports(
		service_http_port,
		ws_port,
		decId
	);
	if (param.err) {
		console.error(`init SharedCyfsStackParam failed, ${param}`);
		return;
	}
	// 打开SharedCyfsStack
	const stack = cyfs.SharedCyfsStack.open(param.unwrap());
	// 等待Stack上线
	while (true) {
		const r = await stack.wait_online(cyfs.JSBI.BigInt(1000000));
		if (r.err) {
			console.error(`wait online err: ${r.val}`);
		} else {
			console.info("online success.");
			break;
		}
	}
	// 配置 acccess 权限
	const access = new cyfs.AccessString(0);
	access.set_group_permissions(
		cyfs.AccessGroup.CurrentZone,
		cyfs.AccessPermissions.WirteAndCall
	);
	access.set_group_permissions(
		cyfs.AccessGroup.CurrentDevice,
		cyfs.AccessPermissions.WirteAndCall
	);
	access.set_group_permissions(
		cyfs.AccessGroup.OwnerDec,
		cyfs.AccessPermissions.WirteAndCall
	);
	const ra = await stack
		.root_state_meta_stub()
		.add_access(cyfs.GlobalStatePathAccessItem.new("/test/hello", access));
	if (ra.err) {
		console.error(`path /test/hello add access error: ${ra}`);
	}
	console.log("add access successed: ", ra.unwrap());
	// 注册路由
	const r = await stack
		.router_handlers()
		.add_post_object_handler(
			cyfs.RouterHandlerChain.Handler,
			"hello-world",
			1,
			undefined,
			"/test/hello",
			cyfs.RouterHandlerAction.Pass,
			new PostRouterReqPathRouterHandler(helloWorldRouter)
		);

	if (r.err) {
		console.error(`add post handler failed, err: ${r}`);
	} else {
		console.log(`add post handler success.`);
	}
}

main();
```

从代码中，不难看出，Service 端的启动程序主要有 3 个步骤：

1. 打开并等待 Stack 上线
   服务端初始化模拟器 Stack 并等待上线的用法与前端用法一致。

2. 为路径配置 acccess 权限
   使用 AccessString 来创建 access 实例，在 access 实例上，使用 AccessGroup 来为不同的`组`配置 acccess 权限，使用 AccessPermissions 为不同的`组`设置多个权限，我们为 CurrentZone、CurrentDevice 和 OwnerDec 这 3 个`组`开放 Write 和 Call 权限。

3. 注册路由
   注册路由的核心方法为 cyfs.SharedCyfsStack 实例的 router_handlers().add_post_object_handler 方法。该方法有以下 7 个参数：

   ```typescript
   add_post_object_handler(chain: RouterHandlerChain, id: string, index: number, filter: string | undefined, req_path: string | undefined, default_action: RouterHandlerAction, routine?: RouterHandlerPostObjectRoutine): Promise<BuckyResult<void>>;
   ```

   我们需要重点关注的是 chain 和 routine 这 2 个参数。
   使用 add_post_object_handler 注册路由，chain 的值为 cyfs.RouterHandlerChain.Handler
   routine 是需要注册的路由对象，该对象必须继承 cyfs.RouterHandlerPostObjectRoutine 接口并实现 call 方法。

细心的你应该已经注意到，我们在文件最开始的地方引入了一个 helloWorldRouter 的模块，该模块被用在了 add_post_object_handler 路由注册方法中，其实，helloWorldRouter 就是我们接下来要实现的 hello world 路由模块。

## Hello World 路由文件

在 hello_service 文件夹下创建 hello_world.ts 文件，代码如下：
**注意** decId 需要替换为当期应用的 app_id，owner 也需要替换为当前模拟器 1 的 peopleId。

```typescript
import * as cyfs from "cyfs-sdk";
// 响应对象
import { HelloResponseObject } from "../common/objs/hello_response_object";
// 请求对象解析器
import { HelloRequestObjectDecoder } from "../common/objs/hello_request_object";
import { AppObjectType } from "../common/types";

export function helloWorldRouter(
	req: cyfs.RouterHandlerPostObjectRequest
): Promise<cyfs.BuckyResult<cyfs.RouterHandlerPostObjectResult>> {
	const { object, object_raw } = req.request.object;
	// 判断是否与预期的 HelloRequestObject 对象编号一致
	if (!object || object.obj_type() !== AppObjectType.HELLO_REQUEST) {
		const msg = `obj_type err.`;
		console.error(msg);
		return Promise.resolve(
			cyfs.Err(new cyfs.BuckyError(cyfs.BuckyErrorCode.InvalidParam, msg))
		);
	}

	// 使用 HelloRequestObjectDecoder 解析 HelloRequestObject 对象
	const decoder = new HelloRequestObjectDecoder();
	const r = decoder.from_raw(object_raw);
	if (r.err) {
		const msg = `decode failed, ${r}.`;
		console.error(msg);
		return Promise.resolve(
			cyfs.Err(new cyfs.BuckyError(cyfs.BuckyErrorCode.InvalidParam, msg))
		);
	}
	// 获取 HelloRequestObject 对象中的 name 值
	const name = r.unwrap().name;
	const respObject = {
		greet: `Hello ${name}, welcome to CYFS world!`,
		// cyfs.config.json -> app_id
		decId: cyfs.ObjectId.from_base_58(
			"9tGpLNnfhjHxqiTKDCpgAipBVh4qjCDjxYGsUEy5p5EZ"
		).unwrap(),
		// mac-> ~/Library/cyfs/etc/zone-simulator/desc_list -> zone1 -> peopleId
		// windows -> C:\cyfs\etc\zone-simulator\desc_list -> zone1 -> peopleId
		owner: cyfs.ObjectId.from_base_58(
			"5r4MYfFSHP9fBMtzQBw3SvRYGztJYaWyTcTitLGWEEjG"
		).unwrap(),
	};
	// 创建 HelloResponseObject 响应对象
	const helloResponseObject = HelloResponseObject.create(respObject);
	return Promise.resolve(
		cyfs.Ok({
			action: cyfs.RouterHandlerAction.Response,
			response: cyfs.Ok({
				object: new cyfs.NONObjectInfo(
					helloResponseObject.desc().object_id(),
					helloResponseObject.encode_to_buf().unwrap()
				),
			}),
		})
	);
}
```

# 打开模拟器并运行项目

通过前面的学习，我们已经完成了实现一个 Hello World 项目所需的前端及服务端全部代码，现在让我们在模拟器中运行起来！

## 启动模拟器

在项目根目录下的 tools/zone-simulator 文件夹下，找到与自己系统对应的模拟器程序，运行 zone-simulator 模拟器程序。
**注意** mac 系统需要修改文件的可执行权限，执行以下指令进行修改：

```shell
chmod 775 zone-simulator
```

## 运行前端项目

在项目根目录输入以下指令：

```shell
npm run dev
```

在 CYFS 浏览器中访问 http://localhost:8088，即可看到前端界面。

## 运行 Service

要运行 Service，我们需要使用 tsc 把项目中的 ts 文件编译成 js，在项目根目录下，打开新的终端，输入以下指令：

```shell
npx tsc
```

接着，把 src/common/objs/obj_proto_pb.js 文件拷贝到 deploy/src/common/objs/obj_proto_pb.js

最后，在根目录输入以下指令：

```shell
node deploy/src/hello_service/app_startup.js
```

## 测试交互效果

在前端页面，点击 Hello World 按钮，弹出提示框，内容为 “Hello Jack, welcome to CYFS world”

## 异常说明

如果最终交互效果不符合预期，请按照此文档，逐行检查自己的代码，尤其注意 decId、ownerId 等需要替换为自己应用的 id 的场景。
