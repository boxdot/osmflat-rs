use pbr::ProgressBar;
use std::sync::{mpsc::sync_channel, Arc, Condvar, Mutex};

// allows producing data in parallel while still consuming it in the main thread
// in order
pub fn parallel_process<Item, Context: Send + 'static, Data: Send + 'static, Error>(
    progress_message: &str,
    iter: impl ExactSizeIterator<Item = Item> + Send + 'static,
    create_thread_context: impl Fn() -> Result<Context, Error>,
    produce: impl Fn(&mut Context, Item) -> Data + Clone + Send + 'static,
    mut consume: impl FnMut(Data) -> Result<(), Error>,
) -> Result<(), Error> {
    let len = iter.len();
    let mut pb = ProgressBar::new(len as u64);
    pb.message(progress_message);
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
    for result in receiver {
        consume(result)?;
        pb.inc();
    }
    pb.finish();
    Ok(())
}
