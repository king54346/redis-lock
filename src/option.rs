use std::time::Duration;

// 默认连接池超过 10 s 释放连接
const DEFAULT_IDLE_TIMEOUT_SECONDS: u64 = 10;
// 默认连接池最大连接数
const DEFAULT_MAX_ACTIVE: u32 = 100;
// 默认连接池最小空闲连接数
const DEFAULT_MIN_IDLE: u32 = 20;


// 默认的分布式锁过期时间
const DEFAULT_LOCK_EXPIRE_SECONDS: u64 = 30;

// 默认的看门狗工作间隔时间
pub const WATCH_DOG_WORK_STEP_SECONDS: u64 = 10;

// 默认的阻塞等待时间
const BLOCK_WAITING_SECONDS: u64 = 5;

// 默认的单个节点锁的超时时间
const DEFAULT_SINGLE_LOCK_TIMEOUT:u64 = 50 ;

#[derive(Debug, Clone)]
pub struct LockOptions {
    // 是否阻塞
    pub block: bool,
    // 阻塞等待时间
    pub block_waiting_seconds: u64,
    // 锁过期时间
    pub expire_seconds: u64,
    // 是否启动看门狗
    pub watch_dog_mode: bool,
}

// default
impl Default for LockOptions {
    fn default() -> Self {
        LockOptions {
            block: false,
            block_waiting_seconds: BLOCK_WAITING_SECONDS,
            expire_seconds: 0,  //根据业务的执行时间而定，锁的时间应该大于业务的执行时间
            watch_dog_mode: false,
        }
    }
}

impl LockOptions {
    pub fn new() -> Self {
        LockOptions::default()
    }
    pub fn block(mut self, block: bool) -> Self {
        self.block = block;
        self
    }
    pub fn block_waiting_seconds(mut self, block_waiting_seconds: u64) -> Self {
        self.block_waiting_seconds = block_waiting_seconds;
        self
    }
    pub fn expire_seconds(mut self, expire_seconds: u64) -> Self {
        self.expire_seconds = expire_seconds;
        self
    }
    pub fn watch_dog_mode(mut self, watch_dog_mode: bool) -> Self {
        self.watch_dog_mode = watch_dog_mode;
        self
    }
}

pub fn repair_lock_option(options:&mut LockOptions) {
    if options.block && options.block_waiting_seconds <= 0 {
        // 默认阻塞等待时间上限为 5 秒
        options.block_waiting_seconds = 5
    }

    // 倘若未设置分布式锁的过期时间，则会启动 watchdog
    if options.expire_seconds > 0 {
        return
    }

    // 用户未显式指定锁的过期时间，则此时会启动看门狗
    options.expire_seconds = DEFAULT_LOCK_EXPIRE_SECONDS;
    options.watch_dog_mode = true

}

pub struct ClientOptions {
    pub min_idle: u32,
    pub idle_timeout_seconds: u64,
    pub max_active: u32,
}
// defalult
impl Default for ClientOptions {
    fn default() -> Self {
        ClientOptions {
            min_idle: DEFAULT_MIN_IDLE,
            idle_timeout_seconds: DEFAULT_IDLE_TIMEOUT_SECONDS,
            max_active: DEFAULT_MAX_ACTIVE
        }
    }
}

impl ClientOptions {
    pub fn new() -> Self {
        ClientOptions::default()
    }
    pub fn min_idle(mut self, min_idle: u32) -> Self {
        self.min_idle = min_idle;
        self
    }
    pub fn idle_timeout_seconds(mut self, idle_timeout_seconds: u64) -> Self {
        self.idle_timeout_seconds = idle_timeout_seconds;
        self
    }
    pub fn max_active(mut self, max_active: u32) -> Self {
        self.max_active = max_active;
        self
    }
}

pub fn repair_client_options(options: &mut ClientOptions) {
    if options.min_idle <= 0 {
        options.min_idle = DEFAULT_MIN_IDLE;
    }
    if options.idle_timeout_seconds <= 0 {
        options.idle_timeout_seconds = DEFAULT_IDLE_TIMEOUT_SECONDS;
    }
    if options.max_active <= 0 {
        options.max_active = DEFAULT_MAX_ACTIVE;
    }
}


pub struct RedLockOptions {
    pub single_nodes_timeout: u64, // 单位毫秒
    pub expire_duration: u64, // 单位秒
}

impl RedLockOptions{
    pub fn new() -> Self {
        RedLockOptions {
            single_nodes_timeout: DEFAULT_SINGLE_LOCK_TIMEOUT,
            expire_duration: 0,
        }
    }
    pub fn single_nodes_timeout(mut self, single_nodes_timeout: u64) -> Self {
        self.single_nodes_timeout = single_nodes_timeout;
        self
    }
    pub fn expire_duration(mut self, expire_duration: u64) -> Self {
        self.expire_duration = expire_duration;
        self
    }
}

impl Default for RedLockOptions {
    fn default() -> Self {
        RedLockOptions::new()
    }
}
pub fn repair_red_lock_options(options: &mut RedLockOptions) {
    if options.single_nodes_timeout <= 0 {
        options.single_nodes_timeout = DEFAULT_SINGLE_LOCK_TIMEOUT;
    }
}
pub struct SingleNodeConf{
    pub username: String,
    pub password: String,
    pub address: String,
    pub opts: Option<ClientOptions>,
}

impl SingleNodeConf {
    pub fn new(username: &str, password: &str, address: &str) -> Self {
        SingleNodeConf {
            username: username.to_string(),
            password: password.to_string(),
            address: address.to_string(),
            opts: None,
        }
    }
    pub fn opts(mut self, opts: ClientOptions) -> Self {
        self.opts = Some(opts);
        self
    }
}


// test
#[cfg(test)]
mod tests {
    use redis::Commands;
    use crate::Client;
    use super::*;

    #[test]
    fn test_lock_option_builder() {
        // let options = LockOptionBuilder::new()
        //     .is_block(true)
        //     .block_waiting_seconds(10)
        //     .expire_seconds(30)
        //     .watch_dog_mode(true)
        //     .build();
        // assert_eq!(options.is_block, true);
        // assert_eq!(options.block_waiting_seconds, 10);
        // assert_eq!(options.expire_seconds, 30);
        // assert_eq!(options.watch_dog_mode, true);
    }

    #[test]
    fn test_repair_lock() {
        // let mut options = LockOptionBuilder::new()
        //     .is_block(true)
        //     .block_waiting_seconds(10)
        //     .build();
        // println!("before repair: {:?}", options);
    }
}