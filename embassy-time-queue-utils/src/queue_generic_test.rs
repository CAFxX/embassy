#[cfg(test)]
mod tests {
    use crate::queue_generic::ConstGenericQueue;
    use core::task::{RawWaker, RawWakerVTable, Waker};
    use core::sync::atomic::{AtomicBool, Ordering};

    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        clone,
        wake,
        wake,
        drop,
    );

    unsafe fn clone(p: *const ()) -> RawWaker {
        RawWaker::new(p, &VTABLE)
    }

    unsafe fn wake(p: *const ()) {
        // Safety: We know this points to a valid AtomicBool in our test stack
        let flag = unsafe { &*(p as *const AtomicBool) };
        flag.store(true, Ordering::Relaxed);
    }

    unsafe fn drop(_: *const ()) {}

    fn waker(flag: &AtomicBool) -> Waker {
        let raw = RawWaker::new(flag as *const AtomicBool as *const (), &VTABLE);
        unsafe { Waker::from_raw(raw) }
    }

    #[test]
    fn test_schedule_wake() {
        let mut queue = ConstGenericQueue::<8>::new();
        let flag = AtomicBool::new(false);
        let w = waker(&flag);

        queue.schedule_wake(10, &w);

        // Advance to 9, nothing should happen
        let next = queue.next_expiration(9);
        assert_eq!(next, 10);
        assert_eq!(flag.load(Ordering::Relaxed), false);

        // Advance to 10, should wake
        let next = queue.next_expiration(10);
        assert_eq!(next, u64::MAX);
        assert_eq!(flag.load(Ordering::Relaxed), true);
    }

    #[test]
    fn test_coalescing_overlap() {
        let mut queue = ConstGenericQueue::<8>::new();
        let flag_a = AtomicBool::new(false);
        let w_a = waker(&flag_a);
        let flag_b = AtomicBool::new(false);
        let w_b = waker(&flag_b);

        // A: T=10, E=8, L=12
        queue.schedule_wake_flexible(10, 8, 12, &w_a);
        // B: T=11, E=9, L=13
        queue.schedule_wake_flexible(11, 9, 13, &w_b);

        // Next expiration should be min(max) = min(12, 13) = 12.
        assert_eq!(queue.next_expiration(0), 12);

        // Advance to 7. Nothing should fire.
        assert_eq!(queue.next_expiration(7), 12);
        assert_eq!(flag_a.load(Ordering::Relaxed), false);
        assert_eq!(flag_b.load(Ordering::Relaxed), false);

        // Advance to 12. Both should fire because min <= 12 for both.
        // A.min = 8 <= 12.
        // B.min = 9 <= 12.
        assert_eq!(queue.next_expiration(12), u64::MAX);
        assert_eq!(flag_a.load(Ordering::Relaxed), true);
        assert_eq!(flag_b.load(Ordering::Relaxed), true);
    }

    #[test]
    fn test_coalescing_separate() {
        let mut queue = ConstGenericQueue::<8>::new();
        let flag_a = AtomicBool::new(false);
        let w_a = waker(&flag_a);
        let flag_b = AtomicBool::new(false);
        let w_b = waker(&flag_b);

        // A: T=10, E=8, L=12
        queue.schedule_wake_flexible(10, 8, 12, &w_a);
        // B: T=20, E=18, L=22
        queue.schedule_wake_flexible(20, 18, 22, &w_b);

        // Next expiration should be min(max) = 12.
        assert_eq!(queue.next_expiration(0), 12);

        // Advance to 12. A should fire.
        // A.min = 8 <= 12.
        // B.min = 18 > 12.
        assert_eq!(queue.next_expiration(12), 22); // Next is B.max = 22
        assert_eq!(flag_a.load(Ordering::Relaxed), true);
        assert_eq!(flag_b.load(Ordering::Relaxed), false);

        // Advance to 22. B should fire.
        assert_eq!(queue.next_expiration(22), u64::MAX);
        assert_eq!(flag_b.load(Ordering::Relaxed), true);
    }

    #[test]
    fn test_earliest_deadline_first() {
        let mut queue = ConstGenericQueue::<8>::new();
        let flag_a = AtomicBool::new(false);
        let w_a = waker(&flag_a);
        let flag_b = AtomicBool::new(false);
        let w_b = waker(&flag_b);
        let flag_c = AtomicBool::new(false);
        let w_c = waker(&flag_c);

        // A: 1..5
        queue.schedule_wake_flexible(1, 1, 5, &w_a);
        // B: 2..4
        queue.schedule_wake_flexible(2, 2, 4, &w_b);
        // C: 3..6
        queue.schedule_wake_flexible(3, 3, 6, &w_c);

        // Expect wakeup at min(5, 4, 6) = 4.
        assert_eq!(queue.next_expiration(0), 4);

        // Wake at 4.
        assert_eq!(queue.next_expiration(4), u64::MAX);
        // All should fire as 4 >= min for all.
        assert_eq!(flag_a.load(Ordering::Relaxed), true);
        assert_eq!(flag_b.load(Ordering::Relaxed), true);
        assert_eq!(flag_c.load(Ordering::Relaxed), true);
    }

    #[test]
    fn test_interrupt_rescheduling() {
        // Test scenario where an independent interrupt wakes us up "early" (before max, but after min).
        let mut queue = ConstGenericQueue::<8>::new();
        let flag_a = AtomicBool::new(false);
        let w_a = waker(&flag_a);

        // A: T=10, E=8, L=12
        queue.schedule_wake_flexible(10, 8, 12, &w_a);

        // Next alarm at 12.
        assert_eq!(queue.next_expiration(0), 12);

        // Interrupt wakes us at 9.
        // A.min = 8 <= 9. Should fire!
        assert_eq!(queue.next_expiration(9), u64::MAX);
        assert_eq!(flag_a.load(Ordering::Relaxed), true);
    }
}
