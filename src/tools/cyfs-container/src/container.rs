use crate::stack::{CyfsStackHolder, CyfsStackParam};
use cyfs_base::*;
// use im_service::ImService;
use cyfs_stack_loader::CyfsServiceLoader;


const BDT_PORT_BEGIN: u16 = 30000;
const HTTP_PORT_BEGIN: u16 = 40000;

// const IM_HTTP_PORT_BEGIN:u16 = 50000;

// 一个容器，里面运行一个cyfs协议栈和一组dec-service
pub(crate) struct CyfsContainer {
    index: u16,
    stack: CyfsStackHolder,
}

impl CyfsContainer {
    pub fn new(index: u16) -> Self {
        Self {
            index,
            stack: Self::create_stack(index),
        }
    }

    pub fn index(&self) -> u16 {
        self.index
    }

    pub async fn start(&self) -> BuckyResult<()> {
        self.stack.start().await?;

        let stack = self.stack.shared_stack();
        stack.online().await.unwrap();

        Ok(())
    }

    fn create_stack(index: u16) -> CyfsStackHolder {
        let bdt_port = BDT_PORT_BEGIN + index;
        let http_port = HTTP_PORT_BEGIN + index * 2;
        let ws_port = HTTP_PORT_BEGIN + index * 2 + 1;
        let device = format!("device{}", index);

        let param = CyfsStackParam {
            device,
            bdt_port,
            http_port,
            ws_port,
        };

        CyfsStackHolder::new(param)
    }
}

pub(crate) struct CyfsContainerManager {
    // 容器的数量
    count: u16,

    list: Vec<CyfsContainer>,
}

impl CyfsContainerManager {
    pub fn new(count: u16) -> Self {
        let mut ret = Self {
            count,
            list: Vec::new(),
        };

        ret.init_stack_list();

        ret
    }

    fn init_stack_list(&mut self) {
        for i in 0..self.count {
            let stack = CyfsContainer::new(i);
            self.list.push(stack);
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        CyfsServiceLoader::prepare_env().await?;

        // 分批初始化container
        let step = 4;

        let mut current = 0;
        loop {
            let mut fut = Vec::new();
            for container in self.list.iter().skip(current).take(step).into_iter() {
                fut.push(container.start());
            }
            if fut.is_empty() {
                break;
            }

            let count = fut.len();
            let rets = futures::future::join_all(fut).await;
            for (i, ret) in rets.into_iter().enumerate() {
                if ret.is_err() {
                    error!("start container error! index={}, {:?}", i + current, ret);
                    return ret;
                }
            }

            current += count;
        }

        warn!("all continers start finish");

        Ok(())
    }
}
