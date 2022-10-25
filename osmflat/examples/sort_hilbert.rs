use std::fs::OpenOptions;

use osmflat::{FileResourceStorage, Osm, NodeHilbertIdx};
use memmap2::MmapMut;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: debug <osmflat-archive>")?;
        
    // let archive = Osm::open(FileResourceStorage::new(archive_dir.clone()))?;

    // println!("Nodes: {}", archive.nodes().len());
    // println!("Ways: {}", archive.ways().len());
    // println!("Relations: {}", archive.relations().len());
    // let len = archive.node_hilbert_index().len();
    // println!("Hilbert: {}", len);

    let len = 128447288_usize;

    // Open index from memmapped file. Turn buffer into slice of NodeHilbertIndex.
    let path = format!("{}/node_hilbert_index", archive_dir);
    let file = OpenOptions::new().read(true).write(true).create(true).open(path)?;
    file.set_len((len * std::mem::size_of::<NodeHilbertIdx>()) as u64)?;
    let mut mmap = unsafe { MmapMut::map_mut(&file)? };
    let slc = &mut mmap[..];
    let node_hilbert_index = unsafe {
        let idx = ::core::slice::from_raw_parts_mut( slc.as_ptr() as *mut NodeHilbertIdx, len);
        idx
    };

    // Sort the index
    // node_hilbert_index.sort_unstable_by_key(|idx| idx.h());

    for idx in &archive.node_hilbert_index()[..30] {
        println!("from archive i {} h {}", idx.i(), idx.h());
    }

    for idx in &node_hilbert_index[..30] {
        println!("i {} h {}", idx.i(), idx.h());
    }

    // i 2055156608 h 0
    // i 5056302008070541176 h 1
    // i 5056302207747390673 h 2
    // i 5056302211303614741 h 3
    // i 5056302212333674826 h 4
    // i 5056310820170987528 h 5
    // i 5056310821372242991 h 6
    // i 5056310821420608715 h 7
    // i 5056310821476497517 h 8
    // i 5056310826349628727 h 9
    // i 5056310848021289904 h 10
    // i 5056310847666641936 h 11
    // i 5056310850547340584 h 12
    // i 5056310850278807225 h 13
    // i 5056310851863377037 h 14
    // i 5056310854834820688 h 15
    // i 5056310875627671114 h 16
    // i 5056310875246931456 h 17
    // i 5056310875188994281 h 18
    // i 5056310862397976238 h 19
    // i 5056310868155413722 h 20
    // i 5056310867238005481 h 21
    // i 5056310920773149833 h 22
    // i 5056310920187033254 h 23
    // i 5056310925397380143 h 24
    // i 5056310928204756389 h 25
    // i 5056334386916356809 h 26
    // i 5056334387284081012 h 27
    // i 5056334382024543885 h 28
    // i 5056334381368769081 h 29

    Ok(())
}
