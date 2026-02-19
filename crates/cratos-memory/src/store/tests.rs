use super::GraphStore;
use crate::types::*;
use chrono::Utc;

async fn test_store() -> GraphStore {
    GraphStore::in_memory().await.unwrap()
}

fn make_turn(id: &str, session: &str, idx: u32, role: TurnRole) -> Turn {
    Turn {
        id: id.into(),
        session_id: session.into(),
        role,
        content: format!("content {id}"),
        summary: format!("summary {id}"),
        turn_index: idx,
        token_count: 10,
        created_at: Utc::now(),
    }
}

fn make_entity(id: &str, name: &str, kind: EntityKind) -> Entity {
    Entity {
        id: id.into(),
        name: name.into(),
        kind,
        first_seen: Utc::now(),
        mention_count: 1,
    }
}

#[tokio::test]
async fn test_insert_and_get_turn() {
    let store = test_store().await;
    let turn = make_turn("t1", "s1", 0, TurnRole::User);
    store.insert_turn(&turn).await.unwrap();

    let got = store.get_turn("t1").await.unwrap().unwrap();
    assert_eq!(got.id, "t1");
    assert_eq!(got.session_id, "s1");
    assert_eq!(got.role, TurnRole::User);
    assert_eq!(got.turn_index, 0);
}

#[tokio::test]
async fn test_idempotent_insert() {
    let store = test_store().await;
    let turn = make_turn("t1", "s1", 0, TurnRole::User);
    store.insert_turn(&turn).await.unwrap();
    store.insert_turn(&turn).await.unwrap(); // no error
    assert_eq!(store.turn_count().await.unwrap(), 1);
}

#[tokio::test]
async fn test_turns_by_session() {
    let store = test_store().await;
    store
        .insert_turn(&make_turn("a", "s1", 0, TurnRole::User))
        .await
        .unwrap();
    store
        .insert_turn(&make_turn("b", "s1", 1, TurnRole::Assistant))
        .await
        .unwrap();
    store
        .insert_turn(&make_turn("c", "s2", 0, TurnRole::User))
        .await
        .unwrap();

    let s1 = store.get_turns_by_session("s1").await.unwrap();
    assert_eq!(s1.len(), 2);
    assert_eq!(s1[0].turn_index, 0);
    assert_eq!(s1[1].turn_index, 1);
}

#[tokio::test]
async fn test_max_turn_index() {
    let store = test_store().await;
    assert_eq!(store.max_turn_index("s1").await.unwrap(), None);

    store
        .insert_turn(&make_turn("a", "s1", 0, TurnRole::User))
        .await
        .unwrap();
    store
        .insert_turn(&make_turn("b", "s1", 3, TurnRole::Assistant))
        .await
        .unwrap();
    assert_eq!(store.max_turn_index("s1").await.unwrap(), Some(3));
}

#[tokio::test]
async fn test_upsert_entity() {
    let store = test_store().await;
    let ent = make_entity("e1", "orchestrator.rs", EntityKind::File);
    store.upsert_entity(&ent).await.unwrap();

    let got = store
        .get_entity_by_name("orchestrator.rs")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.kind, EntityKind::File);
    assert_eq!(got.mention_count, 1);

    // Upsert again increments count
    let ent2 = make_entity("e1-dup", "orchestrator.rs", EntityKind::File);
    store.upsert_entity(&ent2).await.unwrap();
    let got2 = store
        .get_entity_by_name("orchestrator.rs")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got2.mention_count, 2);
}

#[tokio::test]
async fn test_edges_and_graph_traversal() {
    let store = test_store().await;
    let turn = make_turn("t1", "s1", 0, TurnRole::User);
    let ent = make_entity("e1", "foo.rs", EntityKind::File);
    store.insert_turn(&turn).await.unwrap();
    store.upsert_entity(&ent).await.unwrap();

    let edge = TurnEntityEdge {
        turn_id: "t1".into(),
        entity_id: "e1".into(),
        relevance: 0.9,
    };
    store.insert_edge(&edge).await.unwrap();

    // Turn → entities
    let entities = store.get_entities_for_turn("t1").await.unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "foo.rs");

    // Entity → turns
    let turn_ids = store.get_turn_ids_for_entity("e1").await.unwrap();
    assert_eq!(turn_ids, vec!["t1"]);
}

#[tokio::test]
async fn test_cooccurrence() {
    let store = test_store().await;
    // Must insert entities first (FK constraint)
    store
        .upsert_entity(&make_entity("e1", "a.rs", EntityKind::File))
        .await
        .unwrap();
    store
        .upsert_entity(&make_entity("e2", "b.rs", EntityKind::File))
        .await
        .unwrap();
    store
        .upsert_entity(&make_entity("e3", "c.rs", EntityKind::File))
        .await
        .unwrap();

    let ids = vec!["e1".into(), "e2".into(), "e3".into()];
    store.update_cooccurrence(&ids).await.unwrap();
    store
        .update_cooccurrence(&["e1".into(), "e2".into()])
        .await
        .unwrap();

    let co = store.get_cooccurring_entities("e1", 10).await.unwrap();
    // e2 should have count 2 (appeared with e1 twice)
    let e2 = co.iter().find(|(id, _)| id == "e2").unwrap();
    assert_eq!(e2.1, 2);
    // e3 should have count 1
    let e3 = co.iter().find(|(id, _)| id == "e3").unwrap();
    assert_eq!(e3.1, 1);
}

