// 用来定义通用的空参数结构体的辅助宏
#[macro_export]
macro_rules! declare_event_empty_param {
    ($name:ident, $category:ident) => {
        #[derive(Clone)]
        pub struct $name {}

        impl std::fmt::Display for $name {
            fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
                Ok(())
            }
        }

        impl JsonCodec<Self> for $name {
            fn encode_json(&self) -> serde_json::Map<String, serde_json::Value> {
                serde_json::Map::new()
            }

            fn decode_json(
                _obj: &serde_json::Map<String, serde_json::Value>,
            ) -> cyfs_base::BuckyResult<Self> {
                Ok(Self {})
            }
        }

        impl RouterEventCategoryInfo for $name {
            fn category() -> RouterEventCategory {
                RouterEventCategory::$category
            }
        }
    };
}
