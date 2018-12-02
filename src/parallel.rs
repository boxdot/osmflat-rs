use std::sync::{mpsc::sync_channel, Arc, Condvar, Mutex};

// allows producing data in parallel while still consuming it in the main thread
// in order
pub fn parallel_process<Item, Context, Data, Error>(
    iter: impl ExactSizeIterator<Item = Item> + Send + 'static,
    create_thread_context: impl Fn() -> Result<Context, Error>,
    produce: impl Fn(&mut Context, Item) -> Data + Clone + Send + 'static,
    mut consume: impl FnMut(Data) -> Result<(), Error>,
) -> Result<(), Error>
where
    Context: Send + 'static,
    Data: Send + 'static,
{
    let iter = Arc::new(Mutex::new(iter.enumerate()));
    let next = Arc::new((Mutex::new(0_usize), Condvar::new()));
    let (sender, receiver) = sync_channel(rayon::current_num_threads());
    for _ in 1..rayon::current_num_threads() {
        let mut context = create_thread_context()?;
        let iter = iter.clone();
        let next = next.clone();
        let sender = sender.clone();
        let produce = produce.clone();
        rayon::spawn(move || loop {
            let idx = iter.lock().unwrap().next();
            let idx = match idx {
                Some(x) => x,
                None => break,
            };

            let result = produce(&mut context, idx.1);

            let mut guard = next.0.lock().unwrap();
            while *guard != idx.0 {
                guard = next.1.wait(guard).unwrap();
            }
            sender.send(result).unwrap();
            *guard += 1;
            next.1.notify_all();
        });
    }
    drop(sender); // drop to make sure iteration will finish once all senders are out of scope
    for result in receiver {
        consume(result)?;
    }
    Ok(())
}
