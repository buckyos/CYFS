
pub const IM_ACL_CONFIG: &str = r#"
[im]
add-friend = { action = "*-post-object", res = "/core/1001", group = { location = "outer" }, access = "accept" }
remove-friend = { action = "*-post-object", res = "/core/1004", group = { location = "outer" }, access = "accept" }
friend-state = { action = "*-post-object", res = "/core/41", group = { location = "outer" }, access = "accept" }
msg-post = { action = "*-post-object", res = "/dec_app/9tGpLNna8UVtPYCfV1LbRN2Bqa5G9vRBKhDhZiWjd7wA/32769", group = { location = "outer" }, access = "accept" }
msg-get = { action = "*-get-object", res = "/dec_app/9tGpLNna8UVtPYCfV1LbRN2Bqa5G9vRBKhDhZiWjd7wA/32769", group = { location = "outer" }, access = "accept" }
im-group ={ action = "*-post-object", res = "/dec_app/9tGpLNna8UVtPYCfV1LbRN2Bqa5G9vRBKhDhZiWjd7wA/32771", group = { location = "outer" }, access = "accept" }
session-file = { action = "*-get-object", res = "/standard/8", group = { location = "outer" }, access = "accept" }
"#;

pub const DSG_ACL_CONFIG: &str = r#"
[dsg]
dmc_dsg-filter = {action = "*-get-object", res = "/dec_app/9tGpLNnBuQvSVgFee7s8vUqCe373z6vkFbVrRpadM9Sp/**", access = "accept"}
dmc_dsg-filter3 = {action = "*-get-object", res = "/9tGpLNnBuQvSVgFee7s8vUqCe373z6vkFbVrRpadM9Sp/**", access = "accept"}
dmc_dsg-filter2 = {action = "*-post-object", res = "/dec_app/9tGpLNnBuQvSVgFee7s8vUqCe373z6vkFbVrRpadM9Sp/**", access = "accept"}
nft-put-data-filter = {action = "*-put-data", res = "/9tGpLNnBuQvSVgFee7s8vUqCe373z6vkFbVrRpadM9Sp/dmc-dsg/**", access = "accept"}
nft-get-data-filter = {action = "*-get-data", res = "/9tGpLNnBuQvSVgFee7s8vUqCe373z6vkFbVrRpadM9Sp/dmc-dsg/**", access = "accept"}
"#;

pub const GIT_ACL_CONFIG: &str = r#"
[cyfs-git]
post-object = {action="*-post-object",res="/dec_app/9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2/32810",access="accept"}
put-repository = {action="*-put-object",res="/dec_app/9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2/33498",access="accept"}
git-get-object = {action = "*-get", res = "/9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2/**", access = "accept"}
put-object = {action = "*-put-object", res = "/dec_app/9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2/**", access = "handler"}
"#;


pub const DRIVE_ACL_CONFIG: &str = r#"
[drive]
drive-filter = {action = "*-get-object", res = "/9tGpLNnBYrgMNLet1wgFjBZhTUeUgLwML3nFhEvKkLdM/drive/**", access = "accept"}
drive-put-data-filter = {action = "*-put-data", res = "/9tGpLNnBYrgMNLet1wgFjBZhTUeUgLwML3nFhEvKkLdM/drive/**", access = "accept"}
drive-get-data-filter = {action = "*-get-data", res = "/9tGpLNnBYrgMNLet1wgFjBZhTUeUgLwML3nFhEvKkLdM/drive/**", access = "accept"}
"#;

pub const NFT_ACL_CONFIG: &str = r#"
[nft]
nft-filter = {action = "*-get-object", res = "/dec_app/9tGpLNnab9uVtjeaK4bM59QKSkLEGWow1pJq6hjjK9MM/**", access = "accept"}
nft-filter3 = {action = "*-get-object", res = "/9tGpLNnab9uVtjeaK4bM59QKSkLEGWow1pJq6hjjK9MM/**", access = "accept"}
nft-filter4 = {action = "*-get-object", res = "/system/**", access = "accept"}
nft-filter2 = {action = "*-post-object", res = "/dec_app/9tGpLNnab9uVtjeaK4bM59QKSkLEGWow1pJq6hjjK9MM/**", access = "accept"}
nft-put-data-filter = {action = "*-put-data", res = "/9tGpLNnab9uVtjeaK4bM59QKSkLEGWow1pJq6hjjK9MM/nft/**", access = "accept"}
nft-get-data-filter = {action = "*-get-data", res = "/9tGpLNnab9uVtjeaK4bM59QKSkLEGWow1pJq6hjjK9MM/nft/**", access = "accept"}
"#;



