use cyfs_base::*;
use cyfs_base_meta::*;
use cyfs_meta::*;

static TEST_CONFIG: &str = r#"
    {
        "accounts": [
            [
                null,
                0,
                {"id":[4,12,128,3,4,111,225,188,241,91,156,158,69,133,35,214,180,249,93,46,242,30,109,81,222,148,29,18,95,167,111,216]},
                {
                    "peer_type":"Device",
                    "area_code":{
                        "country":1,
                        "carrier":2,
                        "city":3,
                        "inner":4},
                    "peer_unique_id":[49,50,51,0,0,0,0,0,0,0,0,0,0,0,0,0],
                    "owner_id":{"id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]},
                    "public_key":[48,129,159,48,13,6,9,42,134,72,134,247,13,1,1,1,5,0,3,129,141,0,48,129,137,2,129,129,0,190,84,155,157,37,156,118,150,68,207,26,148,14,124,205,169,55,154,205,168,45,186,197,74,56,15,88,129,74,179,40,66,3,95,58,79,75,14,48,71,233,197,211,139,234,239,14,168,51,122,247,115,42,69,124,102,185,230,195,48,37,103,19,169,135,207,193,29,204,119,192,37,118,164,147,16,137,125,39,154,220,176,130,188,22,175,71,207,18,159,239,146,207,187,246,245,115,153,121,140,240,121,58,92,46,22,207,234,223,241,185,244,61,65,219,177,127,156,135,170,185,127,45,132,32,61,40,191,2,3,1,0,1]
                },
                [],
                [],
                [],
                [],
                null,
                {
                    "secret":[48,130,2,92,2,1,0,2,129,129,0,190,84,155,157,37,156,118,150,68,207,26,148,14,124,205,169,55,154,205,168,45,186,197,74,56,15,88,129,74,179,40,66,3,95,58,79,75,14,48,71,233,197,211,139,234,239,14,168,51,122,247,115,42,69,124,102,185,230,195,48,37,103,19,169,135,207,193,29,204,119,192,37,118,164,147,16,137,125,39,154,220,176,130,188,22,175,71,207,18,159,239,146,207,187,246,245,115,153,121,140,240,121,58,92,46,22,207,234,223,241,185,244,61,65,219,177,127,156,135,170,185,127,45,132,32,61,40,191,2,3,1,0,1,2,129,128,64,223,145,210,244,120,234,42,185,245,101,119,197,160,96,113,39,20,202,184,98,45,225,5,53,42,117,222,75,217,217,4,247,37,68,56,182,186,117,86,109,166,18,63,4,170,202,242,37,233,233,226,5,44,126,4,125,164,220,46,210,240,217,80,62,177,153,59,198,145,64,59,161,226,37,80,45,82,183,223,142,234,95,57,36,149,218,230,72,173,55,235,46,73,37,221,219,120,159,248,2,211,247,198,164,238,30,9,87,32,77,77,133,252,3,118,82,241,104,216,232,215,220,116,228,82,239,161,2,65,0,236,243,42,111,138,201,24,223,107,103,125,0,169,160,149,79,38,57,175,147,7,165,1,189,39,3,56,189,31,66,212,14,121,10,87,1,149,62,51,208,92,104,123,190,210,188,77,138,39,175,198,208,209,157,211,217,241,14,134,63,170,11,217,57,2,65,0,205,161,239,135,45,227,215,201,79,10,77,169,228,187,194,38,152,100,165,177,6,157,233,61,115,143,82,94,133,220,44,167,168,33,148,186,56,70,228,58,31,60,73,174,64,7,24,152,135,143,92,63,156,177,100,200,31,94,118,100,97,144,233,183,2,64,1,31,218,72,179,56,231,20,80,87,42,97,177,108,96,169,2,126,109,149,222,8,107,108,177,93,179,140,58,52,191,250,221,154,45,245,132,246,201,154,40,134,26,104,58,105,200,88,106,125,204,12,187,161,235,26,114,169,101,251,177,91,227,9,2,65,0,161,196,164,47,255,45,0,36,49,87,20,179,243,234,181,153,49,71,244,133,104,132,47,234,21,16,10,39,172,61,2,176,62,119,116,142,111,25,110,16,63,100,105,62,120,198,92,86,26,70,240,182,102,105,179,180,47,225,91,88,42,221,26,207,2,64,49,11,34,192,3,201,115,100,90,36,75,156,144,178,10,48,144,222,170,134,60,101,243,8,90,235,55,146,219,225,179,250,123,208,253,77,76,103,193,96,172,179,162,176,113,206,141,172,129,76,178,76,244,207,118,115,197,137,56,95,48,28,126,246]
                }
            ],
            [null,0,{"id":[4,12,128,3,4,189,230,182,223,48,39,241,155,96,136,199,116,134,8,73,60,171,193,54,114,12,75,131,172,170,157,153]},{"peer_type":"Device","area_code":{"country":1,"carrier":2,"city":3,"inner":4},"peer_unique_id":[49,50,51,0,0,0,0,0,0,0,0,0,0,0,0,0],"owner_id":{"id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]},"public_key":[48,129,159,48,13,6,9,42,134,72,134,247,13,1,1,1,5,0,3,129,141,0,48,129,137,2,129,129,0,159,235,74,222,221,30,181,83,71,142,212,162,182,192,170,1,245,12,117,237,244,252,6,221,239,83,165,214,45,226,158,188,101,56,135,114,100,255,150,179,173,163,64,183,222,71,82,156,233,51,62,216,206,20,55,243,143,137,182,193,35,235,214,155,22,85,139,158,241,83,65,52,90,27,96,140,166,101,162,56,125,23,152,88,84,192,191,134,85,228,15,1,203,223,127,78,255,229,160,53,102,88,125,10,224,154,114,199,30,64,106,157,50,255,91,164,203,97,40,135,179,156,90,23,94,95,135,97,2,3,1,0,1]},[],[],[],[],null,{"secret":[48,130,2,92,2,1,0,2,129,129,0,159,235,74,222,221,30,181,83,71,142,212,162,182,192,170,1,245,12,117,237,244,252,6,221,239,83,165,214,45,226,158,188,101,56,135,114,100,255,150,179,173,163,64,183,222,71,82,156,233,51,62,216,206,20,55,243,143,137,182,193,35,235,214,155,22,85,139,158,241,83,65,52,90,27,96,140,166,101,162,56,125,23,152,88,84,192,191,134,85,228,15,1,203,223,127,78,255,229,160,53,102,88,125,10,224,154,114,199,30,64,106,157,50,255,91,164,203,97,40,135,179,156,90,23,94,95,135,97,2,3,1,0,1,2,129,128,15,37,64,31,218,78,76,101,174,64,112,146,171,212,148,101,84,185,123,142,111,251,124,133,1,99,190,133,137,217,77,124,252,147,40,42,37,54,240,80,15,187,177,231,66,187,38,47,171,219,9,114,140,21,237,201,248,157,86,39,32,73,76,226,67,58,140,41,211,181,42,227,181,136,126,160,91,145,166,242,190,165,75,123,95,56,134,23,51,25,142,94,161,109,165,172,54,236,173,122,16,126,86,170,194,234,197,126,231,143,129,72,186,236,207,213,87,207,243,129,74,103,250,53,94,68,234,191,2,65,0,213,91,130,139,116,171,248,7,189,114,174,125,221,231,250,32,185,51,165,126,122,115,169,124,140,18,115,37,51,125,246,34,136,70,188,252,25,17,29,247,176,121,158,39,57,249,115,12,148,238,106,187,57,242,89,82,126,137,248,164,207,200,248,243,2,65,0,191,225,152,111,72,94,115,69,13,127,27,117,77,143,100,37,107,149,98,186,73,1,178,192,172,137,194,244,65,235,153,78,95,151,212,248,122,219,190,64,15,22,250,5,0,76,201,250,201,110,244,119,127,217,179,248,41,101,51,214,71,247,19,91,2,64,88,234,58,13,68,34,38,203,146,248,40,31,236,74,176,194,210,107,227,156,66,11,209,166,69,249,170,98,230,130,168,0,220,42,30,184,48,250,33,51,8,40,166,240,82,8,211,25,123,55,160,227,69,210,233,198,167,85,120,121,182,222,90,97,2,65,0,181,174,64,108,21,228,192,86,0,170,122,112,62,40,232,20,250,97,174,226,93,160,11,5,158,24,174,140,34,34,150,188,251,140,66,173,167,126,139,12,18,125,153,235,196,85,240,228,209,119,138,103,223,153,93,107,58,158,197,79,62,66,182,131,2,64,54,252,131,212,140,242,229,5,125,227,89,16,47,249,121,85,161,56,248,202,10,126,139,19,97,183,89,206,64,160,107,27,65,0,178,145,102,212,135,134,60,123,133,146,252,5,123,0,178,210,11,244,171,6,101,32,64,33,54,165,45,103,234,246]}]
        ],
        "genesis": {
            "coinbase": 0,
            "coins": [
                {
                    "coin_id": 0,
                    "pre_balance": [[0, 10000]]
                }
            ],
            "price": {

            }
        }
    }
"#;

//
// // #[test]
// fn test_trans_balance_tx() {
//     let mut session = IndependDebugSession::<SqliteStorage, SqliteState>::new(TEST_CONFIG.as_bytes(), new_sqlite_storage).unwrap();
//     let (caller, nonce) = session.get_caller(0).unwrap();
//     let to = session.get_account_id(1).unwrap();
//     let coin_id: u8 = 0;
//     let tx = Tx {
//         nonce: nonce,
//         caller: caller,
//         prev: vec![],
//         gas_coin_id: 0,
//         gas_price: 0,
//         max_fee: 0,
//         condition: None,
//         body: TxBody::TransBalance(TransBalanceTx {
//             ctid: CoinTokenId::Coin(coin_id),
//             to: vec![(to, 1)]
//         })
//     };
//     let receipt = session.next_transaction(tx).unwrap();
//     assert_eq!(receipt.result, ERROR_SUCCESS);
//
//     {
//         let view_method = ViewBalanceMethod {
//             account: session.get_account_id(0).unwrap(),
//             ctid: vec![CoinTokenId::Coin(coin_id)]
//         };
//         let view_result = session.view_state_executor(view_method).exec().unwrap();
//         let (_token_id, balance) = view_result.get(0).unwrap();
//         // assert_eq!(_token_id, token_id);
//         assert_eq!(*balance, 9999);
//     }
//
//     {
//         let view_method = ViewBalanceMethod {
//             account: session.get_account_id(1).unwrap(),
//             ctid: vec![CoinTokenId::Coin(coin_id)]
//         };
//         let view_result = session.view_state_executor(view_method).exec().unwrap();
//         let (_token_id, balance) = view_result.get(0).unwrap();
//         // assert_eq!(_token_id, token_id);
//         assert_eq!(*balance, 1);
//     }
// }
//
//
// // #[test]
// fn test_union_tx() {
//     let mut session = IndependDebugSession::<SqliteStorage, SqliteState>::new(TEST_CONFIG.as_bytes(), new_sqlite_storage).unwrap();
//     // create union
//     {
//         let (caller, nonce) = session.get_caller(0).unwrap();
//         let right = session.get_account_id(1).unwrap();
//         let coin_id: u8 = 0;
//
//         let union_desc = Desc::UnionAccountDesc(UnionAccountDesc {
//             left: caller.id(),
//             right: right
//         });
//
//         let union_id = id_from_desc(&union_desc).unwrap();
//
//         let tx = Tx {
//             nonce: nonce,
//             caller: caller,
//             prev: vec![],
//             gas_coin_id: 0,
//             gas_price: 0,
//             max_fee: 0,
//             condition: None,
//             body: TxBody::CreateDesc(CreateDescTx {
//                 coin_id: coin_id,
//                 from: None,
//                 value: 0,
//                 desc : union_desc,
//                 price : 0
//             })
//         };
//
//         let receipt = session.next_transaction(tx).unwrap();
//         assert_eq!(receipt.result, ERROR_SUCCESS);
//
//         {
//             let view_method = ViewDescMethod {
//                 id: union_id
//             };
//             let desc = session.view_state_executor(view_method).exec().unwrap();
//             assert_eq!(TxCaller::from_desc(&desc).unwrap(), union_id);
//         }
//     }
// }
//
//
//
// #[test]
// fn test_create_desc() {
//     let bytes:&[u8] = TEST_CONFIG.as_bytes();
//     let mut session = IndependDebugSession::<SqliteStorage, SqliteState>::new(bytes, new_sqlite_storage).unwrap();
//     // create union
//     {
//         let (caller, nonce) = session.get_caller(0).unwrap();
//         let coin_id: u8 = 0;
//
//
//         let desc = session.get_desc(0).unwrap();
//
//         let tx = Tx {
//             nonce: nonce,
//             caller: caller,
//             prev: vec![],
//             gas_coin_id: 0,
//             gas_price: 0,
//             max_fee: 0,
//             condition: None,
//             body: TxBody::CreateDesc(CreateDescTx {
//                 coin_id: coin_id,
//                 from: None,
//                 value: 0,
//                 desc : desc,
//                 price : 0
//             })
//         };
//
//         let receipt = session.next_transaction(tx).unwrap();
//         assert_eq!(receipt.result, ERROR_SUCCESS);
//
//         {
//             let view_method = ViewDescMethod {
//                 id: session.get_account_id(0).unwrap()
//             };
//             let desc = session.view_state_executor(view_method).exec().unwrap();
//             assert_eq!(TxCaller::from_desc(&desc).unwrap(), session.get_account_id(0).unwrap());
//         }
//     }
// }
//
