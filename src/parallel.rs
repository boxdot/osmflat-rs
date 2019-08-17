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
    let iter = Arc::new(Mutex::new(iter.enumerate()));
    let next = Arc::new((Mutex::new(0_usize), Condvar::new()));

    crossbeam::scope(|s| {
        let (sender, receiver) = sync_channel(rayon::current_num_threads());
        for _ in 0..rayon::current_num_threads() {
            let sender = sender.clone();
            let iter = iter.clone();
            s.spawn(|_| {
                let sender = sender;
                let iter = iter;
                while let Some((i, item)) = iter.lock().unwrap().next() {
                    let data = produce(item);

                    let (counter, cond) = &*next;
                    let mut guard = counter.lock().unwrap();
                    while *guard != i {
                        guard = cond.wait(guard).unwrap();
                    }

                    sender.send(data).unwrap();

                    *guard += 1;
                    cond.notify_all();
                }
            });
        }
        drop(sender); // drop to make sure iteration will finish once all senders are out of scope
        for result in receiver {
            consume(result)?;
        }
        Ok(())
    })
    .expect("thread panicked")
}
