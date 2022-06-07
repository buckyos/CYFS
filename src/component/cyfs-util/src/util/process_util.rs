use std::process::Command;

pub struct ProcessUtil;

impl ProcessUtil {
    // 解析命令行，支持带空格的路径和参数
    // 参数可以使用单引号和双引号前后包括(前后必须相同，都为单引号或者都为双引号)，
    // 避免内部含有空格和引号被分割为多个参数
    pub fn parse_cmd(cmd: &str) -> Vec<&str> {
        let mut args = Vec::new();
        let mut next_quot: Option<char> = None;
        let mut prev_index = 0;
        for (i, c) in cmd.chars().enumerate() {
            if c == '"' || c == '\'' {
                if next_quot.is_none() {
                    next_quot = Some(c);
                } else if *next_quot.as_ref().unwrap() != c {
                    continue;
                } else {
                    next_quot = None;

                    let arg = &cmd[prev_index..(i + 1)];
                    let arg = arg.trim_start_matches(c).trim_end_matches(c);
                    if !arg.is_empty() {
                        args.push(arg);
                    }

                    prev_index = i + 1;
                }
            } else if c.is_whitespace() && next_quot.is_none() {
                let arg = &cmd[prev_index..(i + 1)];
                let arg = arg.trim_start().trim_end();
                if !arg.is_empty() {
                    args.push(arg);
                }

                prev_index = i + 1;
            }
        }

        // 如果存在最后一段，那么加入
        if prev_index < cmd.len() {
            let arg = &cmd[prev_index..cmd.len()];
            let arg = arg.trim_start().trim_end();
            if !arg.is_empty() {
                args.push(arg);
            }
        }
        args
    }

    // windows平台默认不detach，如果设定了detach，要特殊处理
    #[cfg(target_os = "windows")]
    pub fn detach(cmd: &mut Command) {
        use std::os::windows::process::CommandExt;

        pub const DETACHED_PROCESS: u32 = 0x00000008;
        pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        pub const CREATE_NO_WINDOW: u32 = 0x08000000;

        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
        cmd.creation_flags(flags);
    }

    #[cfg(not(target_os = "windows"))]
    pub fn detach(_cmd: &mut Command) {}
}

// #[cfg(test)]
// mod tests {
//     use super::ProcessUtil;

//     #[test]
//     fn test_parse_cmd() {
//         let v = r#""C:\\www www" yyyy 'asd" asd'  'name xxx' --status"#;
//         let ret = ProcessUtil::parse_cmd(v);
//         assert_eq!(ret.len(), 5);
//         assert_eq!(ret[0], r#"C:\\www www"#);
//         assert_eq!(ret[1], r#"yyyy"#);
//         assert_eq!(ret[2], r#"asd" asd"#);
//         assert_eq!(ret[3], r#"name xxx"#);
//         assert_eq!(ret[3], r#"--status"#);

//         println!("{:?}", ret);
//     }
// }
