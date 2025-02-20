use std::sync::Arc;
use std::time::Duration;

use futures::lock::Mutex;
use rings_core::swarm::callback::SwarmCallback;
use rings_core::swarm::impls::ConnectionHandshake;
use rings_transport::core::transport::ConnectionInterface;
use rings_transport::core::transport::WebrtcConnectionState;
use wasm_bindgen_test::*;

use crate::prelude::rings_core::async_trait;
use crate::prelude::rings_core::dht::TStabilize;
use crate::prelude::rings_core::utils;
use crate::prelude::*;
use crate::processor::*;
use crate::tests::wasm::prepare_processor;

async fn listen(p: &Processor) {
    let h = p.swarm.clone();
    let s = Arc::clone(&p.stabilization);

    futures::join!(
        async {
            h.listen().await;
        },
        async {
            s.wait().await;
        }
    );
}

async fn close_all_connections(p: &Processor) {
    futures::future::join_all(p.swarm.get_connections().iter().map(|(_, t)| t.close())).await;
}

struct SwarmCallbackStruct {
    pub msgs: Mutex<Vec<String>>,
}

#[async_trait(?Send)]
impl SwarmCallback for SwarmCallbackStruct {
    async fn on_inbound(&self, payload: &MessagePayload) -> Result<(), Box<dyn std::error::Error>> {
        let msg: Message = payload.transaction.data().map_err(Box::new)?;
        if let Message::CustomMessage(ref msg) = msg {
            let text = String::from_utf8(msg.0.to_vec()).unwrap();
            console_log!("msg received: {}", text);
            let mut msgs = self.msgs.try_lock().unwrap();
            msgs.push(text);
        }
        Ok(())
    }
}

async fn create_connection(p1: &Processor, p2: &Processor) {
    console_log!("create_offer");
    let (conn_1, offer) = p1.swarm.create_offer(p2.did()).await.unwrap();

    console_log!("answer_offer");
    let (conn_2, answer) = p2.swarm.answer_offer(offer).await.unwrap();

    console_log!("accept_answer");
    p1.swarm.accept_answer(answer).await.unwrap();

    loop {
        console_log!("waiting for connection");
        utils::js_utils::window_sleep(1000).await.unwrap();

        console_log!("conn_1 state: {:?}", conn_1.webrtc_connection_state());
        console_log!("conn_2 state: {:?}", conn_2.webrtc_connection_state());

        let s1 = conn_1.get_stats().await;
        let s2 = conn_2.get_stats().await;

        console_log!("conn_1 stats: {:?}", s1);
        console_log!("conn_2 stats: {:?}", s2);

        if conn_1.is_connected().await && conn_2.is_connected().await {
            break;
        }
    }

    futures::try_join!(
        async {
            if conn_1.is_connected().await {
                return Ok(());
            }
            conn_1.webrtc_wait_for_data_channel_open().await
        },
        async {
            if conn_2.is_connected().await {
                return Ok(());
            }
            conn_2.webrtc_wait_for_data_channel_open().await
        }
    )
    .unwrap();
}

