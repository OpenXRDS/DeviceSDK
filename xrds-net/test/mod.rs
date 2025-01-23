/*
 Copyright 2025 KETI

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

      https://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
 */


 #[cfg(test)]
mod tests {
    use xrds_net::client::{Client, ClientBuilder};
    use xrds_net::common::data_structure::NetResponse;
    use xrds_net::common::enums::PROTOCOLS;

    #[test]
    fn test_build_client() {
        let client = ClientBuilder()
            .set_protocol(PROTOCOLS::HTTP)
            .set_host("localhost")
            .set_port(8080)
            .build();

        assert_eq!(client.protocol, PROTOCOLS::HTTP);
        assert_eq!(client.host, "localhost");
        assert_eq!(client.port, 8080);

    }

}



