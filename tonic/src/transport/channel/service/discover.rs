use super::super::{Connection, Endpoint};

use hyper_util::client::legacy::connect::HttpConnector;
use std::{
    hash::Hash,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::Receiver;
use tokio_stream::Stream;
use tower::discover::Change as TowerChange;

/// A change in the service set.
#[derive(Debug, Clone)]
pub enum Change<K, V> {
    /// A new service identified by key `K` was identified.
    Insert(K, V),
    /// The service identified by key `K` disappeared.
    Remove(K),
}

pub(crate) struct DynamicServiceStream<K: Hash + Eq + Clone> {
    changes: Receiver<Change<K, Endpoint>>,
}

impl<K: Hash + Eq + Clone> DynamicServiceStream<K> {
    pub(crate) fn new(changes: Receiver<Change<K, Endpoint>>) -> Self {
        Self { changes }
    }
}

impl<K: Hash + Eq + Clone> Stream for DynamicServiceStream<K> {
    type Item = Result<TowerChange<K, Connection>, crate::BoxError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let c = &mut self.changes;
        match Pin::new(&mut *c).poll_recv(cx) {
            Poll::Pending | Poll::Ready(None) => Poll::Pending,
            Poll::Ready(Some(change)) => match change {
                Change::Insert(k, endpoint) => {
                    let mut http = HttpConnector::new();
                    http.set_nodelay(endpoint.tcp_nodelay);
                    http.set_keepalive(endpoint.tcp_keepalive);
                    http.set_connect_timeout(endpoint.connect_timeout);
                    http.enforce_http(false);

                    let connection = Connection::lazy(endpoint.connector(http), endpoint);
                    let change = Ok(TowerChange::Insert(k, connection));
                    Poll::Ready(Some(change))
                }
                Change::Remove(k) => Poll::Ready(Some(Ok(TowerChange::Remove(k)))),
            },
        }
    }
}

impl<K: Hash + Eq + Clone> Unpin for DynamicServiceStream<K> {}
