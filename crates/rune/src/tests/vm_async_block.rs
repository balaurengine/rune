prelude!();

#[test]
fn test_async_block() {
    let out: i64 = rune! {
        async fn foo(value) {
            let output = value.await;
            output
        }

        let value = 42;
        foo(async { value }).await / foo(async { 2 }).await
    };
    assert_eq!(out, 21);
}

#[test]
fn an_async_closure_returns_a_local() {
    let out: i64 = rune! {
        let f = async || { let ok = 7; ok };
        f().await
    };
    assert_eq!(out, 7);
}
