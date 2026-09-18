//! A function of one unit calling a closure made in another, synchronously:
//! once the closure returns, the caller must go on in its own unit.

prelude!();

use crate::Unit;

fn unit(context: &Context, mut sources: Sources) -> Result<Arc<Unit>> {
    let mut diagnostics = Diagnostics::new();
    let unit = prepare(&mut sources)
        .with_context(context)
        .with_diagnostics(&mut diagnostics)
        .build()?;
    Ok(Arc::new(unit))
}

#[test]
fn a_closure_from_another_unit_returns_to_the_caller_unit() -> Result<()> {
    let context = Context::with_default_modules()?;
    let runtime = Arc::new(context.runtime()?);

    let library = unit(
        &context,
        sources! {
            library => {
                pub fn apply(f) {
                    let padding = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
                    let got = f();
                    got + padding.len()
                }
            }
        },
    )?;
    let apply = Vm::new(runtime.clone(), library).lookup_function(["apply"])?;

    let caller = unit(
        &context,
        sources! {
            caller => {
                pub fn main(apply) {
                    apply(|| 32)
                }
            }
        },
    )?;
    let mut vm = Vm::new(runtime, caller);
    let output: i64 = from_value(vm.call(["main"], (apply,))?)?;
    assert_eq!(output, 42);
    Ok(())
}
