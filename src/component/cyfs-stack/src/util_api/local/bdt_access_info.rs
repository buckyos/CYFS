use cyfs_base::BuckyResult;
use cyfs_lib::*;
use cyfs_bdt::StackGuard;

#[derive(Clone)]
pub(crate) struct BdtNetworkAccessInfoManager {
    stack: StackGuard,
}

impl BdtNetworkAccessInfoManager {
    pub fn new(stack: StackGuard) -> Self {
        Self { stack }
    }

    fn append(info: &mut BdtNetworkAccessInfo, ep: BdtNetworkAccessEndpoint) {
        if ep.lan_ep.addr().is_ipv4() {
            if info.v4.iter().find(|&v| *v == ep).is_some() {
                return;
            }

            info.v4.push(ep);
        } else {
            if info.v6.iter().find(|&v| *v == ep).is_some() {
                return;
            }

            info.v6.push(ep);
        }
    }

    pub fn update_access_info(&self) -> BuckyResult<BdtNetworkAccessInfo> {
        let mut info = BdtNetworkAccessInfo::default();

        // 遍历当前所有bind成功的socket
        let listener = self.stack.net_manager().listener();
        for u in listener.udp() {
            let ep;
            // local 是bind 的lan ep， outer是从sn看到的 wan ep
            if u.outer().is_some() {
                // 如果lan ep 和 wan ep不同，说明udp socket和 sn经过一次成功的ping
                debug!(
                    "udp local_endpoint:{} outer_endpoint:{}",
                    &u.local(),
                    u.outer().unwrap()
                );

                ep = BdtNetworkAccessEndpoint {
                    lan_ep: u.local().clone(),
                    wan_ep: u.outer().unwrap(),
                    access_type: BdtNetworkAccessType::NAT,
                };
            } else if u.local().is_static_wan() {
                ep = BdtNetworkAccessEndpoint {
                    lan_ep: u.local().clone(),
                    wan_ep: u.local().clone(),
                    access_type: BdtNetworkAccessType::NAT,
                };

                // 如果lan ep 和 wan ep 相同，如果ep标明了是固定外网地址，说明udp socket就是固定外网地址
                debug!("udp wan_static_endpoint: {}", &u.local());
            } else {
                // 否则，说明udp socket虽然绑定成功了，但是并不可达（至少没法ping通 sn)
                debug!("udp not reachable: {}", &u.local());
                continue;
            }

            Self::append(&mut info, ep);
        }

        for t in listener.tcp() {
            let ep;

            // local 是bind 的lan ep， outer是拼出来的 wan ep
            if t.local().is_static_wan() {
                // 如果 tcp 的local ep标明了是固定外网地址，直接显式tcp 监听了这个port
                // 如果 tcp 的outer 和 local 不一样，说明同local ip的udp socket 至少ping通过一次sn
                // 这个tcp outer要么是 ipv6外网ep ； 要么是通过stack 的 tcp_map_port 生成的外网ep， 都是可达的
                debug!("tcp wan_staic_endponit:{}", t.outer().unwrap());

                ep = BdtNetworkAccessEndpoint {
                    lan_ep: t.local().clone(),
                    wan_ep: t.local().clone(),
                    access_type: BdtNetworkAccessType::WAN,
                };
            } else if t.outer().is_some() {
                debug!("tcp wan_staic_endponit:{}", t.outer().unwrap());

                ep = BdtNetworkAccessEndpoint {
                    lan_ep: t.local().clone(),
                    wan_ep: t.outer().unwrap(),
                    access_type: BdtNetworkAccessType::WAN,
                };
            } else {
                // 如果tcp 监听的是内网ipv4 ep， 不能保证tcp 可达，至少外网不可达， 内网是否可达不知道（可能是虚拟网卡之类，即便bind成功也没有意义）
                debug!("tcp local_endpoint:{} ", &t.local());

                ep = BdtNetworkAccessEndpoint {
                    lan_ep: t.local().clone(),
                    wan_ep: t.local().clone(),
                    access_type: BdtNetworkAccessType::NAT,
                };
            }

            Self::append(&mut info, ep);
        }

        // 遍历所有sn 的状态
        let sn_client = self.stack.sn_client();
        let sn_list = sn_client.sn_list();
        for sn in sn_list {
            let status = sn_client.status_of(&sn).unwrap();

            debug!("sn status of {} is {}", sn, status);

            info.sn.push(BdtNetworkAccessSn {
                sn,
                sn_status: status,
            });
        }

        Ok(info)
    }
}
