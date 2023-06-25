use std::borrow::BorrowMut;
use std::env::consts::ARCH;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::Context;
use std::thread::sleep;
use std::time::Duration;
use r2d2::{ManageConnection, Pool};

use redis::{Commands, Connection, ConnectionInfo, ConnectionLike, RedisResult, Script, Value};
use tokio::{join, select, time};
use tokio::sync::{broadcast, oneshot};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
use tokio::time::{Sleep, timeout};
use tokio_util::sync::CancellationToken;
use crate::lock::LockError::RedisError;
use crate::option;


use crate::option::{ClientOptions, LockOptions, repair_client_options, repair_lock_option, WATCH_DOG_WORK_STEP_SECONDS};
use crate::utils::GetProcessAndThreadIDStr;


static TASK_ID: AtomicUsize = AtomicUsize::new(0);
const REDIS_LOCK_KEY_PREFIX: &str = "REDIS_LOCK_PREFIX_";

async fn get_lock(con: &mut redis::Connection, lock_key: &str, timeout_ms: u64) -> redis::RedisResult<bool> {
    loop {
        let result: redis::RedisResult<(String, u32)> = redis::cmd("SET")
            .arg(lock_key)
            .arg("1")
            .arg("NX")
            .arg("PX")
            .arg(timeout_ms)
            .query(con);

        match result {
            Ok(_) => return Ok(true),
            Err(_) => {
                // wait for 100ms before retrying
                time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn release_lock(con: &mut redis::Connection, lock_key: &str) -> redis::RedisResult<()> {
    con.del(lock_key)
}

pub struct Client {
    pool: Pool<redis::Client>,
    client_options: ClientOptions,
}

impl Client {
    pub fn new_client(password: &str, username: &str, address: &str, mut client_options:ClientOptions) -> Arc<Self> {
        let connection_info = format!(
            "redis://{}:{}@{}/",
            username,
            password,
            address
        );

        repair_client_options(&mut client_options);
        let result = redis::Client::open(connection_info);
        let client = result.unwrap();

        let pool = r2d2::Pool::builder()
            .min_idle(Some(client_options.min_idle))
            .max_size(client_options.max_active)
            .idle_timeout(Some(Duration::from_secs(client_options.idle_timeout_seconds)))
            .build(client)
            .unwrap();

        Arc::from(Client {
            pool,
            client_options,
        })
    }
}

#[derive(Clone)]
pub struct RedisLock{
    client:Arc<Client>,
    key: String,
    token: String,
    lock_options: option::LockOptions,


    running_dog: Arc<AtomicBool>,
    stop_dog: CancellationToken,
}

// 定义errortype
#[derive(Debug)]
pub enum LockError {
    RedisError(redis::RedisError),
}

impl From<redis::RedisError> for LockError {
    fn from(err: redis::RedisError) -> Self {
        LockError::RedisError(err)
    }
}


impl RedisLock{
    pub fn new(client:Arc<Client>, key: &str) -> Self {
        let mut options = option::LockOptions::default();
        repair_lock_option(&mut options);
        RedisLock {
            client,
            key: key.to_string(),
            token: GetProcessAndThreadIDStr()+format!("taskid:{}",&TASK_ID.fetch_add(1, Ordering::SeqCst)).as_str(),
            lock_options: options,
            running_dog: Arc::new(AtomicBool::new(false)),
            stop_dog: CancellationToken::default(),
        }
    }
    // 倘若锁处于非阻塞模式，则只会执行一次 tryLock 方法进行尝试加锁动作，倘若失败，就直接返回错误
    pub fn new_with_options(client:Arc<Client>, key: &str, mut lock_options: option::LockOptions) -> Self {
        repair_lock_option(&mut lock_options);
        RedisLock {
            client,
            key: key.to_string(),
            token: GetProcessAndThreadIDStr()+format!("taskid:{}",&TASK_ID.fetch_add(1, Ordering::SeqCst)).as_str(),
            lock_options,
            running_dog: Arc::new( AtomicBool::new(false)),
            stop_dog: CancellationToken::default(),
        }
    }

    // 倘若锁处于非阻塞模式，则只会执行一次 tryLock 方法进行尝试加锁动作，倘若失败，就直接返回错误
    //  setNEX 操作实现，即基于原子操作实现 set with expire time only if key not exist 的语义
    pub async fn lock(&mut self) -> Result<bool, LockError> {
        return match self.try_lock() {
            Ok(v) => {
                if !self.lock_options.block {
                    return Ok(false);
                }

                if v {
                    self.watch_dog();
                    Ok(true)
                }else {
                    self.blocking_lock().await
                }
            }
            Err(e) => {
                Err(e)
            }
        };
    }

    // 取锁false or true ， lockerror表示redis错误
    fn try_lock(&self) -> Result<bool, LockError> {
        let mut con = self.client.pool.get().unwrap();
        let result: Value = redis::pipe().atomic()
            .cmd("SET")
            .arg(&self.get_lock_key())
            .arg(&self.token)
            .arg("NX") //if key not exist
            .arg("EX")   //EX seconds PX milliseconds
            .arg(self.lock_options.expire_seconds)  // expire: Duration expire: Duration
            .query(con.deref_mut())?;

        return match result {
            Value::Bulk(r) => {
                if r.first() == Some(&Value::Okay) {
                    Ok(true)
                } else {
                    //  lock failed
                    Ok(false)
                }
            }
            _ => {
                //  lock failed
                Err(LockError::RedisError(redis::RedisError::from((redis::ErrorKind::TypeError, "redis error", ))))
            }
        };
    }


    fn get_lock_key(&self) -> String {
        // REDIS_LOCK_KEY_PREFIX + self.key
        format!("{}{}", REDIS_LOCK_KEY_PREFIX, self.key)
    }
    // 阻塞模式下的取锁 false or true
    async fn blocking_lock(&mut self) -> Result<bool, LockError> {
        // 阻塞模式等锁时间上限
        let time = Duration::from_secs(self.lock_options.block_waiting_seconds);
        // 轮询 ticker，每隔 50 ms 尝试取锁一次
        let mut interval = time::interval(Duration::from_millis(50));
        let res = timeout(time, async {
            loop {
                // 每隔 50 ms 尝试取锁一次
                interval.tick().await;
                // 尝试取锁
                let result = self.try_lock();
                match result {
                    Ok(r) => {
                        if r {
                            self.watch_dog();
                            return Ok(());
                        }
                    }
                    Err(r) => {
                        return Err(r);
                    }
                }
            }
        }).await;
        return match res {
            Ok(_) => {
                Ok(true)
            }
            Err(elapsed) => {
                // 超时
                Ok(false)
            }
        };
    }

    //解锁
    // 解锁动作基于 lua 脚本执行
    //lua 脚本执行内容分为两部分：【（1）校验当前操作者是否拥有锁的所有权（2）倘若是，则释放锁】
    // false 解锁失败，true 解锁成功 ， lockerror表示redis错误
    pub fn unlock(&self) -> Result<bool, LockError> {
        let mut con = self.client.pool.get().unwrap();
        let script = Script::new(
            r#"
              local lockerKey = KEYS[1]
              local getToken = redis.call('get',lockerKey)
              if (not getToken or getToken ~= ARGV[1]) then
                return 0
              else
                return redis.call('del',lockerKey)
              end
        "#);
        let result: RedisResult<i64> = script.key(&self.get_lock_key()).arg(&self.token).invoke(con.deref_mut());
        let result1 = match result {
            Ok(r) => {
                if r == 1 {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(r) => {
                Err(RedisError(r))
            }
        };

        // 关闭看门狗
        if !self.stop_dog.is_cancelled() {
            self.stop_dog.cancel();
        }
        return result1;
    }

    // 看门狗模式
    fn watch_dog(&mut self) {
        // 判断一次是否是看门狗模式
        if !self.lock_options.watch_dog_mode {
            return;
        }
        // 确保只有一个看门狗线程
        let old_value = self.running_dog.load(Ordering::SeqCst);
        let new_value = !old_value;
        while !self.running_dog.load(Ordering::SeqCst) {
            if self.running_dog.compare_exchange(old_value, new_value, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                break;
            }
        }
        self.stop_dog = CancellationToken::new();
        // 启动看门狗线程
        let mut lock = self.clone();
        tokio::spawn(
            async move {
                lock.run_watch_dog(1).await;
                lock.running_dog.store(false, Ordering::SeqCst);
            }
        );
    }

    async fn run_watch_dog(&self, net_delay:u64) {
        let mut ticker = time::interval(Duration::from_secs(WATCH_DOG_WORK_STEP_SECONDS));
        loop {
            ticker.tick().await;
            select! {
                _ = self.stop_dog.cancelled() => {
                    break;
                }
                //
                _ = self.delay_expire(WATCH_DOG_WORK_STEP_SECONDS+net_delay) => {
                    continue;
                }
            }
        }

    }

    // 延长锁的过期时间，基于 lua 脚本实现操作原子性
    async fn delay_expire(&self,expire_seconds:u64) -> Result<bool, LockError> {
        let mut con = self.client.pool.get().unwrap();
        // 第一步：校验当前操作者是否拥有锁的所有权
        // 第二步：倘若是，则延长锁的过期时间
        let script = Script::new(
            r#"
              local lockerKey = KEYS[1]
              local getToken = redis.call('get',lockerKey)
              if (not getToken or getToken ~= ARGV[1]) then
                return 0
              else
                return redis.call('expire',lockerKey,ARGV[2])
              end
        "#);

        let result: RedisResult<i64> = script.key(self.get_lock_key()).arg(&self.token).arg(expire_seconds).invoke(con.deref_mut());
        return match result {
            Ok(r) => {
                if r == 1 {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(r) => {
                Err(RedisError(r))
            }
        };

    }
}