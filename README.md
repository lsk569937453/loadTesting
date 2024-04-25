# READ ME
## How to use
```
kt -t 100 -s 5 http://httpbin.org
```
The above command means use 100 threads to request the httpbin.org for 5 second.

## The options
```
Usage: kt.exe [OPTIONS] <URL>

Arguments:
  <URL>  The request url,like http://www.google.com

Options:
  -c, --threads <Number of workers>
          Number of workers to run concurrently. Total number of requests cannot be smaller than the concurrency level. Default is 50.. [default: 50]
  -z, --sleep-seconds <Duration of application to send requests>
          Duration of application to send requests. When duration is reached,application stops and exits [default: 5]
  -h, --help
          Print help
  -V, --version
          Print version
```

## The report
```
kt -c 100 -z 5  http://127.0.0.1:80
```
Run the above command to generate the following report:
```
Summary:
    Total:          5000 millisecond
    Slowest:        15 millisecond
    Fastest:        0 millisecond
    Average:        0 millisecond
    Requests/sec:   156344
    Total data:     44558040 bytes
    Size/request:   57 bytes

Status code distribution:
    [200] 781720 responses
```