#[wasm_bindgen_test]
async fn test_processor_handshake_and_msg() {
    let callback1 = Arc::new(SwarmCallbackStruct {
        msgs: Mutex::new(vec![]),
    });
    let callback2 = Arc::new(SwarmCallbackStruct {
        msgs: Mutex::new(vec![]),
    });

    let p1 = prepare_processor().await;
    let p2 = prepare_processor().await;

    p1.swarm.set_callback(callback1.clone()).unwrap();
    p2.swarm.set_callback(callback2.clone()).unwrap();

    let test_text1 = "test1";
    let test_text2 = "test2";
    let test_text3 = "test3";
    let test_text4 = "test4";
    let test_text5 = "test5";

    let p1_addr = p1.did().to_string();
    let p2_addr = p2.did().to_string();
    console_log!("p1_addr: {}", p1_addr);
    console_log!("p2_addr: {}", p2_addr);

    console_log!("listen");
    listen(&p1).await;
    listen(&p2).await;

    console_log!("processor_hs_connect_1_2");
    create_connection(&p1, &p2).await;

    fluvio_wasm_timer::Delay::new(Duration::from_secs(2))
        .await
        .unwrap();

    console_log!("processor_send_test_text_messages");
    p1.send_message(p2_addr.as_str(), test_text1.as_bytes())
        .await
        .unwrap();
    console_log!("send test_text1 done");

    p2.send_message(p1_addr.as_str(), test_text2.as_bytes())
        .await
        .unwrap();
    console_log!("send test_text2 done");

    p2.send_message(p1_addr.as_str(), test_text3.as_bytes())
        .await
        .unwrap();
    console_log!("send test_text3 done");

    p1.send_message(p2_addr.as_str(), test_text4.as_bytes())
        .await
        .unwrap();
    console_log!("send test_text4 done");

    p2.send_message(p1_addr.as_str(), test_text5.as_bytes())
        .await
        .unwrap();
    console_log!("send test_text5 done");

    fluvio_wasm_timer::Delay::new(Duration::from_secs(4))
        .await
        .unwrap();

    console_log!("check received");

    let mut msgs1 = callback1.msgs.try_lock().unwrap().as_slice().to_vec();
    msgs1.sort();
    let mut msgs2 = callback2.msgs.try_lock().unwrap().as_slice().to_vec();
    msgs2.sort();

    let mut expect1 = vec![
        test_text2.to_owned(),
        test_text3.to_owned(),
        test_text5.to_owned(),
    ];
    expect1.sort();

    let mut expect2 = vec![test_text1.to_owned(), test_text4.to_owned()];
    expect2.sort();
    assert_eq!(msgs1, expect1);
    assert_eq!(msgs2, expect2);

    console_log!("processor_hs_close_all_connections");
    futures::join!(close_all_connections(&p1), close_all_connections(&p2),);
}

#[wasm_bindgen_test]
async fn test_processor_connect_with_did() {
    super::setup_log();
    let p1 = prepare_processor().await;
    console_log!("p1 address: {}", p1.did());
    let p2 = prepare_processor().await;
    console_log!("p2 address: {}", p2.did());
    let p3 = prepare_processor().await;
    console_log!("p3 address: {}", p3.did());

    p1.swarm.clone().listen().await;
    p2.swarm.clone().listen().await;
    p3.swarm.clone().listen().await;

    console_log!("processor_connect_p1_and_p2");
    create_connection(&p1, &p2).await;
    console_log!("processor_connect_p1_and_p2, done");

    console_log!("processor_connect_p2_and_p3");
    create_connection(&p2, &p3).await;
    console_log!("processor_connect_p2_and_p3, done");

    let p1_peer_dids = p1.swarm.get_connection_ids();
    assert!(
        p1_peer_dids
            .iter()
            .any(|did| did.to_string().eq(&p2.did().to_string())),
        "p2 not in p1's peer list"
    );

    fluvio_wasm_timer::Delay::new(Duration::from_secs(2))
        .await
        .unwrap();

    console_log!("connect p1 and p3");
    // p1 create connect with p3's address
    p1.connect_with_did(p3.did(), true).await.unwrap();
    let conn3 = p1.swarm.get_connection(p3.did()).unwrap();
    console_log!("processor_p1_p3_conntected");
    fluvio_wasm_timer::Delay::new(Duration::from_millis(1000))
        .await
        .unwrap();
    console_log!("processor_detect_connection_state");
    assert_eq!(
        conn3.webrtc_connection_state(),
        WebrtcConnectionState::Connected,
    );

    console_log!("check peers");
    let peer_dids = p1.swarm.get_connection_ids();
    assert!(
        peer_dids
            .iter()
            .any(|did| did.to_string().eq(p3.did().to_string().as_str())),
        "peer list dose NOT contains p3 address"
    );
    console_log!("processor_close_all_connections");
    futures::join!(
        close_all_connections(&p1),
        close_all_connections(&p2),
        close_all_connections(&p3),
    );
}
