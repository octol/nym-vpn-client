// Original file: proto/vpn.proto

import type { Location as _nym_vpn_Location, Location__Output as _nym_vpn_Location__Output } from '../../nym/vpn/Location';

export interface Gateway {
  'id'?: (string);
  'location'?: (_nym_vpn_Location | null);
}

export interface Gateway__Output {
  'id': (string);
  'location': (_nym_vpn_Location__Output | null);
}
