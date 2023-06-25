
use std::sync::Arc;
use std::time::Duration;

use tokio::join;
use crate::lock::{Client, RedisLock};
use crate::option::{ClientOptions, LockOptions};

mod lock;
mod option;
mod redlock;
mod utils;

#[tokio::main]
async fn main() {
    // let lock = RedisLock::new("redis://127.0.0.1/", "lock", "unique_value");
    // match lock.acquire(5000) {
    //     Ok(_got_lock) => {
    //         // 在这里执行你的任务...
    //         std::thread::sleep(Duration::from_secs(10));
    //         lock.release().unwrap();
    //     }
    //     Err(_) => {
    //         // 获取锁失败
    //     }
    // }

    // let mut client = Client::new_client("12345678", "default", "10.0.0.1", ClientOptions::default());
    // // let _:() = c.set("lock", "1").unwrap();
    // // let result:String = c.get("lock").unwrap();
    // // println!("{:?}", result);
    // let lock = RedisLock::new(Arc::clone(&client), "lock");
    // tokio::spawn(async move {
    //     let result1 = lock.blocking_lock().await;
    //     println!("{:?}", result1);
    // });

    let mut client = Client::new_client("12345678", "default", "10.0.0.1", ClientOptions::default());

    let options1 = LockOptions::default().block(true);
    let options2 = LockOptions::default().block(true);
    let mut lock1 = RedisLock::new_with_options(Arc::clone(&client), "lock", options1);
    let lock2 = RedisLock::new_with_options(Arc::clone(&client), "lock",options2);

    let handle1 = tokio::spawn(async move {
        let result = lock1.lock().await;
        println!("resutl1{:?}", result);
        tokio::time::sleep(Duration::from_secs(60)).await;
        let result1 = lock1.unlock();
        println!("resutl12{:?}", result1);
    });
    join!(handle1);
    // let result1 = lock.try_lock(100000);
    // println!("{:?}", result1);
    // let result2 = lock.unlock();
    // println!("{:?}", result2);
}

// test
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use libc::time;
    use tokio::select;
    use tokio::sync::{broadcast, futures};

    use crate::{Client};
    use crate::lock::RedisLock;
    use crate::option::{ClientOptions, LockOptions};


    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_watchdog(){
        let mut client = Client::new_client("12345678", "default", "10.0.0.1", ClientOptions::default());

        let options1 = LockOptions::default().block(true);
        let options2 = LockOptions::default().block(true);
        let mut lock1 = RedisLock::new_with_options(Arc::clone(&client), "lock", options1);
        let mut lock2 = RedisLock::new_with_options(Arc::clone(&client), "lock", options2);

        let handle1 = tokio::spawn(async move {
            let result = lock1.lock().await;
            println!("resutl1{:?}", result);
            tokio::time::sleep(Duration::from_secs(20)).await;
            let result1 = lock1.unlock();
            println!("resutl12{:?}", result1);
        });
        let handle2 = tokio::spawn(async move {
            let result = lock2.lock().await;
            println!("resutl2{:?}", result);
            let result2 = lock2.unlock();
            println!("resutl22{:?}", result2);
            let result3 = lock2.lock().await;
            println!("resutl3{:?}", result3);
            let result4 = lock2.unlock();
            println!("resutl4{:?}", result4);
        });

        tokio::join!(handle1,handle2);
    }
}
