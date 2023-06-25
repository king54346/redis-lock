第一部分介绍如何通过 redis 实现主动轮询模型下的分布式锁
该模型类似于单机锁中的主动轮询 + cas 乐观锁模型，取锁方会持续对分布式锁发出尝试获取动作，如果锁已被占用则会不断发起重试，直到取锁成功为止


主动轮询型分布式锁的实现思路为：

• 针对于同一把分布式锁，使用同一条数据进行标识（以 redis 为例，则为同一个 key 对应的 kv 数据记录）

• 假如在存储介质成功插入了该条数据（要求之前该 key 对应的数据不存在），则被认定为加锁成功

• 把从存储介质中删除该条数据这一行为理解为释放锁操作

• 倘若在插入该条数据时，发现数据已经存在（锁已被他人持有），则持续轮询，直到数据被他人删除（他人释放锁），并由自身完成数据插入动作为止（取锁成功）

关键：
• 由于是并发场景，需要保证【 （1）检查数据是否已被插入（2）数据不存在则插入数据 】这两个步骤之间是原子化不可拆分的（在 redis 中是 set only if not exist —— SETNX 操作）

redis 基于内存实现数据的存储，因此足够高轻便高效. 
此外，redis 基于单线程模型完成数据处理工作，支持 SETNX 原子指令（set only if not exist），能够很方便地支持分布式锁的加锁操作.
set 指令并附加 nx 参数来实现与 setnx 指令
redis中可以通过过期时间 expire time 机制来解决死锁问题，插入分布式锁对应的 kv 数据时设置一个过期时间 expire time，即便使用方因为异常原因导致无法正常解锁，锁对应的数据项也会在达到过期时间阈值后被自动删除
过期时间的设置也引入了新的问题，即假如因为一些异常情况导致占有锁的使用方在业务处理流程中的耗时超过了设置的过期时间阈值(无法精准预估业务逻辑的耗时)，锁被提前释放，其他取锁方可能取锁成功，最终引起数据不一致的并发问题
看门狗策略：在锁的持有方未完成业务逻辑的处理时，会持续对分布式锁的过期阈值进行延期操作
          创建一个定期续约的定时器，在没有完成时帮助自动续约，在完成逻辑的时候去释放锁
            1.设置看门狗的时候可以设置一个较短的过期时间
            2.释放锁的同时还要去停止看门狗
            3.在加锁成功的基础上去异步的启动看门狗


因为redis数据弱一致性引发的问题
由于redis的主从模式是ap的方式，master先会返回ack给client，在异步的与同步给slave数据，此时master加锁后宕机是不会同步到slave的，解决方案是redlock


```java


第二部分介绍如何使用 etcd 实现 watch 回调模型下的分布式锁。(并发激烈程度较高时倾向于 watch 回调型分布式锁，能降低轮询的消耗)

在取锁方发现锁已被他人占用时，会创建 watcher 监视器订阅锁的释放事件，随后不再发起主动取锁的尝试；
当锁被释放后，取锁方能通过之前创建的 watcher 感知到这一变化，然后再重新发起取锁的尝试动作



const UNLOCK_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
  return redis.call("DEL", KEYS[1])
else
  return 0
end

"#;
const EXTEND_SCRIPT: &str = r#"
if redis.call("get", KEYS[1]) ~= ARGV[1] then
  return 0
else
  if redis.call("set", KEYS[1], ARGV[1], "PX", ARGV[2]) ~= nil then
    return 1
  else
    return 0
  end
end
"#;



io-uring + ktls