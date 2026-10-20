//! Needs a server: `docker compose up -d redis`, then
//! `cargo test -p based-redis --test live -- --ignored`.

use based_redis::{
    KeyType, KeyValue, add_entry, connect, delete_key, exec, key_meta, read_value, remove_entry,
    scan_keys, set_string, set_ttl,
};

const URL: &str = "redis://127.0.0.1:16379/15";

#[tokio::test]
#[ignore = "requires redis on localhost:16379"]
async fn key_lifecycle() {
    let mut c = connect(URL).await.unwrap();
    exec(&mut c, "FLUSHDB").await.unwrap();

    set_string(&mut c, "greeting", "hello").await.unwrap();
    set_ttl(&mut c, "greeting", Some(100)).await.unwrap();
    let meta = key_meta(&mut c, "greeting").await.unwrap();
    assert_eq!(meta.kind, Some(KeyType::String));
    assert!(meta.ttl.is_some());
    set_string(&mut c, "greeting", "world").await.unwrap();
    assert!(
        key_meta(&mut c, "greeting").await.unwrap().ttl.is_some(),
        "KEEPTTL must preserve expiry"
    );
    set_ttl(&mut c, "greeting", None).await.unwrap();
    assert_eq!(key_meta(&mut c, "greeting").await.unwrap().ttl, None);
    match read_value(&mut c, "greeting", KeyType::String)
        .await
        .unwrap()
    {
        KeyValue::Text(t) => assert_eq!(t, "world"),
        other => panic!("{other:?}"),
    }

    add_entry(&mut c, "h", KeyType::Hash, "name Ada Lovelace")
        .await
        .unwrap();
    add_entry(&mut c, "z", KeyType::ZSet, "2.5 bob")
        .await
        .unwrap();
    add_entry(&mut c, "l", KeyType::List, "a").await.unwrap();
    add_entry(&mut c, "s", KeyType::Set, "m").await.unwrap();
    let KeyValue::Entries(rows) = read_value(&mut c, "h", KeyType::Hash).await.unwrap() else {
        panic!("hash entries");
    };
    assert_eq!(
        (rows[0].left.as_str(), rows[0].right.as_str()),
        ("name", "Ada Lovelace")
    );
    let KeyValue::Entries(rows) = read_value(&mut c, "z", KeyType::ZSet).await.unwrap() else {
        panic!("zset entries");
    };
    assert_eq!(
        (rows[0].left.as_str(), rows[0].right.as_str()),
        ("2.5", "bob")
    );

    remove_entry(&mut c, "h", KeyType::Hash, "name")
        .await
        .unwrap();
    assert!(key_meta(&mut c, "h").await.unwrap().kind.is_none());

    assert_eq!(
        scan_keys(&mut c, "*", 100).await.unwrap(),
        ["greeting", "l", "s", "z"]
    );
    delete_key(&mut c, "greeting").await.unwrap();
    assert!(key_meta(&mut c, "greeting").await.unwrap().kind.is_none());

    let out = exec(&mut c, r#"RPUSH q "a b" c"#).await.unwrap();
    assert_eq!(out, "(integer) 2");
    let out = exec(&mut c, "LRANGE q 0 -1").await.unwrap();
    assert_eq!(out, "1) \"a b\"\n2) \"c\"");
    exec(&mut c, "FLUSHDB").await.unwrap();
}
