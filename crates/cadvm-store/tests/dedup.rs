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
fn objects_are_compressed_on_disk() {
    let (dir, store) = temp_store();
    // Highly compressible, STEP-like text well above one filesystem block.
    let content = "#1 = CARTESIAN_POINT('', (0.0, 0.0, 0.0));\n".repeat(4000);
    let bytes = content.as_bytes();

    let id = store.put_bytes(bytes).unwrap();

    // Round-trips to the exact original.
    assert_eq!(store.get_bytes(&id).unwrap(), bytes);

    // On disk the object is gzip (magic 1f 8b) and much smaller than the input.
    let path = dir
        .path()
        .join("objects/blobs")
        .join(&id.hex()[0..2])
        .join(&id.hex()[2..4])
        .join(id.hex());
    let on_disk = std::fs::read(&path).unwrap();
    assert_eq!(
        &on_disk[0..2],
        &[0x1f, 0x8b],
        "stored object should be gzip"
    );
    assert!(
        on_disk.len() < bytes.len() / 4,
        "expected strong compression: {} vs {}",
        on_disk.len(),
        bytes.len()
    );
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
