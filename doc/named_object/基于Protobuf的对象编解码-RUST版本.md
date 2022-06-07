使用protobuf编解码
protobuf的编解码是基于raw_codec编解码体系的下层编码，每个扩展对象的DescContent和BodyContent可以选择是否使用 
现在推荐使用静态版本的protobuf，也就是需要生成rust桩代码

# 对应的proto描述文件
对于需要使用编码的DescContent/BodyContent结构体部分，首先需要定义对应的proto描述文件,需要注意以下几点:
+ proto里面message命名规范
    使用和原结构体相同的名字，在rust里面使用时候，使用protos.UserDescContent来引用
+ 编解码可以和RawCodec混合使用
    对于结构体里面每个字段，protobuf和rawcodec可以混合使用，上下浮动边界由用户根据是否需要扩展等条件，自行决定
    比如对于如下结构体的value字段，可以有两种选择方式：
```rust
struct UserDescContent {
    name: String,
    value: HashMap<string, ObjectId>,
}
```
```proto
选择一：
message UserDescContent {
    string name = 1;
    map<string, bytes> value = 2;
}
其中value字段使用protobuf的map格式，对每个(key，value)进行编解码
选择二：
message UserDescContent {
    string name = 1;
    bytes value = 2;
}
其中value字段使用rawcodec编码成bytes数组
```
+ 对于ObjectId这些可以编码成bytes的，在message里面使用bytes定义(不要使用base64编码的string)
+ protobuf里面map的key不支持bytes
    如果key是ObjectId之类，需要编码为bytes的，需要使用数组(repeated)来替代，否则直接使用string做键值会增加编码后的体积
```rust
struct UserDescContent {
    obj_list: HashMap<ObjectId, Vec<u8>>,
}
```
```proto
message {
    // 使用list编码hash_map
    message ObjItem {
        bytes obj_id = 1;
        bytes value = 2;
    }
    repeated ObjItem obj_list = 3;
}
```
+ protobuf 不支持小于32bit的整形，比如u8, u16这些
    如果使用这些，编码直接使用uint32, int32类型，然后在TryFrom里面使用ProtobufCodecHelper::decode_value进行解码，该辅助函数增加了溢出检测，避免意外情况下的转型出错
+ 对于Option类型，使用optional
+ 对于子结构体，可以使用多级嵌套定义
+ 对于rust枚举类型，需要结合protobuf的enum类型和optional修饰符来完成  
比如对于下述的结构体，推荐proto定义:
```rust
enum ObjectInfo {
    Chunk(ObjectId),
    ObjList(Vec<ObjectList>),
}
struct UserDescContent {
    info: ObjectInfo,
}
```
```proto
message UserDescContent {
    enum Type {
        Chunk = 0;
        ObjList = 1;
    }
    Type type = 1;
    optional bytes chunk = 2;
    repeated bytes obj_list = 3;
}
```

# 依赖的rust工程配置
+ rust库使用protobuf
```toml
protobuf = { version = "2", features = ["with-bytes"] }
```
+ proto原型文件编译程序使用官方的protoc
目前cyfs工程在3rd里面引入了protoc工程，其它非cyfs的工程可以自行配置protoc的目标目录
+ 编译生成桩代码
由于桩代码一般在修改proto原型文件后编译一次提交即可，所以目前只做了windows平台的编译，其余平台可以选择对应的protoc程序  
在对应工程的build.rs里面，增加如下的编译代码：
```rust
fn gen_protos() {
    let mut gen = protoc_rust::Codegen::new();
    gen.input("protos/standard_objects.proto")
    .protoc_path("../../3rd/protoc/bin/protoc.exe")
        .out_dir("src/codec/protobuf/protos")
        .customize(protoc_rust::Customize {
            expose_fields: Some(true),
            ..Default::default()
        });

    gen.run().expect("protoc error!");
}
fn main() {
    // 目前只支持在windows下编译生成proto存根文件
    #[cfg(target_os = "windows")]
    gen_protos();
}
```
生成的代码建议放置到工程目录的codec/protos子目录下面，然后用protos模块名来引用对用的定义  
在codec/protos/mod.rs里面：
```rust
pub(crate) mod protos;
```

# 对结构体实现对应的codec编解码
关键点是增加proto结构体到原结构体的转换，也就是protos.UserDescContent<->UserDescContent的转换代码，步骤有三点:
## 1. 重载DescContent/BodyContent里面的format方法
```rust
impl DescContent for UserDescContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}
```
## 2. 需要增加如下的两个trait实现，提供和对应的protos结构体的转码:
```rust
impl TryFrom<&UserDescContent> for protos::UserDescContent {......}
impl TryFrom<protos::UserDescContent> for UserDescContent {......}
```
## 3. 然后使用cyfs_base里面的codec定义宏：
```rust
cyfs_base::impl_default_protobuf_raw_codec!(UserDescContent);
```

