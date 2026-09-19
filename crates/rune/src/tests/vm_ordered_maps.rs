//! Balaur fork: objects, maps and sets iterate in insertion order.

prelude!();

#[test]
fn an_object_iterates_in_the_order_its_keys_were_written() {
    let out: String = eval(
        r#"
        let o = #{};
        for k in ["zeta", "alpha", "mid", "beta"] { o[k] = 1; }
        o.remove("alpha");
        o["alpha"] = 2;
        o["zeta"] = 3;
        let s = "";
        for (k, v) in o { s += `${k}${v},`; }
        for k in o.keys() { s += k; }
        s
        "#,
    );
    assert_eq!(out, "zeta3,mid1,beta1,alpha2,zetamidbetaalpha");
}

#[test]
fn a_hash_map_and_a_hash_set_iterate_in_insertion_order() {
    let out: String = eval(
        r#"
        use std::collections::{HashMap, HashSet};
        let m = HashMap::new();
        for i in [35, 0, 14, 7, 28, 21] { m.insert(i, i); }
        m.remove(14);
        m.insert(14, 1);
        let s = HashSet::new();
        for i in [9, 3, 6] { s.insert(i); }
        s.remove(3);
        let out = "";
        for (k, _) in m { out += `${k},`; }
        for v in m.values() { out += `${v};`; }
        for k in s { out += `${k}.`; }
        out += `${m.get(28).unwrap()}`;
        out
        "#,
    );
    assert_eq!(out, "35,0,7,28,21,14,35;0;7;28;21;1;9.6.28");
}
