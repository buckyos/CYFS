pub const ERROR_SUCCESS:u16 = 0;                    // 执行成功
pub const ERROR_NOT_FOUND:u16 = 1;                  // 找不到操作对象
pub const ERROR_NO_ENOUGH_BALANCE:u16 = 2;          // 账户余额不足
pub const ERROR_GAS_COIN_ID:u16 = 3;                // 手续费货币类型错误
pub const ERROR_GAS_PRICE:u16 = 4;                  // 单位燃料费不足
pub const ERROR_OUT_OF_GAS:u16 = 5;                 // 手续费不足
pub const ERROR_TO_IS_ZERO:u16 = 6;                 // 接收者地址为0地址
pub const ERROR_DESC_TYPE:u16 = 7;                  // 身份公钥错误
pub const ERROR_TX_BODY_TYPE:u16 = 8;               // 交易内容类型错误
pub const ERROR_ACCESS_DENIED:u16 = 9;              // 限制访问, 权限不足
pub const ERROR_NEED_CONDITION:u16 = 10;            // 交易条件为空
pub const ERROR_SAME_FROM_OP:u16 = 11;              // 交易操作标识和执行者相同
pub const ERROR_TOTAL_BALANCE:u16 = 12;             // 联合账户余额不足
pub const ERROR_TOO_SMALL_SEQ:u16 = 13;             // 联合账户操作序号太小了
pub const ERROR_DESC_PRICE:u16 = 14;                // 身份上链价格低于上链费
pub const ERROR_DESC_VALUE:u16 = 15;                // 身份上链费用不足
pub const ERROR_ALREADY_EXIST:u16 = 16;             // 目标对象已经存在
pub const ERROR_DESC_STATE_NOT_NORMAL:u16 = 17;     // 链上身份欠费了， 待续费
pub const ERROR_OP_NOT_FOUND:u16 = 18;              // 操作目标找不到
pub const ERROR_OP_IS_OWNER :u16 = 19;             // 操作对象是交易的所有者
pub const ERROR_NAME_BUY_PRICE:u16 = 20;            // 域名购买价格低于最低购买门槛
pub const ERROR_NAME_RENT_PRICE:u16 = 21;           // 域名租用价格低于最低租用门槛
pub const ERROR_NAME_STATE_ERROR:u16 = 22;          // 域名当前状态异常
pub const ERROR_PARAM_ERROR:u16 = 23;              // 参数解析错误
pub const ERROR_INVALID_PACKAGE:u16 = 24;          // 无效的数据包
pub const ERROR_MEMORY_LITTLE:u16 = 25;            // 程序内存不足
pub const ERROR_VERSION_ERROR:u16 = 26;            // 协议版本错误
pub const ERROR_DESC_ERROR:u16 = 27;                // 创建身份公钥错误
pub const ERROR_RENT_ARREARS:u16 = 28;             // 租用拖欠状态
pub const ERROR_BID_COIN_NOT_MATCH:u16 = 29;       // 购买域名货币类型不匹配
pub const ERROR_BID_PRICE_TOO_LOW:u16 = 30;        // 域名竞价价格太低
pub const ERROR_BID_NO_AUTH:u16 = 31;              // 域名竞价鉴权失败
pub const ERROR_HEIGHT_NOT_CHANGE:u16 = 32;        // 铸币区块高度没有变化
pub const ERROR_HAS_BID:u16 = 33;                  // 重复购买域名
pub const ERROR_OTHER_CHARGED:u16 = 34;            // 其他支付费用
pub const ERROR_CANT_FIND_LEFT_USER_DESC:u16 = 35; // 联合账户找不到左侧用户公钥
pub const ERROR_CANT_FIND_RIGHT_USER_DESC:u16 = 36; //联合账户找不到右侧用户公钥
pub const ERROR_LEFT_ACCOUNT_TYPE:u16 = 37;        //左侧账户类型不匹配
pub const ERROR_RIGHT_ACCOUNT_TYPE:u16 = 38;       // 右侧账户类型不匹配
pub const ERROR_EXCEPTION:u16 = 100;               // 执行操作程序异常
pub const ERROR_INVALID:u16 = 101;                 // 无效的操作
pub const ERROR_GENESIS_MINER_BLOCK_INVALID:u16 = 102; //挖矿区块无效
pub const ERROR_SIGNATURE_ERROR:u16 = 103;         // 签名错误
pub const ERROR_NETWORK_ERROR:u16 = 104;           // 网络连接错误
pub const ERROR_REQUEST_FAILED:u16 = 105;          // 网络请求失败
pub const ERROR_BLOCK_VERIFY_FAILED:u16 = 106;     // 区块验证失败
pub const ERROR_NONE_MINERS:u16 = 107;             // 无效的挖矿矿工
pub const ERROR_PROOF_TYPE_ERROR:u16 = 108;        // 结算证明错误
pub const ERROR_UNKNOWN_CONTRACT_TYPE:u16 = 109;   // 未知合约交易类型
pub const ERROR_CANT_FIND_CONTRACT:u16 = 110;      // 找不到合约交易内容
pub const ERROR_CONTRACT_HAS_STOP:u16 = 111;       // 合约已经终止了
pub const ERROR_NO_BODY_DATA:u16 = 112;            // 找不到交易内容数据
pub const ERROR_PARSE_BODY_FAILED:u16 = 113;       // 解析交易内容失败
pub const ERROR_HASH_ERROR:u16 = 114;              // 交易签名不匹配
pub const ERROR_SUBCHAIN_WITHDRAW_TX_ERROR:u16 = 115;  // 子链提现交易错误
pub const ERROR_BLOCK_DECODE_FAILED:u16 = 116;     // 区块解码失败
pub const ERROR_UNKNOWN_EXTENSION_TX:u16 = 117;    // 未知的扩展类型交易
pub const ERROR_PUBLIC_KEY_NOT_EXIST:u16 = 118;    // 公钥不存在
pub const ERROR_SKIP:u16 = 119;                    // 事件略过忽略
pub const ERROR_TX_DECODE_FAILED:u16 = 120;        // 交易解码失败
pub const ERROR_TX_VERIFY_FAILED:u16 = 121;        // 交易验证失败

