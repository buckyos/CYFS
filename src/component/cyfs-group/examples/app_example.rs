mod Client {
    use cyfs_base::ObjectId;
    use cyfs_core::GroupProposal;
    use cyfs_group::RPathClient;

    pub struct DecClient {}

    impl DecClient {
        async fn do_something(&self, stack: &CyfsStack) {
            let rpath_mgr = stack.group_mgr();

            let rpath_client = rpath_mgr.rpath_client();

            let field_path = "/xxx/yyy";
            let old_value = rpath_client.get_field(field_path).await;
            let param = ObjectId::default(); // param = old_value.value
            let proposal = self.make_proposal(param);
            rpath_client.post_proposal(proposal).await;
        }

        fn make_proposal(&self, param: ObjectId) -> GroupProposal {
            unimplemented!()
        }
    }
}

mod GroupDecService {
    use cyfs_base::*;
    use cyfs_core::{DecAppId, GroupConsensusBlock, GroupProposal};
    use cyfs_group::{DelegateFactory, ExecuteResult, RPathDelegate};

    pub struct DecService {}

    impl DecService {
        pub async fn run(stack: &CyfsStack) {
            let rpath_mgr = stack.group_mgr();

            let appid = DecAppId::default();
            let delegate_factory = GroupRPathDelegateFactory {};

            rpath_mgr.register(appid, delegate_factory)
        }
    }

    pub struct GroupRPathDelegateFactory {}

    impl GroupRPathDelegateFactory {
        pub fn is_accept(
            &self,
            group: &Group,
            rpath: &str,
            with_block: Option<&GroupConsensusBlock>,
        ) -> bool {
            // 由应用定义是否接收该rpath，并启动共识过程，参与该rpath的信息维护
            true
        }
    }

    #[async_trait::async_trait]
    impl DelegateFactory for GroupRPathDelegateFactory {
        async fn create_rpath_delegate(
            &self,
            group: &Group,
            rpath: &str,
            with_block: Option<&GroupConsensusBlock>,
        ) -> BuckyResult<Box<dyn RPathDelegate>> {
            if self.is_accept(group, rpath, with_block) {
                // 如果接受，就提供该rpath的处理响应对象
                Ok(Box::new(MyRPathDelegate::new()))
            } else {
                Err(BuckyError::new(BuckyErrorCode::Reject, ""))
            }
        }

        async fn on_state_changed(
            &self,
            group_id: &ObjectId,
            rpath: &str,
            state_id: Option<ObjectId>,
            pre_state_id: Option<ObjectId>,
        ) {
            unimplemented!()
        }
    }

    pub struct MyRPathDelegate {}

    impl MyRPathDelegate {
        pub fn new() -> Self {
            MyRPathDelegate {}
        }
    }

    impl MyRPathDelegate {
        pub fn execute(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
        ) -> BuckyResult<ExecuteResult> {
            let result_state_id = {
                /**
                 * pre_state_id是一个MAP的操作对象，形式待定，可能就是一个SingleOpEnv，但最好支持多级路径操作
                 */
                ObjectId::default()
            };

            let receipt = {
                /**
                 * 返回给Client的对象，相当于这个请求的结果或者叫回执？
                 */
                None
            };

            let context = {
                /**
                 * 执行请求的上下文，运算过程中可能有验证节点无法得到的上下文信息（比如时间戳，随机数）
                 */
                Some(vec![])
            };

            /**
             * (result_state_id, return_object) = pre_state_id + proposal + context
             */
            Ok(ExecuteResult {
                context,
                result_state_id: Some(result_state_id),
                receipt,
            })
        }

        pub fn verify(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<bool> {
            /**
             * let is_same = (execute_result.result_state_id, execute_result.return_object)
             *  == pre_state_id + proposal + execute_result.context
             */
            Ok(true)
        }
    }

    #[async_trait::async_trait]
    impl RPathDelegate for MyRPathDelegate {
        async fn on_execute(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
        ) -> BuckyResult<ExecuteResult> {
            self.execute(proposal, pre_state_id)
        }

        async fn on_verify(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
        ) -> BuckyResult<bool> {
            self.verify(proposal, pre_state_id, execute_result)
        }

        async fn on_commited(
            &self,
            proposal: &GroupProposal,
            pre_state_id: Option<cyfs_base::ObjectId>,
            execute_result: &ExecuteResult,
        ) {
            // 提交到共识链上了，可能有些善后事宜
            unimplemented!()
        }

        async fn get_group(&self, group_chunk_id: Option<&ObjectId>) -> BuckyResult<Group> {
            // 自定义同步Group版本策略
            unimplemented!()
        }
    }
}
