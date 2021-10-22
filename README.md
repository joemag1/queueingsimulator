# queueingsimulator

QueingSimulator is an application that can be used to build intuitions about behavior of synchronous request/reply systems (such as a modern web service) under different loads and configuration parameters. More specifically, it examines the impact request queueing has on possibility of congestion collapse.

Most modern web service and application frameworks use a queueing system to enqueue incoming requests when there are no idle worker threads available to handle them. These queues can be great for minimizing transient failures, as they allow the application to ride through a temporary spike in incoming requests without dropping any of them. However, for synchronous request/reply system they can also be a big availability risk. This happens when a large enough queue builds up that by the time a request makes it to the front of the queue, the client would’ve timed out and most likely retried with a fresh new request. When this happens, the server is essentially doing throw away work (client is no longer waiting for a reply) and the system can suffer a congestion collapse. There are many different mechanisms for reducing this risk, and some of them include tuning the queue size, or even flipping the queue from a FIFO to a LIFO order. This simulator allows one to examine this behavior by running a simulation with different parameters and recording the results.

Each execution of the simulator runs a single simulator with specified parameters, and then reports the percentage of the requests that failed, either because they were timed out or rejected by the server when its queue was full. The simulator uses a virtual clock, and as such can simulate hours or even days of runtime in milliseconds.

## Parameters:

-r --arrival_rate: Mean arrival rate of new requests per clock tick.

-w —num_workers: Number of workers (such as web server threads) processing incoming requests. (Default: 10)

-t --timeout: Request timeout - a time after which a client gives up on the request, and potentially retries. For a meaningful simulation, this value should be smaller than mean request latency (Default: 1000)

--mean_latency: Mean latency it takes a worker to process each request, after picking it up from the queue. (Default: 50)

--simulation_time: Number of clock ticks to run the simulation. (Default: 1000000)

-q --queue_size: The size of request queue. (Default: 1000)

--lifo: Whether to use LIFO, instead of FIFO queue. (Default: false)

--simulate_spike: Whether to simulate a temporary spike in request latency (as can happen if a server had a temporary slow down (Default: false)

--retry_probability. Probability a failed request will be retried. Must be between 0 and 1. (Default: 0.5)

## Building and running

This is a rust application, so head on down to https://www.rust-lang.org/tools/install to install rust toolchain. Once install, run “cargo build --release" from the repo directory. The compiled application will be in ./target/release directory.

Running the simulator:

queueingsimulator -r 0.1 --simulate_spike --lifo
Failure rate: 0.79%

queueingsimulator -r 0.1 --simulate_spike
Failure rate: 9.91%

queueingsimulator -r 0.5
Failure rate: 86.74%
