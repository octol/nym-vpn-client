// Original file: proto/vpn.proto

import type { Node as _nym_vpn_Node, Node__Output as _nym_vpn_Node__Output } from '../../nym/vpn/Node';

export interface ConnectRequest {
  'entry'?: (_nym_vpn_Node | null);
  'exit'?: (_nym_vpn_Node | null);
}

export interface ConnectRequest__Output {
  'entry': (_nym_vpn_Node__Output | null);
  'exit': (_nym_vpn_Node__Output | null);
}
