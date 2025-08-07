# HTTP 压力测试工具

一个基于 Rust 编写的简单而强大的命令行 HTTP 压力测试工具。它利用 tokio、hyper 和 rustls 提供高并发、异步的性能，用于对您的 Web 端点进行负载测试。

## 功能特性

- **高并发:** 生成可配置数量的并发工作线程，以产生巨大的负载。
- **支持 HTTP/HTTPS:** 开箱即用，无缝测试 http 和安全的 https 链接。
- **可配置时长:** 可在指定时间内运行测试 (例如, 30s, 5m, 1h)。
- **自定义请求头:** 轻松添加多个自定义 HTTP 请求头，以模拟各种客户端场景或身份验证。
- **支持请求体:** 发送带有请求体数据的 POST 请求，可以直接使用字符串或从文件中读取。
- **详细的统计数据:** 在测试结束时，它会提供一份包含关键性能指标的摘要报告，包括每秒请求数、延迟分布和成功率。

## 在 Linux 上安装 (快速开始)

对于 Linux 用户，最快的入门方法是直接从 GitHub Releases 下载预编译的二进制文件。此方法不需要您安装 Rust 工具链。

### 下载最新版本

```
curl -L -o kt https://github.com/lsk569937453/loadTesting/releases/download/0.0.11/kt-x86_64-unknown-linux-gnu
chmod +x ./kt
```

## 从源码构建

要构建和运行此工具，您需要在系统上安装 Rust 工具链（包括 cargo）。

1.  克隆仓库:
    ```
    git clone git@github.com:lsk569937453/loadTesting.git
    cd loadTesting
    ```
2.  构建项目:
    ```
       cargo build --release
    ```
    编译后的二进制文件将位于 `./target/release/kt`。

## 使用方法

该工具完全通过命令行参数进行配置。

## 基本语法

```
./target/release/kt [OPTIONS] <URL>
```

## 命令行选项

| 选项                    | 别名 | 描述                                                                                  | 默认值 |
| :---------------------- | :--- | :------------------------------------------------------------------------------------ | :----- |
| `--concurrency <NUM>`   | `-c` | 运行的并发工作线程数。                                                                | 50     |
| `--duration <DURATION>` | `-d` | 测试的持续时间。有效单位：s (秒), ms (毫秒), m (分钟), d (天)。与 `--requests` 互斥。 | None   |
| `--requests <REQUESTS>` | `-r` | 要发送的总请求数。与 `--duration` 互斥。                                              | 500000 |
| `--header <KEY:VALUE>`  | `-H` | 向请求中添加自定义 HTTP 头。此选项可多次使用。格式: `"Key:Value"`。                   | None   |
| `--body <DATA>`         | `-b` | HTTP 请求体数据。如果值以 `@` 开头，则其余部分被视为要读取的文件路径。                | None   |
| `--help`                | `-h` | 打印帮助信息。                                                                        |        |
| `--version`             | `-V` | 打印版本信息。                                                                        |        |

## 使用示例

### 1. 基本 GET 测试

使用默认设置（50 个并发工作线程，持续 10 秒）运行测试。

```
./target/release/kt http://localhost:8080/
```

### 2. 指定并发数和持续时间

针对安全端点，使用 200 个并发工作线程运行测试 1 分钟。

```
./target/release/kt -c 200 -d 1m https://api.example.com/health
```

### 3. 添加自定义请求头

模拟带有特定 **User-Agent** 和 **Authorization 令牌**的请求。

```
./target/release/kt
-H "User-Agent: MyTestClient/1.0"
-H "Authorization: Bearer my-secret-token"
https://api.example.com/data
```

### 4. 发送带有内联数据的 POST 请求

使用 --body 或 -b 提供请求体将自动将 HTTP 方法更改为 POST。

```
./target/release/kt
-b '{"name":"test","value":"123"}'
-H "Content-Type: application/json"
https://api.example.com/items
```

### 5. 从文件发送 POST 请求体

如果 --body 参数的值以 @ 开头，则字符串的其余部分将被解释为文件路径。该工具将读取文件内容并将其用作请求体。
假设您有一个名为 data.json 的文件：

```
{
"user_id": 12345,
"payload": {
"action": "create",
"details": "..."
}
}
```

您可以这样发送其内容：

```
./target/release/kt
-b @data.json
-H "Content-Type: application/json"
https://api.example.com/v2/events
```

## 输出报告

在指定的持续时间后，应用程序将停止发送新请求，等待所有挂起的请求完成后，在控制台打印详细的摘要报告。该报告全面概述了目标服务器在负载下的性能。

### 报告示例

```
Http Stress Test Summary
[Session]
URL: http://127.0.0.1:8090/
Concurrency: 50 threads
Test Duration: 10.00 s
[Throughput]
Requests/sec: 98945.92
Transfer Rate: 5.85 MB/s
[Latency]
Average: 504 µs
StdDev: 217 µs
Slowest: 108 ms
Fastest: 58 µs
[Latency Percentiles]
P50 (Median): 483 µs
P90: 698 µs
P95: 778 µs
P99: 968 µs
P99.9: 1 ms
[Data Transfer]
Total Data: 58.51 MiB
Size/request: 62.00 bytes
[Status Code Distribution]
[200] 989633 responses (100.00%)
[Error Distribution]
(No errors)
```

### 理解各项指标

- **[Session]:** 此部分总结了测试运行所使用的配置参数。
  - **URL:** 被测试的目标 URL。
  - **Concurrency:** 使用的并发客户端工作线程数。
  - **Test Duration:** 压力测试的计划持续时间。
- **[Throughput]:** 此项衡量服务器处理请求的速率。
  - **Requests/sec:** 每秒完成的平均请求数。这是服务器性能的主要指标（通常缩写为 RPS）。越高越好。
  - **Transfer Rate:** 每秒从服务器传输到客户端的数据的平均速率（例如，以 MB/s 为单位）。
- **[Latency]:** 延迟是指从发送请求到接收到完整响应所花费的时间。越低越好。
  - **Average:** 所有成功请求的平均响应时间。
  - **StdDev:** 标准差，表示延迟的变化程度。值越低表示响应时间越一致。
  - **Slowest:** 单个请求观察到的最大（最差）延迟。
  - **Fastest:** 单个请求观察到的最小（最佳）延迟。
- **[Latency Percentiles]:** 这些指标比简单的平均值更准确地反映了用户体验。
  - **P50 (Median):** 50% 的请求比此值快。
  - **P90:** 90% 的请求比此值快。
  - **P95:** 95% 的请求比此值快。
  - **P99:** 99% 的请求比此值快。这对于了解绝大多数用户的体验非常有用。
  - **P99.9:** 一个更严格的百分位，有助于识别长尾延迟问题。
  - **[Data Transfer]:** 此部分提供有关响应大小的详细信息。
  - **Total Data:** 测试期间在响应体中接收到的总数据量。
  - **Size/request:** 单个响应体的平均大小。
- **[Status Code Distribution]:** 此处显示从服务器收到的所有 HTTP 状态码的分类统计。这对于识别服务器端错误（例如，404 Not Found, 503 Service Unavailable）至关重要。
- **[Error Distribution]:** 此处列出了发生的任何客户端错误，例如连接超时、DNS 故障或其他阻止请求成功完成的问题。
