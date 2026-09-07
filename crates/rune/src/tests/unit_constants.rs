prelude!();

#[test]
fn test_get_const() -> Result<()> {
    let context = Context::with_default_modules()?;

    let mut sources = sources! {
        entry => {
            pub const LEET = 1337;
        }
    };

    let unit = prepare(&mut sources).with_context(&context).build()?;

    assert_eq!(
        unit.constant(&hash!(LEET))
            .context("missing constant")?
            .to_value()?
            .as_signed()?,
        1337
    );
    Ok(())
}

#[test]
fn test_get_const_re_export() -> Result<()> {
    let context = Context::with_default_modules()?;

    let mut sources = sources! {
        entry => {
            mod inner {
                pub const LEET = 1337;
            }

            pub use inner::LEET;
        },
    };

    let unit = prepare(&mut sources).with_context(&context).build()?;

    assert_eq!(
        unit.constant(&hash!(LEET))
            .context("missing constant")?
            .to_value()?
            .as_signed()?,
        1337
    );
    Ok(())
}

#[test]
fn test_get_const_nested() -> Result<()> {
    let context = Context::with_default_modules()?;

    let mut sources = sources! {
        entry => {
            pub mod inner {
                pub const LEET = 1337;
            }
        },
    };

    let unit = prepare(&mut sources).with_context(&context).build()?;

    assert_eq!(
        unit.constant(&hash!(inner::LEET))
            .expect("successful lookup")
            .to_value()
            .expect("could not allocate value")
            .as_signed()
            .expect("the inner value"),
        1337
    );
    Ok(())
}

#[test]
fn exported_constants_are_listed_with_their_kind() -> Result<()> {
    let context = Context::with_default_modules()?;

    let mut sources = sources! {
        entry => {
            #[export] pub const SPEED = 2.0;
            #[export(node)] pub const TARGET = "";
            #[export(asset)] pub const ICON = "";
            pub const PRIVATE = 7;
        }
    };

    let unit = prepare(&mut sources).with_context(&context).build()?;

    let mut listed = unit.exported_constants()?;
    listed.sort_by(|a, b| a.0.cmp(b.0));

    let names: Vec<_> = listed.iter().map(|(n, k, _)| (*n, *k)).collect();
    assert_eq!(
        names,
        [("ICON", "asset"), ("SPEED", "value"), ("TARGET", "node")],
        "a constant without the attribute stays out of the table"
    );

    // The table carries the default, so a tool reads it without a second
    // lookup and without knowing the module the constant sits in.
    let speed = listed
        .iter()
        .find(|(n, _, _)| *n == "SPEED")
        .context("SPEED is listed")?;
    assert_eq!(speed.2.as_float()?, 2.0);

    // The values are still ordinary constants, reachable the usual way.
    assert_eq!(
        unit.constant(&hash!(SPEED))
            .context("missing constant")?
            .to_value()?
            .as_float()?,
        2.0
    );
    Ok(())
}

#[test]
fn an_unknown_export_kind_is_refused() -> Result<()> {
    let context = Context::with_default_modules()?;

    let mut sources = sources! {
        entry => {
            #[export(colour)] pub const TINT = "";
        }
    };

    let built = prepare(&mut sources).with_context(&context).build();
    assert!(built.is_err(), "`colour` is not a kind the engine knows");
    Ok(())
}
