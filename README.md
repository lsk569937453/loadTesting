# HTTP Stress Tester

A simple yet powerful command-line HTTP stress testing tool written in Rust. It leverages tokio, hyper, and rustls to provide high-concurrency, asynchronous performance for load testing your web endpoints.

## Features

- **High Concurrency:** Spawns a configurable number of concurrent workers to generate significant load.
- **HTTP/HTTPS Support:** Seamlessly tests both http and secure https URLs out of the box.
- **Configurable Duration:** Run tests for a specific period (e.g., 30s, 5m, 1h).
- **Custom Headers:** Easily add multiple custom HTTP headers to simulate various client scenarios or authentication.
- **Request Body Support:** Send POST requests with body data, either as a direct string or by reading from a file.
- **Detailed Statistics:** At the end of the test, it provides a summary report with key performance indicators, including requests per second, latency distribution, and success rates.

## Installation on Linux (Quick Start)

For Linux users, the quickest way to get started is by downloading the pre-compiled binary directly from GitHub Releases. This method does not require you to have the Rust toolchain installed.

### Download the Latest Release

```
curl -L -o kt https://github.com/lsk569937453/loadTesting/releases/download/0.0.11/kt-x86_64-unknown-linux-gnu
chmod +x ./kt
```

## Build from Source

To build and run this tool, you need the Rust toolchain (including cargo) installed on your system.

1. Clone the Repository:

```
git clone git@github.com:lsk569937453/loadTesting.git
cd loadTesting
```

2. Build the Project:

```
cargo build --release
```

The compiled binary will be located at ./target/release/kt.

## Usage

The tool is configured entirely through command-line arguments.

## Basic Syntax

```
./target/release/kt [OPTIONS] <URL>

```

## Command-Line Options

| Option                  | Alias | Description                                                                                                                         | Default Value |
| :---------------------- | :---- | :---------------------------------------------------------------------------------------------------------------------------------- | :------------ |
| `--concurrency <NUM>`   | `-c`  | The number of concurrent workers (threads) to run.                                                                                  | 50            |
| `--duration <DURATION>` | `-d`  | The duration of the test. Valid units: s (seconds), ms (milliseconds), m (minutes), d (days). Mutually exclusive with `--requests`. | None          |
| `--requests <REQUESTS>` | `-r`  | The total number of requests to send. Mutually exclusive with `--duration`.                                                         | 500000        |
| `--header <KEY:VALUE>`  | `-H`  | Adds a custom HTTP header to the request. This option can be used multiple times. Format: `"Key:Value"`.                            | None          |
| `--body <DATA>`         | `-b`  | The HTTP request body data. If the value starts with `@`, the rest is treated as a file path to read from.                          | None          |
| `--help`                | `-h`  | Print help information.                                                                                                             |               |
| `--version`             | `-V`  | Print version information.                                                                                                          |               |

## Examples

### 1. Basic GET Test

Run a test with default settings (50 concurrent workers for 10 seconds).

```
./target/release/kt http://localhost:8080/
```

### 2. Specify Concurrency and Duration

Run a test with 200 concurrent workers for 1 minute against a secure endpoint.

```
./target/release/kt -c 200 -d 1m https://api.example.com/health
```

### 3. Add Custom Headers

Simulate a request with a specific **User-Agent** and an **Authorization token**.

```
./target/release/kt \
  -H "User-Agent: MyTestClient/1.0" \
  -H "Authorization: Bearer my-secret-token" \
  https://api.example.com/data
```

### 4. Send a POST Request with Inline Body Data

Providing a body with --body or -b will automatically change the HTTP method to POST.

```
./target/release/kt \
  -b '{"name":"test","value":"123"}' \
  -H "Content-Type: application/json" \
  https://api.example.com/items
```

### 5. Send a POST Request with Body from a File

If the value for the --body argument starts with @, the rest of the string is interpreted as a file path. The tool will read the file's content and use it as the request body.
Assuming you have a file named data.json:

```
{
  "user_id": 12345,
  "payload": {
    "action": "create",
    "details": "..."
  }
}
```

You can send its content like this:

```
./target/release/kt \
  -b @data.json \
  -H "Content-Type: application/json" \
  https://api.example.com/v2/events
```

## Output Report

After the specified duration, the application will stop sending new requests, wait for all pending requests to complete, and then print a detailed summary report to the console. This report provides a comprehensive overview of the performance of the target server under load.

### Example Report

```
Http Stress Test Summary
====================================

[Session]
  URL:              http://127.0.0.1:8090/
  Concurrency:      50 threads
  Test Duration:    10.00 s

[Throughput]
  Requests/sec:     98945.92
  Transfer Rate:    5.85 MB/s

[Latency]
  Average:          504 µs
  StdDev:           217 µs
  Slowest:          108 ms
  Fastest:          58 µs

[Latency Percentiles]
  P50 (Median):     483 µs
  P90:              698 µs
  P95:              778 µs
  P99:              968 µs
  P99.9:            1 ms

[Data Transfer]
  Total Data:       58.51 MiB
  Size/request:     62.00 bytes

[Status Code Distribution]
  [200] 989633 responses (100.00%)

[Error Distribution]
  (No errors)
```

### Understanding the Metrics

- **[Session]:** This section summarizes the configuration parameters used for the test run.
  - **URL:** The target URL that was tested.
  - **Concurrency:** The number of concurrent client workers used.
  - **Test Duration:** The planned duration of the stress test.
- **[Throughput]:** This measures the rate at which the server handled requests.
  - **Requests/sec:** The average number of requests completed per second. This is a primary indicator of server performance (often abbreviated as RPS). Higher is better.
  - **Transfer Rate:** The average rate of data transferred from the server to the client per second (e.g., in MB/s).
- **[Latency]:** Latency is the time it takes from the moment a request is sent until the full response is received. Lower is better.
  - **Average:** The mean response time for all successful requests.
  - **StdDev:** The standard deviation, which indicates how much the latency varies. A low value means response times are consistent.
  - **Slowest:** The maximum (worst) latency observed for a single request.
  - **Fastest:** The minimum (best) latency observed for a single request.
- **[Latency Percentiles]:** These metrics provide a more accurate picture of the user experience than a simple average.
  - **P50 (Median):** 50% of requests were faster than this value.
  - **P90:** 90% of requests were faster than this value.
  - **P95:** 95% of requests were faster than this value.
  - **P99:** 99% of requests were faster. This is useful for understanding the experience of the vast majority of users.
  - **P99.9:** An even stricter percentile, helpful for identifying long-tail latency issues.
- **[Data Transfer]:** This section provides details about the size of the responses.
  - **Total Data:** The total amount of data received in response bodies during the test.
  - **Size/request:** The average size of a single response body.
- **[Status Code Distribution]:** This shows a breakdown of all HTTP status codes received from the server. It is crucial for identifying server-side errors (e.g., 404 Not Found, 503 Service Unavailable).
- **[Error Distribution]:** This lists any client-side errors that occurred, such as connection timeouts, DNS failures, or other issues that prevented a request from completing successfully.
