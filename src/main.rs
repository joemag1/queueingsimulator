use rand::distributions::Distribution;
use rand_distr::Normal;
use std::collections::VecDeque;
use rand::{Rng, thread_rng};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Queueing simulator", about = "Queueing simulator parameters.")]
struct Opt {
    /// Rate at which new requests arrive, must be >0
    #[structopt(short = "r", long = "arrival_rate")]
    request_arrival_rate: f64,

    /// Number of workers to simulate.
    #[structopt(short = "w", long = "workers", default_value = "10")]
    num_workers: u16,

    /// How long before the request is considered timed out and failed. For a meaningful simulation,
    /// this value needs to be larger than mean request process latency.
    #[structopt(short = "t", long = "timeout", default_value = "1000")]
    request_timeout: u32,

    /// Mean request processing latency, has to be larger than 0.
    #[structopt(long = "mean_latency", default_value = "50")]
    mean_request_latency: f64,

    /// Number of ticks to run this simulation.
    #[structopt(long = "simulation_time", default_value = "1000000")]
    simulation_ticks: u32,

    /// Size of the request queue.
    #[structopt(short = "q", long = "queue_size", default_value = "1000")]
    queue_size: usize,

    /// Whether to use LIFO instead of FIFO queue.
    #[structopt(long = "lifo")]
    lifo: bool,

    /// Whether to simulate a temporary spike in the request processing latency (this tends to be the condition that
    /// triggers the congestion collapse).
    #[structopt(long = "simulate_spike")]
    simulate_spike: bool,

    /// Probability a failed request will be tried. Must be between 0 and 1 inclusive.
    #[structopt(long = "retry_probability", default_value = "0.5")]
    retry_probability: f64,
}

fn main() {
    let opt = Opt::from_args();
    if opt.request_arrival_rate <= 0.0 {
        panic!("Request arrival time must be greater than 0.0!");
    }
    if opt.mean_request_latency <= 0.0 {
        panic!("Mean request latency has to be greater than 0.0!");
    }
    if opt.retry_probability < 0.0 || opt.retry_probability > 1.0 {
        panic!("Retry probability must be between 0 and 1!");
    }

    let mut queue: VecDeque<Request> = VecDeque::with_capacity(opt.queue_size);
    let mut workers: Vec<Worker> = (0..opt.num_workers).map(|_| Worker::new()).collect();
    let arrival_distribution =
        Normal::new(opt.request_arrival_rate, opt.request_arrival_rate / 4.0).unwrap();
    // Latency distribution isn't really normal (for example, it can't have negative values). Perhaps a log-normal
    // distribution is a better fit here?
    let latency_distribution =
        Normal::new(opt.mean_request_latency, opt.mean_request_latency / 4.0).unwrap();
    let mut failed_requests = 0;
    let mut total_requests = 0;
    let mut spike_ticks;
    if opt.simulate_spike {
        spike_ticks = opt.simulation_ticks / 1000;
    } else {
        spike_ticks = 0;
    }

    let mut incoming_requests = 0.0;
    for _ in 0..opt.simulation_ticks {
        // Requests that are waiting in the queue are one tick closer to doom.
        queue.iter_mut().for_each(Request::waiting_tick);

        // Compounding arrived requests, so that decimal portions don't get lost (since we can only create
        // even number of requests on each try).
        incoming_requests += arrival_distribution.sample(&mut rand::thread_rng());

        while incoming_requests > 0.0 {
            incoming_requests -= 1.0;
            total_requests += 1;

            // Normal distribution can produce negative results.
            let mut execution_time = 0.0_f64.max(latency_distribution.sample(&mut rand::thread_rng()));
            if spike_ticks > 0 {
                // If we are simulating a short term latency spike, increase the latency of each request by 10x
                spike_ticks -= 1;
                execution_time *= 10.0;
            }

            let request = Request::new(execution_time as u32, opt.request_timeout);
            let idle_worker = workers.iter_mut().find(|w| w.is_free());
            if let Some(worker) = idle_worker {
                worker.take(request);
            } else if queue.len() < opt.queue_size {
                queue.push_back(request);
            } else {
                // Queue is full and all workers busy. This request is failed.
                failed_requests += 1;

                // Some failed requests will be retried.
                if thread_rng().gen_bool(opt.retry_probability) {
                    incoming_requests += 1.0;
                }
            }
        }

        for worker in workers.iter_mut() {
            let request = worker.tick(&mut queue, opt.lifo);
            if request.is_some() && request.unwrap().is_timed_out() {
                // During this tick, a request finished but ended up timing out. This is the case where
                // the client went away, but the server was still processing the request - the worst possible
                // case for a synchronous queueing system.
                failed_requests += 1;

                // Some failed requests will be retried.
                if thread_rng().gen_bool(opt.retry_probability) {
                    incoming_requests += 1.0;
                }
            }
        }
    }

    let failure_rate = failed_requests as f64 / total_requests as f64 * 100.0;
    println!("Failure rate: {:.2}%", failure_rate);
}

struct Worker {
    current_request: Option<Request>,
}

struct Request {
    remaining_ticks: u32,
    timeout_ticks: u32,
}

impl Worker {
    fn new() -> Worker {
        Worker {
            current_request: None,
        }
    }

    /// Spends one tick. If there is current request, works on it. If there isn't one, tries
    /// to pick up a new request from the queue.
    ///
    /// Returns previous request, if it was finished on this tick.
    fn tick(&mut self, queue: &mut VecDeque<Request>, lifo: bool) -> Option<Request> {
        let current_option = &mut self.current_request;

        if let Some(current) = current_option {
            current.working_tick();
            if current.is_done() {
                return self.current_request.take();
            }
        } else {
            // No need to tick here, because that request was already ticked while it was in the queue.
            let next;
            if lifo {
                next = queue.pop_back();
            } else {
                next = queue.pop_front();
            }

            self.current_request = next;
        }

        None
    }

    fn is_free(&self) -> bool {
        self.current_request.is_none()
    }

    fn take(&mut self, request: Request) {
        self.current_request = Some(request);
    }
}

impl Request {
    /// New request, with specified execution time and timeout. It is totally possible (but unlikely)
    /// to end up with a request that takes longer to complete than its timeout, even if the request
    /// was not waiting in the queue. The normal distribution used to generate request cost should make
    /// that probability extremely unlikely, however. That is unless request ends up waiting in the
    /// queue for a long time.
    fn new(execution_time: u32, timeout: u32) -> Request {
        Request {
            remaining_ticks: execution_time,
            timeout_ticks: timeout,
        }
    }

    /// One tick passed while request is waiting in the queue. So we are nearing timeout, but
    /// not making a progress towards completion.
    fn waiting_tick(&mut self) {
        if self.timeout_ticks != 0 {
            self.timeout_ticks -= 1;
        }
    }

    /// One tick passed while request is being worked on. So we are nearing timeout, but also
    /// completion.
    fn working_tick(&mut self) {
        if self.timeout_ticks != 0 {
            self.timeout_ticks -= 1;
        }

        if self.remaining_ticks != 0 {
            self.remaining_ticks -= 1;
        }
    }

    fn is_timed_out(&self) -> bool {
        self.timeout_ticks == 0
    }

    fn is_done(&self) -> bool {
        self.remaining_ticks == 0
    }
}
