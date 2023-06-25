use libc::time;
use tokio::time::Instant;
use crate::lock::{Client, LockError, RedisLock};
use crate::option;
use crate::option::{ClientOptions, LockOptions, repair_red_lock_options, SingleNodeConf};

// 红锁中每个节点默认的处理超时时间为 50 ms
const DEFAULT_SINGLE_LOCK_TIMEOUT: u64 = 50;


struct RedLock {
    locks : Vec<RedisLock>,
    red_lock_options: option::RedLockOptions,
}
/**
    多数派的原则
    1. 每个节点是独立的redis节点
    2. 分别在每个节点上获取锁
    3. 结果汇总，如果成功的节点数大于等于总节点数的一半，则认为获取锁成功
    4. 节点必须大于等于3个的奇数个节点


**/
impl RedLock {
    pub fn new(key:&str, conf:Vec<SingleNodeConf>) -> Self {
        if conf.len()<3 {
            panic!("节点数必须大于3个的奇数个节点");
        }
        let mut red_lock_options = option::RedLockOptions::default();
        let mut locks = Vec::new();
        for c in conf {
            let opts =match c.opts {
                None => {
                    ClientOptions::new()
                }
                Some(v) => {
                    v
                }
            };
            let client = Client::new_client(c.password.as_str(), c.username.as_str(),c.address.as_str(), opts);
            let mut lock = RedisLock::new(client,key);//开启看门狗模式并且设置超时时间为50ms
            locks.push(lock);
        }
        RedLock {
            locks,
            red_lock_options,
        }
    }

    pub fn new_with_options(key:&str, conf:Vec<SingleNodeConf>, mut red_lock_options: option::RedLockOptions) -> Self {
        if conf.len()<=3 {
            panic!("节点数必须大于等于3个的奇数个节点");
        }
        repair_red_lock_options(&mut red_lock_options);
        // 如果expire_duration<0 表示启动看门狗模式， 要求所有节点累计的超时阈值要小于分布式锁过期时间的十分之一
        if red_lock_options.expire_duration>0 && conf.len() as u64*red_lock_options.single_nodes_timeout*10 > red_lock_options.expire_duration*1000 {
                panic!("expire thresholds of single node is too long")
        };
        let mut locks = Vec::new();
        for c in conf {
            let opts =match c.opts {
                None => {
                    ClientOptions::new()
                }
                Some(v) => {
                    v
                }
            };
            let client = Client::new_client(c.password.as_str(), c.username.as_str(), c.address.as_str(), opts);
            let mut lock = RedisLock::new_with_options(client, key, LockOptions::default().expire_seconds(red_lock_options.expire_duration));
            locks.push(lock);
        }
        RedLock {
            locks,
            red_lock_options,
        }
    }

    pub async fn lock(&mut self) -> bool {
        let mut success_count = 0;
        for lock in self.locks.iter_mut() {
            let start = Instant::now();
            match lock.lock().await {
                Ok(v) => {
                    let duration = start.elapsed();
                    if v && duration.as_millis() < self.red_lock_options.single_nodes_timeout as u128{
                        success_count += 1;
                    }
                }
                Err(_) => {}
            };
        }
        if success_count< self.locks.len()>>1+1 {
            self.locks.iter().for_each(|lock| {
                lock.unlock().unwrap();
            });
            return false;
        };
        return true;
    }

    pub fn unlock(& self) {
        self.locks.iter().for_each(|lock| {
            lock.unlock().unwrap();
        });
    }
}
#[cfg(test)]
mod tests {
    use crate::option::SingleNodeConf;
    use crate::redlock::RedLock;

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test1() {
        let mut red_lock1 = SingleNodeConf::new("default", "12345678", "101.37.89.207");
        let mut red_lock2 = SingleNodeConf::new("default", "", "localhost");
        let mut red_lock3 = SingleNodeConf::new("default", "", "10.0.0.106");
        let mut lock = RedLock::new("test", vec![red_lock1, red_lock2, red_lock3]);
        let result = lock.lock().await;
        println!("result:{}", result);
        // lock.unlock();
    }
}