对于之前使用RawCodec的结构体UserDescContent，完整实现如下
```
use crate::codec::protos;

struct UserDescContent {
    name: Option<String>,
    device_list: Vec<DeviceId>,
}
impl DescContent for UserDescContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl TryFrom<&UserDescContent> for protos::UserDescContent {
    type Error = BuckyError;

    fn try_from(value: &UserDescContent) -> BuckyResult<Self> {
        let mut ret = protos::UserDescContent::new();


        ret.set_device_list(ProtobufCodecHelper::encode_buf_list(&value.device_list)?);
        if let Some(name) = &value.name {
            ret.set_name(name.to_owned());
        }

        Ok(ret)
    }
}

impl TryFrom<protos::UserDescContent> for UserDescContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::UserDescContent) -> BuckyResult<Self> {
        let mut ret = Self {
            device_list: ProtobufCodecHelper::decode_buf_list(value.take_device_list())?,
            name: None,
        };

        if value.has_name() {
            ret.name = Some(value.take_name());
        }

        Ok(ret)
    }
}

cyfs_base::impl_default_protobuf_raw_codec!(DeviceBodyContent);
```

# 使用ProtoBuf编解码辅助函数
cyfs_base提供了ProtobufCodecHelper一组辅助函数，可以对支持RawCodec、TryFrom的子数组、子结构体提供方便的转发
+ decode_string_list/encode_string_list 对支持FromStr的数组进行编解码
+ decode_buf_list/encode_buf_list 对支持RawCodec的数组进行编解码
+ decode_nested_item/encode_nested_item 对支持TryFrom的子结构体数组进行编解码
+ decode_nested_item/encode_nested_item 对支持TryFrom的子结构体进行编解码
+ decode_value 对支持标准TryFrom的值进行解码，比如u32到u8的解码等
+ decode_value_list 对支持标准TryFrom的值列表进行解码，比如[u32]到[u8]的解码等

# 使用Protobuf编解码注意事项
+ protobuf 不支持小于32bit的整形，比如u8, u16这些
    如果使用这些，编码直接使用uint32, int32类型，然后在TryFrom里面使用ProtobufCodecHelper::decode_value进行解码
```rust
pub struct FriendOptionContent {
    auto_confirm: Option<u8>,
}
impl TryFrom<&FriendOptionContent> for protos::FriendOptionContent {
    type Error = BuckyError;

    fn try_from(value: &FriendOptionContent) -> BuckyResult<Self> {
        let mut ret = Self::new();
        if let Some(v) = &value.auto_confirm {
            // 编码时候是向上编码，所以直接as就可以
            ret.set_auto_confirm(*v as u32);
        }
        Ok(ret)
    }
}
impl TryFrom<protos::FriendOptionContent> for FriendOptionContent {
    type Error = BuckyError;
    fn try_from(mut value: protos::FriendOptionContent) -> BuckyResult<Self> {
        let mut ret = Self {
            auto_confirm: None,
        };
        if value.has_auto_confirm() {
            // 解码时候是向下解码，如果需要考虑是否溢出的情况，那么必须使用带类型检测的转换方式，不可以直接 as
            ret.auto_confirm = Some(ProtobufCodecHelper::decode_value(value.get_auto_confirm())?);
        }
        Ok(ret)
    }
}
```
+ 对于Option类型，解码时候需要通过has_xxx字段判断是否真正存在该字段，才可以调用get_xxx或者take_xxx进行解码；编码时候也是，如果此字段为空，那么不需要调用set_xxx进行编码


# 对于空结构体，有两种方式
1. 直接保留rawCodec方式
两种情况下空结构编码出来的大小都是0，所以空结构体没必要再protobuf里面定义Message了，直接使用RawCodec；以后增加了内容之后，再改为protobuf，并考虑兼容空内容
2. 使用一致的protobuf方式
由于空结构体都是一样的，所以不需要额外定义对应的Message，直接使用cyfs_base::impl_empty_protobuf_raw_codec!宏即可

**上面两个一致只是基于DescContent和BodyContent两个顶级结构体，如果结构体内部嵌套了子结构体，那么需要使用一致的protobuf编解码，否则解码时候可能会产生不一致的buf长度，导致解码失败；当然对于内部使用的空结构体，解码时候可以直接跳过解码步骤，构造一个空结构对象即可**
```rust
#[derive(RawEncode, RawDecode)]
pub struct FriendOptionDescContent {}

// or 

cyfs_base::impl_empty_protobuf_raw_codec!(FriendOptionDescContent)
```