use rpg_encoder::NodeId;

#[test]
fn test_node_id_new() {
    let id = NodeId::new(42);
    assert_eq!(id.index(), 42);
}

#[test]
fn test_node_id_equality() {
    let id1 = NodeId::new(5);
    let id2 = NodeId::new(5);
    let id3 = NodeId::new(6);

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_node_id_index() {
    let id = NodeId::new(0);
    assert_eq!(id.index(), 0);

    let id2 = NodeId::new(100);
    assert_eq!(id2.index(), 100);
}
