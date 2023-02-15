use zone_simulator::*;

const USER_MNEMONIC: &str =
    "paper grant gap across doctor hockey life decline sauce what aunt jelly";

const USER1_DATA: TestUserDataString = TestUserDataString {
    people_id: "5r4MYfFUT9H4pBnWgbqiegv7ogkwoWwSTe9P8whF5Sa8",
    ood_id: "5aSixgLnhrKFTs4TPU4hgurJdSTyTng5ZR9xDWuQ3HKc",
    standby_ood_id: Some("5aSixgMDeopu2b36ixCri7weXpa7nnSPYc2ipbwE3S9i"),
    device1_id: "5aSixgPDCmbij7JRy2mbBDYZbEcrLB8fMVpXhhCWrnrv",
    device2_id: "5aSixgRc7HjfqC3dwMSqTj3993L9vgA7PgMYDFJ2EULo",
};

const USER2_DATA: TestUserDataString = TestUserDataString {
    people_id: "5r4MYfFakpgtRQR4ypH1cmxAyNxScDBRHtpT3d5zgfrn",
    ood_id: "5aSixgMFYYQgTacZdfkpVbQAvHB5bhChRyg6aJJySeuE",
    standby_ood_id: None,
    device1_id: "5aSixgPgGFwokDz8uFoe2whmwaewrDjTrGctw3GQpsf8",
    device2_id: "5aSixgS42ZWSH7tZ6QqnPRAqh9YTxVwM8PKfeJHox8r4",
};

// 校验助记词到id生成规则是否匹配
async fn check_user() {
    let (user1, user2) = zone_simulator::TestLoader::load_users(USER_MNEMONIC, false,false).await;
    user1.user_data().check_equal(&USER1_DATA);
    user2.user_data().check_equal(&USER2_DATA);
}

pub async fn test() {
    check_user().await;
}

#[cfg(test)]
mod test {
    #[test]
    pub fn test() {
        async_std::task::block_on(super::check_user());
    
        info!("test all router handler case success!");
    }
}
