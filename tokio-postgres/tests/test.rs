extern crate env_logger;
extern crate futures;
extern crate tokio;
extern crate tokio_postgres;

use std::time::{Duration, Instant};
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;
use tokio::timer::Delay;
use tokio_postgres::error::SqlState;
use tokio_postgres::types::Type;
use tokio_postgres::TlsMode;

fn smoke_test(url: &str) {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(url.parse().unwrap(), TlsMode::None);
    let (mut client, connection) = runtime.block_on(handshake).unwrap();
    let connection = connection.map_err(|e| panic!("{}", e));
    runtime.handle().spawn(connection).unwrap();

    let prepare = client.prepare("SELECT 1::INT4");
    let statement = runtime.block_on(prepare).unwrap();
    let select = client.query(&statement, &[]).collect().map(|rows| {
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get::<_, i32>(0), 1);
    });
    runtime.block_on(select).unwrap();

    drop(statement);
    drop(client);
    runtime.run().unwrap();
}

#[test]
fn plain_password_missing() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://pass_user@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    match runtime.block_on(handshake) {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.as_connection().is_some() => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn plain_password_wrong() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://pass_user:foo@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    match runtime.block_on(handshake) {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.code() == Some(&SqlState::INVALID_PASSWORD) => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn plain_password_ok() {
    smoke_test("postgres://pass_user:password@localhost:5433/postgres");
}

#[test]
fn md5_password_missing() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://md5_user@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    match runtime.block_on(handshake) {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.as_connection().is_some() => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn md5_password_wrong() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://md5_user:foo@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    match runtime.block_on(handshake) {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.code() == Some(&SqlState::INVALID_PASSWORD) => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn md5_password_ok() {
    smoke_test("postgres://md5_user:password@localhost:5433/postgres");
}

#[test]
fn scram_password_missing() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://scram_user@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    match runtime.block_on(handshake) {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.as_connection().is_some() => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn scram_password_wrong() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://scram_user:foo@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    match runtime.block_on(handshake) {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.code() == Some(&SqlState::INVALID_PASSWORD) => {}
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn scram_password_ok() {
    smoke_test("postgres://scram_user:password@localhost:5433/postgres");
}

#[test]
fn pipelined_prepare() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://postgres@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    let (mut client, connection) = runtime.block_on(handshake).unwrap();
    let connection = connection.map_err(|e| panic!("{}", e));
    runtime.handle().spawn(connection).unwrap();

    let prepare1 = client.prepare("SELECT 1::BIGINT WHERE $1::BOOL");
    let prepare2 = client.prepare("SELECT ''::TEXT, 1::FLOAT4 WHERE $1::VARCHAR IS NOT NULL");
    let prepare = prepare1.join(prepare2);
    let (statement1, statement2) = runtime.block_on(prepare).unwrap();

    assert_eq!(statement1.params(), &[Type::BOOL]);
    assert_eq!(statement1.columns().len(), 1);
    assert_eq!(statement1.columns()[0].type_(), &Type::INT8);

    assert_eq!(statement2.params(), &[Type::VARCHAR]);
    assert_eq!(statement2.columns().len(), 2);
    assert_eq!(statement2.columns()[0].type_(), &Type::TEXT);
    assert_eq!(statement2.columns()[1].type_(), &Type::FLOAT4);

    drop(statement1);
    drop(statement2);
    drop(client);
    runtime.run().unwrap();
}

#[test]
fn insert_select() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://postgres@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    let (mut client, connection) = runtime.block_on(handshake).unwrap();
    let connection = connection.map_err(|e| panic!("{}", e));
    runtime.handle().spawn(connection).unwrap();

    runtime
        .block_on(
            client
                .prepare("CREATE TEMPORARY TABLE foo (id SERIAL, name TEXT)")
                .and_then(|create| client.execute(&create, &[]))
                .map(|n| assert_eq!(n, 0)),
        )
        .unwrap();

    let insert = client.prepare("INSERT INTO foo (name) VALUES ($1), ($2)");
    let select = client.prepare("SELECT id, name FROM foo ORDER BY id");
    let prepare = insert.join(select);
    let (insert, select) = runtime.block_on(prepare).unwrap();

    let insert = client
        .execute(&insert, &[&"alice", &"bob"])
        .map(|n| assert_eq!(n, 2));
    let select = client.query(&select, &[]).collect().map(|rows| {
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<_, i32>(0), 1);
        assert_eq!(rows[0].get::<_, &str>(1), "alice");
        assert_eq!(rows[1].get::<_, i32>(0), 2);
        assert_eq!(rows[1].get::<_, &str>(1), "bob");
    });
    let tests = insert.join(select);
    runtime.block_on(tests).unwrap();
}

#[test]
fn cancel_query() {
    let _ = env_logger::try_init();
    let mut runtime = Runtime::new().unwrap();

    let handshake = tokio_postgres::connect(
        "postgres://postgres@localhost:5433".parse().unwrap(),
        TlsMode::None,
    );
    let (mut client, connection) = runtime.block_on(handshake).unwrap();
    let cancel_data = connection.cancel_data();
    let connection = connection.map_err(|e| panic!("{}", e));
    runtime.handle().spawn(connection).unwrap();

    let sleep = client.prepare("SELECT pg_sleep(100)");
    let sleep = runtime.block_on(sleep).unwrap();

    let sleep = client.execute(&sleep, &[]).then(|r| match r {
        Ok(_) => panic!("unexpected success"),
        Err(ref e) if e.code() == Some(&SqlState::QUERY_CANCELED) => Ok::<(), ()>(()),
        Err(e) => panic!("unexpected error {}", e),
    });
    let cancel = Delay::new(Instant::now() + Duration::from_millis(100))
        .then(|r| {
            r.unwrap();
            tokio_postgres::cancel_query(
                "postgres://postgres@localhost:5433".parse().unwrap(),
                TlsMode::None,
                cancel_data,
            )
        })
        .then(|r| {
            r.unwrap();
            Ok::<(), ()>(())
        });

    let ((), ()) = runtime.block_on(sleep.join(cancel)).unwrap();
}
