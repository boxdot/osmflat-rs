use std::fs::OpenOptions;

use osmflat::{FileResourceStorage, Osm, NodeHilbertIdx};
use memmap2::MmapMut;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let archive_dir = std::env::args()
        .nth(1)
        .ok_or("USAGE: debug <osmflat-archive>")?;

    let archive = Osm::open(FileResourceStorage::new(archive_dir.clone()))?;

    println!("Nodes: {}", archive.nodes().len());
    println!("Ways: {}", archive.ways().len());
    println!("Relations: {}", archive.relations().len());
    let len = archive.node_hilbert_index().len();
    println!("Hilbert: {}", len);

    // let len = 128447288_usize;

    // Open index from memmapped file. Turn buffer into slice of NodeHilbertIndex.
    let path = format!("{}/node_hilbert_index", archive_dir);
    let file = OpenOptions::new().read(true).write(true).create(true).open(path)?;
    // file.set_len((len * std::mem::size_of::<NodeHilbertIdx>() + 8) as u64)?;
    let mut mmap = unsafe { MmapMut::map_mut(&file)? };

    // There are 8 bytes of padding at the beginning.
    // Not sure what it is...
    let mut mem = core::mem::MaybeUninit::<u64>::uninit();
    let num = unsafe {
      core::ptr::copy_nonoverlapping(
        mmap[..8].as_ptr(),
        mem.as_mut_ptr() as *mut u8,
        core::mem::size_of::<u64>(),
      );
      mem.assume_init()
    };
    println!("num {}", num);

    // Ignore the first 8 bytes.
    let slc = &mut mmap[8..];
    let node_hilbert_index = unsafe {
        let idx = ::core::slice::from_raw_parts_mut( slc.as_ptr() as *mut NodeHilbertIdx, len);
        idx
    };    

    let a = &archive.node_hilbert_index()[0];
    let b = &node_hilbert_index[0];

    println!("archive[0] {} {}", a.i(), a.h());
    println!("memmap[0]  {} {}", b.i(), b.h());

    for idx in &archive.node_hilbert_index()[..30] {
        println!("archive i {} h {}", idx.i(), idx.h());
    }

    for idx in &node_hilbert_index[..30] {
        println!("memmap  i {} h {}", idx.i(), idx.h());
    }

    // Sort the index
    {
        println!("Sorting");
        node_hilbert_index.sort_unstable_by_key(|idx| idx.h());
        println!("Sorting done");
    }

    for idx in &archive.node_hilbert_index()[..30] {
        println!("archive i {} h {}", idx.i(), idx.h());
    }

    for idx in &node_hilbert_index[..30] {
        println!("memmap  i {} h {}", idx.i(), idx.h());
    }

    for idx in &archive.node_hilbert_index()[(len-30)..] {
        println!("end archive i {} h {}", idx.i(), idx.h());
    }

    for idx in &node_hilbert_index[(len-30)..] {
        println!("end memmap  i {} h {}", idx.i(), idx.h());
    }

    Ok(())
}
