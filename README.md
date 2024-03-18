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
  -t, --threads <Threads count>              The thread count [default: 20]
  -s, --sleep-seconds <The running seconds>  The thread count [default: 3]
  -h, --help                                 Print help
  -V, --version                              Print version
```