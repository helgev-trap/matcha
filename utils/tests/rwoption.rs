use utils::rwoption::RwOption;

#[test]
fn test_get_set_take() {
    let opt = RwOption::new();
    assert!(opt.is_none());
    assert!(opt.get().is_none());

    opt.set(5);
    assert!(opt.is_some());
    let g = opt.get().expect("should be Some after set");
    assert_eq!(*g, 5);
    // drop the read guard before acquiring a write lock in `take` to avoid deadlock
    drop(g);

    let taken = opt.take();
    assert_eq!(taken, Some(5));
    assert!(opt.is_none());
}

#[test]
#[allow(clippy::unwrap_used)]
fn test_get_or_insert_and_try() {
    let opt = RwOption::new();

    // get_or_insert should create the value
    {
        let g = opt.get_or_insert(10);
        assert_eq!(*g, 10);
    }
    assert!(opt.is_some());
    {
        let g2 = opt.get().unwrap();
        assert_eq!(*g2, 10);
    }

    // get_or_try_insert_with should propagate Err and not set
    let opt2: RwOption<i32> = RwOption::new();
    let res: Result<_, &'static str> =
        opt2.get_or_try_insert_with(|| -> Result<i32, &'static str> { Err("nope") });
    assert!(res.is_err());
    assert!(opt2.is_none());

    // Now succeed
    let res_ok = opt2
        .get_or_try_insert_with(|| -> Result<i32, &'static str> { Ok(7) })
        .expect("should insert");
    assert_eq!(*res_ok, 7);
}

#[test]
#[allow(clippy::unwrap_used)]
fn test_get_mut_and_mut_or_insert() {
    let opt = RwOption::new();

    // get_mut on None returns None
    assert!(opt.get_mut().is_none());

    // get_mut_or_insert_with should insert and return a mutable guard
    {
        let mut g = opt.get_mut_or_insert_with(|| 3);
        *g += 2;
    }
    // value changed
    assert_eq!(*opt.get().unwrap(), 5);

    // get_mut should return a mutable guard that can modify
    {
        let mut g2 = opt.get_mut().expect("should exist");
        *g2 *= 2;
    }
    assert_eq!(*opt.get().unwrap(), 10);
}

#[test]
#[allow(clippy::unwrap_used)]
fn test_with_read_and_write_helpers() {
    let opt = RwOption::new();

    // with_read on None -> None
    assert!(opt.with_read(|v: &i32| *v).is_none());

    // with_read_or_insert_with initializes and returns value result
    let res = opt.with_read_or_insert_with(|| 21, |v| *v + 1);
    assert_eq!(res, 22);

    // with_write on Some -> modify via closure and return result
    let maybe = opt.with_write(|v: &mut i32| {
        *v += 2;
        *v
    });
    assert_eq!(maybe, Some(23));

    // with_write_or_insert_with when Some should call write closure
    let res2 = opt.with_write_or_insert_with(
        || 100,
        |v| {
            *v += 1;
            *v
        },
    );
    assert_eq!(res2, 24);

    // ensure final stored value
    assert_eq!(*opt.get().unwrap(), 24);
}

#[test]
fn test_is_some_and() {
    let opt = RwOption::new();
    assert!(!opt.is_some_and(|&v: &i32| v > 0));
    opt.set(42);
    assert!(opt.is_some_and(|&v: &i32| v == 42));
}
