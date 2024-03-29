import type * as grpc from '@grpc/grpc-js';
import type { EnumTypeDefinition, MessageTypeDefinition } from '@grpc/proto-loader';

import type { NymVpnServiceClient as _nym_vpn_NymVpnServiceClient, NymVpnServiceDefinition as _nym_vpn_NymVpnServiceDefinition } from './nym/vpn/NymVpnService';

type SubtypeConstructor<Constructor extends new (...args: any) => any, Subtype> = {
  new(...args: ConstructorParameters<Constructor>): Subtype;
};

export interface ProtoGrpcType {
  nym: {
    vpn: {
      ConnectRequest: MessageTypeDefinition
      ConnectResponse: MessageTypeDefinition
      ConnectionProgress: MessageTypeDefinition
      ConnectionStatus: EnumTypeDefinition
      ConnectionStatusUpdate: MessageTypeDefinition
      Empty: MessageTypeDefinition
      Error: MessageTypeDefinition
      Gateway: MessageTypeDefinition
      GatewayResponse: MessageTypeDefinition
      GetVpnModeResponse: MessageTypeDefinition
      Location: MessageTypeDefinition
      LocationListResponse: MessageTypeDefinition
      Node: MessageTypeDefinition
      NymVpnService: SubtypeConstructor<typeof grpc.Client, _nym_vpn_NymVpnServiceClient> & { service: _nym_vpn_NymVpnServiceDefinition }
      PingRequest: MessageTypeDefinition
      SetUserCredentialsRequest: MessageTypeDefinition
      SetVpnModeRequest: MessageTypeDefinition
      VpnMode: EnumTypeDefinition
    }
  }
}

