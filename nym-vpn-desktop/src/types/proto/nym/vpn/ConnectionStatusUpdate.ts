// Original file: proto/vpn.proto

import type { ConnectionStatus as _nym_vpn_ConnectionStatus, ConnectionStatus__Output as _nym_vpn_ConnectionStatus__Output } from '../../nym/vpn/ConnectionStatus';
import type { ConnectionProgress as _nym_vpn_ConnectionProgress, ConnectionProgress__Output as _nym_vpn_ConnectionProgress__Output } from '../../nym/vpn/ConnectionProgress';
import type { Error as _nym_vpn_Error, Error__Output as _nym_vpn_Error__Output } from '../../nym/vpn/Error';

export interface ConnectionStatusUpdate {
  'status'?: (_nym_vpn_ConnectionStatus);
  'connectionProgress'?: (_nym_vpn_ConnectionProgress)[];
  'error'?: (_nym_vpn_Error | null);
  '_error'?: "error";
}

export interface ConnectionStatusUpdate__Output {
  'status': (_nym_vpn_ConnectionStatus__Output);
  'connectionProgress': (_nym_vpn_ConnectionProgress__Output)[];
  'error'?: (_nym_vpn_Error__Output | null);
  '_error': "error";
}
