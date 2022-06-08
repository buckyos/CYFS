use crate::coreobj::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Debug, Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::TextDescContent)]
pub struct TextDescContent {
    id: String,
    header: String,
}
impl TextDescContent {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn header(&self) -> &str {
        &self.header
    }
}

impl DescContent for TextDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::Text as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::TextContent)]
pub struct TextContent {
    value: String,
}

impl BodyContent for TextContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl TextContent {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut String {
        &mut self.value
    }
}

type TextType = NamedObjType<TextDescContent, TextContent>;
type TextBuilder = NamedObjectBuilder<TextDescContent, TextContent>;
type TextDesc = NamedObjectDesc<TextDescContent>;

pub type TextId = NamedObjectId<TextType>;
pub type Text = NamedObjectBase<TextType>;

pub trait TextObj {
    fn build(id: &str, header: impl Into<String>, value: impl Into<String>) -> TextBuilder;
    fn create(id: &str, header: impl Into<String>, value: impl Into<String>) -> Self;

    fn id(&self) -> &str;

    fn header(&self) -> &str;

    fn value(&self) -> &str;
    fn value_mut(&mut self) -> &mut String;

    fn into_header(self) -> String;
    fn into_value(self) -> String;

    fn text_id(&self) -> TextId;
}

impl TextObj for Text {
    fn create(id: &str, header: impl Into<String>, value: impl Into<String>) -> Self {
        Self::build(id, header, value).no_create_time().build()
    }

    fn build(id: &str, header: impl Into<String>, value: impl Into<String>) -> TextBuilder {
        let desc = TextDescContent {
            id: id.to_owned(),
            header: header.into(),
        };
        let body = TextContent {
            value: value.into(),
        };
        TextBuilder::new(desc, body)
    }

    fn id(&self) -> &str {
        &self.desc().content().id
    }

    fn header(&self) -> &str {
        &self.desc().content().header
    }

    fn value(&self) -> &str {
        &self.body().as_ref().unwrap().content().value
    }

    fn value_mut(&mut self) -> &mut String {
        &mut self.body_mut().as_mut().unwrap().content_mut().value
    }

    fn into_header(self) -> String {
        self.into_desc().into_content().header
    }

    fn into_value(self) -> String {
        self.into_body().unwrap().into_content().value.to_string()
    }

    fn text_id(&self) -> TextId {
        self.desc().calculate_id().try_into().unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;

    #[test]
    fn test() {
        let header = "cyfs system";
        let value = "xxxxx";
        let obj = Text::create("cyfs", header, value);
        assert!(obj.desc().content().id() == "cyfs");

        let obj2 = obj.clone();
        let header2 = obj2.into_header();
        assert_eq!(header2, header);

        let obj2 = obj.clone();
        let value2 = obj2.into_value();
        assert_eq!(value2, value);

        let value = "yyyyyyy";
        let mut obj2 = obj.clone();
        *obj2.value_mut() = value.to_owned();
        let value2 = obj2.into_value();
        assert_eq!(value2, value);
    }

    #[test]
    fn test_empty() {
        let text_obj = Text::create("", "", "");
        let buf = text_obj.to_vec().unwrap();

        println!("empty text_id: {}", text_obj.text_id());
        let path = cyfs_util::get_app_data_dir("tests");
        std::fs::create_dir_all(&path).unwrap();
        let name = path.join("text_empty.desc");
        std::fs::write(&name, buf).unwrap();
    }

    #[test]
    fn test_codec() {
        let id = "test_text";
        let header = "test_header";
        let value = "test_value";
        let text_obj = Text::create(id, header, value);
        let text_id = text_obj.desc().calculate_id();
        let buf = text_obj.to_vec().unwrap();

        let text_obj2 = Text::clone_from_slice(&buf).unwrap();
        assert_eq!(text_id, text_obj2.desc().calculate_id());
        assert_eq!(text_obj.id(), text_obj2.id());
        assert_eq!(text_obj.header(), text_obj2.header());
        assert_eq!(text_obj.value(), text_obj2.value());

        let (any, left_buf) = AnyNamedObject::raw_decode(&buf).unwrap();
        assert_eq!(left_buf.len(), 0);
        info!("any id={}", any.calculate_id());
        assert_eq!(text_id, any.calculate_id());

        let buf2 = any.to_vec().unwrap();
        assert_eq!(buf.len(), buf2.len());
        assert_eq!(buf, buf2);

        // 保存到文件
        let path = cyfs_util::get_app_data_dir("tests");
        std::fs::create_dir_all(&path).unwrap();
        let name = path.join("text.desc");
        std::fs::write(&name, buf2).unwrap();
    }
}
