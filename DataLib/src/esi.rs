use std::collections::VecDeque;
use std::fmt::Debug;
use std::time::{Duration, Instant};


#[derive(Debug)]
pub struct DeadlineQueue<T> {
    queue: VecDeque<(Instant, VecDeque<T>)>,
    max_delay: Duration,
    deadline_item_count: u32,
    current_deadline: Instant
}

impl<T> DeadlineQueue<T> {
    pub fn new(max_delay: Duration) -> Self {
        DeadlineQueue {
            queue: VecDeque::new(),
            max_delay,
            deadline_item_count: 0,
            current_deadline: Instant::now(),
        }
    }

    pub fn with_capacity(max_delay: Duration, capacity: usize) -> Self {
        DeadlineQueue {
            queue: VecDeque::with_capacity(capacity),
            max_delay,
            deadline_item_count: 0,
            current_deadline: Instant::now(),
        }
    }

    fn recalc_delay(&mut self) {
        let now = Instant::now();
        let mut delay = f64::MAX;
        let mut item_count = 0;
        let mut deadline_item_count = 0;
        let mut deadline = now;

        for i in 0..self.queue.len() {
            let entry = &self.queue[i];
            item_count += entry.1.len();

            // The desired delay between items is the time from now until the `i`-th sub-queue's deadline, divided by the total amount of items in `i` and all preceding sub-queues.
            let desired_delay = entry.0.saturating_duration_since(now).as_secs_f64() / (item_count as f64);
            if desired_delay < delay {
                delay = desired_delay;
                deadline_item_count = item_count;
                deadline = entry.0;
            }
        }

        self.deadline_item_count = deadline_item_count as u32;
        self.current_deadline = deadline;
    }

    pub fn push<I: IntoIterator<Item=T>>(&mut self, deadline: Instant, items: I) {
        // Optimization: Peek the end of the queue before doing a binary search, as it's likely calls to `push` will be in-order
        match self.queue.back_mut() {
            // If queue is empty, append a new entry
            None => self.queue.push_back((deadline, VecDeque::from_iter(items))),
            // If last item in queue is before the newly added deadline, append a new entry; Optimizes for in-order calls to `push`
            Some((d, _)) if *d < deadline => self.queue.push_back((deadline, VecDeque::from_iter(items))),
            // If last item in queue has same deadline, append items to sub-queue; Optimizes for repeated calls to `push` with the same deadline
            Some((d, sub_queue)) if *d == deadline => sub_queue.extend(items),
            // Otherwise (deadline is before the end of the current queue), binary search to find insertion location
            _ => match self.queue.binary_search_by(|(d, _)| d.cmp(&deadline)) {
                Ok(idx) => self.queue[idx].1.extend(items),
                Err(idx) => self.queue.insert(idx, (deadline, VecDeque::from_iter(items))),
            },
        }
        self.recalc_delay();
    }

    pub fn pop_item(&mut self) -> Option<(T, Duration)> {
        let old_len = self.queue.len();
        loop {
            match self.queue.front_mut() {
                Some((_, items)) => {
                    match items.pop_front() {
                        Some(item) => {
                            if items.len() == 0 { self.queue.pop_front(); } // Clean up sub-queue if we've just emptied it
                            debug_assert!(self.deadline_item_count > 0, "Queue with deadline_item_count zero"); // Requirement for the division below to not panic. `recalc_delay` guarantees that deadline_item_count is at least 1 for a non-empty queue
                            let duration = self.current_deadline.saturating_duration_since(Instant::now()) / self.deadline_item_count;
                            if self.queue.len() == old_len {
                                self.deadline_item_count = self.deadline_item_count - 1;
                            } else {
                                self.recalc_delay();
                            }
                            break Some((item, duration.min(self.max_delay)))
                        }
                        None => { self.queue.pop_front(); continue; } // Empty sub-queues should be cleaned up pre-emptively, but handling the counterfactual is cheap/easy.
                    }
                },
                None => break None
            }
        }
    }
}

impl<T> Iterator for DeadlineQueue<T> {
    type Item = (T, Duration);

    fn next(&mut self) -> Option<Self::Item> {
        self.pop_item()
    }
}

#[test]
fn test_queue() {
    let mut queue = DeadlineQueue::new(Duration::from_secs(5));

    let now = Instant::now();
    queue.push(now + Duration::from_secs(5), [1, 2, 3]);
    queue.push(now + Duration::from_secs(10), [4, 5, 6]);
    queue.push(now + Duration::from_secs(15), [7, 8, 9]);
    queue.push(now + Duration::from_secs(20), [10, 11, 12]);

    println!("{:?}", queue);

    let mut sum = Duration::ZERO;

    for (item, duration) in queue {
        sum = sum + duration;
        println!("{} {:?} ({:?})", item, duration, sum);
    }
}











// #[derive(Eq, PartialEq, Hash, Copy, Clone)]
// pub enum RateLimitGroup {
//     Status,
// }
//
// impl RateLimitGroup {}
//
// pub enum QueueGroup {}
//
// pub trait Endpoint {
//     type Response: DeserializeOwned;
//     fn ratelimit_group(&self) -> RateLimitGroup;
//     fn queue_group() -> QueueGroup;
//     fn method(&self) -> reqwest::Method;
//     fn url(&self) -> String;
// }
//
// pub enum ESIError {
//     Reqwest(reqwest::Error)
//
// }
//
// impl From<reqwest::Error> for ESIError {
//     fn from(value: reqwest::Error) -> Self {
//         Self::Reqwest(value)
//     }
// }
//
// pub struct ESI {
//     client: reqwest::Client,
//     buckets: HashMap<RateLimitGroup, Arc<Semaphore>>,
//     queue_groups: HashMap<QueueGroup, ()>
// }
//
// impl ESI {
//     pub async fn request_immediate<T: Endpoint>(&mut self, request: T) -> Result<T::Response, ESIError> {
//         let mut tokens = self.buckets[&request.ratelimit_group()].clone().acquire_many_owned(5).await.expect("Semaphores are never closed");
//
//         let response = self.client.request(request.method(), request.url()).send().await;
//         if let Ok(response) = &response {
//             if let Some(tokens_consumed) = response.headers().get("X-Ratelimit-Used").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<usize>().ok()) {
//                 drop(tokens.split(tokens_consumed));
//             }
//         }
//         tokio::spawn(async {
//             tokio::time::sleep(Duration::from_mins(15) + Duration::from_secs(rand::random_range(0..60))).await;
//             drop(tokens)
//         });
//
//         response?.json::<T::Response>().await.map_err(ESIError::from)
//     }
//
//     pub async fn request_queue<T: Endpoint, I: Iterator<Item=T> + ExactSizeIterator, F: FnMut(T) -> Result<(), ESIError>>(&mut self, requests: I, within: Duration, handler: F) -> Result<(), ESIError> {
//         Ok(())
//     }
// }