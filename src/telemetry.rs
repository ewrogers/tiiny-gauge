use core::{cell::Cell, fmt::Write, net::Ipv4Addr};
use embassy_net::{
    Stack,
    dns::DnsSocket,
    tcp::client::{TcpClient, TcpClientState},
};
use embassy_time::{Duration, Instant, Ticker, with_timeout};
use reqwless::{
    client::HttpClient,
    request::{Method, RequestBuilder},
};
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, IgnoredAny, SeqAccess, Visitor},
};
use serde_json_core::heapless::String;

#[derive(Clone, Copy, Default)]
pub(crate) struct Memory {
    pub used: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Clone, Copy)]
pub(crate) struct Snapshot {
    pub cpu: Option<f32>,
    pub npu: Option<f32>,
    pub soc_ram: Memory,
    pub npu_ram: Memory,
    pub models: Models,
    pub busy_since: Option<Instant>,
}

#[derive(Clone, Copy)]
pub(crate) enum Status {
    Setup,
    WifiLost,
    TiinyUnavailable,
    Live(Snapshot),
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Models {
    name: [u8; 32],
    len: usize,
    pub count: usize,
}

impl Models {
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.len]).unwrap_or("")
    }
}

impl<'de> Deserialize<'de> for Models {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Running;
        impl<'de> Visitor<'de> for Running {
            type Value = Models;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a list of running model IDs")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Models, A::Error> {
                let mut models = Models::default();
                if let Some(id) = seq.next_element::<&str>()? {
                    let short = id.rsplit('/').next().unwrap_or(id);
                    for (slot, byte) in models.name.iter_mut().zip(short.bytes()) {
                        *slot = if byte.is_ascii_graphic() {
                            byte.to_ascii_uppercase()
                        } else {
                            b' '
                        };
                        models.len += 1;
                    }
                    if short.len() > models.len {
                        models.name[models.len - 3..].copy_from_slice(b"...");
                    }
                    models.count = 1;
                    while seq.next_element::<IgnoredAny>()?.is_some() {
                        models.count += 1;
                    }
                }
                Ok(models)
            }
        }
        deserializer.deserialize_seq(Running)
    }
}

#[derive(Deserialize)]
struct RunningModels {
    running: Models,
}

#[derive(Default, Deserialize)]
struct Cpu {
    total_percent: Option<f32>,
}

#[derive(Default, Deserialize)]
struct SocMemory {
    used_bytes: Option<u64>,
    total_bytes: Option<u64>,
}

#[derive(Deserialize)]
struct Npu {
    utilization_percent: Option<f32>,
}

#[derive(Deserialize)]
struct SystemStatus {
    #[serde(default)]
    cpu: Cpu,
    #[serde(default)]
    memory: SocMemory,
    #[serde(default)]
    npus: serde_json_core::heapless::Vec<Npu, 4>,
}

#[derive(Deserialize)]
struct NpuDevice {
    mem_used_mb: Option<u64>,
    mem_total_mb: Option<u64>,
}

#[derive(Deserialize)]
struct NpuStatus {
    hm_smi_available: bool,
    devices: serde_json_core::heapless::Vec<NpuDevice, 4>,
}

impl NpuStatus {
    fn memory(&self) -> Option<Memory> {
        if !self.hm_smi_available {
            return None;
        }
        let device = self.devices.first()?;
        let used = device.mem_used_mb?;
        let total = device.mem_total_mb?;
        if total == 0 || used > total {
            return None;
        }
        Some(Memory {
            used: Some(used.checked_mul(1_048_576)?),
            total: Some(total.checked_mul(1_048_576)?),
        })
    }
}

type TiinyClient<'a, 'net> = HttpClient<'a, TcpClient<'net, 1, 1024, 2048>, DnsSocket<'net>>;

async fn get<T: DeserializeOwned>(
    client: &mut TiinyClient<'_, '_>,
    buffer: &mut [u8],
    url: &str,
    host: Option<&str>,
    headers: &[(&str, &str)],
) -> Option<T> {
    let mut request = client
        .request(Method::GET, url)
        .await
        .ok()?
        .headers(headers);
    if let Some(host) = host {
        request = request.host(host);
    }
    let response = request.send(buffer).await.ok()?;
    if response.status.0 != 200 {
        return None;
    }
    let data = response.body().read_to_end().await.ok()?;
    serde_json_core::from_slice(data)
        .ok()
        .map(|(value, _)| value)
}

pub(crate) async fn poll(stack: Stack<'_>, ip: Ipv4Addr, key: &str, state: &Cell<Status>) -> ! {
    let mut auth = String::<512>::new();
    if write!(auth, "Bearer {key}").is_err() {
        state.set(Status::Setup);
        core::future::pending::<()>().await;
    }
    let headers = [("Authorization", auth.as_str())];
    let mut sys_url = String::<80>::new();
    let mut models_url = String::<80>::new();
    let mut npu_url = String::<80>::new();
    let _ = write!(sys_url, "http://{ip}/api/v1/sys/status");
    let _ = write!(models_url, "http://{ip}/api/v1/models/running");
    let _ = write!(npu_url, "http://{ip}/api/v1/npu/status");
    let gateway = Some("p8800.api.tiiny");
    let tcp_state = TcpClientState::<1, 1024, 2048>::new();
    let tcp = TcpClient::new(stack, &tcp_state);
    let dns = DnsSocket::new(stack);
    let mut client = HttpClient::new(&tcp, &dns);
    let mut buffer = [0; 8192];
    let mut ticks = Ticker::every(Duration::from_secs(2));
    loop {
        if !stack.is_config_up() {
            state.set(Status::WifiLost);
            stack.wait_config_up().await;
            ticks.reset();
        }
        let cycle = async {
            let sys: SystemStatus = get(&mut client, &mut buffer, &sys_url, None, &headers).await?;
            let models: RunningModels =
                get(&mut client, &mut buffer, &models_url, gateway, &headers).await?;
            let utilization = sys.npus.first().and_then(|npu| npu.utilization_percent);
            let previous = match state.get() {
                Status::Live(sample) if sample.models.name() == models.running.name() => {
                    sample.busy_since
                }
                _ => None,
            };
            let threshold = if previous.is_some() { 5.0 } else { 10.0 };
            let busy_since = (models.running.count > 0
                && utilization.is_some_and(|load| load >= threshold))
            .then(|| previous.unwrap_or_else(Instant::now));
            Some(Snapshot {
                cpu: sys.cpu.total_percent,
                npu: utilization,
                soc_ram: Memory {
                    used: sys.memory.used_bytes,
                    total: sys.memory.total_bytes,
                },
                npu_ram: Memory::default(),
                models: models.running,
                busy_since,
            })
        };
        let deadline = Instant::now() + Duration::from_millis(1800);
        state.set(
            match with_timeout(Duration::from_millis(1800), cycle).await {
                Ok(Some(mut snapshot)) => {
                    let diagnostics =
                        get::<NpuStatus>(&mut client, &mut buffer, &npu_url, gateway, &headers);
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if let Ok(Some(diagnostics)) =
                        with_timeout(remaining.min(Duration::from_millis(350)), diagnostics).await
                        && let Some(memory) = diagnostics.memory()
                    {
                        snapshot.npu_ram = memory;
                    }
                    Status::Live(snapshot)
                }
                _ if stack.is_config_up() => Status::TiinyUnavailable,
                _ => Status::WifiLost,
            },
        );
        ticks.next().await;
    }
}
