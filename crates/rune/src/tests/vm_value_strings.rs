//! Balaur fork: strings and byte buffers are values, copied when a second
//! name would share one.

prelude!();

#[test]
fn a_string_bound_twice_is_two_strings() {
    let out: String = eval(
        r#"
        let a = "hi";
        let b = a;
        b += "!";
        let c = "yo";
        let d = c;
        d.push_str("?");
        let o = #{ name: a };
        a += "?";
        let list = [c];
        list.push(c);
        c += "!";
        let grow = |s| { s += "+"; s };
        let e = grow(a);
        `${a} ${b} ${c} ${d} ${o.name} ${list[0]}${list[1]} ${e}`
        "#,
    );
    assert_eq!(out, "hi? hi! yo! yo? hi yoyo hi?+");
}

#[test]
fn a_byte_buffer_bound_twice_is_two_buffers() {
    let out: (usize, usize) = eval(
        r#"
        let a = Bytes::from_vec([1, 2]);
        let b = a;
        b.push(3);
        (a.len(), b.len())
        "#,
    );
    assert_eq!(out, (2, 3));
}

#[test]
fn a_string_built_in_a_loop_keeps_every_append() {
    let out: usize = eval(
        r#"
        let s = "";
        for i in 0..1000 { s += "x"; }
        s.len()
        "#,
    );
    assert_eq!(out, 1000);
}
