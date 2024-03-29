// Original file: proto/vpn.proto

import type { Gateway as _nym_vpn_Gateway, Gateway__Output as _nym_vpn_Gateway__Output } from '../../nym/vpn/Gateway';

export interface GatewayResponse {
  'gateways'?: (_nym_vpn_Gateway | null);
}

export interface GatewayResponse__Output {
  'gateways': (_nym_vpn_Gateway__Output | null);
}
