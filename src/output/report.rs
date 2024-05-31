use core::time;
use itertools::Itertools;
use std::collections::HashMap;

pub struct StatisticList {
    pub response_list: Vec<Result<ResponseStatistic, anyhow::Error>>,
}
pub struct ResponseStatistic {
    pub time_cost: u128,
    pub staus_code: u16,
    pub content_length: u64,
}
impl StatisticList {
    pub fn print(&self, total: u128) {
        let mut slow = 0;
        let mut fast = 10000000;
        let mut average = 0;
        let mut rps = 0;
        let mut total_data = 0;
        let mut size_per_request = 0;

        let mut hashmap = HashMap::new();

        let mut total_time_cost = 0;
        for result in &self.response_list {
            match result {
                Ok(item) => {
                    let time_cost = item.time_cost;
                    let status_code = item.staus_code;
                    let content_len = item.content_length;
                    if time_cost > slow {
                        slow = time_cost;
                    }
                    if time_cost < fast {
                        fast = time_cost;
                    }
                    total_time_cost += time_cost;
                    total_data += content_len;
                    size_per_request = content_len;
                    hashmap
                        .entry(status_code)
                        .and_modify(|counter| *counter += 1)
                        .or_insert(1);
                }
                Err(e) => {
                    println!("{}", e);
                }
            }
        }
        let mapdata = hashmap
            .iter()
            .map(|(k, v)| format!("[{}] {} responses", k, v))
            .join(", ");

        average = total_time_cost / self.response_list.len() as u128;
        rps = self.response_list.len() as u128 / (total / 1000);

        let format_str = format!(
            r#"
Summary:
    Total:          {total} millisecond 
    Slowest:        {slow} millisecond 
    Fastest:        {fast} millisecond 
    Average:        {average} millisecond 
    Requests/sec:   {rps}
    Total data:     {total_data} bytes
    Size/request:   {size_per_request} bytes
        
Status code distribution:
    {mapdata}
"#
        );
        println!("{}", format_str);
    }
}
pub fn print() {
    let x = 42;
    let y = 123;

    let s = format!(
        r#"
        Summary:
          Total:	{{ formatNumber .Total.Seconds }} secs
          Slowest:	{{ formatNumber .Slowest }} secs
          Fastest:	{{ formatNumber .Fastest }} secs
          Average:	{{ formatNumber .Average }} secs
          Requests/sec:	{{ formatNumber .Rps }}
          {{ if gt .SizeTotal 0 }}
          Total data:	{{ .SizeTotal }} bytes
          Size/request:	{{ .SizeReq }} bytes{{ end }}
        
        Response time histogram:
        {{ histogram .Histogram }}
        
        Latency distribution:{{ range .LatencyDistribution }}
          {{ .Percentage }}%% in {{ formatNumber .Latency }} secs{{ end }}
        
        Details (average, fastest, slowest):
          DNS+dialup:	{{ formatNumber .AvgConn }} secs, {{ formatNumber .ConnMax }} secs, {{ formatNumber .ConnMin }} secs
          DNS-lookup:	{{ formatNumber .AvgDNS }} secs, {{ formatNumber .DnsMax }} secs, {{ formatNumber .DnsMin }} secs
          req write:	{{ formatNumber .AvgReq }} secs, {{ formatNumber .ReqMax }} secs, {{ formatNumber .ReqMin }} secs
          resp wait:	{{ formatNumber .AvgDelay }} secs, {{ formatNumber .DelayMax }} secs, {{ formatNumber .DelayMin }} secs
          resp read:	{{ formatNumber .AvgRes }} secs, {{ formatNumber .ResMax }} secs, {{ formatNumber .ResMin }} secs
        
        Status code distribution:{{ range $code, $num := .StatusCodeDist }}
          [{{ $code }}]	{{ $num }} responses{{ end }}
        
        {{ if gt (len .ErrorDist) 0 }}Error distribution:{{ range $err, $num := .ErrorDist }}
          [{{ $num }}]	{{ $err }}{{ end }}{{ end }}
        "#
    );
}