// 先将EVM的Reason转换到这里的Code，和Receipt保持一致
pub const ERROR_SUCCESS_STOPPED:u16 = 122;         // 成功执行， 无返回值
pub const ERROR_SUCCESS_SUICIDED:u16 = 123;        // 执行成功， 销毁合约
pub const ERROR_STACK_UNDERFLOW:u16 = 124;         // 栈溢出下溢
pub const ERROR_STACK_OVERFLOW:u16 = 125;          // 栈溢出上溢
pub const ERROR_INVALID_JUMP:u16 = 126;            // 无效的跳转操作
pub const ERROR_INVALID_RANGE:u16 = 127;           //无效的遍历操作
pub const ERROR_DESIGNATED_INVALID:u16 = 128;      // 特定执行程序失败
pub const ERROR_CALL_TOO_DEEP:u16 = 129;           // 调用堆栈太深了
pub const ERROR_CREATE_COLLISION:u16 = 130;        // 创建冲突操作
pub const ERROR_CREATE_CONTRACT_LIMIT:u16 = 131;   // 创建合约限制
pub const ERROR_OUT_OF_OFFSET:u16 = 132;           // 偏移量越界了
pub const ERROR_PC_UNDERFLOW:u16 = 133;            // PC 溢出
pub const ERROR_CREATE_EMPTY:u16 = 134;            // 创建交易为空
pub const ERROR_OTHER_EVM_ERR:u16 = 135;           // 其他运行时错误
pub const ERROR_REVERT:u16 = 136;                  // 交易回滚
pub const ERROR_NOT_SUPPORT:u16 = 137;             // 不支持的操作
pub const ERROR_UNHANDLED_INTERRUPT:u16 = 138;     // 异常中断
pub const ERROR_OTHER_EVM_FATAL:u16 = 139;         // 其他运行时终止
pub const ERROR_NOT_ENOUGH_FEE:u16 = 140;          // 手续费不够

