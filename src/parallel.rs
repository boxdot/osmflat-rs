use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::{mpsc::sync_channel, Arc, Condvar, Mutex};

pub fn parallel_process<Iter, Item, Producer, Data, Consumer, Error>(
    iter: Iter,
    produce: Producer,
    mut consume: Consumer,
) -> Result<(), Error>
where
    Iter: Iterator<Item = Item> + Send,
    Producer: Fn(Item) -> Data + Sync,
    Data: Send,
    Consumer: FnMut(Data) -> Result<(), Error>,
{
    let num_threads = rayon::current_num_threads();

    let iter = Arc::new(Mutex::new(iter.enumerate()));
    let next = Arc::new((Mutex::new(2 * num_threads), Condvar::new()));

    crossbeam::scope(|s| {
        let (sender, receiver) = sync_channel(2 * num_threads);
        for _ in 0..num_threads {
            let sender = sender.clone();
            let iter = iter.clone();
            s.spawn(|_| {
                let sender = sender;
                let iter = iter;
                loop {
                    let (i, item) = match iter.lock().unwrap().next() {
                        None => break,
                        Some(x) => x,
                    };
                    let data = produce(item);

                    let (counter, cond) = &*next;
                    let mut guard = counter.lock().unwrap();
                    while *guard <= i {
                        guard = cond.wait(guard).unwrap();
                    }

                    sender.send((i, data)).unwrap();
                }
            });
        }
        drop(sender); // drop to make sure iteration will finish once all senders are out of scope

        let mut pending = BTreeMap::new();
        let mut next_idx = 0;
        for result in receiver {
            pending.insert(Reverse(result.0), result.1);
            while let Some(data) = pending.remove(&Reverse(next_idx)) {
                {
                    let mut guard = next.0.lock().unwrap();
                    *guard += 1;
                    next.1.notify_all();
                }

                next_idx += 1;
                consume(data)?;
            }
        }
        Ok(())
    })
    .expect("thread panicked")
}
