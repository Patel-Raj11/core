use crate::error::{PayError, PayErrorCode};
use crate::types::{DiscoverResult, DiscoveryResponse, Protocol, Service};

const CDP_DISCOVERY_URL: &str = "https://api.cdp.coinbase.com/platform/v2/x402/discovery/resources";

const TESTNETS: &[&str] = &[
    "base-sepolia",
    "eip155:84532",
    "eip155:11155111",
    "solana-devnet",
];

// ===========================================================================
// Unified discovery (public API)
// ===========================================================================

/// Discover payable services.
///
/// Fetches the x402 directory with the given pagination parameters,
/// filters testnets, and returns services with pagination metadata.
pub async fn discover_all(
    query: Option<&str>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<DiscoverResult, PayError> {
    let limit = limit.unwrap_or(100);
    let offset = offset.unwrap_or(0);

    let resp = fetch_x402(limit, offset).await?;
    let total = resp.total;

    let mut services = Vec::new();

    for svc in resp.items {
        let accept = match svc.accepts.first() {
            Some(a) => a,
            None => continue,
        };

        let is_testnet = TESTNETS.iter().any(|t| accept.network.contains(t));
        if is_testnet {
            continue;
        }

        if let Some(q) = query {
            let q = q.to_lowercase();
            let url_match = svc.resource.to_lowercase().contains(&q);
            let accepts_desc = accept
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&q))
                .unwrap_or(false);
            let meta_desc = svc
                .metadata
                .as_ref()
                .and_then(|m| m.description.as_ref())
                .map(|d| d.to_lowercase().contains(&q))
                .unwrap_or(false);
            if !url_match && !accepts_desc && !meta_desc {
                continue;
            }
        }

        let desc = accept
            .description
            .as_deref()
            .or_else(|| svc.metadata.as_ref().and_then(|m| m.description.as_deref()))
            .unwrap_or("");

        services.push(Service {
            protocol: Protocol::X402,
            name: svc.resource.clone(),
            url: svc.resource,
            description: truncate(desc, 80),
            price: format_usdc(&accept.amount),
            network: accept.network.clone(),
            tags: vec![],
        });
    }

    Ok(DiscoverResult {
        services,
        total,
        limit,
        offset,
    })
}

// ===========================================================================
// x402 fetching (internal)
// ===========================================================================

struct FetchResult {
    items: Vec<crate::types::DiscoveredService>,
    total: u64,
}

async fn fetch_x402(limit: u64, offset: u64) -> Result<FetchResult, PayError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(CDP_DISCOVERY_URL)
        .query(&[("limit", limit.to_string()), ("offset", offset.to_string())])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PayError::new(
            PayErrorCode::DiscoveryFailed,
            format!("x402 discovery returned {status}: {body}"),
        ));
    }

    let body: DiscoveryResponse = resp.json().await.map_err(|e| {
        PayError::new(
            PayErrorCode::DiscoveryFailed,
            format!("failed to parse x402 discovery: {e}"),
        )
    })?;

    let total = body.pagination.map(|p| p.total).unwrap_or(0);

    Ok(FetchResult {
        items: body.items,
        total,
    })
}

// ===========================================================================
// Formatting helpers
// ===========================================================================

pub(crate) fn format_usdc(amount_str: &str) -> String {
    let amount: u128 = amount_str.parse().unwrap_or(0);
    let whole = amount / 1_000_000;
    let frac = amount % 1_000_000;
    let frac_str = format!("{frac:06}");
    let trimmed = frac_str.trim_end_matches('0');
    let trimmed = if trimmed.is_empty() { "00" } else { trimmed };
    format!("${whole}.{trimmed}")
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.len() > max {
        format!("{}...", &first_line[..max.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}
