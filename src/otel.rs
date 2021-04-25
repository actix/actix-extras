use opentelemetry::propagation::Extractor;

pub(crate) struct RequestHeaderCarrier<'a> {
    headers: &'a actix_web::http::HeaderMap,
}

impl<'a> RequestHeaderCarrier<'a> {
    pub(crate) fn new(headers: &'a actix_web::http::HeaderMap) -> Self {
        RequestHeaderCarrier { headers }
    }
}

impl<'a> Extractor for RequestHeaderCarrier<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(|header| header.as_str()).collect()
    }
}
