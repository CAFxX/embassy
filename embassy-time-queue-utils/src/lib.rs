#![no_std]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

#[cfg(feature = "_generic-queue")]
pub mod queue_generic;
#[cfg(all(feature = "_generic-queue", test))]
mod queue_generic_test;
#[cfg(not(feature = "_generic-queue"))]
pub mod queue_integrated;

#[cfg(feature = "_generic-queue")]
pub use queue_generic::Queue;
#[cfg(not(feature = "_generic-queue"))]
pub use queue_integrated::Queue;

/// Utility function to schedule a wakeup with an alarm.
///
/// This function encapsulates the common pattern of scheduling a wakeup in the queue,
/// checking if the new expiration is earlier than the previous one, and if so,
/// setting the hardware alarm. It handles the race condition where the alarm time
/// passed while setting it by retrying.
pub fn schedule_wake_with_alarm(
    queue: &mut Queue,
    at: u64,
    min: u64,
    max: u64,
    waker: &core::task::Waker,
    now: impl Fn() -> u64,
    mut set_alarm: impl FnMut(u64) -> bool,
) {
    if queue.schedule_wake_flexible(at, min, max, waker) {
        let mut next = queue.next_expiration(now());
        while !set_alarm(next) {
            next = queue.next_expiration(now());
        }
    }
}
