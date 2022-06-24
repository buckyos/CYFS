use crate::state_storage::{StateRef};
use cyfs_base_meta::{BlockHeader, EventType, Event, CoinTokenId, RentParam, NameRentParam};
use cyfs_base::ERROR_EXCEPTION;
use log::*;
use crate::executor::context::{ConfigRef};
use super::context;

/**
    把扣租金，改变Name状态都做成一种Event，以后还可以再加Event的类型
    event有 两个参数(EventType(Param), BlockNumber)，表示事件类型，参数，触发的块高度
    通用做法：
        1. 添加一个新Event ref_state.add_event(EventType, height)，把event加到数据库
        2. 在每块执行的时候，execute_event对每种Event，调用对应的event_xxx(height)
        3. 在event_xxx实现里，查找对应EventType和height的数据，然后对每个数据去执行逻辑
*/
pub struct EventContext {
    ref_state: StateRef,
    block: context::Block,
    config: ConfigRef
}

pub fn execute_event(block: &BlockHeader, ref_state: &StateRef, config: ConfigRef) -> Result<(), u32> {
    // 每一种Event都会在这里有个对应的执行函数

    let context = EventContext {
        ref_state: ref_state.clone(),
        block: context::Block::new(block),
        config
    };
    // 做成单独实现，实现者自己在内部再调用getEvent(type, height)去取
    // 考虑1： 减少单次查询量
    // 考虑2： 执行者想要输出一些汇总信息，比如rent输出该块总租金到coinbase tx
    event_rent(&context)?;
    event_name_rent(&context)?;
    event_check_name_state(&context)?;

    ref_state.drop_event(block.number())?;
    Ok(())
}

fn event_rent(context: &EventContext) -> Result<(), u32> {
    for event in context.ref_state.get_event(EventType::Rent, context.block.number())? {
        if let Event::Rent(param) = event {
            let mut state = context.ref_state.get_desc_extra(&param.id)?;
            let num = if state.data_len % 1024 == 0 {state.data_len/1024} else {state.data_len/1024 + 1};
            let rent_value = (state.rent_value as i64 * num) as i64;
            let balance = context.ref_state.get_balance(&param.id, &CoinTokenId::Coin(state.coin_id))?;
            if balance >= rent_value {
                context.ref_state.dec_balance(&CoinTokenId::Coin(state.coin_id), &state.obj_id, rent_value)?;
                context.ref_state.inc_balance(&CoinTokenId::Coin(state.coin_id), context.block.coinbase(), rent_value)?;
            } else if balance > 0 {
                context.ref_state.dec_balance(&CoinTokenId::Coin(state.coin_id), &state.obj_id, balance)?;
                context.ref_state.inc_balance(&CoinTokenId::Coin(state.coin_id), context.block.coinbase(), balance)?;
            }
            let rent_arrears = rent_value - balance;
            if rent_arrears > 0 {
                state.rent_arrears += rent_arrears;
                context.ref_state.update_desc_extra(&state)?;
            }

            context.ref_state.add_or_update_event(param.id.to_string().as_str(), Event::Rent(RentParam {id: param.id.clone()})
                                          , context.block.number() + context.config.get_rent_cycle() as i64)?
        }
    }
    Ok(())
}

fn event_name_rent(context: &EventContext) -> Result<(), u32> {
    for event in context.ref_state.get_event(EventType::NameRent, context.block.number())? {
        if let Event::NameRent(param) = event {
            let mut state = context.ref_state.get_name_extra(param.name_id.as_str())?;

            let coin_id = context.config.name_rent_coin_id();
            let balance = context.ref_state.get_balance(&state.owner, &CoinTokenId::Coin(coin_id))?;
            let old_rent_arrears = state.rent_arrears;
            if balance >= state.rent_value + state.rent_arrears {
                context.ref_state.dec_balance(&CoinTokenId::Coin(coin_id), &state.owner, state.rent_value + state.rent_arrears)?;
                context.ref_state.inc_balance(&CoinTokenId::Coin(coin_id), context.block.coinbase(), state.rent_value + state.rent_arrears)?;
                state.rent_arrears = 0;
            } else if balance > 0 {
                context.ref_state.dec_balance(&CoinTokenId::Coin(coin_id), &state.owner, balance)?;
                context.ref_state.inc_balance(&CoinTokenId::Coin(coin_id), context.block.coinbase(), balance)?;
                state.rent_arrears = state.rent_arrears + state.rent_value - balance;
            } else {
                state.rent_arrears += state.rent_value;
            }
            if old_rent_arrears != state.rent_arrears {
                context.ref_state.add_or_update_name_extra(&state)?;
            }

            context.ref_state.add_or_update_event(param.name_id.as_str(), Event::NameRent(NameRentParam{name_id: param.name_id.clone()}), context.block.number() + context.config.get_rent_cycle())?
        }
    }

    Ok(())
}

fn event_check_name_state(context: &EventContext) -> Result<(), u32> {
    for event in context.ref_state.get_event(EventType::ChangeName, context.block.number())? {
        if let Event::ChangeNameEvent(param) = event {
            context.ref_state.update_name_state(&param.name, param.to)?;
        } else {
            error!("find error type event: {}", event.get_type() as i32);
            return Err(ERROR_EXCEPTION);
        }
    }
    Ok(())
}
