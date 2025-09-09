# XRNet

XRDS network library

## Supported Platform/Architecture

- Linux x86/x64
- Android arm/arm64
- Windows x86/x64

## Supported Protocols

- Http(s) [1.1/3]
- (S)FTP
- FILE
- MQTT
- WS(S)
  - Websocket
- CoAP

## Dependencies

### Http(s), FILE

- curl-rust: 0.4.47
- [https://docs.rs/curl/latest/curl/](https://docs.rs/curl/latest/curl/)

#### ws(s)

* websocket: 0.27.1
* [https://docs.rs/websocket/latest/websocket/](https://docs.rs/websocket/latest/websocket/)
* default port: 80, secured: 443

#### mqtt

- rumqttc: 0.24.0
- [https://crates.io/crates/rumqttc](https://crates.io/crates/rumqttc)
- [chrome-extension://efaidnbmnnnibpcajpcglclefindmkaj/https://www.witree.co.kr/layouts/witree_2015/data/product/Manual/MQTT_guide.pdf](chrome-extension://efaidnbmnnnibpcajpcglclefindmkaj/https://www.witree.co.kr/layouts/witree_2015/data/product/Manual/MQTT_guide.pdf)
- default port: 1883, secured: 8883

#### coap

- coap: 0.19.1 [[https://docs.rs/coap/latest/coap/](https://docs.rs/coap/latest/coap/)]
- coap-lite: 0.11.3 [https://docs.rs/coap-lite/0.3.1/coap_lite/]

#### WebRTC

- webrtc: 0.12.0
- [https://docs.rs/webrtc/latest/webrtc/](https://docs.rs/webrtc/latest/webrtc/)

#### (S)FTP

- suppaftp: ^6
- [https://docs.rs/suppaftp/latest/suppaftp/](https://docs.rs/suppaftp/latest/suppaftp/)

#### DASH

- dash-mpd: 0.17.4
- [https://docs.rs/dash-mpd/latest/dash_mpd/](https://docs.rs/dash-mpd/latest/dash_mpd/)

#### Http3 & QUIC

- quiche: 0.22.0

## Test Command

```
cargo test -p xrds-net -- --nocapture
```

##### 특정 유닛 테스트 수행 예시

```
cargo test --package xrds-net tests::test_server_list -- --nocapture
```
