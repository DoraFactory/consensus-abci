use crate::{Transaction, QueryInfo};

use eyre::WrapErr;
use futures::SinkExt;
use tendermint_proto::abci::ResponseQuery;
use tokio::spawn;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneShotSender};

use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use warp::{Filter, Rejection};

use core::panic;
use std::net::SocketAddr;
/// Client Api which will provide a exposed port(eg:26657) for users to get and post msg
pub struct ClientApi<T> {
    // commonly: 26657 port
    abci_client_address: SocketAddr,
    req: Sender<(OneShotSender<T>, QueryInfo)>,
}

impl<T: Send + Sync + std::fmt::Debug> ClientApi<T> {
    pub fn new(
        abci_client_address: SocketAddr,
        req: Sender<(OneShotSender<T>, QueryInfo)>,
    ) -> Self {
        Self {
            abci_client_address,
            req,
        }
    }
}

impl ClientApi<ResponseQuery> {
    pub fn get_routes(
        self,
        tx_req: Sender<Transaction>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
        let route_broadcast_tx = warp::path("broadcast_tx")
            .and(warp::query::<Transaction>())
            .and_then( move |req: Transaction| {
                let abci_tx = tx_req.clone();
                async move {
                    println!("broadcast tx: {:?}", req.clone());
    
                    // 将整个Transaction结构发送到共识层
                    if let Err(e) = abci_tx.send(req.clone()).await {
                        Ok::<_, Rejection>(format!("ERROR IN: broadcast_tx: {:?}. Err: {}", req, e))
                    }else{
                        Ok::<_, Rejection>(format!("broadcast_tx: {:?}", req))
                    }
                }
            });

        let route_abci_query = warp::path("abci_query")
            .and(warp::query::<QueryInfo>())
            .and_then(move |req: QueryInfo| {
                let tx_abci_queries = self.req.clone();
                async move {
                    println!("query request: {:?}", req.clone());

                    // 创建一个单生产者单消费者的管道，向ABCI client发送一个消息，这个消息是一个元组(管道，req)
                    let (tx_query, rx_query) = oneshot_channel();
                    match tx_abci_queries.send((tx_query, req.clone())).await {
                        Ok(_) => {}
                        Err(err) => log::error!("Error forwarding abci query: {}", err),
                    };
                    let resp = rx_query.await.unwrap();
                    // Return the value
                    Ok::<_, Rejection>(resp.value)
                }
            });

        route_broadcast_tx.or(route_abci_query)
    }
}
