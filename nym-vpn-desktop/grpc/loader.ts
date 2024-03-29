import { loadPackageDefinition } from '@grpc/grpc-js';
import { loadSync } from '@grpc/proto-loader';
import type { ProtoGrpcType } from '../src/types/proto/vpn';

const PROTO_PATH = __dirname + '/../../protos/route_guide.proto';

// Suggested options for similarity to existing grpc.load behavior
const packageDefinition = loadSync(PROTO_PATH, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});

const protoDescriptor = loadPackageDefinition(
  packageDefinition,
) as unknown as ProtoGrpcType;
// The protoDescriptor object has the full package hierarchy
const nymVpnPackage = protoDescriptor.nym.vpn;

export default nymVpnPackage;
