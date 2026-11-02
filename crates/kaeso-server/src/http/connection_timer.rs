use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

type Alarm = Pin<Box<tokio::time::Sleep>>;

// hyper arms a new header-read timer for every request on a kept-alive connection and drops it
// once the head is read. tokio's timer wheel sits behind one lock, so each connection keeps one
// timer instead and moves it on only when it rings before the deadline it now guards.
#[derive(Clone, Debug, Default)]
pub struct ConnectionTimer {
    spare: Arc<Mutex<Option<Alarm>>>,
}

impl ConnectionTimer {
    fn take_spare(&self) -> Option<Alarm> {
        self.spare
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

impl hyper::rt::Timer for ConnectionTimer {
    fn now(&self) -> Instant {
        tokio::time::Instant::now().into_std()
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn hyper::rt::Sleep>> {
        self.sleep_until(self.now() + duration)
    }

    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn hyper::rt::Sleep>> {
        Box::pin(Reused {
            alarm: self.take_spare(),
            deadline: deadline.into(),
            home: self.spare.clone(),
        })
    }

    fn reset(&self, sleep: &mut Pin<Box<dyn hyper::rt::Sleep>>, new_deadline: Instant) {
        if let Some(mut reused) = sleep.as_mut().downcast_mut_pin::<Reused>() {
            reused.deadline = new_deadline.into();
        } else {
            *sleep = self.sleep_until(new_deadline);
        }
    }
}

struct Reused {
    alarm: Option<Alarm>,
    deadline: tokio::time::Instant,
    home: Arc<Mutex<Option<Alarm>>>,
}

impl Future for Reused {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let deadline = self.deadline;
        let alarm = self
            .alarm
            .get_or_insert_with(|| Box::pin(tokio::time::sleep_until(deadline)));
        if alarm.deadline() > deadline {
            alarm.as_mut().reset(deadline);
        }
        while alarm.as_mut().poll(cx).is_ready() {
            if tokio::time::Instant::now() >= deadline {
                return Poll::Ready(());
            }
            alarm.as_mut().reset(deadline);
        }
        Poll::Pending
    }
}

impl hyper::rt::Sleep for Reused {}

impl Drop for Reused {
    fn drop(&mut self) {
        if let Some(alarm) = self.alarm.take() {
            *self.home.lock().unwrap_or_else(PoisonError::into_inner) = Some(alarm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::rt::Timer;

    fn has_spare(timer: &ConnectionTimer) -> bool {
        timer
            .spare
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    #[tokio::test(start_paused = true)]
    async fn a_connection_reuses_its_timer_and_it_rings_at_the_new_deadline() {
        let timer = ConnectionTimer::default();
        let mut first = timer.sleep(Duration::from_secs(30));
        assert!(futures::poll!(first.as_mut()).is_pending());
        drop(first);
        assert!(has_spare(&timer));
        tokio::time::advance(Duration::from_secs(10)).await;
        let started = tokio::time::Instant::now();
        let second = timer.sleep(Duration::from_secs(30));
        assert!(!has_spare(&timer));
        second.await;
        assert_eq!(started.elapsed(), Duration::from_secs(30));
    }

    #[tokio::test(start_paused = true)]
    async fn a_reset_moves_the_deadline_either_way() {
        let timer = ConnectionTimer::default();
        let started = tokio::time::Instant::now();
        let mut sooner = timer.sleep(Duration::from_secs(30));
        assert!(futures::poll!(sooner.as_mut()).is_pending());
        timer.reset(&mut sooner, timer.now() + Duration::from_secs(5));
        sooner.await;
        assert_eq!(started.elapsed(), Duration::from_secs(5));

        let started = tokio::time::Instant::now();
        let mut later = timer.sleep(Duration::from_secs(1));
        timer.reset(&mut later, timer.now() + Duration::from_secs(20));
        later.await;
        assert_eq!(started.elapsed(), Duration::from_secs(20));
    }
}
