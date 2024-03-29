// Original file: proto/vpn.proto

import type * as grpc from '@grpc/grpc-js'
import type { MethodDefinition } from '@grpc/proto-loader'
import type { Empty as _nym_vpn_Empty, Empty__Output as _nym_vpn_Empty__Output } from '../../nym/vpn/Empty';
import type { PingRequest as _nym_vpn_PingRequest, PingRequest__Output as _nym_vpn_PingRequest__Output } from '../../nym/vpn/PingRequest';

export interface NymVpnServiceClient extends grpc.Client {
  ping(argument: _nym_vpn_PingRequest, metadata: grpc.Metadata, options: grpc.CallOptions, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, metadata: grpc.Metadata, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, options: grpc.CallOptions, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, metadata: grpc.Metadata, options: grpc.CallOptions, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, metadata: grpc.Metadata, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, options: grpc.CallOptions, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  ping(argument: _nym_vpn_PingRequest, callback: grpc.requestCallback<_nym_vpn_Empty__Output>): grpc.ClientUnaryCall;
  
}

export interface NymVpnServiceHandlers extends grpc.UntypedServiceImplementation {
  ping: grpc.handleUnaryCall<_nym_vpn_PingRequest__Output, _nym_vpn_Empty>;
  
}

export interface NymVpnServiceDefinition extends grpc.ServiceDefinition {
  ping: MethodDefinition<_nym_vpn_PingRequest, _nym_vpn_Empty, _nym_vpn_PingRequest__Output, _nym_vpn_Empty__Output>
}
