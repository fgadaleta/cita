//use contracts::permission_management::contains_resource;
use cita_types::{Address, H256, U256};
use grpc::Server;
use grpc_contracts::service_registry;
use libexecutor::executor::Executor;
use libproto::executor::{LoadRequest, LoadResponse, RegisterRequest, RegisterResponse};
use libproto::executor_grpc::{ExecutorService, ExecutorServiceServer};
use std::str::FromStr;
use std::sync::Arc;
use types::ids::BlockId;

pub struct ExecutorServiceImpl {
    ext: Arc<Executor>,
}

impl ExecutorService for ExecutorServiceImpl {
    // add code here
    fn register(
        &self,
        _o: ::grpc::RequestOptions,
        req: RegisterRequest,
    ) -> ::grpc::SingleResponse<RegisterResponse> {
        let mut r = RegisterResponse::new();
        {
            let caddr = req.get_contract_address().to_string();
            let ip = req.get_ip();
            let port = req.get_port();

            if let Ok(iport) = port.parse::<u16>() {
                service_registry::register_contract(
                    Address::from_str(&caddr).unwrap(),
                    ip.to_string(),
                    iport,
                    0,
                );
                r.set_state(format!("OK {}---{}:{}", caddr, ip, port));
            } else {
                r.set_state(format!("Error Register {}---{}:{}", caddr, ip, port));
            }
        }
        ::grpc::SingleResponse::completed(r)
    }

    fn load(
        &self,
        _o: ::grpc::RequestOptions,
        req: LoadRequest,
    ) -> ::grpc::SingleResponse<LoadResponse> {
        let mut r = LoadResponse::new();

        let caddr = req.get_contract_address().to_string();
        let req_key = req.get_key();
        let key = H256::from_slice(String::from(req_key).as_bytes());

        let address = Address::from_str(&caddr).unwrap();
        let mut hi: u64 = 0;
        if let Some(info) = service_registry::find_contract(address, true) {
            hi = info.height;
        }
        //
        //        if hi == 0 {
        //            if let Some(value) = self.ext.db.read().read(db::COL_EXTRA, &address) {
        //                hi = value.height
        //            }
        //        }
        if hi == 0 {
            error!("contract address {} have not created", caddr);
            r.set_value("".to_string());
        } else {
            let out = self.get_bytes(BlockId::Number(hi), &address, key);
            let value = String::from_utf8(out).unwrap();
            trace!("load find value: {}", value);
            r.set_value(value);
        }
        ::grpc::SingleResponse::completed(r)
    }
}

impl ExecutorServiceImpl {
    pub fn new(ext: Arc<Executor>) -> Self {
        ExecutorServiceImpl { ext: ext }
    }

    //  get vec
    fn get_bytes(&self, block_id: BlockId, address: &Address, key: H256) -> Vec<u8> {
        let mut out = Vec::new();
        match self.ext.state_at(block_id) {
            Some(state) => {
                if let Ok(len) = state.storage_at(&address, &key) {
                    let len = len.low_u64();
                    let hlen = len / 32;
                    let modnum = len & 0x1f;
                    let mut pos = U256::from(key);

                    for _ in 0..hlen {
                        pos = pos + U256::one();
                        if let Ok(val) = state.storage_at(&address, &H256::from(pos)) {
                            out.extend_from_slice(val.as_ref());
                        } else {
                            return Vec::new();
                        }
                    }

                    if modnum > 0 {
                        pos = pos + U256::one();
                        if let Ok(val) = state.storage_at(&address, &H256::from(pos)) {
                            out.extend_from_slice(&(val.as_ref() as &[u8])[0..modnum as usize])
                        }
                    }
                }
                out
            }
            None => {
                error!("state do not find by height");
                out
            }
        }
    }
}

pub fn vm_grpc_server(port: u16, ext: Arc<Executor>) -> Option<Server> {
    let mut server = ::grpc::ServerBuilder::new_plain();
    server.http.set_port(port);
    server.add_service(ExecutorServiceServer::new_service_def(
        ExecutorServiceImpl::new(ext),
    ));
    server.http.set_cpu_pool_threads(4);
    match server.build() {
        Ok(server) => Some(server),
        Err(_) => None,
    }
}
