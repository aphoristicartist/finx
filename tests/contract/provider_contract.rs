use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use ferrotick_core::{
    AlpacaAdapter, AlphaVantageAdapter, BarsRequest, DataSource, FundamentalsRequest, Interval,
    PolygonAdapter, ProviderId, QuoteRequest, SearchRequest, SourceErrorKind, Symbol, YahooAdapter,
};

#[derive(Clone)]
struct ProviderCase {
    id: ProviderId,
    source: Arc<dyn DataSource>,
    supports_fundamentals: bool,
    supports_search: bool,
}

fn provider_cases() -> Vec<ProviderCase> {
    vec![
        ProviderCase {
            id: ProviderId::Polygon,
            source: Arc::new(PolygonAdapter::default()),
            supports_fundamentals: true,
            supports_search: true,
        },
        ProviderCase {
            id: ProviderId::Alpaca,
            source: Arc::new(AlpacaAdapter::default()),
            supports_fundamentals: false,
            supports_search: false,
        },
        ProviderCase {
            id: ProviderId::Alphavantage,
            source: Arc::new(AlphaVantageAdapter::default()),
            supports_fundamentals: true,
            supports_search: true,
        },
        ProviderCase {
            id: ProviderId::Yahoo,
            source: Arc::new(YahooAdapter::default()),
            supports_fundamentals: true,
            supports_search: true,
        },
    ]
}

#[test]
fn quote_returns_valid_structure_for_all_providers() {
    let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
        .expect("valid quote request");

    for case in provider_cases() {
        let response = block_on(case.source.quote(request.clone())).unwrap_or_else(|error| {
            panic!("provider '{}' quote failed: {error}", case.id.as_str())
        });
        assert_eq!(
            response.quotes.len(),
            1,
            "provider '{}': quote count",
            case.id
        );

        let quote = &response.quotes[0];
        assert_eq!(
            quote.symbol.as_str(),
            "AAPL",
            "provider '{}': symbol",
            case.id
        );
        assert!(
            quote.price > 0.0,
            "provider '{}': price must be positive",
            case.id
        );
        assert_eq!(quote.currency, "USD", "provider '{}': currency", case.id);
        assert!(quote.bid.is_some(), "provider '{}': bid present", case.id);
        assert!(quote.ask.is_some(), "provider '{}': ask present", case.id);
    }
}

#[test]
fn bars_respects_limit_for_all_providers() {
    let request = BarsRequest::new(
        Symbol::parse("MSFT").expect("valid symbol"),
        Interval::OneDay,
        7,
    )
    .expect("valid bars request");

    for case in provider_cases() {
        let response = block_on(case.source.bars(request.clone()))
            .unwrap_or_else(|error| panic!("provider '{}' bars failed: {error}", case.id.as_str()));
        assert_eq!(
            response.symbol.as_str(),
            "MSFT",
            "provider '{}': symbol",
            case.id
        );
        assert_eq!(
            response.interval,
            Interval::OneDay,
            "provider '{}': interval",
            case.id
        );
        assert_eq!(response.bars.len(), 7, "provider '{}': bar limit", case.id);
    }
}

#[test]
fn unsupported_endpoints_return_expected_error() {
    let fundamentals_req =
        FundamentalsRequest::new(vec![Symbol::parse("NVDA").expect("valid symbol")])
            .expect("valid fundamentals request");
    let search_req = SearchRequest::new("apple", 3).expect("valid search request");

    for case in provider_cases() {
        let fundamentals_result = block_on(case.source.fundamentals(fundamentals_req.clone()));
        if case.supports_fundamentals {
            assert!(
                fundamentals_result.is_ok(),
                "provider '{}': fundamentals should be supported",
                case.id
            );
        } else {
            let error = fundamentals_result.expect_err("fundamentals should be unsupported");
            assert_eq!(
                error.kind(),
                SourceErrorKind::UnsupportedEndpoint,
                "provider '{}': fundamentals unsupported error",
                case.id
            );
        }

        let search_result = block_on(case.source.search(search_req.clone()));
        if case.supports_search {
            assert!(
                search_result.is_ok(),
                "provider '{}': search should be supported",
                case.id
            );
        } else {
            let error = search_result.expect_err("search should be unsupported");
            assert_eq!(
                error.kind(),
                SourceErrorKind::UnsupportedEndpoint,
                "provider '{}': search unsupported error",
                case.id
            );
        }
    }
}

#[test]
fn canonical_output_parity_across_providers() {
    let quote_req = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid symbol")])
        .expect("valid quote request");
    let bars_req = BarsRequest::new(
        Symbol::parse("AAPL").expect("valid symbol"),
        Interval::FiveMinutes,
        3,
    )
    .expect("valid bars request");

    let mut quote_signatures = Vec::new();
    let mut bars_signatures = Vec::new();

    for case in provider_cases() {
        let quote = block_on(case.source.quote(quote_req.clone()))
            .unwrap_or_else(|error| panic!("provider '{}' quote failed: {error}", case.id))
            .quotes
            .into_iter()
            .next()
            .expect("expected one quote");

        quote_signatures.push((
            quote.symbol.as_str().to_owned(),
            quote.currency.clone(),
            quote.bid.is_some(),
            quote.ask.is_some(),
            quote.volume.is_some(),
        ));

        let series = block_on(case.source.bars(bars_req.clone()))
            .unwrap_or_else(|error| panic!("provider '{}' bars failed: {error}", case.id));

        bars_signatures.push((
            series.symbol.as_str().to_owned(),
            series.interval,
            series.bars.len(),
            series.bars.iter().all(|bar| bar.volume.is_some()),
        ));
    }

    for signature in quote_signatures.iter().skip(1) {
        assert_eq!(signature, &quote_signatures[0]);
    }

    for signature in bars_signatures.iter().skip(1) {
        assert_eq!(signature, &bars_signatures[0]);
    }
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn noop_waker() -> Waker {
    // SAFETY: The vtable functions never dereference the data pointer and are no-op operations.
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_RAW_WAKER_VTABLE)
}

unsafe fn noop_raw_waker_clone(_: *const ()) -> RawWaker {
    noop_raw_waker()
}

unsafe fn noop_raw_waker_wake(_: *const ()) {}

unsafe fn noop_raw_waker_wake_by_ref(_: *const ()) {}

unsafe fn noop_raw_waker_drop(_: *const ()) {}

static NOOP_RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    noop_raw_waker_clone,
    noop_raw_waker_wake,
    noop_raw_waker_wake_by_ref,
    noop_raw_waker_drop,
);
