use cyfs_base::BuckyResult;
use std::process::Command;

pub struct DecUserManager {}
impl DecUserManager {
    pub fn new() -> DecUserManager {
        DecUserManager {}
    }

    // 名字14位: dec_<dec_id最后10位>
    pub fn get_user_name(dec_id: &str) -> String {
        let name = "dec_".to_owned() + &dec_id[dec_id.len() - 10..dec_id.len()];
        name
    }

    // for decApp install
    pub fn create(dec_id: &str) -> BuckyResult<()> {
        let name = DecUserManager::get_user_name(dec_id);
        DecUserManager::_add_user_group(&name[..])?;
        Ok(())
    }

    // for decApp uninstall
    pub fn remove(dec_id: &str) -> BuckyResult<()> {
        let name = DecUserManager::get_user_name(dec_id);
        DecUserManager::_remove_user_group(&name[..])?;
        Ok(())
    }

    // 创建user and group, 把user 加到group里
    fn _add_user_group(name: &str) -> BuckyResult<()> {
        Command::new("useradd").args([name]).output()?;
        Command::new("groupadd").args([name]).output()?;
        Command::new("usermod")
            .args(["-a", "-G", name, name])
            .output()?;
        Ok(())
    }

    fn _remove_user_group(name: &str) -> BuckyResult<()> {
        Command::new("groupdel").args(["-f", name]).output()?;
        Command::new("userdel").args(["-f", name]).output()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let test_device: &str = "5bnZVFXKHd5LqRCP5jPjDjm7zcmkHeYPmeyymY3784jz";
        let result = DecUserManager::create(test_device);
        assert!(result.is_ok());

        // 组和用户都添加成功
        let output = Command::new("cat").args(["/etc/group"]).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        println!("stdout {}", stdout);
        let re = regex::Regex::new(r"dec_yymY3784jz:.+dec_yymY3784jz").unwrap();
        assert!(re.is_match(&stdout[..]));

        // 检查 /etc/passwd 里存在这个用户
        {
            let output = Command::new("cat").args(["/etc/passwd"]).output().unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            println!("stdout {}", stdout);
            let re = regex::Regex::new(r"dec_yymY3784jz").unwrap();
            assert!(re.is_match(&stdout[..]));
        }

        let _ = DecUserManager::remove(test_device);
    }

    #[test]
    fn test_remove() {
        // 添加完之后删除
        let test_device: &str = "5bnZVFXKHd5LqRCP5jPjDjm7zcmkHeYPmeyymY3784jz";
        // name-> dec_yymY3784jz
        let _ = DecUserManager::create(test_device);
        let result = DecUserManager::remove(test_device);
        assert!(result.is_ok());

        // 检查 /etc/group里没有这个组
        let output = Command::new("cat").args(["/etc/group"]).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        println!("stdout {}", stdout);
        let re = regex::Regex::new(r"dec_yymY3784jz:.+dec_yymY3784jz").unwrap();
        assert!(!re.is_match(&stdout[..]));

        // 检查 /etc/passwd 里没有这个用户
        {
            let output = Command::new("cat").args(["/etc/passwd"]).output().unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            println!("stdout {}", stdout);
            let re = regex::Regex::new(r"dec_yymY3784jz").unwrap();
            assert!(!re.is_match(&stdout[..]));
        }
    }
}
