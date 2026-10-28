use super::checkpoint_and_close;
use std::{
    cell::Cell,
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn closes_resource_after_successful_checkpoint() -> TestResult {
    let closed = Cell::new(false);
    let mut future = pin!(checkpoint_and_close(async { Ok::<(), &str>(()) }, async {
        closed.set(true);
    },));
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(Ok(())));
    assert!(closed.get());
    Ok(())
}

#[test]
fn closes_resource_even_when_checkpoint_fails() -> TestResult {
    let closed = Cell::new(false);
    let mut future = pin!(checkpoint_and_close(
        async { Err::<(), &str>("checkpoint_failed") },
        async {
            closed.set(true);
        },
    ));
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err("checkpoint_failed"))
    );
    assert!(closed.get());
    Ok(())
}

#[test]
fn awaits_checkpoint_before_closing_and_awaits_close_before_returning() -> TestResult {
    let checkpoint_polled = Cell::new(false);
    let checkpoint_finished = Cell::new(false);
    let close_polled = Cell::new(false);
    let closed = Cell::new(false);
    let checkpoint = std::future::poll_fn(|context| {
        if checkpoint_polled.replace(true) {
            checkpoint_finished.set(true);
            Poll::Ready(Err::<(), &str>("checkpoint_failed"))
        } else {
            context.waker().wake_by_ref();
            Poll::Pending
        }
    });
    let close = std::future::poll_fn(|context| {
        assert!(checkpoint_finished.get());
        if close_polled.replace(true) {
            closed.set(true);
            Poll::Ready(())
        } else {
            context.waker().wake_by_ref();
            Poll::Pending
        }
    });
    let mut future = pin!(checkpoint_and_close(checkpoint, close));
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(future.as_mut().poll(&mut context), Poll::Pending);
    assert!(!close_polled.get());
    assert_eq!(future.as_mut().poll(&mut context), Poll::Pending);
    assert!(!closed.get());
    assert_eq!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err("checkpoint_failed"))
    );
    assert!(closed.get());
    Ok(())
}
