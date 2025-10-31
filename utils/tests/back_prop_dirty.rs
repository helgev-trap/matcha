use std::sync::{Arc, Barrier};
use std::thread;
use utils::back_prop_dirty::BackPropDirty;

/// Basic single-thread propagation.
#[test]
fn test_single_thread_propagation() {
    let root = BackPropDirty::new(false);
    let child = BackPropDirty::with_parent(&root);

    assert!(!root.is_dirty());
    assert!(!child.is_dirty());

    child.mark_dirty();

    // Both child and root should be observed dirty.
    assert!(child.is_dirty());
    assert!(root.is_dirty());

    // take_dirty clears and returns previous state.
    assert!(root.take_dirty());
    assert!(!root.is_dirty());

    // Clearing child independently should work too.
    assert!(child.take_dirty());
    assert!(!child.is_dirty());
}

/// Concurrent marks from multiple threads all propagate to the same root.
/// After the threads finish, root.take_dirty() should return true once,
/// and subsequent take_dirty() should return false until another mark.
#[test]
fn test_multithread_propagation() {
    let root = BackPropDirty::new(false);

    // Create several child nodes that point to the same parent.
    let mut children = Vec::new();
    for _ in 0..16 {
        children.push(BackPropDirty::with_parent(&root));
    }

    let barrier = Arc::new(Barrier::new(children.len()));
    let mut handles = Vec::new();

    for child in children.into_iter() {
        let c = Arc::new(child);
        let b = barrier.clone();

        handles.push(thread::spawn(move || {
            // synchronize start to increase contention chance
            b.wait();
            c.mark_dirty();
        }));
    }

    for h in handles {
        h.join().expect("thread panicked");
    }

    // Root should be dirty after concurrent marks.
    assert!(root.is_dirty());

    // Clearing should return true once, then false.
    assert!(root.take_dirty());
    assert!(!root.is_dirty());
    assert!(!root.take_dirty());
}

/// Deep chain propagation — ensures propagation reaches the root through many ancestors.
#[test]
fn test_deep_chain_propagation() {
    // Depth chosen to be reasonably large but not pathological.
    let depth = 1000usize;
    let mut nodes = Vec::with_capacity(depth + 1);

    nodes.push(BackPropDirty::new(false)); // root at index 0
    for _ in 0..depth {
        let parent = nodes.last().unwrap();
        nodes.push(BackPropDirty::with_parent(parent));
    }

    // Mark the deepest node dirty.
    let last = nodes.last().unwrap();
    last.mark_dirty();

    // Root should be dirty.
    assert!(nodes[0].is_dirty());
    assert!(nodes[depth].is_dirty());

    // Clearing root should succeed.
    assert!(nodes[0].take_dirty());
    assert!(!nodes[0].is_dirty());
}
