use criterion::{criterion_group, criterion_main, Criterion};
use rust_widgets::event::{Event, EventPriority, EventQueue};
use std::hint::black_box;

fn bench_event_queue_post_and_dequeue_10k(c: &mut Criterion) {
    c.bench_function("event_queue_post_dequeue_10k", |b| {
        b.iter(|| {
            let queue = EventQueue::new();
            let sender = queue.sender();
            for i in 0..10_000_u64 {
                let event = Event::MouseMove {
                    pos: rust_widgets::core::Point::new((i % 1024) as i32, (i % 768) as i32),
                };
                sender
                    .post_with_priority(i, event, EventPriority::Normal)
                    .expect("event post should succeed");
            }

            let mut drained = 0_u64;
            while let Some((id, event, priority)) = queue.dequeue() {
                black_box((id, event, priority));
                drained += 1;
            }
            black_box(drained);
        })
    });
}

criterion_group!(benches, bench_event_queue_post_and_dequeue_10k);
criterion_main!(benches);