pub const ERROR_NFT_IS_AUCTIONING: u16 = 141;
pub const ERROR_NFT_IS_SELLING: u16 = 142;
pub const ERROR_NFT_IS_NOT_AUCTIONING: u16 = 143;
pub const ERROR_NFT_IS_NOT_SELLING: u16 = 144;
pub const ERROR_NFT_HAS_APPLY_OTHER_COIN: u16 = 145;
pub const ERROR_NFT_USER_NOT_APPLY_BUY: u16 = 146;
pub const ERROR_NFT_IS_OWNER: u16 = 147;
pub const ERROR_NFT_IS_NORMAL: u16 = 148;
pub const ERROR_NFT_CREATE_ONLY_OWNER: u16 = 149;
pub const ERROR_NFT_IS_SUB: u16 = 150;
pub const ERROR_NFT_LIST_HAS_SELLED_ANY: u16 = 151;

pub const ERROR_BUCKY_ERR_START:u16 = 10000;       // 用于需要MetaErrCode返回，但是又接收到了一个BuckyError的情况，这里做BuckyErrCode到MetaErrCode的转换

// 先将EVM的错误码转换成code，和Receipt的返回码保持一致
use crate::evm_def::{ExitReason, ExitSucceed, ExitError, ExitFatal};

pub fn evm_error_to_code(err: ExitError) -> u16 {
    match err {
        ExitError::StackUnderflow => ERROR_STACK_UNDERFLOW,
        ExitError::StackOverflow => ERROR_STACK_OVERFLOW,
        ExitError::InvalidJump => ERROR_INVALID_JUMP,
        ExitError::InvalidRange => ERROR_INVALID_RANGE,
        ExitError::DesignatedInvalid => ERROR_DESIGNATED_INVALID,
        ExitError::CallTooDeep => ERROR_CALL_TOO_DEEP,
        ExitError::CreateCollision => ERROR_CREATE_COLLISION,
        ExitError::CreateContractLimit => ERROR_CREATE_CONTRACT_LIMIT,
        ExitError::OutOfOffset => ERROR_OUT_OF_OFFSET,
        ExitError::OutOfGas => ERROR_OUT_OF_GAS,
        ExitError::OutOfFund => ERROR_NO_ENOUGH_BALANCE,
        ExitError::PCUnderflow => ERROR_PC_UNDERFLOW,
        ExitError::CreateEmpty => ERROR_CREATE_EMPTY,
        ExitError::Other(_) => ERROR_OTHER_EVM_ERR,
    }
}

pub fn evm_reason_to_code(reason: ExitReason) -> u16 {
    match reason {
        /*
        * 按我自己的理解
        * Stopped：函数成功执行，没有返回值
        * Returned：函数成功执行，有返回值
        * Suicided：函数自己终止了自己？也算成功执行
        */
        ExitReason::Succeed(succ) => {
            match succ {
                ExitSucceed::Stopped => ERROR_SUCCESS,
                ExitSucceed::Returned => ERROR_SUCCESS,
                ExitSucceed::Suicided => ERROR_SUCCESS,
            }
        }
        ExitReason::Error(err) => evm_error_to_code(err),
        ExitReason::Revert(_) => ERROR_REVERT,
        ExitReason::Fatal(fatal) => {
            match fatal {
                ExitFatal::NotSupported => ERROR_NOT_SUPPORT,
                ExitFatal::UnhandledInterrupt => ERROR_UNHANDLED_INTERRUPT,
                ExitFatal::CallErrorAsFatal(err) => evm_error_to_code(err),
                ExitFatal::Other(_) => ERROR_OTHER_EVM_FATAL,
            }
        }
    }
}
