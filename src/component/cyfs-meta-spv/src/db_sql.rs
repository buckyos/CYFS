// 当前的数据库版本
pub(super) const CURRENT_VERSION: i32 = 0;

pub(super) const CREATE_MAIN_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS erc20_contract_tx (
    address TEXT NOT NULL,
    hash CHAR(45) NOT NULL,
    number INTEGER NOT NULL,
    _from CHAR(45) NOT NULL,
    _to CHAR(45) NOT NULL,

    gas_price INTEGER,
    created_time INTEGER,
    value INTEGER,
    result INTEGER
)"#;

pub(super) const MAIN_TABLE_ADDRESS_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `main_table_address_index` on `erc20_contract_tx` (`address`);
"#;

pub(super) const MAIN_TABLE_FROM_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `main_table_from_index` on `erc20_contract_tx` (`_from`, `number`);
"#;

pub(super) const MAIN_TABLE_TO_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `main_table_to_index` on `erc20_contract_tx` (`_to`, `number`);
"#;

pub(super) const INIT_ERC20_CONTRACT_TX_SQL_LIST: [&'static str; 4] = [
    CREATE_MAIN_TABLE,
    MAIN_TABLE_ADDRESS_INDEX,
    MAIN_TABLE_FROM_INDEX,
    MAIN_TABLE_TO_INDEX,
];

pub(super) const INSERT_CALL_CONTRACT_SQL: &str = r#"
    insert into erc20_contract_tx (
        address, 
        hash, 
        number, 
        _from, 
        _to,

        value,
        gas_price,
        created_time,
        result
        ) 
        values 
        (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);
"#;

pub(super) const SELECT_SQL: &str = r#"
    select 
        address, 
        hash, 
        number, 
        _from,
        _to, 
        value
         
        from  erc20_contract_tx
        where
        address = ?1 and number between ?2 and ?3  and _from in (?4) and _to in (?5) order by number DESC limit 20;
"#;

/*
// 需要注意新加的列只能在末尾！！！
// 版本0->1的升级
pub(super) const MAIN_TABLE_UPDATE_1_1: &'static str = r#"
ALTER TABLE `erc20_contract_tx` ADD COLUMN zone_seq UNSIGNED BIG INT DEFAULT 0
"#;
pub(super) const MAIN_TABLE_UPDATE_1_2: &'static str = r#"
ALTER TABLE `erc20_contract_tx` ADD COLUMN rank TINYINT
"#;

pub(super) const MAIN_TABLE_UPDATE_1: [&'static str; 3] =
    [MAIN_TABLE_UPDATE_1_1, MAIN_TABLE_UPDATE_1_2, MAIN_TABLE_ZONE_SEQ_INDEX];
*/

// 所有的版本升级, MAIN_TABLE_UPDATE_LIST[CURRENT_VERSION - 1]就是对应的升级sql
pub(super) const MAIN_TABLE_UPDATE_LIST: [[&'static str; 0]; CURRENT_VERSION as usize] = [];
