//! Deduplication guarantees for the content-addressed store.

use cadvm_store::{Category, Store, CHUNK_SIZE};

fn temp_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("objects")).unwrap();
    (dir, store)
}

#[test]
fn object_store_deduplicates_identical_blobs() {
    let (_d, store) = temp_store();
    let content = b"identical STEP file content";

    let a = store.put_bytes(content).unwrap();
    let b = store.put_bytes(content).unwrap();

    // Same content => same id, and only one stored blob on disk.
    assert_eq!(a, b);
    assert_eq!(store.list(Category::Blob).unwrap().len(), 1);
    assert_eq!(store.get_bytes(&a).unwrap(), content);
}

#[test]
fn chunk_store_deduplicates_identical_chunks() {
    let (_d, store) = temp_store();

    // Two files that share their first chunk exactly but differ in the second.
    let mut file_a = vec![0xABu8; CHUNK_SIZE];
    file_a.extend(vec![0x11u8; CHUNK_SIZE]);
    let mut file_b = vec![0xABu8; CHUNK_SIZE]; // identical first chunk
    file_b.extend(vec![0x22u8; CHUNK_SIZE]); // different second chunk

    let ref_a = store.put_file_content(&file_a).unwrap();
    let ref_b = store.put_file_content(&file_b).unwrap();

    // Each file is two chunks; the shared first chunk has the same hash.
    assert_eq!(ref_a.chunks.len(), 2);
    assert_eq!(ref_b.chunks.len(), 2);
    assert_eq!(ref_a.chunks[0].hash, ref_b.chunks[0].hash);
    assert_ne!(ref_a.chunks[1].hash, ref_b.chunks[1].hash);

    // The shared chunk is stored once: 3 distinct chunks total, not 4.
    assert_eq!(store.list(Category::Chunk).unwrap().len(), 3);

    // Round-trips from chunks reconstruct the originals.
    assert_eq!(store.read_file_content_from_chunks(&ref_a).unwrap(), file_a);
    assert_eq!(store.read_file_content_from_chunks(&ref_b).unwrap(), file_b);
}
