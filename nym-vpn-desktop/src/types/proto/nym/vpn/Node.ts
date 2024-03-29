// Original file: proto/vpn.proto

import type { Location as _nym_vpn_Location, Location__Output as _nym_vpn_Location__Output } from '../../nym/vpn/Location';
import type { Gateway as _nym_vpn_Gateway, Gateway__Output as _nym_vpn_Gateway__Output } from '../../nym/vpn/Gateway';
import type { Empty as _nym_vpn_Empty, Empty__Output as _nym_vpn_Empty__Output } from '../../nym/vpn/Empty';

export interface Node {
  'location'?: (_nym_vpn_Location | null);
  'gateway'?: (_nym_vpn_Gateway | null);
  'fastest'?: (_nym_vpn_Empty | null);
  'node'?: "location"|"gateway"|"fastest";
}

export interface Node__Output {
  'location'?: (_nym_vpn_Location__Output | null);
  'gateway'?: (_nym_vpn_Gateway__Output | null);
  'fastest'?: (_nym_vpn_Empty__Output | null);
  'node': "location"|"gateway"|"fastest";
}