#[tokio::test]
async fn test_get_turns_by_ids() {
    let store = test_store().await;
    store
        .insert_turn(&make_turn("t1", "s1", 0, TurnRole::User))
        .await
        .unwrap();
    store
        .insert_turn(&make_turn("t2", "s1", 1, TurnRole::Assistant))
        .await
        .unwrap();
    store
        .insert_turn(&make_turn("t3", "s1", 2, TurnRole::User))
        .await
        .unwrap();

    let turns = store
        .get_turns_by_ids(&["t1".into(), "t3".into()])
        .await
        .unwrap();
    assert_eq!(turns.len(), 2);

    // Empty list
    let empty = store.get_turns_by_ids(&[]).await.unwrap();
    assert!(empty.is_empty());
}

fn make_explicit(name: &str, content: &str) -> ExplicitMemory {
    ExplicitMemory {
        id: format!("em-{name}"),
        name: name.into(),
        content: content.into(),
        category: "general".into(),
        tags: vec!["test".into()],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        access_count: 0,
    }
}

#[tokio::test]
async fn test_save_and_get_explicit_memory() {
    let store = test_store().await;
    let mem = make_explicit("my-note", "Remember to fix the bug");
    store.save_explicit_memory(&mem).await.unwrap();

    let got = store
        .get_explicit_by_name("my-note")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.name, "my-note");
    assert_eq!(got.content, "Remember to fix the bug");
    assert_eq!(got.category, "general");
    assert_eq!(got.tags, vec!["test"]);
}

#[tokio::test]
async fn test_explicit_upsert_by_name() {
    let store = test_store().await;
    let mem = make_explicit("note", "version 1");
    store.save_explicit_memory(&mem).await.unwrap();

    let mut updated = make_explicit("note", "version 2");
    updated.id = "em-note-v2".into();
    store.save_explicit_memory(&updated).await.unwrap();

    // Should have been updated (not duplicated)
    let got = store.get_explicit_by_name("note").await.unwrap().unwrap();
    assert_eq!(got.content, "version 2");
}

#[tokio::test]
async fn test_search_explicit() {
    let store = test_store().await;
    store
        .save_explicit_memory(&make_explicit("api-key", "The API key is xyz"))
        .await
        .unwrap();
    store
        .save_explicit_memory(&make_explicit("db-config", "Database on port 5432"))
        .await
        .unwrap();

    let results = store.search_explicit("API", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "api-key");

    // Search by category
    let results = store
        .search_explicit("API", Some("knowledge"), 10)
        .await
        .unwrap();
    assert!(results.is_empty()); // category mismatch
}

#[tokio::test]
async fn test_list_explicit() {
    let store = test_store().await;
    store
        .save_explicit_memory(&make_explicit("a", "first"))
        .await
        .unwrap();
    store
        .save_explicit_memory(&make_explicit("b", "second"))
        .await
        .unwrap();

    let all = store.list_explicit(None, 10).await.unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_delete_explicit() {
    let store = test_store().await;
    store
        .save_explicit_memory(&make_explicit("to-delete", "bye"))
        .await
        .unwrap();

    assert!(store.delete_explicit("to-delete").await.unwrap());
    assert!(store
        .get_explicit_by_name("to-delete")
        .await
        .unwrap()
        .is_none());
    // Deleting again returns false
    assert!(!store.delete_explicit("to-delete").await.unwrap());
}

#[tokio::test]
async fn test_increment_access_count() {
    let store = test_store().await;
    store
        .save_explicit_memory(&make_explicit("counted", "data"))
        .await
        .unwrap();

    store.increment_access_count("em-counted").await.unwrap();
    store.increment_access_count("em-counted").await.unwrap();

    let got = store
        .get_explicit_by_name("counted")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.access_count, 2);
}

#[tokio::test]
async fn test_memory_entity_edges() {
    let store = test_store().await;
    store
        .save_explicit_memory(&make_explicit("orch-fix", "Fixed orchestrator bug"))
        .await
        .unwrap();
    store
        .upsert_entity(&make_entity("e1", "orchestrator.rs", EntityKind::File))
        .await
        .unwrap();

    store
        .insert_memory_entity_edge("em-orch-fix", "e1", 0.9)
        .await
        .unwrap();

    let mems = store.get_explicit_by_entity("e1").await.unwrap();
    assert_eq!(mems.len(), 1);
    assert_eq!(mems[0].name, "orch-fix");
}
